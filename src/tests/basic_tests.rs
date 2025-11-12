//! Basic functionality tests for SMR-Swap
//! 
//! Tests core operations: creation, reading, updating, and basic reader operations

use crate::{new, SwapReader, Swapper};

/// Test basic creation and reading with integers
/// 测试基本的创建和读取（整数）
#[test]
fn test_basic_new_and_read_int() {
    let (_swapper, reader) = new(42);
    let guard = reader.read().unwrap();
    assert_eq!(*guard, 42);
}

/// Test basic creation and reading with strings
/// 测试基本的创建和读取（字符串）
#[test]
fn test_basic_new_and_read_string() {
    let (_swapper, reader) = new(String::from("hello"));
    let guard = reader.read().unwrap();
    assert_eq!(*guard, "hello");
}

/// Test basic creation and reading with vectors
/// 测试基本的创建和读取（向量）
#[test]
fn test_basic_new_and_read_vector() {
    let (_swapper, reader) = new(vec![1, 2, 3, 4, 5]);
    let guard = reader.read().unwrap();
    assert_eq!(*guard, vec![1, 2, 3, 4, 5]);
}

/// Test basic update operation with integers
/// 测试基本的更新操作（整数）
#[test]
fn test_basic_update_int() {
    let (mut swapper, reader) = new(10);
    assert_eq!(*reader.read().unwrap(), 10);

    swapper.update(20);
    let guard = reader.read().unwrap();
    assert_eq!(*guard, 20);
}

/// Test basic update operation with strings
/// 测试基本的更新操作（字符串）
#[test]
fn test_basic_update_string() {
    let (mut swapper, reader) = new(String::from("hello"));
    assert_eq!(*reader.read().unwrap(), "hello");

    swapper.update(String::from("world"));
    let guard = reader.read().unwrap();
    assert_eq!(*guard, "world");
}

/// Test multiple sequential updates
/// 测试多个连续的更新
#[test]
fn test_multiple_updates() {
    let (mut swapper, reader) = new(0);

    for i in 1..=10 {
        swapper.update(i);
        assert_eq!(*reader.read().unwrap(), i);
    }
}

/// Test reader cloning with integers
/// 测试读取者克隆（整数）
#[test]
fn test_reader_clone_int() {
    let (mut swapper, reader1) = new(10);
    let reader2 = reader1.clone();

    assert_eq!(*reader1.read().unwrap(), 10);
    assert_eq!(*reader2.read().unwrap(), 10);

    swapper.update(20);

    assert_eq!(*reader1.read().unwrap(), 20);
    assert_eq!(*reader2.read().unwrap(), 20);
}

/// Test reader cloning with strings
/// 测试读取者克隆（字符串）
#[test]
fn test_reader_clone_string() {
    let (mut swapper, reader1) = new(String::from("initial"));
    let reader2 = reader1.clone();
    let reader3 = reader2.clone();

    assert_eq!(*reader1.read().unwrap(), "initial");
    assert_eq!(*reader2.read().unwrap(), "initial");
    assert_eq!(*reader3.read().unwrap(), "initial");

    swapper.update(String::from("updated"));

    assert_eq!(*reader1.read().unwrap(), "updated");
    assert_eq!(*reader2.read().unwrap(), "updated");
    assert_eq!(*reader3.read().unwrap(), "updated");
}

/// Test multiple readers see consistent values
/// 测试多个读取者看到一致的值
#[test]
fn test_multiple_readers_consistency() {
    let (mut swapper, reader1) = new(100);
    let reader2 = reader1.clone();
    let reader3 = reader1.clone();
    let reader4 = reader1.clone();

    // All readers should see the same initial value
    // 所有读取者应该看到相同的初始值
    assert_eq!(*reader1.read().unwrap(), 100);
    assert_eq!(*reader2.read().unwrap(), 100);
    assert_eq!(*reader3.read().unwrap(), 100);
    assert_eq!(*reader4.read().unwrap(), 100);

    // Update and verify all readers see the new value
    // 更新并验证所有读取者看到新值
    swapper.update(200);
    assert_eq!(*reader1.read().unwrap(), 200);
    assert_eq!(*reader2.read().unwrap(), 200);
    assert_eq!(*reader3.read().unwrap(), 200);
    assert_eq!(*reader4.read().unwrap(), 200);
}

/// Test that read guard holds value across updates
/// 测试读取守卫在更新后仍然持有值
#[test]
fn test_read_guard_holds_value() {
    let (mut swapper, reader) = new(10);

    let guard_v1 = reader.read().unwrap();
    assert_eq!(*guard_v1, 10);

    swapper.update(20);

    let guard_v2 = reader.read().unwrap();
    assert_eq!(*guard_v2, 20);

    // Critical: V1's guard must still be valid!
    // 关键：V1 的 guard 必须仍然有效！
    assert_eq!(*guard_v1, 10);

    drop(guard_v2);
    drop(guard_v1);

    swapper.update(30);
    assert_eq!(*reader.read().unwrap(), 30);
}

/// Test that multiple held guards remain valid
/// 测试多个持有的 guard 保持有效
#[test]
fn test_multiple_held_guards() {
    let (mut swapper, reader) = new(1);

    let guard1 = reader.read().unwrap();
    assert_eq!(*guard1, 1);

    swapper.update(2);
    let guard2 = reader.read().unwrap();
    assert_eq!(*guard2, 2);

    swapper.update(3);
    let guard3 = reader.read().unwrap();
    assert_eq!(*guard3, 3);

    // All guards should still be valid
    // 所有 guard 应该仍然有效
    assert_eq!(*guard1, 1);
    assert_eq!(*guard2, 2);
    assert_eq!(*guard3, 3);

    drop(guard1);
    drop(guard2);
    drop(guard3);
}

/// Test Send+Sync trait bounds compilation
/// 测试 Send+Sync 特性约束编译
#[test]
fn test_send_sync_compilation() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<Swapper<String>>();
    assert_send_sync::<SwapReader<String>>();
    assert_send_sync::<Swapper<i32>>();
    assert_send_sync::<SwapReader<i32>>();
}

/// Test with Box-wrapped values
/// 测试使用 Box 包装的值
#[test]
fn test_with_boxed_values() {
    let (mut swapper, reader) = new(Box::new(vec![1, 2, 3]));

    let guard = reader.read().unwrap();
    assert_eq!(**guard, vec![1, 2, 3]);
    drop(guard);

    swapper.update(Box::new(vec![4, 5, 6]));
    let guard = reader.read().unwrap();
    assert_eq!(**guard, vec![4, 5, 6]);
}

/// Test with Arc-wrapped values
/// 测试使用 Arc 包装的值
#[test]
fn test_with_arc_values() {
    use std::sync::Arc;

    let (mut swapper, reader) = new(Arc::new(String::from("initial")));

    let guard = reader.read().unwrap();
    assert_eq!(**guard, "initial");
    drop(guard);

    swapper.update(Arc::new(String::from("updated")));
    let guard = reader.read().unwrap();
    assert_eq!(**guard, "updated");
}
