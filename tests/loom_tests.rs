//! Loom-based concurrency tests for smr-swap
//!
//! Run with: `cargo test --features loom --test loom_tests`

#![cfg(feature = "loom")]

use loom::sync::Arc;
use loom::thread;
use loom::model::Builder;
use smr_swap::SmrSwap;

/// Test: Basic update and read concurrency
#[test]
fn loom_basic_update_read() {
    loom::model(|| {
        let mut swap = SmrSwap::new(0);
        let reader = swap.reader().fork();

        let t = thread::spawn(move || {
            let guard = reader.load();
            // Reader might see 0 or 1 depending on interleaving
            assert!(*guard == 0 || *guard == 1);
        });

        swap.update(1);
        t.join().unwrap();
    });
}

/// Test: Multiple updates and reads
#[test]
fn loom_multiple_updates_reads() {
    loom::model(|| {
        let mut swap = SmrSwap::new(0);
        let reader = swap.reader().fork();

        let t = thread::spawn(move || {
            for _ in 0..2 {
                let guard = reader.load();
                assert!(*guard >= 0 && *guard <= 2);
                drop(guard);
                thread::yield_now();
            }
        });

        swap.update(1);
        thread::yield_now();
        swap.update(2);

        t.join().unwrap();
    });
}

/// Test: Swapper read (via SmrSwap)
#[test]
fn loom_swapper_read() {
    loom::model(|| {
        let mut swap = SmrSwap::new(10);

        assert_eq!(*swap.load(), 10);

        swap.update(20);

        assert_eq!(*swap.load(), 20);
    });
}

/// Test: Map operation
#[test]
fn loom_map() {
    loom::model(|| {
        let mut swap = SmrSwap::new(5);
        let reader = swap.reader().fork();

        let t = thread::spawn(move || {
            let res = reader.map(|v| v * 2);
            assert!(res == 10 || res == 20);
        });

        swap.update(10);
        t.join().unwrap();
    });
}

/// Test: Update and fetch
#[test]
fn loom_update_and_fetch() {
    loom::model(|| {
        let mut swap = SmrSwap::new(10);
        let reader = swap.reader().fork();

        let t = thread::spawn(move || {
            let guard = reader.load();
            assert!(*guard == 10 || *guard == 11);
        });

        let new_val = swap.update_and_fetch(|v| v + 1);
        assert_eq!(*new_val, 11);

        t.join().unwrap();
    });
}

/// Test: Arc Swap
#[test]
fn loom_arc_swap() {
    loom::model(|| {
        let mut swap = SmrSwap::new(Arc::new(100));
        let reader = swap.reader().fork();

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
#[test]
fn loom_filter() {
    loom::model(|| {
        let mut swap = SmrSwap::new(10);
        let reader = swap.reader().fork();

        let t = thread::spawn(move || {
            // Should find it if it's 10 or 20 (depending on timing)
            // But we filter for > 15
            let val = reader.filter(|v| *v > 15);

            if let Some(v) = val {
                assert_eq!(*v, 20);
            }
        });

        swap.update(20);
        t.join().unwrap();
    });
}

/// Test: Concurrent Readers
#[test]
fn loom_concurrent_readers() {
    let mut builder = Builder::new();
    builder.preemption_bound = Some(3);
    builder.check(|| {
        let mut swap = SmrSwap::new(0);
        let mut handles = vec![];

        for _ in 0..2 {
            let reader = swap.reader().fork();
            handles.push(thread::spawn(move || {
                let guard = reader.load();
                assert!(*guard == 0 || *guard == 1);
            }));
        }

        swap.update(1);

        for h in handles {
            h.join().unwrap();
        }
    });
}
