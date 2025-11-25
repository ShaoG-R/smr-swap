//! A minimal locking, epoch-based concurrent swap library.
//!
//! This library provides a mechanism to swap values atomically while allowing concurrent readers
//! to access the old value until they are done. It relies on `swmr-epoch` for epoch-based garbage collection.
//!
//! # Example
//!
//! ```rust
//! use smr_swap::SmrSwap;
//! use std::thread;
//!
//! let mut swap = SmrSwap::new(0);
//!
//! // Reader reads the value
//! let reader = swap.reader().fork();
//!
//! // Writer updates the value
//! swap.update(1);
//!
//! let handle = thread::spawn(move || {
//!     let guard = reader.load();
//!     assert_eq!(*guard, 1);
//! });
//!
//! handle.join().unwrap();
//! ```

#[cfg(feature = "loom")]
use loom::sync::Arc;
#[cfg(not(feature = "loom"))]
use std::sync::Arc;
pub use swmr_epoch::{EpochGcDomain, EpochPtr, GcHandle, LocalEpoch, PinGuard};

/// Writer type, not cloneable
///
/// 写入者类型，不可Clone
pub struct Swapper<T> {
    current: Arc<EpochPtr<T>>,
    // Garbage collector handle, held directly by Swapper
    // 垃圾回收器句柄，由 Swapper 直接持有
    gc: GcHandle,
}

/// Reader type, not cloneable (use fork() to create a new reader for another thread)
///
/// 读取者类型，不可Clone（使用 fork() 为另一个线程创建新的读取者）
pub struct SwapReader<T> {
    current: Arc<EpochPtr<T>>,
    domain: EpochGcDomain,
    epoch: LocalEpoch,
}

/// Main entry point for the SMR swap library
///
/// SMR swap 库的主入口点
pub struct SmrSwap<T> {
    swapper: Swapper<T>,
    reader: SwapReader<T>,
}

/// RAII guard for reading values
///
/// 用于读取值的 RAII 守卫
pub struct ReaderGuard<'a, T> {
    _guard: PinGuard<'a>,
    ptr: *const T,
}

impl<'a, T> std::ops::Deref for ReaderGuard<'a, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: The data is protected by `guard` which is held in this struct.
        // The pointer was obtained from a valid load protected by a pin that is still active (via `guard`).
        unsafe { &*self.ptr }
    }
}

/// Create a new pair of Swapper and SwapReader
///
/// 创建一对新的 Swapper 和 SwapReader
#[inline]
pub fn new_smr_pair<T: 'static>(initial: T) -> (Swapper<T>, SwapReader<T>) {
    // Create the epoch GC domain
    // 创建 epoch GC 域
    let (gc, domain) = EpochGcDomain::builder()
        .auto_reclaim_threshold(None)
        .cleanup_interval(2)
        .build();

    let current = Arc::new(EpochPtr::new(initial));

    let swapper = Swapper {
        current: current.clone(),
        gc,
    };

    let reader_epoch = domain.register_reader();
    let reader = SwapReader {
        current,
        domain,
        epoch: reader_epoch,
    };

    (swapper, reader)
}

impl<T: 'static> SmrSwap<T> {
    /// Create a new SMR container
    ///
    /// 创建新的SMR容器
    #[inline]
    pub fn new(initial: T) -> Self {
        let (swapper, reader) = new_smr_pair(initial);
        Self { swapper, reader }
    }

    /// Get a reference to the inner Swapper
    ///
    /// 获取内部 Swapper 的引用
    #[inline]
    pub fn swapper(&mut self) -> &mut Swapper<T> {
        &mut self.swapper
    }

    /// Get a reference to the inner SwapReader
    ///
    /// 获取内部 SwapReader 的引用
    #[inline]
    pub fn reader(&self) -> &SwapReader<T> {
        &self.reader
    }

    /// Split into Swapper and SwapReader
    ///
    /// 拆分为 Swapper 和 SwapReader
    #[inline]
    pub fn into_components(self) -> (Swapper<T>, SwapReader<T>) {
        (self.swapper, self.reader)
    }

    /// Perform a write operation to update the current version
    ///
    /// 执行写入操作，更新当前版本
    #[inline]
    pub fn update(&mut self, new_value: T) {
        self.swapper.update(new_value);
    }

    /// Read the current version (lock-free) with RAII guard
    ///
    /// 使用 RAII 守卫读取当前版本（无锁）
    #[inline]
    pub fn load(&self) -> ReaderGuard<'_, T> {
        self.reader.load()
    }

    /// Apply a closure function to the current value and return the result
    ///
    /// The closure receives a reference to the current value and returns a new value.
    /// Returns a reference to the new value.
    ///
    /// 对当前值应用闭包函数并返回结果
    ///
    /// 闭包接收当前值的引用，返回新值
    /// 返回新值的引用
    #[inline]
    pub fn update_and_fetch<F>(&mut self, f: F) -> ReaderGuard<'_, T>
    where
        F: FnOnce(&T) -> T,
    {
        let guard = self.reader.epoch.pin();
        let old_val = self.reader.current.load(&guard);
        let new_value = f(old_val);
        self.swapper.update(new_value);
        let ptr = self.reader.current.load(&guard) as *const T;
        ReaderGuard { _guard: guard, ptr }
    }
}

impl<T: 'static> SmrSwap<Arc<T>> {
    /// Atomically swap the current Arc value with a new one
    ///
    /// 原子地将当前 Arc 值与新值交换
    #[inline]
    pub fn swap(&mut self, new_value: Arc<T>) -> Arc<T> {
        // Read the current value before swapping
        // 在交换前读取当前值
        let guard = self.reader.epoch.pin();
        let old_value = self.reader.current.load(&guard).clone();
        drop(guard);

        self.swapper.update(new_value);

        old_value
    }

    /// Apply a closure function to the current value and return the result as an Arc
    ///
    /// 对当前值应用闭包函数并将结果作为 Arc 返回
    #[inline]
    pub fn update_and_fetch_arc<F>(&mut self, f: F) -> Arc<T>
    where
        F: FnOnce(&Arc<T>) -> Arc<T>,
    {
        let guard = self.reader.epoch.pin();
        let current = self.reader.current.load(&guard);
        let new_value = f(current);
        drop(guard);
        self.swapper.update(new_value.clone());
        new_value
    }
}

impl<T: 'static> Swapper<T> {
    /// Perform a write operation to update the current version
    ///
    /// 执行写入操作，更新当前版本
    #[inline]
    pub fn update(&mut self, new_value: T) {
        // Store the new value and retire the old one
        // 存储新值并退休旧值
        self.current.store(new_value, &mut self.gc);

        // Trigger garbage collection
        // 触发垃圾回收
        self.gc.collect();
    }
}

impl<T: 'static> SwapReader<T> {
    /// Create a new reader for the current thread
    ///
    /// This replaces `Clone`. Since `LocalEpoch` is thread-local, we must register a new one
    /// when creating a reader for a new thread.
    ///
    /// 为当前线程创建一个新的读取者
    ///
    /// 这替代了 `Clone`。由于 `LocalEpoch` 是线程本地的，我们在为新线程创建读取者时必须注册一个新的。
    #[inline]
    pub fn fork(&self) -> Self {
        Self {
            current: self.current.clone(),
            domain: self.domain.clone(),
            epoch: self.domain.register_reader(),
        }
    }

    /// Read the current version (lock-free) with RAII guard
    ///
    /// Returns a `ReaderGuard` that holds the pin and the reference.
    /// The pin is automatically released when the guard is dropped.
    ///
    /// 使用 RAII 守卫读取当前版本（无锁）
    ///
    /// 返回一个持有 pin 和引用的 `ReaderGuard`。
    /// 当守卫被 drop 时，pin 会自动释放。
    /// Read the current version (lock-free) with RAII guard
    ///
    /// Returns a `ReaderGuard` that holds the pin and the reference.
    /// The pin is automatically released when the guard is dropped.
    ///
    /// 使用 RAII 守卫读取当前版本（无锁）
    ///
    /// 返回一个持有 pin 和引用的 `ReaderGuard`。
    /// 当守卫被 drop 时，pin 会自动释放。
    #[inline]
    pub fn load(&self) -> ReaderGuard<'_, T> {
        let _guard = self.epoch.pin();
        let ptr = self.current.load(&_guard) as *const T;
        ReaderGuard { _guard, ptr }
    }

    /// Apply a closure function to the current value and transform the result
    ///
    /// This method reads the current value, applies the closure to transform it,
    /// and returns the transformed result.
    ///
    /// 对当前值应用闭包函数并转换结果
    ///
    /// 这个方法读取当前值，应用闭包进行转换，并返回转换后的结果
    #[inline]
    pub fn map<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        let guard = self.epoch.pin();
        f(self.current.load(&guard))
    }

    /// Apply a closure function to the current value, returning Some if the closure returns true, otherwise None
    ///
    /// 对当前值应用闭包函数，如果闭包返回 true 则返回 Some，否则返回 None
    #[inline]
    pub fn filter<F>(&self, f: F) -> Option<ReaderGuard<'_, T>>
    where
        F: FnOnce(&T) -> bool,
    {
        let _guard = self.epoch.pin();

        // Use a block to limit the scope of the borrow
        let ptr = {
            let val = self.current.load(&_guard);
            if !f(val) {
                return None;
            }
            val as *const T
        };

        Some(ReaderGuard { _guard, ptr })
    }
}

#[cfg(test)]
mod tests;
