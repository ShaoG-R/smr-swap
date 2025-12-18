//! Advanced API tests for SMR-Swap
//!
//! Tests for advanced operations: map, filter, update_and_fetch, swap, and reader operations

extern crate std;
use crate::SmrSwap;
use std::prelude::v1::*;

/// Test reader map operation with integers
/// 测试读取者 map 操作（整数）
#[test]
fn test_reader_map_int() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    let result = reader.map(|x| x * 2);
    assert_eq!(result, 20);

    swap.store(5);
    let result = reader.map(|x| x + 100);
    assert_eq!(result, 105);
}

/// Test reader map operation with strings
/// 测试读取者 map 操作（字符串）
#[test]
fn test_reader_map_string() {
    let mut swap = SmrSwap::new(String::from("hello"));
    let reader = swap.local();

    let result = reader.map(|s| s.len());
    assert_eq!(result, 5);

    swap.store(String::from("world!"));
    let result = reader.map(|s| s.to_uppercase());
    assert_eq!(result, String::from("WORLD!"));
}

/// Test reader map operation with vectors
/// 测试读取者 map 操作（向量）
#[test]
fn test_reader_map_vector() {
    let mut swap = SmrSwap::new(std::vec![1, 2, 3]);
    let reader = swap.local();

    let result = reader.map(|v| v.len());
    assert_eq!(result, 3);

    swap.store(std::vec![1, 2, 3, 4, 5]);
    let result = reader.map(|v| v.iter().sum::<i32>());
    assert_eq!(result, 15);
}

/// Test reader filter operation
/// 测试读取者 filter 操作
#[test]
fn test_reader_filter() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    let val = reader.filter(|x| *x > 5);
    assert!(val.is_some());
    assert_eq!(*val.unwrap(), 10);

    swap.store(3);
    let val = reader.filter(|x| x > &5);
    assert!(val.is_none());
}

/// Test reader filter with strings
/// 测试读取者 filter（字符串）
#[test]
fn test_reader_filter_string() {
    let mut swap = SmrSwap::new(String::from("hello"));
    let reader = swap.local();

    let val = reader.filter(|s| s.len() > 3);
    assert!(val.is_some());
    assert_eq!(*val.unwrap(), "hello");

    swap.store(String::from("hi"));
    let val = reader.filter(|s| s.len() > 3);
    assert!(val.is_none());
}

/// Test reader filter with vectors
/// 测试读取者 filter（向量）
#[test]
fn test_reader_filter_vector() {
    let mut swap = SmrSwap::new(std::vec![1, 2, 3, 4, 5]);
    let reader = swap.local();

    let val = reader.filter(|v| v.len() > 3);
    assert!(val.is_some());
    assert_eq!(*val.unwrap(), std::vec![1, 2, 3, 4, 5]);

    swap.store(std::vec![1, 2]);
    let val = reader.filter(|v| v.len() > 3);
    assert!(val.is_none());
}

/// Test writer update_and_fetch operation
/// 测试写入者 update_and_fetch 操作
#[test]
fn test_writer_update_and_fetch() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    {
        let val1 = swap.update_and_fetch(|x| x * 2);
        assert_eq!(*val1, 20);
    }
    assert_eq!(*reader.load(), 20);

    let val2 = swap.update_and_fetch(|x| x + 5);
    assert_eq!(*val2, 25);
    assert_eq!(*reader.load(), 25);
}

/// Test writer update_and_fetch with strings
/// 测试写入者 update_and_fetch（字符串）
#[test]
fn test_writer_update_and_fetch_string() {
    let mut swap = SmrSwap::new(String::from("hello"));
    let reader = swap.local();

    {
        let val1 = swap.update_and_fetch(|s| s.to_uppercase());
        assert_eq!(*val1, "HELLO");
    }
    assert_eq!(*reader.load(), "HELLO");

    let val2 = swap.update_and_fetch(|s| std::format!("{} world", s));
    assert_eq!(*val2, "HELLO world");
    assert_eq!(*reader.load(), "HELLO world");
}

/// Test writer update_and_fetch with vectors
/// 测试写入者 update_and_fetch（向量）
#[test]
fn test_writer_update_and_fetch_vector() {
    let mut swap = SmrSwap::new(std::vec![1, 2, 3]);
    let reader = swap.local();

    let val = swap.update_and_fetch(|v| {
        let mut new_v = v.clone();
        new_v.push(4);
        new_v
    });
    assert_eq!(&*val, &std::vec![1, 2, 3, 4]);
    assert_eq!(&*reader.load(), &std::vec![1, 2, 3, 4]);
}

/// Test writer read capability
/// 测试写入者读取能力
#[test]
fn test_writer_read() {
    let mut swap = SmrSwap::new(42);
    let reader = swap.local();

    let val = swap.load();
    assert_eq!(*val, 42);
    drop(val);

    swap.store(100);
    assert_eq!(*swap.load(), 100);
    assert_eq!(*reader.load(), 100);
}

/// Test writer read with strings
/// 测试写入者读取（字符串）
#[test]
fn test_writer_read_string() {
    let mut swap = SmrSwap::new(String::from("test"));
    let reader = swap.local();

    let val = swap.load();
    assert_eq!(*val, "test");
    drop(val);

    swap.store(String::from("updated"));
    assert_eq!(*swap.load(), "updated");
    assert_eq!(*reader.load(), "updated");
}

/// Test chained operations
/// 测试链式操作
#[test]
fn test_chained_operations() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    // Chain: read -> filter -> map
    // 链式：read -> filter -> map
    let guard = reader.load();
    let result = if *guard > 5 { *guard * 2 } else { 0 };
    assert_eq!(result, 20);
    drop(guard);

    swap.store(5);
    let guard = reader.load();
    let result = if *guard > 5 { *guard * 2 } else { 0 };
    assert_eq!(result, 0); // Value is 5, which is not > 5
}

/// Test multiple operations on same guard
/// 测试在同一 guard 上的多个操作
#[test]
fn test_multiple_operations_same_guard() {
    let swap = SmrSwap::new(std::vec![1, 2, 3, 4, 5]);
    let reader = swap.local();
    let guard = reader.load();

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
    let mut swap = SmrSwap::new(std::vec![1, 2, 3, 4, 5]);
    let reader = swap.local();

    let result = reader.map(|v| v.iter().filter(|x| *x % 2 == 0).map(|x| x * 2).sum::<i32>());
    assert_eq!(result, 12); // (2*2 + 4*2) = 12

    swap.store(std::vec![10, 20, 30]);
    let result = reader.map(|v| v.iter().sum::<i32>());
    assert_eq!(result, 60);
}

/// Test filter with complex condition
/// 测试 filter 与复杂条件
#[test]
fn test_filter_complex_condition() {
    let mut swap = SmrSwap::new(String::from("hello"));
    let reader = swap.local();

    let val = reader.filter(|s| s.len() > 3 && s.contains('l'));
    assert!(val.is_some());

    swap.store(String::from("hi"));
    let val = reader.filter(|s| s.len() > 3 && s.contains('l'));
    assert!(val.is_none());
}

/// Test update_and_fetch with side effects
/// 测试 update_and_fetch 与副作用
#[test]
fn test_update_and_fetch_side_effects() {
    let mut swap = SmrSwap::new(std::vec![1, 2, 3]);
    let reader = swap.local();

    let mut call_count = 0;
    let val = swap.update_and_fetch(|v| {
        call_count += 1;
        let mut new_v = v.clone();
        new_v.push(4);
        new_v
    });

    assert_eq!(call_count, 1);
    assert_eq!(&*val, &std::vec![1, 2, 3, 4]);
    assert_eq!(&*reader.load(), &std::vec![1, 2, 3, 4]);
}

/// Test map returns None for empty option
/// 测试 map 对空 option 返回 None
#[test]
fn test_map_with_none_value() {
    let mut swap = SmrSwap::new(Some(42));
    let reader = swap.local();

    // Map on Some value
    let result = reader.map(|opt| opt.unwrap_or(0));
    assert_eq!(result, 42);

    // Map on None (after update)
    swap.store(None::<i32>);
    let result = reader.map(|opt| opt.unwrap_or(0));
    assert_eq!(result, 0);
}

/// Test filter with always-true condition
/// 测试 filter 与总是真的条件
#[test]
fn test_filter_always_true() {
    let swap = SmrSwap::new(42);
    let reader = swap.local();

    let val = reader.filter(|_| true);
    assert!(val.is_some());
    assert_eq!(*val.unwrap(), 42);
}

/// Test filter with always-false condition
/// 测试 filter 与总是假的条件
#[test]
fn test_filter_always_false() {
    let swap = SmrSwap::new(42);
    let reader = swap.local();

    let val = reader.filter(|_| false);
    assert!(val.is_none());
}

/// Test swap operation
/// 测试 swap 操作
#[test]
fn test_swap_operation() {
    let mut swap = SmrSwap::new(10);
    let reader = swap.local();

    let old = swap.swap(20);
    assert_eq!(old, 10);
    assert_eq!(*reader.load(), 20);

    let old = swap.swap(30);
    assert_eq!(old, 20);
    assert_eq!(*reader.load(), 30);
}

/// Test swap with complex types
/// 测试 swap 与复杂类型
#[test]
fn test_swap_complex_types() {
    let mut swap = SmrSwap::new(std::vec![1, 2, 3]);
    let reader = swap.local();

    let old = swap.swap(std::vec![4, 5, 6]);
    assert_eq!(old, std::vec![1, 2, 3]);
    assert_eq!(*reader.load(), std::vec![4, 5, 6]);
}

/// Test ReadGuard clone
/// 测试 ReadGuard 克隆
#[test]
fn test_read_guard_clone() {
    let swap = SmrSwap::new(42);
    let reader = swap.local();

    let guard1 = reader.load();
    let guard2 = guard1.clone();

    assert_eq!(*guard1, 42);
    assert_eq!(*guard2, 42);
}

/// Test multiple update_and_fetch in sequence
/// 测试连续多次 update_and_fetch
#[test]
fn test_multiple_update_and_fetch() {
    let mut swap = SmrSwap::new(1);
    let reader = swap.local();

    for i in 2..=10 {
        let val = swap.update_and_fetch(|x| x + 1);
        assert_eq!(*val, i);
    }

    assert_eq!(*reader.load(), 10);
}
