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
//! // Get a shareable reader (Send + Sync)
//! let reader = swap.reader().clone();
//!
//! // Writer updates the value
//! swap.update(1);
//!
//! let handle = thread::spawn(move || {
//!     // Create a thread-local handle to read
//!     let local = reader.handle();
//!     let guard = local.load();
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

/// Shareable reader (Send + Sync), can be stored in structs and shared across threads
///
/// 可共享的读取者（Send + Sync），可以存储在结构体中并跨线程共享
///
/// This type is designed to be safely shared across threads and stored in structs.
/// To perform actual read operations, call `handle()` to create a thread-local `ReaderHandle`.
///
/// 此类型设计为可以安全地跨线程共享并存储在结构体中。
/// 要执行实际的读取操作，请调用 `handle()` 创建线程本地的 `ReaderHandle`。
pub struct SwapReader<T> {
    current: Arc<EpochPtr<T>>,
    domain: EpochGcDomain,
}

/// Thread-local reader handle, not Sync (use SwapReader::handle() to create)
///
/// 线程本地的读取句柄，不是 Sync（使用 SwapReader::handle() 创建）
///
/// This type contains a thread-local `LocalEpoch` and cannot be shared across threads.
/// Create one per thread using `SwapReader::handle()`.
///
/// 此类型包含线程本地的 `LocalEpoch`，不能跨线程共享。
/// 使用 `SwapReader::handle()` 为每个线程创建一个。
pub struct ReaderHandle<T> {
    reader: SwapReader<T>,
    epoch: LocalEpoch,
}

/// Main entry point for the SMR swap library
///
/// SMR swap 库的主入口点
pub struct SmrSwap<T> {
    swapper: Swapper<T>,
    handle: ReaderHandle<T>,
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

    let reader = SwapReader { current, domain };

    (swapper, reader)
}

impl<T: 'static> SmrSwap<T> {
    /// Create a new SMR container
    ///
    /// 创建新的SMR容器
    #[inline]
    pub fn new(initial: T) -> Self {
        let (swapper, reader) = new_smr_pair(initial);
        let handle = reader.handle();
        Self { swapper, handle }
    }

    /// Get a reference to the inner Swapper
    ///
    /// 获取内部 Swapper 的引用
    #[inline]
    pub fn swapper(&mut self) -> &mut Swapper<T> {
        &mut self.swapper
    }

    /// Get a reference to the inner SwapReader (Send + Sync)
    ///
    /// 获取内部 SwapReader 的引用（Send + Sync）
    #[inline]
    pub fn reader(&self) -> &SwapReader<T> {
        &self.handle.reader
    }

    /// Get a reference to the internal ReaderHandle for direct reading
    ///
    /// 获取内部 ReaderHandle 的引用，用于直接读取
    #[inline]
    pub fn handle(&self) -> &ReaderHandle<T> {
        &self.handle
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
        self.handle.load()
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
        let guard = self.handle.epoch.pin();
        let old_val = self.handle.reader.current.load(&guard);
        let new_value = f(old_val);
        self.swapper.update(new_value);
        let ptr = self.handle.reader.current.load(&guard) as *const T;
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
        let guard = self.handle.epoch.pin();
        let old_value = self.handle.reader.current.load(&guard).clone();
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
        let guard = self.handle.epoch.pin();
        let current = self.handle.reader.current.load(&guard);
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
    /// Create a new thread-local reader handle
    ///
    /// This is the only way to create a `ReaderHandle` from a `SwapReader`.
    /// Call this method when you need to perform read operations.
    ///
    /// 创建一个新的线程本地读取句柄
    ///
    /// 这是从 `SwapReader` 创建 `ReaderHandle` 的唯一方法。
    /// 当你需要执行读取操作时，请调用此方法。
    #[inline]
    pub fn handle(&self) -> ReaderHandle<T> {
        ReaderHandle {
            reader: self.clone(),
            epoch: self.domain.register_reader(),
        }
    }
}

impl<T> Clone for SwapReader<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            current: self.current.clone(),
            domain: self.domain.clone(),
        }
    }
}

impl<T: 'static> ReaderHandle<T> {
    /// Get a reference to the inner `SwapReader` (Send + Sync)
    ///
    /// Use this to get a shareable reference that can be stored in structs.
    ///
    /// 获取内部 `SwapReader` 的引用（Send + Sync）
    ///
    /// 使用此方法获取可存储在结构体中的可共享引用。
    #[inline]
    pub fn reader(&self) -> &SwapReader<T> {
        &self.reader
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
    #[inline]
    pub fn load(&self) -> ReaderGuard<'_, T> {
        let _guard = self.epoch.pin();
        let ptr = self.reader.current.load(&_guard) as *const T;
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
        f(self.reader.current.load(&guard))
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
            let val = self.reader.current.load(&_guard);
            if !f(val) {
                return None;
            }
            val as *const T
        };

        Some(ReaderGuard { _guard, ptr })
    }
}

/// Clone implementation for ReaderHandle
///
/// Each cloned handle creates a new `LocalEpoch` registration via `handle()`.
/// This ensures each handle has its own independent epoch for safe concurrent access.
///
/// ReaderHandle 的 Clone 实现
///
/// 每个克隆的句柄通过 `handle()` 创建新的 `LocalEpoch` 注册。
/// 这确保每个句柄拥有独立的 epoch，以实现安全的并发访问。
impl<T: 'static> Clone for ReaderHandle<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.reader.handle()
    }
}

#[cfg(test)]
mod tests;
