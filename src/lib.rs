use crossbeam_epoch::{self as epoch, Atomic, Guard, Owned, Shared};
use std::ops::Deref;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Internal shared state
///
/// 内部共享状态
pub struct SwapState<T> {
    // Current version pointer, the version seen by readers
    // 当前版本指针，读取者看到的版本
    pub(crate) current: Atomic<T>,
}

/// Writer type, not cloneable
///
/// 写入者类型，不可Clone
pub struct Swapper<T> {
    inner: Arc<SwapState<T>>,
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
pub fn new<T>(initial: T) -> (Swapper<T>, SwapReader<T>) {
    // Fix 1: Must use pin() to safely register the initial value
    // 修复 1: 必须使用 pin() 来安全地注册初始值
    let guard = &epoch::pin();
    let owned = Owned::new(initial);
    let shared = owned.into_shared(guard);

    let inner = Arc::new(SwapState {
        current: Atomic::from(shared),
    });

    let writer = Swapper {
        inner: inner.clone(),
    };

    let reader = SwapReader { inner };

    (writer, reader)
}


impl<T> Swapper<T> {
    /// Perform a write operation to update the current version
    ///
    /// 执行写入操作，更新当前版本
    pub fn update(&mut self, new_value: T) {
        // Must use pin() to safely perform defer_destroy
        // 必须使用 pin() 来安全地进行 defer_destroy
        let guard = &epoch::pin();
        let new_owned = Owned::new(new_value);
        let new_shared = new_owned.into_shared(guard);

        // Atomically swap the pointer
        // 原子地切换指针
        let old = self.inner.current.swap(new_shared, Ordering::Release, guard);

        // Defer destruction of the old version
        // 延迟回收旧版本
        // SAFETY: `guard` is obtained via pin() and is valid
        // SAFETY: `guard` 是通过 pin() 获取的，是有效的
        unsafe {
            guard.defer_destroy(old);
        }
    }

    /// Get a read-only reference to the current value (via SwapGuard)
    ///
    /// This is a convenience method that allows writers to also read the current value
    ///
    /// 获取当前值的只读引用（通过SwapGuard）
    ///
    /// 这是一个便利方法，允许写入者也能读取当前值
    pub fn read(&self) -> Option<SwapGuard<T>> {
        let guard = epoch::pin();
        let current = self.inner.current.load(Ordering::Acquire, &guard);

        if current.is_null() {
            return None;
        }

        let value = unsafe {
            current.as_ref().unwrap() as *const T
        };

        Some(SwapGuard {
            _guard: guard,
            value,
        })
    }

    /// Apply a closure function to the current value and return the result
    ///
    /// The closure receives a reference to the current value and returns a new value.
    /// Returns None if the container has been destroyed.
    ///
    /// 对当前值应用闭包函数并返回结果
    ///
    /// 闭包接收当前值的引用，返回新值
    /// 如果容器已被销毁，返回 None
    pub fn update_and_fetch<F>(&mut self, f: F) -> Option<SwapGuard<T>>
    where
        F: FnOnce(&T) -> T,
    {
        if let Some(guard) = self.read() {
            let new_value = f(&*guard);
            drop(guard);
            self.update(new_value);
            self.read()
        } else {
            None
        }
    }
}

impl<T> Swapper<Arc<T>> {
    /// Atomically swap the current Arc value with a new one
    ///
    /// This method replaces the current Arc-wrapped value with a new one and returns the old value.
    /// Returns None if the container has been destroyed.
    ///
    /// 原子地将当前 Arc 值与新值交换
    ///
    /// 这个方法用新的 Arc 包装值替换当前值，并返回旧值
    /// 如果容器已被销毁，返回 None
    pub fn swap(&mut self, new_value: Arc<T>) -> Option<Arc<T>> {
        // Must use pin() to safely perform defer_destroy
        // 必须使用 pin() 来安全地进行 defer_destroy
        let guard = &epoch::pin();

        let new_owned = Owned::new(new_value);
        let new_shared = new_owned.into_shared(guard);

        // Atomically swap the pointer
        // 原子地切换指针
        let old_shared = self.inner.current.swap(new_shared, Ordering::Release, guard);

        if old_shared.is_null() {
            return None;
        }

        unsafe {
            // SAFETY:
            // - We checked that old_shared is not null
            // - guard ensures the epoch does not advance and the version is not reclaimed
            // SAFETY:
            // - 我们检查了 old_shared 不为 null
            // - guard 确保 epoch 不会推进，版本不会被回收
            let old_owned = old_shared.deref().clone();

            // Defer destruction of the old version
            // 延迟回收旧版本
            guard.defer_destroy(old_shared);

            Some(old_owned)
        }
    }

    /// Apply a closure function to the current value and return the result as an Arc
    ///
    /// This method reads the current value, passes it to the closure which returns a new value,
    /// then wraps the new value in an Arc and swaps it with the current value.
    /// Returns None if the container has been destroyed.
    ///
    /// 对当前值应用闭包函数并将结果作为 Arc 返回
    ///
    /// 这个方法读取当前值，将其传递给闭包（闭包返回新值），
    /// 然后将新值包装在 Arc 中并与当前值交换
    /// 如果容器已被销毁，返回 None
    pub fn update_and_fetch_arc<F>(&mut self, f: F) -> Option<Arc<T>>
    where
        F: FnOnce(&Arc<T>) -> Arc<T>,
    {
        let current_guard = self.read();
        if let Some(guard) = current_guard {
            let new_value = f(&*guard);
            drop(guard);
            self.swap(new_value.clone());
            Some(new_value)
        } else {
            None
        }
    }
}

impl<T> SwapReader<T> {
    /// Read the current version (lock-free)
    ///
    /// Returns a SwapGuard that ensures the version will not be reclaimed while in use.
    /// Returns None if the container has been destroyed.
    ///
    /// 读取当前版本（无锁）
    ///
    /// 返回一个SwapGuard，确保在使用期间版本不会被回收
    /// 如果容器已被销毁，则返回 None。
    pub fn read(&self) -> Option<SwapGuard<T>> {
        let guard = epoch::pin();
        let current = self.inner.current.load(Ordering::Acquire, &guard);

        // Check for null to prevent race condition panic during drop
        // 检查是否为 null，防止 drop 时的竞态 panic
        if current.is_null() {
            return None;
        }

        let value = unsafe {
            // SAFETY:
            // - We checked that current is not null
            // - guard ensures the epoch does not advance and the version is not reclaimed
            // SAFETY:
            // - 我们检查了 current 不为 null
            // - guard 确保 epoch 不会推进，版本不会被回收
            current.as_ref().unwrap() as *const T
        };

        Some(SwapGuard {
            _guard: guard,
            value,
        })
    }

    /// Apply a closure function to the current value
    ///
    /// This method reads the current value, passes it to the closure, and returns the closure's result.
    /// The closure is executed under the protection of SwapGuard, ensuring the value is not modified.
    ///
    /// 对当前值应用闭包函数
    ///
    /// 这个方法读取当前值，将其传递给闭包，然后返回闭包的结果
    /// 闭包在 SwapGuard 的保护下执行，确保值不会被修改
    pub fn map<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        self.read().map(|guard| f(&*guard))
    }

    /// Apply a closure function to the current value, returning Some if the closure returns true, otherwise None
    ///
    /// 对当前值应用闭包函数，如果闭包返回 true 则返回 Some，否则返回 None
    pub fn filter<F>(&self, f: F) -> Option<SwapGuard<T>>
    where
        F: FnOnce(&T) -> bool,
    {
        let guard = self.read()?;
        if f(&*guard) {
            Some(guard)
        } else {
            None
        }
    }

    /// Try to read, returning a default value if it fails
    ///
    /// 尝试读取，如果失败则返回默认值
    pub fn read_or<F>(&self, f: F) -> SwapGuard<T>
    where
        F: FnOnce() -> T,
    {
        self.read().unwrap_or_else(|| {
            let guard = epoch::pin();
            let default_value = f();
            let default_ptr = Box::leak(Box::new(default_value)) as *const T;
            SwapGuard {
                _guard: guard,
                value: default_ptr,
            }
        })
    }

    /// Get a clone of the internal Arc
    ///
    /// 获取对内部 Arc 的克隆
    pub fn clone_inner(&self) -> Arc<SwapState<T>> {
        self.inner.clone()
    }

    /// Try to convert SwapGuard to an owned value (if possible)
    ///
    /// This method attempts to clone the value (if T implements Clone).
    ///
    /// 尝试将 SwapGuard 转换为拥有的值（如果可能）
    ///
    /// 这个方法尝试克隆值（如果 T 实现了 Clone）
    pub fn try_clone_value(&self) -> Option<T>
    where
        T: Clone,
    {
        self.read().map(|guard| guard.deref().clone())
    }
}

/// Read guard that holds an epoch pin to ensure the version is not reclaimed
///
/// 读取守卫，持有epoch pin确保版本不被回收
#[must_use = "SwapGuard must be held to ensure data is not reclaimed during access / SwapGuard 必须被持有以确保数据在访问期间不会被回收"]
pub struct SwapGuard<T> {
    _guard: Guard,
    value: *const T,
}

impl<T> Deref for SwapGuard<T> {
    type Target = T;

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

impl<T: Clone> SwapGuard<T> {
    /// Clone the protected value
    ///
    /// 克隆被保护的值
    pub fn clone_value(&self) -> T {
        self.deref().clone()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for SwapGuard<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwapGuard")
            .field("value", &**self)
            .finish()
    }
}

impl<T: std::fmt::Display> std::fmt::Display for SwapGuard<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", **self)
    }
}

impl<T> Drop for SwapState<T> {
    fn drop(&mut self) {
        // Fix 1: Must use pin() to safely destroy the last value
        // 修复 1: 必须使用 pin() 来安全地销毁最后一个值
        let guard = &epoch::pin();
        let current = self.current.swap(Shared::null(), Ordering::Relaxed, guard);
        if !current.is_null() {
            // SAFETY: `guard` is obtained via pin() and is valid
            // SAFETY: `guard` 是通过 pin() 获取的，是有效的
            unsafe {
                guard.defer_destroy(current);
            }
        }
    }
}

// SAFETY: Swapper<T> is Send+Sync when T is Send+Sync
// SAFETY: Swapper<T>是Send+Sync当T是Send+Sync
unsafe impl<T: Send + Sync> Send for Swapper<T> {}
unsafe impl<T: Send + Sync> Sync for Swapper<T> {}

// SAFETY: SwapReader<T> is Send+Sync when T is Send+Sync
// SAFETY: SwapReader<T>是Send+Sync当T是Send+Sync
unsafe impl<T: Send + Sync> Send for SwapReader<T> {}
unsafe impl<T: Send + Sync> Sync for SwapReader<T> {}


#[cfg(test)]
mod tests;