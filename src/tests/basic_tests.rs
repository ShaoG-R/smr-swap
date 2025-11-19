//! Basic functionality tests for SMR-Swap
//!
//! Tests core operations: creation, reading, updating, and basic reader operations

use crate::{SwapReader, Swapper, new};

/// Test basic creation and reading with integers
/// 测试基本的创建和读取（整数）
#[test]
fn test_basic_new_and_read_int() {
    let (_swapper, reader) = new(42);
    let local_epoch = reader.register_reader();
    let guard = local_epoch.pin();
    let val = reader.read(&guard);
    assert_eq!(*val, 42);
}

/// Test basic creation and reading with strings
/// 测试基本的创建和读取（字符串）
#[test]
fn test_basic_new_and_read_string() {
    let (_swapper, reader) = new(String::from("hello"));
    let local_epoch = reader.register_reader();
    let guard = local_epoch.pin();
    let val = reader.read(&guard);
    assert_eq!(*val, "hello");
}

/// Test basic creation and reading with vectors
/// 测试基本的创建和读取（向量）
#[test]
fn test_basic_new_and_read_vector() {
    let (_swapper, reader) = new(vec![1, 2, 3, 4, 5]);
    let local_epoch = reader.register_reader();
    let guard = local_epoch.pin();
    let val = reader.read(&guard);
    assert_eq!(*val, vec![1, 2, 3, 4, 5]);
}

/// Test basic update operation with integers
/// 测试基本的更新操作（整数）
#[test]
fn test_basic_update_int() {
    let (mut swapper, reader) = new(10);
    let local_epoch = reader.register_reader();
    {
        let guard = local_epoch.pin();
        assert_eq!(*reader.read(&guard), 10);
    }

    swapper.update(20);
    let guard = local_epoch.pin();
    let val = reader.read(&guard);
    assert_eq!(*val, 20);
}

/// Test basic update operation with strings
/// 测试基本的更新操作（字符串）
#[test]
fn test_basic_update_string() {
    let (mut swapper, reader) = new(String::from("hello"));
    let local_epoch = reader.register_reader();
    {
        let guard = local_epoch.pin();
        assert_eq!(*reader.read(&guard), "hello");
    }

    swapper.update(String::from("world"));
    let guard = local_epoch.pin();
    let val = reader.read(&guard);
    assert_eq!(*val, "world");
}

/// Test multiple sequential updates
/// 测试多个连续的更新
#[test]
fn test_multiple_updates() {
    let (mut swapper, reader) = new(0);
    let local_epoch = reader.register_reader();

    for i in 1..=10 {
        swapper.update(i);
        let guard = local_epoch.pin();
        assert_eq!(*reader.read(&guard), i);
    }
}

/// Test reader cloning with integers
/// 测试读取者克隆（整数）
#[test]
fn test_reader_clone_int() {
    let (mut swapper, reader1) = new(10);
    let reader2 = reader1.clone();
    let local_epoch1 = reader1.register_reader();
    let local_epoch2 = reader2.register_reader();

    {
        let guard1 = local_epoch1.pin();
        let guard2 = local_epoch2.pin();
        assert_eq!(*reader1.read(&guard1), 10);
        assert_eq!(*reader2.read(&guard2), 10);
    }

    swapper.update(20);

    let guard1 = local_epoch1.pin();
    let guard2 = local_epoch2.pin();
    assert_eq!(*reader1.read(&guard1), 20);
    assert_eq!(*reader2.read(&guard2), 20);
}

/// Test reader cloning with strings
/// 测试读取者克隆（字符串）
#[test]
fn test_reader_clone_string() {
    let (mut swapper, reader1) = new(String::from("initial"));
    let reader2 = reader1.clone();
    let reader3 = reader2.clone();
    let local_epoch1 = reader1.register_reader();
    let local_epoch2 = reader2.register_reader();
    let local_epoch3 = reader3.register_reader();

    {
        let guard1 = local_epoch1.pin();
        let guard2 = local_epoch2.pin();
        let guard3 = local_epoch3.pin();
        assert_eq!(*reader1.read(&guard1), "initial");
        assert_eq!(*reader2.read(&guard2), "initial");
        assert_eq!(*reader3.read(&guard3), "initial");
    }

    swapper.update(String::from("updated"));

    let guard1 = local_epoch1.pin();
    let guard2 = local_epoch2.pin();
    let guard3 = local_epoch3.pin();
    assert_eq!(*reader1.read(&guard1), "updated");
    assert_eq!(*reader2.read(&guard2), "updated");
    assert_eq!(*reader3.read(&guard3), "updated");
}

/// Test multiple readers see consistent values
/// 测试多个读取者看到一致的值
#[test]
fn test_multiple_readers_consistency() {
    let (mut swapper, reader1) = new(0);
    let reader2 = reader1.clone();
    let reader3 = reader2.clone();
    let local_epoch1 = reader1.register_reader();
    let local_epoch2 = reader2.register_reader();
    let local_epoch3 = reader3.register_reader();

    // All readers should see the same initial value
    // 所有读取者应该看到相同的初始值
    {
        let guard1 = local_epoch1.pin();
        let guard2 = local_epoch2.pin();
        let guard3 = local_epoch3.pin();
        assert_eq!(*reader1.read(&guard1), 0);
        assert_eq!(*reader2.read(&guard2), 0);
        assert_eq!(*reader3.read(&guard3), 0);
    }

    // Update the value
    // 更新值
    swapper.update(1);

    // All readers should see the same updated value
    // 所有读取者应该看到相同的更新值
    {
        let guard1 = local_epoch1.pin();
        let guard2 = local_epoch2.pin();
        let guard3 = local_epoch3.pin();
        assert_eq!(*reader1.read(&guard1), 1);
        assert_eq!(*reader2.read(&guard2), 1);
        assert_eq!(*reader3.read(&guard3), 1);
    }

    // Update again
    // 再次更新
    swapper.update(2);

    // All readers should see the same new value
    // 所有读取者应该看到相同的新值
    let guard1 = local_epoch1.pin();
    let guard2 = local_epoch2.pin();
    let guard3 = local_epoch3.pin();
    assert_eq!(*reader1.read(&guard1), 2);
    assert_eq!(*reader2.read(&guard2), 2);
    assert_eq!(*reader3.read(&guard3), 2);
}

/// Test that read guard holds value across updates
/// 测试读取守卫在更新后仍然持有值
#[test]
fn test_read_guard_holds_value() {
    let (mut swapper, reader) = new(10);
    let local_epoch = reader.register_reader();

    let guard1 = local_epoch.pin();
    let v1 = reader.read(&guard1);
    assert_eq!(*v1, 10);

    swapper.update(20);

    let guard2 = local_epoch.pin();
    let v2 = reader.read(&guard2);
    assert_eq!(*v2, 20);

    // Critical: V1's guard must still be valid!
    // 关键：V1 的 guard 必须仍然有效！
    assert_eq!(*v1, 10);

    drop(guard2);
    drop(guard1);

    swapper.update(30);
    let guard3 = local_epoch.pin();
    assert_eq!(*reader.read(&guard3), 30);
}

/// Test that multiple held guards remain valid
/// 测试多个持有的 guard 保持有效
#[test]
fn test_multiple_held_guards() {
    let (mut swapper, reader) = new(0);
    let local_epoch = reader.register_reader();

    // Get multiple guards
    // 获取多个守卫
    let guard1 = local_epoch.pin();
    let v1 = reader.read(&guard1);
    let guard2 = local_epoch.pin();
    let v2 = reader.read(&guard2);
    let guard3 = local_epoch.pin();
    let v3 = reader.read(&guard3);

    // All guards should see the same value
    // 所有守卫应该看到相同的值
    assert_eq!(*v1, 0);
    assert_eq!(*v2, 0);
    assert_eq!(*v3, 0);

    // Update the value
    // 更新值
    swapper.update(1);

    // Old guards should still be valid
    // 旧的守卫应该仍然有效
    assert_eq!(*v1, 0);
    assert_eq!(*v2, 0);
    assert_eq!(*v3, 0);

    // New guard should see the new value
    // 新的守卫应该看到新值
    let guard4 = local_epoch.pin();
    let v4 = reader.read(&guard4);
    assert_eq!(*v4, 1);
}

/// Test Send+Sync trait bounds compilation
/// 测试 Send+Sync 特性约束编译
#[test]
fn test_send_sync_compilation<'a>() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<Swapper<i32>>();
    assert_send::<SwapReader<i32>>();
    assert_sync::<SwapReader<i32>>();

    assert_send::<SwapReader<&'a i32>>();
    assert_sync::<SwapReader<&'a i32>>();
}

/// Test with Box-wrapped values
/// 测试使用 Box 包装的值
#[test]
fn test_with_boxed_values() {
    let (mut swapper, reader) = new(Box::new(42));
    let local_epoch = reader.register_reader();

    let guard = local_epoch.pin();
    assert_eq!(**reader.read(&guard), 42);
    drop(guard);

    swapper.update(Box::new(100));
    let guard = local_epoch.pin();
    assert_eq!(**reader.read(&guard), 100);
}

/// Test with Arc-wrapped values
/// 测试使用 Arc 包装的值
#[test]
fn test_with_arc_values() {
    use std::sync::Arc;

    let (mut swapper, reader) = new(Arc::new(42));
    let local_epoch = reader.register_reader();

    let guard1 = local_epoch.pin();
    let value1 = reader.read(&guard1);
    assert_eq!(**value1, 42);

    swapper.update(Arc::new(100));
    let guard2 = local_epoch.pin();
    let value2 = reader.read(&guard2);
    assert_eq!(**value2, 100);
}

/// Test map functionality
/// 测试 map 功能
#[test]
fn test_map() {
    let (mut swapper, reader) = new(10);
    let local_epoch = reader.register_reader();

    // Map the value to a new type
    let doubled = reader.map(&local_epoch, |val| *val * 2);
    assert_eq!(doubled, 20);

    // Update the value
    swapper.update(20);

    // Map the new value
    let tripled = reader.map(&local_epoch, |val| *val * 3);
    assert_eq!(tripled, 60);
}

/// Test update_and_fetch functionality
/// 测试 update_and_fetch 功能
#[test]
fn test_update_and_fetch() {
    let (mut swapper, _) = new(5);
    let local_epoch = swapper.register_reader();

    // Update and get the new value
    let guard1 = local_epoch.pin();
    let val1 = swapper.update_and_fetch(&guard1, |x| *x * 2);
    assert_eq!(*val1, 10);

    // Another update
    let guard2 = local_epoch.pin();
    let val2 = swapper.update_and_fetch(&guard2, |x| x + 5);
    assert_eq!(*val2, 15);
}

/// Test filter functionality
/// 测试 filter 功能
#[test]
fn test_filter() {
    let (mut swapper, reader) = new(Some(42));
    let local_epoch = reader.register_reader();

    // Filter with Some
    let guard = local_epoch.pin();
    let val = reader.filter(&guard, |val| val.is_some()).unwrap();
    assert_eq!(*val, Some(42));

    // Update to None
    swapper.update(None::<i32>);

    // Filter with None
    let guard2 = local_epoch.pin();
    let result = reader.filter(&guard2, |val| val.is_some());
    assert!(result.is_none());
}
