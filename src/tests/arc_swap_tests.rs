//! Tests for Arc-specialized swap operations
//!
//! Tests for Swapper<Arc<T>> specialized methods: swap and update_and_fetch_arc

use crate::new;
use std::sync::Arc;

/// Test basic swap operation with Arc-wrapped integers
/// 测试基本的 swap 操作（Arc 包装的整数）
#[test]
fn test_arc_swap_basic_int() {
    let (mut swapper, reader) = new(Arc::new(42));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let old_value = swapper.swap(&writer_epoch, Arc::new(100));
    assert_eq!(*old_value, 42);
    let guard = reader_epoch.pin();
    assert_eq!(**reader.read(&guard), 100);
}

/// Test basic swap operation with Arc-wrapped strings
/// 测试基本的 swap 操作（Arc 包装的字符串）
#[test]
fn test_arc_swap_basic_string() {
    let (mut swapper, reader) = new(Arc::new(String::from("hello")));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let old_value = swapper.swap(&writer_epoch, Arc::new(String::from("world")));
    assert_eq!(*old_value, "hello");
    let guard = reader_epoch.pin();
    assert_eq!(**reader.read(&guard), "world");
}

/// Test basic swap operation with Arc-wrapped vectors
/// 测试基本的 swap 操作（Arc 包装的向量）
#[test]
fn test_arc_swap_basic_vector() {
    let (mut swapper, reader) = new(Arc::new(vec![1, 2, 3]));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let old_value = swapper.swap(&writer_epoch, Arc::new(vec![4, 5, 6]));
    assert_eq!(*old_value, vec![1, 2, 3]);
    let guard = reader_epoch.pin();
    assert_eq!(**reader.read(&guard), vec![4, 5, 6]);
}

/// Test multiple sequential swaps
/// 测试多个连续的 swap 操作
#[test]
fn test_arc_multiple_swaps() {
    let (mut swapper, reader) = new(Arc::new(0));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    for _ in 1..=10 {
        for i in 1..=10 {
            let old = swapper.swap(&writer_epoch, Arc::new(i));
            assert_eq!(*old, i - 1);
            let guard = reader_epoch.pin();
            assert_eq!(**reader.read(&guard), i);
        }
        // Reset to 0 for the next iteration
        // 重置为 0 以便下一次迭代
        swapper.swap(&writer_epoch, Arc::new(0));
    }
}

/// Test swap returns old Arc value
/// 测试 swap 返回旧的 Arc 值
#[test]
fn test_arc_swap_returns_old_value() {
    let (mut swapper, _reader) = new(Arc::new(String::from("original")));
    let writer_epoch = swapper.register_reader();

    let old_arc = swapper.swap(&writer_epoch, Arc::new(String::from("new")));

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
    let (mut swapper, _reader) = new(Arc::new(vec![1, 2, 3]));
    let writer_epoch = swapper.register_reader();

    let old_arc = swapper.swap(&writer_epoch, Arc::new(vec![4, 5, 6]));

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
    let (mut swapper, reader) = new(Arc::new(10));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let result = swapper.update_and_fetch_arc(&writer_epoch, |x| Arc::new(**x * 2));
    assert_eq!(*result, 20);
    let guard = reader_epoch.pin();
    assert_eq!(**reader.read(&guard), 20);
}

/// Test update_and_fetch_arc with strings
/// 测试 update_and_fetch_arc（字符串）
#[test]
fn test_arc_update_and_fetch_arc_string() {
    let (mut swapper, reader) = new(Arc::new(String::from("hello")));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let result = swapper.update_and_fetch_arc(&writer_epoch, |s| Arc::new(s.to_uppercase()));
    assert_eq!(*result, "HELLO");
    let guard = reader_epoch.pin();
    assert_eq!(**reader.read(&guard), "HELLO");
}

/// Test update_and_fetch_arc with vectors
/// 测试 update_and_fetch_arc（向量）
#[test]
fn test_arc_update_and_fetch_arc_vector() {
    let (mut swapper, reader) = new(Arc::new(vec![1, 2, 3]));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let result = swapper.update_and_fetch_arc(&writer_epoch, |v| {
        let mut new_v = (**v).clone();
        new_v.push(4);
        Arc::new(new_v)
    });

    assert_eq!(*result, vec![1, 2, 3, 4]);
    let guard = reader_epoch.pin();
    assert_eq!(**reader.read(&guard), vec![1, 2, 3, 4]);
}

/// Test update_and_fetch_arc multiple times
/// 测试多次 update_and_fetch_arc
#[test]
fn test_arc_update_and_fetch_arc_multiple() {
    let (mut swapper, reader) = new(Arc::new(0));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    for i in 1..=5 {
        let result = swapper.update_and_fetch_arc(&writer_epoch, |x| Arc::new(**x + i));
        let expected = (1..=i).sum::<i32>();
        assert_eq!(*result, expected);
        let guard = reader_epoch.pin();
        assert_eq!(**reader.read(&guard), expected);
    }
}

/// Test update_and_fetch_arc returns Arc
/// 测试 update_and_fetch_arc 返回 Arc
#[test]
fn test_arc_update_and_fetch_arc_returns_arc() {
    let (mut swapper, _reader) = new(Arc::new(String::from("test")));
    let writer_epoch = swapper.register_reader();

    let result_arc =
        swapper.update_and_fetch_arc(&writer_epoch, |s| Arc::new(format!("{}_updated", s)));

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
    let (mut swapper, reader1) = new(Arc::new(String::from("v1")));
    let reader2 = reader1.clone();
    let writer_epoch = swapper.register_reader();
    let reader1_epoch = reader1.register_reader();
    let reader2_epoch = reader2.register_reader();

    let old = swapper.swap(&writer_epoch, Arc::new(String::from("v2")));

    // Both readers should see the new value
    // 两个读取者都应该看到新值
    let guard1 = reader1_epoch.pin();
    let guard2 = reader2_epoch.pin();
    assert_eq!(**reader1.read(&guard1), "v2");
    assert_eq!(**reader2.read(&guard2), "v2");

    // Old Arc is still valid
    // 旧的 Arc 仍然有效
    assert_eq!(*old, "v1");
}

/// Test update_and_fetch_arc with Arc sharing
/// 测试 update_and_fetch_arc 与 Arc 共享
#[test]
fn test_arc_update_and_fetch_arc_shared() {
    let (mut swapper, reader1) = new(Arc::new(vec![1, 2, 3]));
    let reader2 = reader1.clone();
    let writer_epoch = swapper.register_reader();
    let reader1_epoch = reader1.register_reader();
    let reader2_epoch = reader2.register_reader();

    let result = swapper.update_and_fetch_arc(&writer_epoch, |v| {
        let mut new_v = (**v).clone();
        new_v.push(4);
        Arc::new(new_v)
    });

    // All readers should see the updated value
    // 所有读取者都应该看到更新后的值
    let guard1 = reader1_epoch.pin();
    let guard2 = reader2_epoch.pin();
    assert_eq!(**reader1.read(&guard1), vec![1, 2, 3, 4]);
    assert_eq!(**reader2.read(&guard2), vec![1, 2, 3, 4]);
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

    let (mut swapper, reader) = new(Arc::new(Data {
        id: 1,
        name: String::from("first"),
        values: vec![1, 2, 3],
    }));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let old = swapper.swap(
        &writer_epoch,
        Arc::new(Data {
            id: 2,
            name: String::from("second"),
            values: vec![4, 5, 6],
        }),
    );

    assert_eq!(old.id, 1);
    assert_eq!(old.name, "first");

    let guard = reader_epoch.pin();
    let current = reader.read(&guard);
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

    let (mut swapper, reader) = new(Arc::new(Counter {
        count: 0,
        label: String::from("initial"),
    }));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let result = swapper.update_and_fetch_arc(&writer_epoch, |c| {
        Arc::new(Counter {
            count: c.count + 1,
            label: format!("{}_incremented", c.label),
        })
    });

    assert_eq!(result.count, 1);
    assert_eq!(result.label, "initial_incremented");

    let guard = reader_epoch.pin();
    let current = reader.read(&guard);
    assert_eq!(current.count, 1);
    assert_eq!(current.label, "initial_incremented");
}

/// Test swap and update_and_fetch_arc interleaved
/// 测试 swap 和 update_and_fetch_arc 交错
#[test]
fn test_arc_swap_and_update_interleaved() {
    let (mut swapper, reader) = new(Arc::new(0));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    // First swap
    let old1 = swapper.swap(&writer_epoch, Arc::new(10));
    assert_eq!(*old1, 0);
    let guard1 = reader_epoch.pin();
    assert_eq!(**reader.read(&guard1), 10);
    drop(guard1);

    // Then update_and_fetch_arc
    let result1 = swapper.update_and_fetch_arc(&writer_epoch, |x| Arc::new(**x * 2));
    assert_eq!(*result1, 20);
    let guard2 = reader_epoch.pin();
    assert_eq!(**reader.read(&guard2), 20);
    drop(guard2);

    // Another swap
    let old2 = swapper.swap(&writer_epoch, Arc::new(100));
    assert_eq!(*old2, 20);
    let guard3 = reader_epoch.pin();
    assert_eq!(**reader.read(&guard3), 100);
    drop(guard3);

    // Another update_and_fetch_arc
    let result2 = swapper.update_and_fetch_arc(&writer_epoch, |x| Arc::new(**x + 50));
    assert_eq!(*result2, 150);
    let guard4 = reader_epoch.pin();
    assert_eq!(**reader.read(&guard4), 150);
}

/// Test Arc swap with nested Arc
/// 测试 Arc swap 与嵌套 Arc
#[test]
fn test_arc_swap_nested_arc() {
    let (mut swapper, reader) = new(Arc::new(Arc::new(String::from("nested"))));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let old = swapper.swap(
        &writer_epoch,
        Arc::new(Arc::new(String::from("new_nested"))),
    );

    assert_eq!(**old, "nested");
    let guard = reader_epoch.pin();
    assert_eq!(***reader.read(&guard), "new_nested");
}

/// Test update_and_fetch_arc with side effects
/// 测试 update_and_fetch_arc 与副作用
#[test]
fn test_arc_update_and_fetch_arc_side_effects() {
    let (mut swapper, reader) = new(Arc::new(vec![1, 2, 3]));
    let writer_epoch = swapper.register_reader();
    let reader_epoch = reader.register_reader();

    let mut call_count = 0;
    let result = swapper.update_and_fetch_arc(&writer_epoch, |v| {
        call_count += 1;
        let mut new_v = (**v).clone();
        new_v.push(4);
        Arc::new(new_v)
    });

    assert_eq!(call_count, 1);
    assert_eq!(*result, vec![1, 2, 3, 4]);
    let guard = reader_epoch.pin();
    assert_eq!(**reader.read(&guard), vec![1, 2, 3, 4]);
}
