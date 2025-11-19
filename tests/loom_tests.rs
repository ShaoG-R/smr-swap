//! Loom-based concurrency tests for smr-swap
//!
//! Run with: `RUSTFLAGS="--cfg loom" cargo test --test loom_tests --features loom --release`

#![cfg(loom)]

use loom::sync::Arc;
use loom::thread;
use smr_swap::new;

/// Test: Basic update and read concurrency
#[test]
fn loom_basic_update_read() {
    loom::model(|| {
        let (mut swapper, reader) = new(0);

        let reader_clone = reader.clone();
        let t = thread::spawn(move || {
            let local = reader_clone.register_reader();
            let guard = local.pin();
            let val = reader_clone.read(&guard);
            // Reader might see 0 or 1 depending on interleaving
            assert!(*val == 0 || *val == 1);
        });

        swapper.update(1);
        t.join().unwrap();
    });
}

/// Test: Multiple updates and reads
#[test]
fn loom_multiple_updates_reads() {
    loom::model(|| {
        let (mut swapper, reader) = new(0);

        let reader_clone = reader.clone();
        let t = thread::spawn(move || {
            let local = reader_clone.register_reader();
            for _ in 0..2 {
                let guard = local.pin();
                let val = reader_clone.read(&guard);
                assert!(*val >= 0 && *val <= 2);
                drop(guard);
                thread::yield_now();
            }
        });

        swapper.update(1);
        thread::yield_now();
        swapper.update(2);

        t.join().unwrap();
    });
}

/// Test: Swapper read (writer reading its own value)
#[test]
fn loom_swapper_read() {
    loom::model(|| {
        let (mut swapper, _reader) = new(10);

        let local = swapper.register_reader();
        let guard = local.pin();
        assert_eq!(*swapper.read(&guard), 10);
        drop(guard);

        swapper.update(20);

        let guard = local.pin();
        assert_eq!(*swapper.read(&guard), 20);
    });
}

/// Test: Map operation
#[test]
fn loom_map() {
    loom::model(|| {
        let (mut swapper, reader) = new(5);

        let reader_clone = reader.clone();
        let t = thread::spawn(move || {
            let local = reader_clone.register_reader();
            let res = reader_clone.map(&local, |v| v * 2);
            assert!(res == 10 || res == 20);
        });

        swapper.update(10);
        t.join().unwrap();
    });
}

/// Test: Update and fetch
#[test]
fn loom_update_and_fetch() {
    loom::model(|| {
        let (mut swapper, reader) = new(10);

        let reader_clone = reader.clone();
        let t = thread::spawn(move || {
            let local = reader_clone.register_reader();
            let guard = local.pin();
            let val = reader_clone.read(&guard);
            assert!(*val == 10 || *val == 11);
        });

        let local = swapper.register_reader();
        let guard = local.pin();
        let new_val = swapper.update_and_fetch(&guard, |v| v + 1);
        assert_eq!(*new_val, 11);

        t.join().unwrap();
    });
}

/// Test: Arc Swap
#[test]
fn loom_arc_swap() {
    loom::model(|| {
        let (mut swapper, reader) = new(Arc::new(100));

        let reader_clone = reader.clone();
        let t = thread::spawn(move || {
            let local = reader_clone.register_reader();
            let guard = local.pin();
            let val = reader_clone.read(&guard);
            assert!(**val == 100 || **val == 200);
        });

        let local = swapper.register_reader();
        let old = swapper.swap(&local, Arc::new(200));
        assert_eq!(*old, 100);

        t.join().unwrap();
    });
}

/// Test: Filter
#[test]
fn loom_filter() {
    loom::model(|| {
        let (mut swapper, reader) = new(10);

        let reader_clone = reader.clone();
        let t = thread::spawn(move || {
            let local = reader_clone.register_reader();
            let guard = local.pin();

            // Should find it if it's 10 or 20 (depending on timing)
            // But we filter for > 15
            let val = reader_clone.filter(&guard, |v| *v > 15);

            if let Some(v) = val {
                assert_eq!(*v, 20);
            }
        });

        swapper.update(20);
        t.join().unwrap();
    });
}

/// Test: Concurrent Readers
#[test]
fn loom_concurrent_readers() {
    loom::model(|| {
        let (mut swapper, reader) = new(0);
        let mut handles = vec![];

        for _ in 0..2 {
            let reader_clone = reader.clone();
            handles.push(thread::spawn(move || {
                let local = reader_clone.register_reader();
                let guard = local.pin();
                let val = reader_clone.read(&guard);
                assert!(*val == 0 || *val == 1);
            }));
        }

        swapper.update(1);

        for h in handles {
            h.join().unwrap();
        }
    });
}
