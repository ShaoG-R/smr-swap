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
//! // Writer stores a new value
//! swap.store(1);
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

use std::fmt;
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

    /// Store a new value, making it visible to readers.
    ///
    /// The old value is retired and will be garbage collected when safe.
    ///
    /// 存储新值，使其对读者可见。
    ///
    /// 旧值已退休，将在安全时被垃圾回收。
    #[inline]
    pub fn store(&mut self, new_value: T) {
        self.cell.store(new_value);
    }

    /// Get a reference to the current value (writer-only, no pinning required).
    ///
    /// This is only accessible from the writer thread since `SmrSwap` is `!Sync`.
    ///
    /// 获取当前值的引用（仅写者可用，无需 pin）。
    ///
    /// 这只能从写者线程访问，因为 `SmrSwap` 是 `!Sync` 的。
    #[inline]
    pub fn get(&self) -> &T {
        self.cell.get()
    }

    /// Update the value using a closure.
    ///
    /// The closure receives the current value and should return the new value.
    /// This is equivalent to `swap.store(f(swap.get()))` but more ergonomic.
    ///
    /// 使用闭包更新值。
    ///
    /// 闭包接收当前值并应返回新值。
    /// 这相当于 `swap.store(f(swap.get()))` 但更符合人体工程学。
    #[inline]
    pub fn update<F>(&mut self, f: F)
    where
        F: FnOnce(&T) -> T,
    {
        self.cell.update(f);
    }

    /// Get the current global version.
    ///
    /// The version is incremented each time `store()` is called.
    ///
    /// 获取当前全局版本。
    ///
    /// 每次调用 `store()` 时版本会增加。
    #[inline]
    pub fn version(&self) -> usize {
        self.cell.version()
    }

    /// Get the number of retired objects waiting for garbage collection.
    ///
    /// 获取等待垃圾回收的已退休对象数量。
    #[inline]
    pub fn garbage_count(&self) -> usize {
        self.cell.garbage_count()
    }

    /// Get a reference to the previously stored value, if any.
    ///
    /// Returns `None` if no previous value exists (i.e., only the initial value has been stored).
    ///
    /// 获取上一个存储值的引用（如果存在）。
    ///
    /// 如果不存在上一个值（即只存储了初始值），则返回 `None`。
    #[inline]
    pub fn previous(&self) -> Option<&T> {
        self.cell.previous()
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

    /// Load the current value and clone it.
    ///
    /// This is a convenience method equivalent to `self.load().cloned()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use smr_swap::SmrSwap;
    ///
    /// let swap = SmrSwap::new(String::from("hello"));
    ///
    /// // Instead of: (*swap.load()).clone()
    /// // Or: swap.load().cloned()
    /// let value: String = swap.load_cloned();
    /// assert_eq!(value, "hello");
    /// ```
    ///
    /// 加载当前值并克隆它。
    ///
    /// 这是一个便捷方法，等同于 `self.load().cloned()`。
    #[inline]
    pub fn load_cloned(&self) -> T
    where
        T: Clone,
    {
        self.load().cloned()
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
        let old_value = self.cell.get().clone();
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
        let new_value = f(self.cell.get());
        self.cell.store(new_value);
        self.local.load()
    }

    /// Apply a closure function to the current value with mutable access to both.
    ///
    /// The closure receives the current value and should return the new value.
    /// Returns a guard to the old value (before update).
    ///
    /// 对当前值应用闭包函数，具有对两者的可变访问。
    ///
    /// 闭包接收当前值并应返回新值。
    /// 返回旧值（更新前）的守卫。
    #[inline]
    pub fn fetch_and_update<F>(&mut self, f: F) -> ReadGuard<'_, T>
    where
        F: FnOnce(&T) -> T,
    {
        let old_guard = self.local.load();
        let new_value = f(self.cell.get());
        self.cell.store(new_value);
        old_guard
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

    /// Check if this reader is currently pinned.
    ///
    /// 检查此读者当前是否被 pin。
    #[inline]
    pub fn is_pinned(&self) -> bool {
        self.inner.is_pinned()
    }

    /// Get the current global version.
    ///
    /// Note: This returns the global version, not the pinned version.
    /// To get the pinned version, use `ReadGuard::version()`.
    ///
    /// 获取当前全局版本。
    ///
    /// 注意：这返回全局版本，而不是 pin 的版本。
    /// 要获取 pin 的版本，请使用 `ReadGuard::version()`。
    #[inline]
    pub fn version(&self) -> usize {
        self.inner.version()
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

    /// Load the current value and clone it.
    ///
    /// This is a convenience method equivalent to `self.load().cloned()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use smr_swap::SmrSwap;
    ///
    /// let swap = SmrSwap::new(String::from("hello"));
    /// let local = swap.local();
    ///
    /// // Instead of: (*local.load()).clone()
    /// // Or: local.load().cloned()
    /// let value: String = local.load_cloned();
    /// assert_eq!(value, "hello");
    /// ```
    ///
    /// 加载当前值并克隆它。
    ///
    /// 这是一个便捷方法，等同于 `self.load().cloned()`。
    #[inline]
    pub fn load_cloned(&self) -> T
    where
        T: Clone,
    {
        self.load().cloned()
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

impl<T: 'static> fmt::Debug for LocalReader<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalReader")
            .field("is_pinned", &self.is_pinned())
            .field("version", &self.version())
            .finish()
    }
}

// ============================================================================
// ReadGuard additional implementations
// ============================================================================

impl<T: 'static> ReadGuard<'_, T> {
    /// Get the version that this guard is pinned to.
    ///
    /// 获取此守卫被 pin 到的版本。
    #[inline]
    pub fn version(&self) -> usize {
        self.inner.version()
    }

    /// Clone the inner value and return it.
    ///
    /// This is useful when you need to return the value instead of the guard.
    ///
    /// # Example
    ///
    /// ```rust
    /// use smr_swap::SmrSwap;
    ///
    /// let swap = SmrSwap::new(42);
    /// let value: i32 = swap.load().cloned();
    /// assert_eq!(value, 42);
    /// ```
    ///
    /// 克隆内部值并返回。
    ///
    /// 当你需要返回值而不是守卫时很有用。
    #[inline]
    pub fn cloned(&self) -> T
    where
        T: Clone,
    {
        self.deref().clone()
    }

    /// Convert the guard into the inner value by cloning.
    ///
    /// This consumes the guard and returns a clone of the inner value.
    ///
    /// 通过克隆将守卫转换为内部值。
    ///
    /// 这会消耗守卫并返回内部值的克隆。
    #[inline]
    pub fn into_inner(self) -> T
    where
        T: Clone,
    {
        self.deref().clone()
    }
}

impl<T: 'static> AsRef<T> for ReadGuard<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T: fmt::Debug + 'static> fmt::Debug for ReadGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadGuard")
            .field("value", &self.deref())
            .field("version", &self.version())
            .finish()
    }
}

// ============================================================================
// Standard Trait Implementations
// 标准 trait 实现
// ============================================================================

impl<T: Default + 'static> Default for SmrSwap<T> {
    /// Create a new SmrSwap with the default value.
    ///
    /// 使用默认值创建一个新的 SmrSwap。
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: 'static> From<T> for SmrSwap<T> {
    /// Create a new SmrSwap from a value.
    ///
    /// 从一个值创建一个新的 SmrSwap。
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: fmt::Debug + 'static> fmt::Debug for SmrSwap<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmrSwap")
            .field("value", self.get())
            .field("version", &self.version())
            .field("garbage_count", &self.garbage_count())
            .finish()
    }
}

#[cfg(test)]
mod tests;
