use std::ops::Deref;
use std::sync::Arc;
use swmr_epoch::{EpochGcDomain, EpochPtr, GcHandle, LocalEpoch, PinGuard};

/// Internal shared state
///
/// 内部共享状态
pub struct SwapState<T> {
    // Current version pointer, the version seen by readers
    // 当前版本指针，读取者看到的版本
    pub(crate) current: EpochPtr<T>,
    // GC domain for managing garbage collection
    // 用于管理垃圾回收的 GC 域
    pub(crate) domain: EpochGcDomain,
}

/// Writer type, not cloneable
///
/// 写入者类型，不可Clone
pub struct Swapper<T> {
    inner: Arc<SwapState<T>>,
    // Garbage collector handle, held directly by Swapper
    // 垃圾回收器句柄，由 Swapper 直接持有
    gc: GcHandle,
}

/// Reader type, cloneable
///
/// 读取者类型，可Clone
#[derive(Clone)]
pub struct SwapReader<T> {
    inner: Arc<SwapState<T>>,
}

/// Create a new SMR container, returning a (Swapper, SwapReader) tuple
///
/// 创建新的SMR容器，返回(Swapper, SwapReader)元组
pub fn new<T: 'static>(initial: T) -> (Swapper<T>, SwapReader<T>) {
    // Create the epoch GC domain
    // 创建 epoch GC 域
    let (gc, domain) = EpochGcDomain::builder()
        .auto_reclaim_threshold(None)
        .cleanup_interval(2)
        .build();

    let inner = Arc::new(SwapState {
        current: EpochPtr::new(initial),
        domain: domain.clone(),
    });

    let swapper = Swapper {
        inner: inner.clone(),
        gc,
    };

    let reader = SwapReader { inner };

    (swapper, reader)
}

impl<T: 'static> Swapper<T> {
    /// Perform a write operation to update the current version
    ///
    /// 执行写入操作，更新当前版本
    #[inline]
    pub fn update(&mut self, new_value: T) {
        // Store the new value and retire the old one
        // 存储新值并退休旧值
        self.inner.current.store(new_value, &mut self.gc);

        // Trigger garbage collection
        // 触发垃圾回收
        self.gc.collect();
    }

    /// Get a read-only reference to the current value (via SwapGuard)
    ///
    /// This is a convenience method that allows writers to also read the current value.
    /// The writer must provide a LocalEpoch to pin itself.
    ///
    /// 获取当前值的只读引用（通过SwapGuard）
    ///
    /// 这是一个便利方法，允许写入者也能读取当前值。
    /// 写入者必须提供 LocalEpoch 来 pin 自己。
    #[inline]
    pub fn read<'a>(&self, local_epoch: &'a LocalEpoch) -> SwapGuard<'a, T> {
        let guard = local_epoch.pin();
        let value_ref = self.inner.current.load(&guard);
        let value_ptr = value_ref as *const T;

        SwapGuard {
            _guard: guard,
            value: value_ptr,
        }
    }

    /// Execute a closure with a guard, allowing multiple operations on the same pinned version
    ///
    /// This method pins once and passes the SwapGuard to the closure.
    /// Useful for performing multiple operations on the same version without re-pinning.
    /// The guard is held for the entire duration of the closure execution.
    ///
    /// 使用guard执行闭包，允许在同一个pinned版本上执行多个操作
    ///
    /// 这个方法只pin一次，并将SwapGuard传递给闭包
    /// 用于在不重新pin的情况下对同一版本执行多个操作
    /// guard在整个闭包执行期间被持有
    #[inline]
    pub fn read_with_guard<F, R>(&self, local_epoch: &LocalEpoch, f: F) -> R
    where
        F: FnOnce(&SwapGuard<T>) -> R,
    {
        let guard = self.read(local_epoch);
        f(&guard)
    }

    /// Apply a closure function to the current value and transform the result
    ///
    /// This method reads the current value, applies the closure to transform it,
    /// and returns the transformed result. The guard is automatically released after transformation.
    ///
    /// 对当前值应用闭包函数并转换结果
    ///
    /// 这个方法读取当前值，应用闭包进行转换，并返回转换后的结果
    /// guard在转换后自动释放
    #[inline]
    pub fn map<F, U>(&self, local_epoch: &LocalEpoch, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        let guard = self.read(local_epoch);
        f(&*guard)
    }

    /// Apply a closure function to the current value and return the result
    ///
    /// The closure receives a reference to the current value and returns a new value.
    /// Returns a SwapGuard to the new value.
    ///
    /// 对当前值应用闭包函数并返回结果
    ///
    /// 闭包接收当前值的引用，返回新值
    /// 返回新值的 SwapGuard
    #[inline]
    pub fn update_and_fetch<'a, F>(&mut self, local_epoch: &'a LocalEpoch, f: F) -> SwapGuard<'a, T>
    where
        F: FnOnce(&T) -> T,
    {
        let guard = self.read(local_epoch);
        let new_value = f(&*guard);
        drop(guard);
        self.update(new_value);

        // Trigger garbage collection
        // 触发垃圾回收
        self.gc.collect();

        self.read(local_epoch)
    }

    /// Register a new reader for the current thread
    ///
    /// Returns a `LocalEpoch` that should be stored per-thread.
    /// The caller is responsible for ensuring that each `LocalEpoch` is used
    /// by only one thread.
    ///
    /// 为当前线程注册一个新的读者
    ///
    /// 返回一个应该在每个线程中存储的 `LocalEpoch`。
    /// 调用者有责任确保每个 `LocalEpoch` 仅由一个线程使用。
    #[inline]
    pub fn register_reader(&self) -> LocalEpoch {
        self.inner.domain.register_reader()
    }
}

impl<T: 'static> Swapper<Arc<T>> {
    /// Atomically swap the current Arc value with a new one
    ///
    /// This method replaces the current Arc-wrapped value with a new one and returns the old value.
    ///
    /// 原子地将当前 Arc 值与新值交换
    ///
    /// 这个方法用新的 Arc 包装值替换当前值，并返回旧值
    #[inline]
    pub fn swap(&mut self, local_epoch: &LocalEpoch, new_value: Arc<T>) -> Arc<T> {
        // Read the current value before swapping
        // 在交换前读取当前值
        let guard = local_epoch.pin();
        let old_value = self.inner.current.load(&guard).clone();
        drop(guard);

        // Store the new value and retire the old one
        // 存储新值并退休旧值
        self.inner.current.store(new_value, &mut self.gc);

        old_value
    }

    /// Apply a closure function to the current value and return the result as an Arc
    ///
    /// This method reads the current value, passes it to the closure which returns a new value,
    /// then wraps the new value in an Arc and swaps it with the current value.
    /// Returns the new Arc value.
    ///
    /// 对当前值应用闭包函数并将结果作为 Arc 返回
    ///
    /// 这个方法读取当前值，将其传递给闭包（闭包返回新值），
    /// 然后将新值包装在 Arc 中并与当前值交换
    /// 返回新的 Arc 值
    #[inline]
    pub fn update_and_fetch_arc<F>(&mut self, local_epoch: &LocalEpoch, f: F) -> Arc<T>
    where
        F: FnOnce(&Arc<T>) -> Arc<T>,
    {
        let guard = self.read(local_epoch);
        let new_value = f(&*guard);
        drop(guard);
        self.swap(local_epoch, new_value.clone());
        new_value
    }
}

impl<T: 'static> SwapReader<T> {
    /// Read the current version (lock-free)
    ///
    /// Returns a SwapGuard that ensures the version will not be reclaimed while in use.
    /// The reader must provide a LocalEpoch to pin itself.
    ///
    /// 读取当前版本（无锁）
    ///
    /// 返回一个SwapGuard，确保在使用期间版本不会被回收
    /// 读取者必须提供 LocalEpoch 来 pin 自己
    #[inline]
    pub fn read<'a>(&self, local_epoch: &'a LocalEpoch) -> SwapGuard<'a, T> {
        let guard = local_epoch.pin();
        let value_ref = self.inner.current.load(&guard);
        let value_ptr = value_ref as *const T;

        SwapGuard {
            _guard: guard,
            value: value_ptr,
        }
    }

    /// Execute a closure with a guard, allowing multiple operations on the same pinned version
    ///
    /// This method pins once and passes the SwapGuard to the closure.
    /// Useful for performing multiple operations on the same version without re-pinning.
    /// The guard is held for the entire duration of the closure execution.
    ///
    /// 使用guard执行闭包，允许在同一个pinned版本上执行多个操作
    ///
    /// 这个方法只pin一次，并将SwapGuard传递给闭包
    /// 用于在不重新pin的情况下对同一版本执行多个操作
    /// guard在整个闭包执行期间被持有
    #[inline]
    pub fn read_with_guard<'a, F, R>(&self, local_epoch: &'a LocalEpoch, f: F) -> R
    where
        F: FnOnce(&SwapGuard<'a, T>) -> R,
    {
        let guard = self.read(local_epoch);
        f(&guard)
    }

    /// Apply a closure function to the current value and transform the result
    ///
    /// This method reads the current value, applies the closure to transform it,
    /// and returns the transformed result. The guard is automatically released after transformation.
    ///
    /// 对当前值应用闭包函数并转换结果
    ///
    /// 这个方法读取当前值，应用闭包进行转换，并返回转换后的结果
    /// guard在转换后自动释放
    #[inline]
    pub fn map<'a, F, U>(&self, local_epoch: &'a LocalEpoch, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        let guard = self.read(local_epoch);
        f(&*guard)
    }

    /// Apply a closure function to the current value, returning Some if the closure returns true, otherwise None
    ///
    /// 对当前值应用闭包函数，如果闭包返回 true 则返回 Some，否则返回 None
    #[inline]
    pub fn filter<'a, F>(&self, local_epoch: &'a LocalEpoch, f: F) -> Option<SwapGuard<'a, T>>
    where
        F: FnOnce(&T) -> bool,
    {
        let guard = self.read(local_epoch);
        if f(&*guard) { Some(guard) } else { None }
    }

    /// Register a new reader for the current thread
    ///
    /// Returns a `LocalEpoch` that should be stored per-thread.
    /// The caller is responsible for ensuring that each `LocalEpoch` is used
    /// by only one thread.
    ///
    /// 为当前线程注册一个新的读者
    ///
    /// 返回一个应该在每个线程中存储的 `LocalEpoch`。
    /// 调用者有责任确保每个 `LocalEpoch` 仅由一个线程使用。
    #[inline]
    pub fn register_reader(&self) -> LocalEpoch {
        self.inner.domain.register_reader()
    }
}

/// Read guard that holds an epoch pin to ensure the version is not reclaimed
///
/// 读取守卫，持有epoch pin确保版本不被回收
#[must_use = "SwapGuard must be held to ensure data is not reclaimed during access / SwapGuard 必须被持有以确保数据在访问期间不会被回收"]
pub struct SwapGuard<'a, T> {
    _guard: PinGuard<'a>,
    value: *const T,
}

impl<'a, T> Deref for SwapGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe {
            // SAFETY:
            // - The value pointer is valid for the lifetime of SwapGuard
            // - SwapGuard holds _guard to ensure the version is not reclaimed
            // SAFETY:
            // - value 指针在 SwapGuard 生命周期内有效
            // - SwapGuard 持有 _guard 确保版本不被回收
            &*self.value
        }
    }
}

impl<'a, T: Clone> SwapGuard<'a, T> {
    /// Clone the protected value
    ///
    /// 克隆被保护的值
    #[inline]
    pub fn clone_value(&self) -> T {
        self.deref().clone()
    }
}

impl<'a, T: std::fmt::Debug> std::fmt::Debug for SwapGuard<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwapGuard").field("value", &**self).finish()
    }
}

impl<'a, T: std::fmt::Display> std::fmt::Display for SwapGuard<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", **self)
    }
}

// SAFETY: Swapper<T> is Send when T is Send + 'static
// SAFETY: Swapper<T>是Send当T是Send + 'static
unsafe impl<T: Send + 'static> Send for Swapper<T> {}

#[cfg(test)]
mod tests;
