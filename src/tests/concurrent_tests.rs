//! Concurrent read/write tests for SMR-Swap
//!
//! Tests concurrent behavior with multiple readers and writers,
//! stress tests, and race condition handling

use crate::SmrSwap;
use std::sync::{Arc, Barrier};
use std::thread;

/// Stress test with concurrent readers and writers
/// 并发读写的压力测试
#[test]
fn test_concurrent_stress() {
    let mut swap = SmrSwap::new(Box::new(0));
    let num_updates = 1000;
    let num_readers = 4;

    // Create readers before moving swap to writer thread
    let readers: Vec<_> = (0..num_readers).map(|_| swap.local()).collect();

    thread::scope(|s| {
        s.spawn(|| {
            for i in 1..=num_updates {
                swap.update(Box::new(i));
            }
        });

        for reader in readers {
            s.spawn(move || {
                for _ in 0..10000 {
                    let guard = reader.load();
                    let val = **guard;
                    assert!(val <= num_updates, "Read invalid value: {}", val);
                }
            });
        }
    });

    let reader = swap.local();
    let guard = reader.load();
    assert_eq!(**guard, num_updates);
}

/// Test concurrent readers with multiple updates
/// 测试多个读取者与多个更新的并发
#[test]
fn test_concurrent_multiple_readers() {
    let mut swap = SmrSwap::new(0);
    let num_readers = 8;
    let num_updates = 100;

    // Create readers before moving swap to writer thread
    let readers: Vec<_> = (0..num_readers).map(|_| swap.local()).collect();

    thread::scope(|s| {
        s.spawn(|| {
            for i in 1..=num_updates {
                swap.update(i);
            }
        });

        for reader in readers {
            s.spawn(move || {
                for _ in 0..1000 {
                    let guard = reader.load();
                    let val = *guard;
                    assert!(val <= num_updates, "Read invalid value: {}", val);
                }
            });
        }
    });
}

/// Test drop behavior in concurrent context
/// 测试并发上下文中的 drop 行为
#[test]
fn test_drop_behavior() {
    let swap = SmrSwap::new(10);
    let reader1 = swap.local();
    let reader2 = swap.local();

    let barrier = Arc::new(Barrier::new(3));

    let b1 = barrier.clone();
    let h_owner = thread::spawn(move || {
        b1.wait();
        drop(swap);
    });

    let b2 = barrier.clone();
    let h_reader1 = thread::spawn(move || {
        b2.wait();
        drop(reader1);
    });

    let b3 = barrier.clone();
    let h_reader2 = thread::spawn(move || {
        b3.wait();
        let guard = reader2.load();
        assert_eq!(*guard, 10);
        drop(guard);
        drop(reader2);
    });

    h_owner.join().unwrap();
    h_reader1.join().unwrap();
    h_reader2.join().unwrap();
}

/// Test concurrent readers with held guards
/// 测试并发读取者持有 guard
#[test]
fn test_concurrent_readers_with_held_guards() {
    let mut swap = SmrSwap::new(0);
    let num_readers = 4;
    let num_updates = 50;

    // Create readers before moving swap to writer thread
    let readers: Vec<_> = (0..num_readers).map(|_| swap.local()).collect();

    thread::scope(|s| {
        s.spawn(|| {
            for i in 1..=num_updates {
                swap.update(i);
            }
        });

        for reader in readers {
            s.spawn(move || {
                // Hold multiple guards concurrently
                // 并发持有多个 guard
                let guard1 = reader.load();
                thread::sleep(std::time::Duration::from_millis(5));
                let guard2 = reader.load();

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
    let mut swap = SmrSwap::new(0);
    let num_updates = 50;
    let barrier = Arc::new(Barrier::new(2));

    let reader = swap.local();

    thread::scope(|s| {
        let b_writer = barrier.clone();
        s.spawn(move || {
            for i in 1..=num_updates {
                swap.update(i);
            }
            // Wait for reader to finish before exiting
            b_writer.wait();
        });

        let b_reader = barrier.clone();
        s.spawn(move || {
            // Hold a guard for a while
            // 持有 guard 一段时间
            let guard = reader.load();
            let initial_value = *guard;

            thread::sleep(std::time::Duration::from_millis(10));

            // Guard should still be valid even after updates
            // 即使在更新后，guard 仍应有效
            assert_eq!(*guard, initial_value);

            // Signal writer can exit
            b_reader.wait();
        });
    });
}

/// Test many concurrent readers with frequent updates
/// 测试许多并发读取者与频繁的更新
#[test]
fn test_many_concurrent_readers_frequent_updates() {
    let mut swap = SmrSwap::new(0);
    let num_readers = 16;
    let num_updates = 200;

    // Create readers before moving swap to writer thread
    let readers: Vec<_> = (0..num_readers).map(|_| swap.local()).collect();

    thread::scope(|s| {
        s.spawn(|| {
            for i in 1..=num_updates {
                swap.update(i);
            }
        });

        for reader in readers {
            s.spawn(move || {
                for _ in 0..5000 {
                    let guard = reader.load();
                    let val = *guard;
                    assert!(val <= num_updates, "Read invalid value: {}", val);
                }
            });
        }
    });

    let reader = swap.local();
    let guard = reader.load();
    assert_eq!(*guard, num_updates);
}

/// Test rapid reader cloning
/// 测试快速读取者克隆
#[test]
fn test_rapid_reader_cloning() {
    let mut swap = SmrSwap::new(0);

    // Create readers before moving swap to writer thread
    let readers: Vec<_> = (0..10).map(|_| swap.local()).collect();

    thread::scope(|s| {
        s.spawn(|| {
            for i in 1..=50 {
                swap.update(i);
                thread::yield_now();
            }
        });

        for reader in readers {
            s.spawn(move || {
                for _ in 0..1000 {
                    let guard = reader.load();
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
    let mut swap = SmrSwap::new(vec![0]);
    let num_updates = 100;

    // Create readers before moving swap to writer thread
    let readers: Vec<_> = (0..4).map(|_| swap.local()).collect();

    thread::scope(|s| {
        s.spawn(|| {
            for i in 1..=num_updates {
                swap.update(vec![i]);
            }
        });

        for reader in readers {
            s.spawn(move || {
                for _ in 0..1000 {
                    // Each read should return a valid vector with a single element
                    // 每次读取都应返回一个有效的向量，包含单个元素
                    let guard = reader.load();
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
    let mut swap = SmrSwap::new(0);
    let barrier = Arc::new(Barrier::new(5));

    // Create readers before moving swap to writer thread
    let readers: Vec<_> = (0..4).map(|_| swap.local()).collect();

    thread::scope(|s| {
        let b = barrier.clone();
        s.spawn(move || {
            b.wait();
            for i in 1..=20 {
                swap.update(i);
            }
        });

        for reader in readers {
            let b = barrier.clone();
            s.spawn(move || {
                b.wait();
                for _ in 0..1000 {
                    let guard = reader.load();
                    let _ = *guard;
                }
            });
        }
    });
}

/// Test concurrent read and write with string values
/// 测试字符串值的并发读写
#[test]
fn test_concurrent_string_values() {
    let mut swap = SmrSwap::new(String::from("initial"));
    let num_updates = 100;

    let readers: Vec<_> = (0..4).map(|_| swap.local()).collect();

    thread::scope(|s| {
        s.spawn(|| {
            for i in 1..=num_updates {
                swap.update(format!("value_{}", i));
            }
        });

        for reader in readers {
            s.spawn(move || {
                for _ in 0..1000 {
                    let guard = reader.load();
                    // Value should be either "initial" or "value_N"
                    assert!(
                        guard.starts_with("initial") || guard.starts_with("value_"),
                        "Invalid value: {}",
                        *guard
                    );
                }
            });
        }
    });
}

/// Test guard validity across updates
/// 测试跨更新的 guard 有效性
#[test]
fn test_guard_validity_across_updates() {
    let mut swap = SmrSwap::new(0);
    let reader = swap.local();

    // Get a guard before any updates
    let guard_v0 = reader.load();
    assert_eq!(*guard_v0, 0);

    // Update multiple times
    for i in 1..=10 {
        swap.update(i);

        // Old guard should still be valid
        assert_eq!(*guard_v0, 0);

        // New guard should see new value
        let new_guard = reader.load();
        assert_eq!(*new_guard, i);
    }

    // Original guard still valid
    assert_eq!(*guard_v0, 0);
}

/// Test concurrent guard holding
/// 测试并发 guard 持有
#[test]
fn test_concurrent_guard_holding() {
    let mut swap = SmrSwap::new(0);
    let barrier = Arc::new(Barrier::new(5));

    let readers: Vec<_> = (0..4).map(|_| swap.local()).collect();

    thread::scope(|s| {
        let b = barrier.clone();
        s.spawn(move || {
            b.wait();
            for i in 1..=100 {
                swap.update(i);
            }
        });

        for reader in readers {
            let b = barrier.clone();
            s.spawn(move || {
                b.wait();
                // Hold multiple guards across iterations
                let mut guards = Vec::new();
                for _ in 0..10 {
                    guards.push(reader.load());
                    thread::yield_now();
                }
                // All guards should be valid
                for guard in &guards {
                    let _ = **guard;
                }
            });
        }
    });
}
