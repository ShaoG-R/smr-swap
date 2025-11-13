//! Advanced API tests for SMR-Swap
//! 
//! Tests for advanced operations: map, filter, try_clone_value,
//! update_and_fetch, swap, and reader operations

use crate::new;

/// Test reader map operation with integers
/// 测试读取者 map 操作（整数）
#[test]
fn test_reader_map_int() {
    let (mut swapper, reader) = new(10);

    let result = reader.map(|x| x * 2);
    assert_eq!(result, 20);

    swapper.update(5);
    let result = reader.map(|x| x + 100);
    assert_eq!(result, 105);
}

/// Test reader map operation with strings
/// 测试读取者 map 操作（字符串）
#[test]
fn test_reader_map_string() {
    let (mut swapper, reader) = new(String::from("hello"));

    let result = reader.map(|s| s.len());
    assert_eq!(result, 5);

    swapper.update(String::from("world!"));
    let result = reader.map(|s| s.to_uppercase());
    assert_eq!(result, String::from("WORLD!"));
}

/// Test reader map operation with vectors
/// 测试读取者 map 操作（向量）
#[test]
fn test_reader_map_vector() {
    let (mut swapper, reader) = new(vec![1, 2, 3]);

    let result = reader.map(|v| v.len());
    assert_eq!(result, 3);

    swapper.update(vec![1, 2, 3, 4, 5]);
    let result = reader.map(|v| v.iter().sum::<i32>());
    assert_eq!(result, 15);
}

/// Test reader filter operation
/// 测试读取者 filter 操作
#[test]
fn test_reader_filter() {
    let (mut swapper, reader) = new(10);

    let guard = reader.filter(|x| *x > 5);
    assert!(guard.is_some());
    assert_eq!(*guard.unwrap(), 10);

    swapper.update(3);
    let guard = reader.filter(|x| x > &5);
    assert!(guard.is_none());
}

/// Test reader filter with strings
/// 测试读取者 filter（字符串）
#[test]
fn test_reader_filter_string() {
    let (mut swapper, reader) = new(String::from("hello"));

    let guard = reader.filter(|s| s.len() > 3);
    assert!(guard.is_some());
    assert_eq!(*guard.unwrap(), "hello");

    swapper.update(String::from("hi"));
    let guard = reader.filter(|s| s.len() > 3);
    assert!(guard.is_none());
}

/// Test reader filter with vectors
/// 测试读取者 filter（向量）
#[test]
fn test_reader_filter_vector() {
    let (mut swapper, reader) = new(vec![1, 2, 3, 4, 5]);

    let guard = reader.filter(|v| v.len() > 3);
    assert!(guard.is_some());
    assert_eq!(*guard.unwrap(), vec![1, 2, 3, 4, 5]);

    swapper.update(vec![1, 2]);
    let guard = reader.filter(|v| v.len() > 3);
    assert!(guard.is_none());
}

/// Test writer update_and_fetch operation
/// 测试写入者 update_and_fetch 操作
#[test]
fn test_writer_update_and_fetch() {
    let (mut swapper, reader) = new(10);

    let guard = swapper.update_and_fetch(|x| x * 2);
    assert_eq!(*guard, 20);
    assert_eq!(*reader.read(), 20);

    let guard = swapper.update_and_fetch(|x| x + 5);
    assert_eq!(*guard, 25);
    assert_eq!(*reader.read(), 25);
}

/// Test writer update_and_fetch with strings
/// 测试写入者 update_and_fetch（字符串）
#[test]
fn test_writer_update_and_fetch_string() {
    let (mut swapper, reader) = new(String::from("hello"));

    let guard = swapper.update_and_fetch(|s| s.to_uppercase());
    assert_eq!(*guard, "HELLO");
    assert_eq!(*reader.read(), "HELLO");

    let guard = swapper.update_and_fetch(|s| format!("{} world", s));
    assert_eq!(*guard, "HELLO world");
    assert_eq!(*reader.read(), "HELLO world");
}

/// Test writer update_and_fetch with vectors
/// 测试写入者 update_and_fetch（向量）
#[test]
fn test_writer_update_and_fetch_vector() {
    let (mut swapper, reader) = new(vec![1, 2, 3]);

    let guard = swapper.update_and_fetch(|v| {
        let mut new_v = v.clone();
        new_v.push(4);
        new_v
    });
    assert_eq!(&*guard, &vec![1, 2, 3, 4]);
    assert_eq!(&*reader.read(), &vec![1, 2, 3, 4]);
}

/// Test writer read capability
/// 测试写入者读取能力
#[test]
fn test_writer_read() {
    let (mut swapper, reader) = new(42);

    let guard = swapper.read();
    assert_eq!(*guard, 42);
    drop(guard);

    swapper.update(100);
    assert_eq!(*swapper.read(), 100);
    assert_eq!(*reader.read(), 100);
}

/// Test writer read with strings
/// 测试写入者读取（字符串）
#[test]
fn test_writer_read_string() {
    let (mut swapper, reader) = new(String::from("test"));

    let guard = swapper.read();
    assert_eq!(*guard, "test");
    drop(guard);

    swapper.update(String::from("updated"));
    assert_eq!(*swapper.read(), "updated");
    assert_eq!(*reader.read(), "updated");
}

/// Test chained operations
/// 测试链式操作
#[test]
fn test_chained_operations() {
    let (mut swapper, reader) = new(10);

    // Chain: read -> filter -> map
    // 链式：read -> filter -> map
    let guard = reader.read();
    let result = if *guard > 5 { *guard * 2 } else { 0 };
    assert_eq!(result, 20);

    swapper.update(5);
    let guard = reader.read();
    let result = if *guard > 5 { *guard * 2 } else { 0 };
    assert_eq!(result, 0); // Value is 5, which is not > 5
}

/// Test multiple operations on same guard
/// 测试在同一 guard 上的多个操作
#[test]
fn test_multiple_operations_same_guard() {
    let (_swapper, reader) = new(vec![1, 2, 3, 4, 5]);
    let guard = reader.read();

    // Multiple operations on the same guard
    // 在同一 guard 上的多个操作
    assert_eq!(guard.len(), 5);
    assert_eq!(guard.iter().sum::<i32>(), 15);
    assert!(guard.contains(&3));
    assert_eq!(guard[0], 1);
}

/// Test map with complex transformation
/// 测试 map 与复杂转换
#[test]
fn test_map_complex_transformation() {
    let (mut swapper, reader) = new(vec![1, 2, 3, 4, 5]);

    let result = reader.map(|v| {
        v.iter()
            .filter(|x| *x % 2 == 0)
            .map(|x| x * 2)
            .sum::<i32>()
    });
    assert_eq!(result, 12); // (2*2 + 4*2) = 12

    swapper.update(vec![10, 20, 30]);
    let result = reader.map(|v| v.iter().sum::<i32>());
    assert_eq!(result, 60);
}

/// Test filter with complex condition
/// 测试 filter 与复杂条件
#[test]
fn test_filter_complex_condition() {
    let (mut swapper, reader) = new(String::from("hello"));

    let guard = reader.filter(|s| {
        s.len() > 3 && s.contains('l')
    });
    assert!(guard.is_some());

    swapper.update(String::from("hi"));
    let guard = reader.filter(|s| {
        s.len() > 3 && s.contains('l')
    });
    assert!(guard.is_none());
}

/// Test update_and_fetch with side effects
/// 测试 update_and_fetch 与副作用
#[test]
fn test_update_and_fetch_side_effects() {
    let (mut swapper, reader) = new(vec![1, 2, 3]);

    let mut call_count = 0;
    let guard = swapper.update_and_fetch(|v| {
        call_count += 1;
        let mut new_v = v.clone();
        new_v.push(4);
        new_v
    });

    assert_eq!(call_count, 1);
    assert_eq!(&*guard, &vec![1, 2, 3, 4]);
    assert_eq!(&*reader.read(), &vec![1, 2, 3, 4]);
}

/// Test map returns None for empty option
/// 测试 map 对空 option 返回 None
#[test]
fn test_map_with_none_value() {
    let (mut swapper, reader) = new(Some(42));
    
    // Map on Some value
    let result = reader.map(|opt| opt.unwrap_or(0));
    assert_eq!(result, 42);
    
    // Map on None (after update)
    swapper.update(None::<i32>);
    let result = reader.map(|opt| opt.unwrap_or(0));
    assert_eq!(result, 0);
}

/// Test filter with always-true condition
/// 测试 filter 与总是真的条件
#[test]
fn test_filter_always_true() {
    let (_swapper, reader) = new(42);

    let guard = reader.filter(|_| true);
    assert!(guard.is_some());
    assert_eq!(*guard.unwrap(), 42);
}

/// Test filter with always-false condition
/// 测试 filter 与总是假的条件
#[test]
fn test_filter_always_false() {
    let (_swapper, reader) = new(42);

    let guard = reader.filter(|_| false);
    assert!(guard.is_none());
}
