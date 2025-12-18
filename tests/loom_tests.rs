//! Loom-based concurrency tests for smr-swap
//!
//! Run with: `cargo test --features loom --test loom_tests`

#![cfg(feature = "loom")]

use loom::model::Builder;
use loom::sync::Arc;
use loom::thread;
use smr_swap::SmrSwap;

/// Test: Basic update and read concurrency
/// 测试：基本的更新和读取并发
#[test]
fn loom_basic_update_read() {
    loom::model(|| {
        let mut swap = SmrSwap::new(0);
        let reader = swap.local();

        let t = thread::spawn(move || {
            let guard = reader.load();
            // Reader might see 0 or 1 depending on interleaving
            // 读者可能看到 0 或 1，取决于交错
            assert!(*guard == 0 || *guard == 1);
        });

        swap.update(|_| 1); // fix: update takes a closure, store takes a value
        t.join().unwrap();
    });
}

/// Test: Multiple updates and reads
/// 测试：多次更新和读取
#[test]
fn loom_multiple_updates_reads() {
    loom::model(|| {
        let mut swap = SmrSwap::new(0);
        let reader = swap.local();

        let t = thread::spawn(move || {
            for _ in 0..2 {
                let guard = reader.load();
                assert!(*guard >= 0 && *guard <= 2);
                drop(guard);
                thread::yield_now();
            }
        });

        swap.store(1);
        thread::yield_now();
        swap.store(2);

        t.join().unwrap();
    });
}

/// Test: SmrSwap read
/// 测试：SmrSwap 读取
#[test]
fn loom_smrswap_read() {
    loom::model(|| {
        let mut swap = SmrSwap::new(10);

        assert_eq!(*swap.load(), 10);

        swap.store(20);

        assert_eq!(*swap.load(), 20);
    });
}

/// Test: Map operation
/// 测试：Map 操作
#[test]
fn loom_map() {
    loom::model(|| {
        let mut swap = SmrSwap::new(5);
        let reader = swap.local();

        let t = thread::spawn(move || {
            let res = reader.map(|v| v * 2);
            assert!(res == 10 || res == 20);
        });

        swap.store(10);
        t.join().unwrap();
    });
}

/// Test: Update and fetch
/// 测试：更新并获取
#[test]
fn loom_update_and_fetch() {
    loom::model(|| {
        let mut swap = SmrSwap::new(10);
        let reader = swap.local();

        let t = thread::spawn(move || {
            let guard = reader.load();
            assert!(*guard == 10 || *guard == 11);
        });

        let new_val = swap.update_and_fetch(|v| v + 1);
        assert_eq!(*new_val, 11);

        t.join().unwrap();
    });
}

/// Test: Fetch and update
/// 测试：获取并更新
#[test]
fn loom_fetch_and_update() {
    loom::model(|| {
        let mut swap = SmrSwap::new(10);
        let reader = swap.local();

        let t = thread::spawn(move || {
            let guard = reader.load();
            assert!(*guard == 10 || *guard == 11);
        });

        {
            let old_val = swap.fetch_and_update(|v| v + 1);
            assert_eq!(*old_val, 10);
        }
        assert_eq!(*swap.load(), 11);

        t.join().unwrap();
    });
}

/// Test: Arc Swap
/// 测试：Arc 交换
#[test]
fn loom_arc_swap() {
    loom::model(|| {
        let mut swap = SmrSwap::new(Arc::new(100));
        let reader = swap.local();

        let t = thread::spawn(move || {
            let guard = reader.load();
            assert!(**guard == 100 || **guard == 200);
        });

        let old = swap.swap(Arc::new(200));
        assert_eq!(*old, 100);

        t.join().unwrap();
    });
}

/// Test: Filter
/// 测试：Filter 操作
#[test]
fn loom_filter() {
    loom::model(|| {
        let mut swap = SmrSwap::new(10);
        let reader = swap.local();

        let t = thread::spawn(move || {
            // Should find it if it's 10 or 20 (depending on timing)
            // But we filter for > 15
            // 应该找到它，如果它是 10 或 20（取决于时序）
            // 但我们筛选 > 15
            let val = reader.filter(|v| *v > 15);

            if let Some(v) = val {
                assert_eq!(*v, 20);
            }
        });

        swap.store(20);
        t.join().unwrap();
    });
}

/// Test: Concurrent Readers
/// 测试：并发读者
#[test]
fn loom_concurrent_readers() {
    let mut builder = Builder::new();
    builder.preemption_bound = Some(3);
    builder.check(|| {
        let mut swap = SmrSwap::new(0);
        let mut handles = vec![];

        // Create readers before spawning threads
        // 在生成线程之前创建读者
        for _ in 0..2 {
            let reader = swap.local();
            handles.push(thread::spawn(move || {
                let guard = reader.load();
                assert!(*guard == 0 || *guard == 1);
            }));
        }

        swap.store(1);

        for h in handles {
            h.join().unwrap();
        }
    });
}

/// Test: Guard validity across updates
/// 测试：跨更新的 guard 有效性
#[test]
fn loom_guard_validity() {
    loom::model(|| {
        let mut swap = SmrSwap::new(0);
        let reader = swap.local();

        // Get guard before update
        let guard = reader.load();
        assert_eq!(*guard, 0);

        // Update should not invalidate existing guard
        swap.store(1);

        // Old guard should still be valid
        assert_eq!(*guard, 0);

        // New guard should see new value
        let new_guard = reader.load();
        assert_eq!(*new_guard, 1);
    });
}

/// Test: ReadGuard clone
/// 测试：ReadGuard 克隆
#[test]
fn loom_read_guard_clone() {
    loom::model(|| {
        let swap = SmrSwap::new(42);
        let reader = swap.local();

        let guard1 = reader.load();
        let guard2 = guard1.clone();

        assert_eq!(*guard1, 42);
        assert_eq!(*guard2, 42);
    });
}

/// Test: SmrReader creation and usage
/// 测试：SmrReader 创建和使用
#[test]
fn loom_smr_reader() {
    loom::model(|| {
        let mut swap = SmrSwap::new(0);
        let smr_reader = swap.reader();

        let t = thread::spawn(move || {
            let local = smr_reader.local();
            let guard = local.load();
            assert!(*guard == 0 || *guard == 1);
        });

        swap.store(1);
        t.join().unwrap();
    });
}

/// Test: LocalReader share (create SmrReader)
/// 测试：LocalReader share (创建 SmrReader)
#[test]
fn loom_share_reader() {
    loom::model(|| {
        let mut swap = SmrSwap::new(0);
        let local = swap.local();
        let smr_reader = local.share();

        let t = thread::spawn(move || {
            let local2 = smr_reader.local();
            let guard = local2.load();
            assert!(*guard == 0 || *guard == 1);
        });

        swap.store(1);
        t.join().unwrap();
    });
}

/// Test: LocalReader into_swmr
/// 测试：LocalReader into_swmr
#[test]
fn loom_into_swmr() {
    loom::model(|| {
        let mut swap = SmrSwap::new(0);
        let local = swap.local();
        let smr_reader = local.into_swmr();

        let t = thread::spawn(move || {
            let local2 = smr_reader.local();
            let guard = local2.load();
            assert!(*guard == 0 || *guard == 1);
        });

        swap.store(1);
        t.join().unwrap();
    });
}
