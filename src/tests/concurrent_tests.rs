//! Concurrent read/write tests for SMR-Swap
//!
//! Tests concurrent behavior with multiple readers and writers,
//! stress tests, and race condition handling

use crate::new;
use std::sync::{Arc, Barrier};
use std::thread;

/// Stress test with concurrent readers and writers
/// 并发读写的压力测试
#[test]
fn test_concurrent_stress() {
    let (mut swapper, reader) = new(Box::new(0));
    let num_updates = 1000;
    let num_readers = 4;

    thread::scope(|s| {
        s.spawn(move || {
            for i in 1..=num_updates {
                swapper.update(Box::new(i));
            }
        });

        for _ in 0..num_readers {
            let reader_clone = reader.clone();
            s.spawn(move || {
                let reader_epoch = reader_clone.register_reader();
                for _ in 0..10000 {
                    let guard = reader_clone.read(&reader_epoch);
                    let val = **guard;
                    assert!(val <= num_updates, "Read invalid value: {}", val);
                }
            });
        }
    });

    let reader_epoch = reader.register_reader();
    assert_eq!(**reader.read(&reader_epoch), num_updates);
}

/// Test concurrent readers with multiple updates
/// 测试多个读取者与多个更新的并发
#[test]
fn test_concurrent_multiple_readers() {
    let (mut swapper, reader) = new(0);
    let num_readers = 8;
    let num_updates = 100;

    thread::scope(|s| {
        s.spawn(move || {
            for i in 1..=num_updates {
                swapper.update(i);
            }
        });

        for _ in 0..num_readers {
            let reader_clone = reader.clone();
            s.spawn(move || {
                let reader_epoch = reader_clone.register_reader();
                for _ in 0..1000 {
                    let guard = reader_clone.read(&reader_epoch);
                    let val = *guard;
                    assert!(val <= num_updates, "Read invalid value: {}", val);
                }
            });
        }
    });
}

/// Test drop behavior and None returns in race conditions
/// 测试 drop 行为和竞态中的 None 返回
#[test]
fn test_drop_behavior_and_none() {
    let (swapper, reader) = new(10);
    let reader_clone = reader.clone();

    let barrier = Arc::new(Barrier::new(3));

    let b1 = barrier.clone();
    let h_writer = thread::spawn(move || {
        b1.wait();
        drop(swapper);
    });

    let b2 = barrier.clone();
    let h_reader1 = thread::spawn(move || {
        b2.wait();
        drop(reader);
    });

    let b3 = barrier.clone();
    let h_reader2 = thread::spawn(move || {
        b3.wait();
        let reader_epoch = reader_clone.register_reader();
        let guard = reader_clone.read(&reader_epoch);
        assert_eq!(*guard, 10);
        drop(reader_clone);
    });

    h_writer.join().unwrap();
    h_reader1.join().unwrap();
    h_reader2.join().unwrap();

    // Test race condition between read() and drop()
    // 测试 read() 和 drop() 之间的竞态
    let (swapper, reader) = new(10);
    drop(swapper);

    thread::scope(|s| {
        let reader_clone = reader.clone();
        s.spawn(move || {
            drop(reader_clone);
        });

        s.spawn(move || {
            let reader_epoch = reader.register_reader();
            for _ in 0..1000 {
                let guard = reader.read(&reader_epoch);
                assert_eq!(*guard, 10);
            }
        });
    });
}

/// Test concurrent readers with held guards
/// 测试并发读取者持有 guard
#[test]
fn test_concurrent_readers_with_held_guards() {
    let (mut swapper, reader) = new(0);
    let num_readers = 4;
    let num_updates = 50;

    thread::scope(|s| {
        s.spawn(move || {
            for i in 1..=num_updates {
                swapper.update(i);
            }
        });

        for _ in 0..num_readers {
            let reader_clone = reader.clone();
            s.spawn(move || {
                let reader_epoch = reader_clone.register_reader();
                // Hold multiple guards concurrently
                // 并发持有多个 guard
                let guard1 = reader_clone.read(&reader_epoch);
                thread::sleep(std::time::Duration::from_millis(5));
                let guard2 = reader_clone.read(&reader_epoch);

                // Both guards should be valid
                // 两个 guard 都应该有效
                let _ = (*guard1, *guard2);
            });
        }
    });
}

/// Test reader holds guard while writer updates
/// 测试读取者在写入者更新时持有 guard
#[test]
fn test_reader_holds_guard_during_updates() {
    let (mut swapper, reader) = new(0);
    let num_updates = 50;

    thread::scope(|s| {
        s.spawn(move || {
            for i in 1..=num_updates {
                swapper.update(i);
            }
        });

        s.spawn(move || {
            let reader_epoch = reader.register_reader();
            // Hold a guard for a while
            // 持有 guard 一段时间
            let guard = reader.read(&reader_epoch);
            let initial_value = *guard;
            thread::sleep(std::time::Duration::from_millis(10));
            // Guard should still be valid even after updates
            // 即使在更新后，guard 仍应有效
            assert_eq!(*guard, initial_value);
        });
    });
}

/// Test many concurrent readers with frequent updates
/// 测试许多并发读取者与频繁的更新
#[test]
fn test_many_concurrent_readers_frequent_updates() {
    let (mut swapper, reader) = new(0);
    let num_readers = 16;
    let num_updates = 200;

    thread::scope(|s| {
        s.spawn(move || {
            for i in 1..=num_updates {
                swapper.update(i);
            }
        });

        for _ in 0..num_readers {
            let reader_clone = reader.clone();
            s.spawn(move || {
                let reader_epoch = reader_clone.register_reader();
                for _ in 0..5000 {
                    let guard = reader_clone.read(&reader_epoch);
                    let val = *guard;
                    assert!(val <= num_updates, "Read invalid value: {}", val);
                }
            });
        }
    });

    let reader_epoch = reader.register_reader();
    assert_eq!(*reader.read(&reader_epoch), num_updates);
}

/// Test rapid reader creation and cloning
/// 测试快速读取者创建和克隆
#[test]
fn test_rapid_reader_creation() {
    let (mut swapper, reader) = new(0);

    thread::scope(|s| {
        s.spawn(move || {
            for i in 1..=50 {
                swapper.update(i);
                thread::yield_now();
            }
        });

        for _ in 0..10 {
            let reader_clone = reader.clone();
            s.spawn(move || {
                let reader_epoch = reader_clone.register_reader();
                for _ in 0..1000 {
                    let guard = reader_clone.read(&reader_epoch);
                    let _ = *guard;
                }
            });
        }
    });
}

/// Test reader consistency across concurrent updates
/// 测试并发更新中的读取者一致性
#[test]
fn test_reader_consistency_concurrent_updates() {
    let (mut swapper, reader) = new(vec![0]);
    let num_updates = 100;

    thread::scope(|s| {
        s.spawn(move || {
            for i in 1..=num_updates {
                swapper.update(vec![i]);
            }
        });

        for _ in 0..4 {
            let reader_clone = reader.clone();
            s.spawn(move || {
                let reader_epoch = reader_clone.register_reader();
                for _ in 0..1000 {
                    // Each read should return a valid vector with a single element
                    // 每次读取都应返回一个有效的向量，包含单个元素
                    let guard = reader_clone.read(&reader_epoch);
                    assert_eq!(guard.len(), 1);
                    let val = guard[0];
                    assert!(val <= num_updates, "Read invalid value: {}", val);
                }
            });
        }
    });
}

/// Test synchronization with barrier
/// 测试使用 barrier 的同步
#[test]
fn test_synchronization_with_barrier() {
    let (mut swapper, reader) = new(0);
    let barrier = Arc::new(Barrier::new(5));

    thread::scope(|s| {
        let b = barrier.clone();
        s.spawn(move || {
            b.wait();
            for i in 1..=20 {
                swapper.update(i);
            }
        });

        for _ in 0..4 {
            let reader_clone = reader.clone();
            let b = barrier.clone();
            s.spawn(move || {
                let reader_epoch = reader_clone.register_reader();
                b.wait();
                for _ in 0..1000 {
                    let guard = reader_clone.read(&reader_epoch);
                    let _ = *guard;
                }
            });
        }
    });
}
