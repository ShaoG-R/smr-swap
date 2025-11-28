//! Basic functionality tests for SMR-Swap
//!
//! Tests core operations: creation, reading, updating, and basic reader operations

use crate::{LocalReader, SmrSwap};

/// Test basic creation and reading with integers
/// 测试基本的创建和读取（整数）
#[test]
fn test_basic_new_and_read_int() {
    let swap = SmrSwap::new(42);
    let guard = swap.load();
    assert_eq!(*guard, 42);
}

/// Test basic creation and reading with strings
/// 测试基本的创建和读取（字符串）
#[test]
fn test_basic_new_and_read_string() {
    let swap = SmrSwap::new(String::from("hello"));
    let guard = swap.load();
    assert_eq!(*guard, "hello");
}

/// Test basic creation and reading with vectors
/// 测试基本的创建和读取（向量）
#[test]
fn test_basic_new_and_read_vector() {
    let swap = SmrSwap::new(vec![1, 2, 3, 4, 5]);
    let guard = swap.load();
    assert_eq!(*guard, vec![1, 2, 3, 4, 5]);
}

/// Test basic store operation with integers
/// 测试基本的存储操作（整数）
#[test]
fn test_basic_store_int() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    assert_eq!(*reader.load(), 10);

    swap.store(20);
    assert_eq!(*reader.load(), 20);
}

/// Test basic store operation with strings
/// 测试基本的存储操作（字符串）
#[test]
fn test_basic_store_string() {
    let mut swap = SmrSwap::new(String::from("hello"));
    let reader = swap.local();

    assert_eq!(*reader.load(), "hello");

    swap.store(String::from("world"));
    assert_eq!(*reader.load(), "world");
}

/// Test multiple sequential stores
/// 测试多个连续的存储
#[test]
fn test_multiple_stores() {
    let mut swap = SmrSwap::new(0);
    let reader = swap.local();

    for i in 1..=10 {
        swap.store(i);
        assert_eq!(*reader.load(), i);
    }
}

/// Test reader cloning with integers
/// 测试读取者克隆（整数）
#[test]
fn test_reader_clone_int() {
    let mut swap = SmrSwap::new(10);
    let reader1 = swap.local();
    let reader2 = reader1.clone();

    assert_eq!(*reader1.load(), 10);
    assert_eq!(*reader2.load(), 10);

    swap.store(20);

    assert_eq!(*reader1.load(), 20);
    assert_eq!(*reader2.load(), 20);
}

/// Test reader cloning with strings
/// 测试读取者克隆（字符串）
#[test]
fn test_reader_clone_string() {
    let mut swap = SmrSwap::new(String::from("initial"));
    let reader1 = swap.local();
    let reader2 = reader1.clone();
    let reader3 = reader2.clone();

    assert_eq!(*reader1.load(), "initial");
    assert_eq!(*reader2.load(), "initial");
    assert_eq!(*reader3.load(), "initial");

    swap.store(String::from("updated"));

    assert_eq!(*reader1.load(), "updated");
    assert_eq!(*reader2.load(), "updated");
    assert_eq!(*reader3.load(), "updated");
}

/// Test multiple readers see consistent values
/// 测试多个读取者看到一致的值
#[test]
fn test_multiple_readers_consistency() {
    let mut swap = SmrSwap::new(0);
    let reader1 = swap.local();
    let reader2 = reader1.clone();
    let reader3 = reader2.clone();

    // All readers should see the same initial value
    // 所有读取者应该看到相同的初始值
    assert_eq!(*reader1.load(), 0);
    assert_eq!(*reader2.load(), 0);
    assert_eq!(*reader3.load(), 0);

    // Store a new value
    // 存储新值
    swap.store(1);

    // All readers should see the same updated value
    // 所有读取者应该看到相同的更新值
    assert_eq!(*reader1.load(), 1);
    assert_eq!(*reader2.load(), 1);
    assert_eq!(*reader3.load(), 1);

    // Store again
    // 再次存储
    swap.store(2);

    // All readers should see the same new value
    // 所有读取者应该看到相同的新值
    assert_eq!(*reader1.load(), 2);
    assert_eq!(*reader2.load(), 2);
    assert_eq!(*reader3.load(), 2);
}

/// Test that read guard holds value across updates
/// 测试读取守卫在更新后仍然持有值
#[test]
fn test_read_guard_holds_value() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    let guard1 = reader.load();
    assert_eq!(*guard1, 10);

    swap.store(20);

    let guard2 = reader.load();
    assert_eq!(*guard2, 20);

    // Critical: V1's guard must still be valid!
    // 关键：V1 的 guard 必须仍然有效！
    assert_eq!(*guard1, 10);

    drop(guard2);
    drop(guard1);

    swap.store(30);
    assert_eq!(*reader.load(), 30);
}

/// Test that multiple held guards remain valid
/// 测试多个持有的 guard 保持有效
#[test]
fn test_multiple_held_guards() {
    let mut swap = SmrSwap::new(0);
    let reader = swap.local();

    // Get multiple guards
    // 获取多个守卫
    let guard1 = reader.load();
    let guard2 = reader.load();
    let guard3 = reader.load();

    // All guards should see the same value
    // 所有守卫应该看到相同的值
    assert_eq!(*guard1, 0);
    assert_eq!(*guard2, 0);
    assert_eq!(*guard3, 0);

    // Store a new value
    // 存储新值
    swap.store(1);

    // Old guards should still be valid
    // 旧的守卫应该仍然有效
    assert_eq!(*guard1, 0);
    assert_eq!(*guard2, 0);
    assert_eq!(*guard3, 0);

    // New guard should see the new value
    // 新的守卫应该看到新值
    let guard4 = reader.load();
    assert_eq!(*guard4, 1);
}

/// Test Send trait bounds compilation
/// 测试 Send 特性约束编译
#[test]
fn test_send_compilation() {
    fn assert_send<T: Send>() {}

    // SmrSwap is Send
    assert_send::<SmrSwap<i32>>();
    // LocalReader is Send but not Sync
    assert_send::<LocalReader<i32>>();
}

/// Test with Box-wrapped values
/// 测试使用 Box 包装的值
#[test]
fn test_with_boxed_values() {
    let mut swap = SmrSwap::new(Box::new(42));
    let reader = swap.local();

    let guard = reader.load();
    assert_eq!(**guard, 42);
    drop(guard);

    swap.store(Box::new(100));
    assert_eq!(**reader.load(), 100);
}

/// Test with Arc-wrapped values
/// 测试使用 Arc 包装的值
#[test]
fn test_with_arc_values() {
    use std::sync::Arc;

    let mut swap = SmrSwap::new(Arc::new(42));
    let reader = swap.local();

    let guard1 = reader.load();
    assert_eq!(**guard1, 42);

    swap.store(Arc::new(100));
    let guard2 = reader.load();
    assert_eq!(**guard2, 100);
}

/// Test map functionality
/// 测试 map 功能
#[test]
fn test_map() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    // Map the value to a new type
    let doubled = reader.map(|val| *val * 2);
    assert_eq!(doubled, 20);

    // Store a new value
    swap.store(20);

    // Map the new value
    let tripled = reader.map(|val| *val * 3);
    assert_eq!(tripled, 60);
}

/// Test filter functionality
/// 测试 filter 功能
#[test]
fn test_filter() {
    let mut swap = SmrSwap::new(Some(42));
    let reader = swap.local();

    // Filter with Some
    let val = reader.filter(|val| val.is_some()).unwrap();
    assert_eq!(*val, Some(42));

    // Store None
    swap.store(None::<i32>);

    // Filter with None
    let result = reader.filter(|val| val.is_some());
    assert!(result.is_none());
}

/// Test update_and_fetch functionality
/// 测试 update_and_fetch 功能
#[test]
fn test_update_and_fetch() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    let guard = swap.update_and_fetch(|val| val + 5);
    assert_eq!(*guard, 15);

    // Verify with a reader
    assert_eq!(*reader.load(), 15);
}

/// Test swap functionality
/// 测试 swap 功能
#[test]
fn test_swap() {
    let mut swap = SmrSwap::new(10);

    let old = swap.swap(20);
    assert_eq!(old, 10);

    let reader = swap.local();
    assert_eq!(*reader.load(), 20);
}

/// Test swap with Arc
/// 测试 Arc 的 swap 功能
#[test]
fn test_swap_arc() {
    use std::sync::Arc;
    let mut swap = SmrSwap::new(Arc::new(10));

    let old = swap.swap(Arc::new(20));
    assert_eq!(*old, 10);

    let reader = swap.local();
    assert_eq!(**reader.load(), 20);
}

/// Test manual garbage collection
/// 测试手动垃圾回收
#[test]
fn test_manual_collect() {
    let mut swap = SmrSwap::new(0);

    for i in 1..=100 {
        swap.store(i);
    }

    // Manual collect should not panic
    swap.collect();

    assert_eq!(*swap.load(), 100);
}

/// Test load from SmrSwap directly
/// 测试从 SmrSwap 直接读取
#[test]
fn test_smrswap_load() {
    let mut swap = SmrSwap::new(42);

    assert_eq!(*swap.load(), 42);

    swap.store(100);
    assert_eq!(*swap.load(), 100);
}

/// Test update with closure
/// 测试使用闭包更新
#[test]
fn test_update_with_closure() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    swap.update(|val| val + 5);
    assert_eq!(*reader.load(), 15);

    swap.update(|val| val * 2);
    assert_eq!(*reader.load(), 30);
}

/// Test get method (writer-only)
/// 测试 get 方法（仅写者）
#[test]
fn test_get() {
    let mut swap = SmrSwap::new(42);

    assert_eq!(*swap.get(), 42);

    swap.store(100);
    assert_eq!(*swap.get(), 100);
}

/// Test version tracking
/// 测试版本跟踪
#[test]
fn test_version() {
    let mut swap = SmrSwap::new(0);
    let reader = swap.local();

    let v0 = swap.version();

    swap.store(1);
    let v1 = swap.version();
    assert!(v1 > v0);

    swap.store(2);
    let v2 = swap.version();
    assert!(v2 > v1);

    // Reader version should match
    assert_eq!(reader.version(), v2);
}

/// Test garbage_count
/// 测试 garbage_count
#[test]
fn test_garbage_count() {
    let mut swap = SmrSwap::new(0);

    assert_eq!(swap.garbage_count(), 0);

    swap.store(1);
}

/// Test previous value
/// 测试上一个值
#[test]
fn test_previous() {
    let mut swap = SmrSwap::new(1);

    assert!(swap.previous().is_none()); // No previous value yet

    swap.store(2);
    assert_eq!(swap.previous(), Some(&1));

    swap.store(3);
    assert_eq!(swap.previous(), Some(&2));
}

/// Test fetch_and_update
/// 测试 fetch_and_update
#[test]
fn test_fetch_and_update() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    let old_guard = swap.fetch_and_update(|val| val + 5);
    assert_eq!(*old_guard, 10); // Returns old value
    assert_eq!(*reader.load(), 15); // New value
}

/// Test Default trait
/// 测试 Default trait
#[test]
fn test_default() {
    let swap: SmrSwap<i32> = SmrSwap::default();
    assert_eq!(*swap.get(), 0);

    let swap: SmrSwap<String> = SmrSwap::default();
    assert_eq!(*swap.get(), "");
}

/// Test From trait
/// 测试 From trait
#[test]
fn test_from() {
    let swap: SmrSwap<i32> = SmrSwap::from(42);
    assert_eq!(*swap.get(), 42);

    let swap: SmrSwap<String> = SmrSwap::from(String::from("hello"));
    assert_eq!(*swap.get(), "hello");
}

/// Test Debug trait
/// 测试 Debug trait
#[test]
fn test_debug() {
    let swap = SmrSwap::new(42);
    let debug_str = format!("{:?}", swap);
    assert!(debug_str.contains("SmrSwap"));
    assert!(debug_str.contains("42"));

    let reader = swap.local();
    let debug_str = format!("{:?}", reader);
    assert!(debug_str.contains("LocalReader"));

    let guard = reader.load();
    let debug_str = format!("{:?}", guard);
    assert!(debug_str.contains("ReadGuard"));
    assert!(debug_str.contains("42"));
}

/// Test ReadGuard version
/// 测试 ReadGuard 版本
#[test]
fn test_read_guard_version() {
    let mut swap = SmrSwap::new(0);
    let reader = swap.local();

    let guard1 = reader.load();
    let v1 = guard1.version();

    swap.store(1);

    let guard2 = reader.load();
    let v2 = guard2.version();

    // guard1 should still have the old version
    assert_eq!(guard1.version(), v1);
    // guard2 should have same version
    assert!(v2 == v1);
}

/// Test is_pinned
/// 测试 is_pinned
#[test]
fn test_is_pinned() {
    let swap = SmrSwap::new(0);
    let reader = swap.local();

    assert!(!reader.is_pinned());

    let guard = reader.load();
    assert!(reader.is_pinned());

    drop(guard);
    assert!(!reader.is_pinned());
}
