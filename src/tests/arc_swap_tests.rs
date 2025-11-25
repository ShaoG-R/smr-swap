//! Tests for Arc-specialized swap operations
//!
//! Tests for Swapper<Arc<T>> specialized methods: swap and update_and_fetch_arc

use crate::SmrSwap;
use std::sync::Arc;

/// Test basic swap operation with Arc-wrapped integers
/// 测试基本的 swap 操作（Arc 包装的整数）
#[test]
fn test_arc_swap_basic_int() {
    let mut swap = SmrSwap::new(Arc::new(42));

    let old_value = swap.swap(Arc::new(100));
    assert_eq!(*old_value, 42);

    let guard = swap.load();
    assert_eq!(**guard, 100);
}

/// Test basic swap operation with Arc-wrapped strings
/// 测试基本的 swap 操作（Arc 包装的字符串）
#[test]
fn test_arc_swap_basic_string() {
    let mut swap = SmrSwap::new(Arc::new(String::from("hello")));

    let old_value = swap.swap(Arc::new(String::from("world")));
    assert_eq!(*old_value, "hello");

    let guard = swap.load();
    assert_eq!(**guard, "world");
}

/// Test basic swap operation with Arc-wrapped vectors
/// 测试基本的 swap 操作（Arc 包装的向量）
#[test]
fn test_arc_swap_basic_vector() {
    let mut swap = SmrSwap::new(Arc::new(vec![1, 2, 3]));

    let old_value = swap.swap(Arc::new(vec![4, 5, 6]));
    assert_eq!(*old_value, vec![1, 2, 3]);

    let guard = swap.load();
    assert_eq!(**guard, vec![4, 5, 6]);
}

/// Test multiple sequential swaps
/// 测试多个连续的 swap 操作
#[test]
fn test_arc_multiple_swaps() {
    let mut swap = SmrSwap::new(Arc::new(0));

    for _ in 1..=10 {
        for i in 1..=10 {
            let old = swap.swap(Arc::new(i));
            assert_eq!(*old, i - 1);

            let guard = swap.load();
            assert_eq!(**guard, i);
        }
        // Reset to 0 for the next iteration
        // 重置为 0 以便下一次迭代
        swap.swap(Arc::new(0));
    }
}

/// Test swap returns old Arc value
/// 测试 swap 返回旧的 Arc 值
#[test]
fn test_arc_swap_returns_old_value() {
    let mut swap = SmrSwap::new(Arc::new(String::from("original")));

    let old_arc = swap.swap(Arc::new(String::from("new")));

    // Verify we got the old Arc
    // 验证我们得到了旧的 Arc
    assert_eq!(*old_arc, "original");

    // We can still use the old Arc after swap
    // swap 后我们仍然可以使用旧的 Arc
    let cloned = old_arc.clone();
    assert_eq!(*cloned, "original");
}

/// Test swap with Arc reference counting
/// 测试 swap 与 Arc 引用计数
#[test]
fn test_arc_swap_reference_counting() {
    let mut swap = SmrSwap::new(Arc::new(vec![1, 2, 3]));

    let old_arc = swap.swap(Arc::new(vec![4, 5, 6]));

    // Arc may have reference count > 1 due to deferred destruction in SMR
    // Arc 可能有 > 1 的引用计数，因为 SMR 中的延迟回收
    let initial_count = Arc::strong_count(&old_arc);
    assert!(initial_count >= 1);

    // Clone the Arc
    // 克隆 Arc
    let cloned = old_arc.clone();
    assert_eq!(Arc::strong_count(&old_arc), initial_count + 1);
    assert_eq!(Arc::strong_count(&cloned), initial_count + 1);
}

/// Test update_and_fetch_arc basic operation
/// 测试 update_and_fetch_arc 基本操作
#[test]
fn test_arc_update_and_fetch_arc_basic() {
    let mut swap = SmrSwap::new(Arc::new(10));

    let result = swap.update_and_fetch_arc(|x| Arc::new(**x * 2));
    assert_eq!(*result, 20);

    let guard = swap.load();
    assert_eq!(**guard, 20);
}

/// Test update_and_fetch_arc with strings
/// 测试 update_and_fetch_arc（字符串）
#[test]
fn test_arc_update_and_fetch_arc_string() {
    let mut swap = SmrSwap::new(Arc::new(String::from("hello")));

    let result = swap.update_and_fetch_arc(|s| Arc::new(s.to_uppercase()));
    assert_eq!(*result, "HELLO");

    let guard = swap.load();
    assert_eq!(**guard, "HELLO");
}

/// Test update_and_fetch_arc with vectors
/// 测试 update_and_fetch_arc（向量）
#[test]
fn test_arc_update_and_fetch_arc_vector() {
    let mut swap = SmrSwap::new(Arc::new(vec![1, 2, 3]));

    let result = swap.update_and_fetch_arc(|v| {
        let mut new_v = (**v).clone();
        new_v.push(4);
        Arc::new(new_v)
    });

    assert_eq!(*result, vec![1, 2, 3, 4]);

    let guard = swap.load();
    assert_eq!(**guard, vec![1, 2, 3, 4]);
}

/// Test update_and_fetch_arc multiple times
/// 测试多次 update_and_fetch_arc
#[test]
fn test_arc_update_and_fetch_arc_multiple() {
    let mut swap = SmrSwap::new(Arc::new(0));

    for i in 1..=5 {
        let result = swap.update_and_fetch_arc(|x| Arc::new(**x + i));
        let expected = (1..=i).sum::<i32>();
        assert_eq!(*result, expected);

        let guard = swap.load();
        assert_eq!(**guard, expected);
    }
}

/// Test update_and_fetch_arc returns Arc
/// 测试 update_and_fetch_arc 返回 Arc
#[test]
fn test_arc_update_and_fetch_arc_returns_arc() {
    let mut swap = SmrSwap::new(Arc::new(String::from("test")));

    let result_arc = swap.update_and_fetch_arc(|s| Arc::new(format!("{}_updated", s)));

    // Result should be an Arc that we can clone
    // 结果应该是一个我们可以克隆的 Arc
    let initial_count = Arc::strong_count(&result_arc);
    let cloned = result_arc.clone();
    assert_eq!(*cloned, "test_updated");
    // After cloning, the count should increase by 1
    // 克隆后，计数应该增加 1
    assert_eq!(Arc::strong_count(&result_arc), initial_count + 1);
}

/// Test swap with Arc sharing across readers
/// 测试 swap 与 Arc 在读取者间的共享
#[test]
fn test_arc_swap_shared_across_readers() {
    let mut swap = SmrSwap::new(Arc::new(String::from("v1")));
    let reader1 = swap.handle().clone();
    let reader2 = reader1.clone();

    let old = swap.swap(Arc::new(String::from("v2")));

    // Both readers should see the new value
    // 两个读取者都应该看到新值
    assert_eq!(**reader1.load(), "v2");
    assert_eq!(**reader2.load(), "v2");

    // Old Arc is still valid
    // 旧的 Arc 仍然有效
    assert_eq!(*old, "v1");
}

/// Test update_and_fetch_arc with Arc sharing
/// 测试 update_and_fetch_arc 与 Arc 共享
#[test]
fn test_arc_update_and_fetch_arc_shared() {
    let mut swap = SmrSwap::new(Arc::new(vec![1, 2, 3]));
    let reader1 = swap.handle().clone();
    let reader2 = reader1.clone();

    let result = swap.update_and_fetch_arc(|v| {
        let mut new_v = (**v).clone();
        new_v.push(4);
        Arc::new(new_v)
    });

    // All readers should see the updated value
    // 所有读取者都应该看到更新后的值
    assert_eq!(**reader1.load(), vec![1, 2, 3, 4]);
    assert_eq!(**reader2.load(), vec![1, 2, 3, 4]);
    assert_eq!(*result, vec![1, 2, 3, 4]);
}

/// Test swap with complex Arc-wrapped struct
/// 测试 swap 与复杂的 Arc 包装结构体
#[test]
fn test_arc_swap_complex_struct() {
    #[derive(Debug, Clone, PartialEq)]
    struct Data {
        id: i32,
        name: String,
        values: Vec<i32>,
    }

    let mut swap = SmrSwap::new(Arc::new(Data {
        id: 1,
        name: String::from("first"),
        values: vec![1, 2, 3],
    }));
    let reader = swap.handle().clone();

    let old = swap.swap(Arc::new(Data {
        id: 2,
        name: String::from("second"),
        values: vec![4, 5, 6],
    }));

    assert_eq!(old.id, 1);
    assert_eq!(old.name, "first");

    let current = reader.load();
    assert_eq!(current.id, 2);
    assert_eq!(current.name, "second");
}

/// Test update_and_fetch_arc with complex struct
/// 测试 update_and_fetch_arc 与复杂结构体
#[test]
fn test_arc_update_and_fetch_arc_complex_struct() {
    #[derive(Debug, Clone, PartialEq)]
    struct Counter {
        count: i32,
        label: String,
    }

    let mut swap = SmrSwap::new(Arc::new(Counter {
        count: 0,
        label: String::from("initial"),
    }));
    let reader = swap.handle().clone();

    let result = swap.update_and_fetch_arc(|c| {
        Arc::new(Counter {
            count: c.count + 1,
            label: format!("{}_incremented", c.label),
        })
    });

    assert_eq!(result.count, 1);
    assert_eq!(result.label, "initial_incremented");

    let current = reader.load();
    assert_eq!(current.count, 1);
    assert_eq!(current.label, "initial_incremented");
}

/// Test swap and update_and_fetch_arc interleaved
/// 测试 swap 和 update_and_fetch_arc 交错
#[test]
fn test_arc_swap_and_update_interleaved() {
    let mut swap = SmrSwap::new(Arc::new(0));

    // First swap
    let old1 = swap.swap(Arc::new(10));
    assert_eq!(*old1, 0);
    assert_eq!(**swap.load(), 10);

    // Then update_and_fetch_arc
    let result1 = swap.update_and_fetch_arc(|x| Arc::new(**x * 2));
    assert_eq!(*result1, 20);
    assert_eq!(**swap.load(), 20);

    // Another swap
    let old2 = swap.swap(Arc::new(100));
    assert_eq!(*old2, 20);
    assert_eq!(**swap.load(), 100);

    // Another update_and_fetch_arc
    let result2 = swap.update_and_fetch_arc(|x| Arc::new(**x + 50));
    assert_eq!(*result2, 150);
    assert_eq!(**swap.load(), 150);
}

/// Test Arc swap with nested Arc
/// 测试 Arc swap 与嵌套 Arc
#[test]
fn test_arc_swap_nested_arc() {
    let mut swap = SmrSwap::new(Arc::new(Arc::new(String::from("nested"))));

    let old = swap.swap(Arc::new(Arc::new(String::from("new_nested"))));

    assert_eq!(**old, "nested");
    assert_eq!(***swap.load(), "new_nested");
}

/// Test update_and_fetch_arc with side effects
/// 测试 update_and_fetch_arc 与副作用
#[test]
fn test_arc_update_and_fetch_arc_side_effects() {
    let mut swap = SmrSwap::new(Arc::new(vec![1, 2, 3]));

    let mut call_count = 0;
    let result = swap.update_and_fetch_arc(|v| {
        call_count += 1;
        let mut new_v = (**v).clone();
        new_v.push(4);
        Arc::new(new_v)
    });

    assert_eq!(call_count, 1);
    assert_eq!(*result, vec![1, 2, 3, 4]);
    assert_eq!(**swap.load(), vec![1, 2, 3, 4]);
}
