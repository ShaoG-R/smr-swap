//! A minimal locking, version-based concurrent swap library.
//!
//! This library provides a mechanism to swap values atomically while allowing concurrent readers
//! to access the old value until they are done. It uses `swmr-cell` for version-based garbage collection.
//!
//! # Example
//!
//! ```rust
//! use smr_swap::SmrSwap;
//! use std::thread;
//!
//! let mut swap = SmrSwap::new(0);
//!
//! // Get a thread-local reader
//! let local = swap.local();
//!
//! // Writer updates the value
//! swap.update(1);
//!
//! // Read in another thread
//! let local2 = swap.local();
//! let handle = thread::spawn(move || {
//!     let guard = local2.load();
//!     assert_eq!(*guard, 1);
//! });
//!
//! handle.join().unwrap();
//! ```

use std::ops::Deref;
use swmr_cell::SwmrCell;

// Re-export for backward compatibility
pub use swmr_cell::{LocalReader as CellLocalReader, PinGuard};

/// Main entry point for the SMR swap library.
///
/// A single-writer, multi-reader swap container with version-based garbage collection.
///
/// SMR swap 库的主入口点。
///
/// 单写多读的交换容器，带有基于版本的垃圾回收。
pub struct SmrSwap<T: 'static> {
    cell: SwmrCell<T>,
    local: LocalReader<T>,
}

/// Thread-local reader handle, not Sync.
///
/// Each thread should create its own `LocalReader` via `SmrSwap::local()` and reuse it.
/// `LocalReader` is `!Sync` and should not be shared between threads.
///
/// 线程本地的读取句柄，不是 Sync。
///
/// 每个线程应该通过 `SmrSwap::local()` 创建自己的 `LocalReader` 并重复使用。
/// `LocalReader` 是 `!Sync` 的，不应在线程之间共享。
pub struct LocalReader<T: 'static> {
    inner: CellLocalReader<T>,
}

/// RAII guard for reading values.
///
/// Dereference to access the value. The value is protected until the guard is dropped.
///
/// 用于读取值的 RAII 守卫。
///
/// 解引用以访问值。在守卫被 drop 之前，值是受保护的。
pub struct ReadGuard<'a, T: 'static> {
    inner: PinGuard<'a, T>,
}

impl<'a, T> Deref for ReadGuard<'a, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &*self.inner
    }
}

impl<'a, T> Clone for ReadGuard<'a, T> {
    #[inline]
    fn clone(&self) -> Self {
        ReadGuard {
            inner: self.inner.clone(),
        }
    }
}

// ============================================================================
// SmrSwap implementation
// ============================================================================

impl<T: 'static> SmrSwap<T> {
    /// Create a new SMR container with the given initial value.
    ///
    /// 使用给定的初始值创建新的 SMR 容器。
    #[inline]
    pub fn new(initial: T) -> Self {
        let cell = SwmrCell::builder().auto_reclaim_threshold(Some(4)).build(initial);
        let local = LocalReader {
            inner: cell.local(),
        };
        Self { cell, local }
    }

    /// Create a new thread-local reader for this container.
    ///
    /// Each thread should create its own `LocalReader` and reuse it.
    /// The `LocalReader` is `Send` but not `Sync`.
    ///
    /// 为此容器创建一个新的线程本地读取者。
    ///
    /// 每个线程应该创建自己的 `LocalReader` 并重复使用。
    /// `LocalReader` 是 `Send` 但不是 `Sync`。
    #[inline]
    pub fn local(&self) -> LocalReader<T> {
        LocalReader {
            inner: self.cell.local(),
        }
    }

    /// Perform a write operation to update the current value.
    ///
    /// The old value is retired and will be garbage collected when safe.
    ///
    /// 执行写入操作，更新当前值。
    ///
    /// 旧值已退休，将在安全时被垃圾回收。
    #[inline]
    pub fn update(&mut self, new_value: T) {
        self.cell.store(new_value);
    }

    /// Manually trigger garbage collection.
    ///
    /// This is usually not necessary as garbage is collected automatically.
    ///
    /// 手动触发垃圾回收。
    ///
    /// 通常不需要，因为垃圾会自动回收。
    #[inline]
    pub fn collect(&mut self) {
        self.cell.collect();
    }

    /// Read the current value with RAII guard.
    ///
    /// Returns a `ReadGuard` that can be dereferenced to access the value.
    /// The value is protected until the guard is dropped.
    ///
    /// 使用 RAII 守卫读取当前值。
    ///
    /// 返回一个可以解引用来访问值的 `ReadGuard`。
    /// 在守卫被 drop 之前，值是受保护的。
    #[inline]
    pub fn load(&self) -> ReadGuard<'_, T> {
        self.local.load()
    }

    /// Atomically swap the current value with a new one.
    ///
    /// Returns the old value.
    ///
    /// 原子地将当前值与新值交换。
    ///
    /// 返回旧的值。
    #[inline]
    pub fn swap(&mut self, new_value: T) -> T
    where
        T: Clone,
    {
        let old_value = (*self.local.load()).clone();
        self.cell.store(new_value);
        old_value
    }

    /// Apply a closure function to the current value and return the result.
    ///
    /// The closure receives a reference to the current value and returns a new value.
    /// Returns a guard to the new value.
    ///
    /// 对当前值应用闭包函数并返回结果。
    ///
    /// 闭包接收当前值的引用，返回新值。
    /// 返回新值的守卫。
    #[inline]
    pub fn update_and_fetch<F>(&mut self, f: F) -> ReadGuard<'_, T>
    where
        F: FnOnce(&T) -> T,
    {
        let old_val = self.local.load();
        let new_value = f(&*old_val);
        drop(old_val);
        self.cell.store(new_value);
        self.local.load()
    }
}

// ============================================================================
// LocalReader implementation
// ============================================================================

impl<T: 'static> LocalReader<T> {
    /// Read the current value with RAII guard.
    ///
    /// Returns a `ReadGuard` that holds the pin and the reference.
    /// The pin is automatically released when the guard is dropped.
    ///
    /// 使用 RAII 守卫读取当前值。
    ///
    /// 返回一个持有 pin 和引用的 `ReadGuard`。
    /// 当守卫被 drop 时，pin 会自动释放。
    #[inline]
    pub fn load(&self) -> ReadGuard<'_, T> {
        ReadGuard {
            inner: self.inner.pin(),
        }
    }

    /// Apply a closure function to the current value and transform the result.
    ///
    /// This method reads the current value, applies the closure to transform it,
    /// and returns the transformed result.
    ///
    /// 对当前值应用闭包函数并转换结果。
    ///
    /// 这个方法读取当前值，应用闭包进行转换，并返回转换后的结果。
    #[inline]
    pub fn map<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        let guard = self.inner.pin();
        f(&*guard)
    }

    /// Apply a closure function to the current value, returning Some if the closure returns true.
    ///
    /// 对当前值应用闭包函数，如果闭包返回 true 则返回 Some。
    #[inline]
    pub fn filter<F>(&self, f: F) -> Option<ReadGuard<'_, T>>
    where
        F: FnOnce(&T) -> bool,
    {
        let guard = self.inner.pin();
        if f(&*guard) {
            Some(ReadGuard { inner: guard })
        } else {
            None
        }
    }
}

impl<T: 'static> Clone for LocalReader<T> {
    #[inline]
    fn clone(&self) -> Self {
        LocalReader {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
