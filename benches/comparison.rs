use arc_swap::ArcSwap;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use smr_swap::SmrSwap;
use std::hint::black_box;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// 常量配置
// ============================================================================
const DATA_SIZE: usize = 64;
const LARGE_DATA_SIZE: usize = 10000;
const BATCH_SIZE: usize = 10;
const BATCH_THREADS: usize = 4;
const PRESSURE_WARMUP: usize = 1000;
const GUARD_HOLD_MICROS: u64 = 10;

// ============================================================================
// 数据创建工厂
// ============================================================================
#[inline]
fn create_data(size: usize) -> Vec<u32> {
    vec![1u32; size]
}

#[inline]
fn create_indexed_data(i: u64, size: usize) -> Vec<u32> {
    vec![i as u32; size]
}

// ============================================================================
// 数据结构操作封装
// ============================================================================

/// SmrSwap 操作
mod smr_ops {
    use smr_swap::LocalReader;

    use super::*;

    #[inline]
    pub fn new(size: usize) -> SmrSwap<Vec<u32>> {
        SmrSwap::new(create_data(size))
    }

    #[inline]
    pub fn create_readers(swap: &SmrSwap<Vec<u32>>, count: usize) -> Vec<LocalReader<Vec<u32>>> {
        (0..count).map(|_| swap.local()).collect()
    }

    #[inline]
    pub fn read(local: &LocalReader<Vec<u32>>) {
        let guard = local.load();
        black_box(&*guard);
    }

    #[inline]
    pub fn write(swap: &mut SmrSwap<Vec<u32>>, i: u64, size: usize) {
        swap.store(create_indexed_data(i, size));
    }
}

/// ArcSwap 操作
mod arc_ops {
    use super::*;

    pub type SharedArcSwap = Arc<ArcSwap<Vec<u32>>>;

    #[inline]
    pub fn new(size: usize) -> SharedArcSwap {
        Arc::new(ArcSwap::new(Arc::new(create_data(size))))
    }

    #[inline]
    pub fn new_local(size: usize) -> ArcSwap<Vec<u32>> {
        ArcSwap::new(Arc::new(create_data(size)))
    }

    #[inline]
    pub fn read(arc_swap: &ArcSwap<Vec<u32>>) {
        let guard = arc_swap.load();
        black_box(&*guard);
    }

    #[inline]
    pub fn write(arc_swap: &ArcSwap<Vec<u32>>, i: u64, size: usize) {
        arc_swap.store(Arc::new(create_indexed_data(i, size)));
    }
}

/// Mutex 操作 - 原地修改，无内存分配
mod mutex_ops {
    use super::*;

    pub type SharedMutex = Arc<Mutex<Vec<u32>>>;

    #[inline]
    pub fn new(size: usize) -> SharedMutex {
        Arc::new(Mutex::new(create_data(size)))
    }

    #[inline]
    pub fn read(mutex: &Mutex<Vec<u32>>) {
        let guard = mutex.lock().unwrap();
        black_box(&*guard);
    }

    /// 原地修改，避免内存分配
    #[inline]
    pub fn write_inplace(mutex: &Mutex<Vec<u32>>, i: u64) {
        let mut guard = mutex.lock().unwrap();
        guard.fill(i as u32);
    }
}

/// RwLock 操作 - 原地修改，无内存分配
mod rwlock_ops {
    use super::*;

    pub type SharedRwLock = Arc<RwLock<Vec<u32>>>;

    #[inline]
    pub fn new(size: usize) -> SharedRwLock {
        Arc::new(RwLock::new(create_data(size)))
    }

    #[inline]
    pub fn read(rwlock: &RwLock<Vec<u32>>) {
        let guard = rwlock.read().unwrap();
        black_box(&*guard);
    }

    /// 原地修改，避免内存分配
    #[inline]
    pub fn write_inplace(rwlock: &RwLock<Vec<u32>>, i: u64) {
        let mut guard = rwlock.write().unwrap();
        guard.fill(i as u32);
    }
}

// ============================================================================
// 基准测试 0: Handle 操作性能 (Clone 等)
// ============================================================================
fn bench_handle_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("handle_ops");

    // 1. Clone 性能
    group.bench_function("smr_swap_local_clone", |b| {
        let swap = smr_ops::new(DATA_SIZE);
        let local = swap.local();
        b.iter(|| {
            let _ = black_box(local.clone());
        });
    });

    group.bench_function("arc_swap_clone", |b| {
        let arc_swap = arc_ops::new(DATA_SIZE);
        b.iter(|| {
            let _ = black_box(arc_swap.clone());
        });
    });

    group.bench_function("mutex_clone", |b| {
        let mutex = mutex_ops::new(DATA_SIZE);
        b.iter(|| {
            let _ = black_box(mutex.clone());
        });
    });

    // 2. SmrSwap Local 特有操作
    group.bench_function("smr_swap_is_pinned", |b| {
        let swap = smr_ops::new(DATA_SIZE);
        let local = swap.local();
        b.iter(|| {
            let _ = black_box(local.is_pinned());
        });
    });

    group.bench_function("smr_swap_version", |b| {
        let swap = smr_ops::new(DATA_SIZE);
        let local = swap.local();
        b.iter(|| {
            let _ = black_box(local.version());
        });
    });

    group.finish();
}

// ============================================================================
// 基准测试 0.1: 结构体创建性能
// ============================================================================
fn bench_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("creation");

    group.bench_function("smr_swap", |b| {
        b.iter(|| {
            let _ = black_box(smr_ops::new(DATA_SIZE));
        });
    });

    group.bench_function("arc_swap", |b| {
        b.iter(|| {
            let _ = black_box(arc_ops::new(DATA_SIZE));
        });
    });

    group.bench_function("mutex", |b| {
        b.iter(|| {
            let _ = black_box(mutex_ops::new(DATA_SIZE));
        });
    });

    group.finish();
}

// ============================================================================
// 基准测试 0.2: Drop 性能
// ============================================================================
fn bench_drop(c: &mut Criterion) {
    let mut group = c.benchmark_group("drop");

    group.bench_function("smr_swap", |b| {
        b.iter_with_setup(
            || smr_ops::new(DATA_SIZE),
            |swap| std::mem::drop(black_box(swap)),
        );
    });

    group.bench_function("arc_swap", |b| {
        b.iter_with_setup(
            || arc_ops::new(DATA_SIZE),
            |swap| std::mem::drop(black_box(swap)),
        );
    });

    group.bench_function("mutex", |b| {
        b.iter_with_setup(
            || mutex_ops::new(DATA_SIZE),
            |mutex| std::mem::drop(black_box(mutex)),
        );
    });

    group.finish();
}

// ============================================================================
// 基准测试 1: 单线程读取性能
// ============================================================================
fn bench_single_thread_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_read");
    group.sample_size(100);

    group.bench_function("smr_swap", |b| {
        let swap = smr_ops::new(DATA_SIZE);
        let local = swap.local();
        b.iter(|| smr_ops::read(&local));
    });

    group.bench_function("arc_swap", |b| {
        let arc_swap = arc_ops::new_local(DATA_SIZE);
        b.iter(|| arc_ops::read(&arc_swap));
    });

    group.finish();
}

// ============================================================================
// 基准测试 2: 单线程写入性能
// ============================================================================
fn bench_single_thread_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_write");
    group.sample_size(100);

    group.bench_function("smr_swap", |b| {
        let mut swap = smr_ops::new(DATA_SIZE);
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            smr_ops::write(&mut swap, counter, DATA_SIZE);
        });
    });

    group.bench_function("arc_swap", |b| {
        let arc_swap = arc_ops::new_local(DATA_SIZE);
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            arc_ops::write(&arc_swap, counter, DATA_SIZE);
        });
    });

    group.finish();
}

// ============================================================================
// 基准测试 3: 多线程读取性能 (N个读取者)
// ============================================================================
fn bench_multi_thread_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread_read");
    group.sample_size(50);

    for num_readers in [2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("smr_swap", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let swap = smr_ops::new(DATA_SIZE);
                    let readers = smr_ops::create_readers(&swap, num_readers);

                    let start = Instant::now();
                    thread::scope(|s| {
                        for reader in readers {
                            s.spawn(move || {
                                for _ in 0..iters {
                                    smr_ops::read(&reader);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("arc_swap", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let arc_swap = arc_ops::new(DATA_SIZE);

                    let start = Instant::now();
                    thread::scope(|s| {
                        for _ in 0..num_readers {
                            let arc_swap = arc_swap.clone();
                            s.spawn(move || {
                                for _ in 0..iters {
                                    arc_ops::read(&arc_swap);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// 基准测试 4: 混合读写性能 (1个写入者 + N个读取者)
// ============================================================================
fn bench_mixed_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_read_write");
    group.sample_size(30);

    for num_readers in [2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("smr_swap", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let mut swap = smr_ops::new(DATA_SIZE);
                    let readers = smr_ops::create_readers(&swap, num_readers);

                    let start = Instant::now();
                    thread::scope(|s| {
                        s.spawn(|| {
                            for i in 0..iters {
                                smr_ops::write(&mut swap, i, DATA_SIZE);
                            }
                        });

                        for reader in readers {
                            s.spawn(move || {
                                for _ in 0..iters {
                                    smr_ops::read(&reader);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("arc_swap", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let arc_swap = arc_ops::new(DATA_SIZE);

                    let start = Instant::now();
                    thread::scope(|s| {
                        let writer = arc_swap.clone();
                        s.spawn(move || {
                            for i in 0..iters {
                                arc_ops::write(&writer, i, DATA_SIZE);
                            }
                        });

                        for _ in 0..num_readers {
                            let reader = arc_swap.clone();
                            s.spawn(move || {
                                for _ in 0..iters {
                                    arc_ops::read(&reader);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// 基准测试 5: 多写多读性能 (M个写入者 + N个读取者)
// ============================================================================
fn bench_multi_writer_multi_reader(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_writer_multi_reader");
    group.sample_size(30);

    const NUM_WRITERS: usize = 4;

    for num_readers in [4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("mutex", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let mutex = mutex_ops::new(DATA_SIZE);

                    let start = Instant::now();
                    thread::scope(|s| {
                        for _ in 0..NUM_WRITERS {
                            let mutex = mutex.clone();
                            s.spawn(move || {
                                for i in 0..iters {
                                    mutex_ops::write_inplace(&mutex, i);
                                }
                            });
                        }

                        for _ in 0..num_readers {
                            let mutex = mutex.clone();
                            s.spawn(move || {
                                for _ in 0..iters {
                                    mutex_ops::read(&mutex);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("smr_swap", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let swap = Arc::new(Mutex::new(smr_ops::new(DATA_SIZE)));
                    let readers: Vec<_> = (0..num_readers)
                        .map(|_| swap.lock().unwrap().local())
                        .collect();

                    let start = Instant::now();
                    thread::scope(|s| {
                        for _ in 0..NUM_WRITERS {
                            let swap = swap.clone();
                            s.spawn(move || {
                                for i in 0..iters {
                                    swap.lock()
                                        .unwrap()
                                        .store(create_indexed_data(i, DATA_SIZE));
                                }
                            });
                        }

                        for reader in readers {
                            s.spawn(move || {
                                for _ in 0..iters {
                                    smr_ops::read(&reader);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("arc_swap", num_readers),
            &num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let arc_swap = arc_ops::new(DATA_SIZE);

                    let start = Instant::now();
                    thread::scope(|s| {
                        for _ in 0..NUM_WRITERS {
                            let arc_swap = arc_swap.clone();
                            s.spawn(move || {
                                for i in 0..iters {
                                    arc_ops::write(&arc_swap, i, DATA_SIZE);
                                }
                            });
                        }

                        for _ in 0..num_readers {
                            let arc_swap = arc_swap.clone();
                            s.spawn(move || {
                                for _ in 0..iters {
                                    arc_ops::read(&arc_swap);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// 基准测试 6: 读取延迟（持有读取守卫期间的写入）
// ============================================================================
fn bench_read_latency_with_held_guard(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_latency_with_held_guard");
    group.sample_size(50);

    group.bench_function("smr_swap", |b| {
        let mut swap = smr_ops::new(DATA_SIZE);

        b.iter_custom(|iters| {
            let reader = swap.local();
            let start = Instant::now();
            thread::scope(|s| {
                s.spawn(move || {
                    let _guard = reader.load();
                    std::thread::sleep(Duration::from_micros(GUARD_HOLD_MICROS));
                });

                for i in 0..iters {
                    smr_ops::write(&mut swap, i, DATA_SIZE);
                }
            });
            start.elapsed()
        });
    });

    group.bench_function("arc_swap", |b| {
        let arc_swap = arc_ops::new(DATA_SIZE);

        b.iter_custom(|iters| {
            let start = Instant::now();
            thread::scope(|s| {
                let reader = arc_swap.clone();
                s.spawn(move || {
                    let _guard = reader.load();
                    std::thread::sleep(Duration::from_micros(GUARD_HOLD_MICROS));
                });

                for i in 0..iters {
                    arc_ops::write(&arc_swap, i, DATA_SIZE);
                }
            });
            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// 基准测试 7: 批量读取（减少 pin 开销）
// ============================================================================
fn bench_batch_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_read");
    group.sample_size(50);

    group.bench_function("smr_swap_batch", |b| {
        let swap = smr_ops::new(DATA_SIZE);

        b.iter_custom(|iters| {
            let locals = smr_ops::create_readers(&swap, BATCH_THREADS);

            let start = Instant::now();
            thread::scope(|s| {
                for local in locals {
                    s.spawn(move || {
                        for _ in 0..iters {
                            let guard = local.load();
                            for _ in 0..BATCH_SIZE {
                                black_box(&*guard);
                            }
                        }
                    });
                }
            });
            start.elapsed()
        });
    });

    group.bench_function("arc_swap_batch", |b| {
        let arc_swap = arc_ops::new(DATA_SIZE);

        b.iter_custom(|iters| {
            let start = Instant::now();
            thread::scope(|s| {
                for _ in 0..BATCH_THREADS {
                    let arc_swap = arc_swap.clone();
                    s.spawn(move || {
                        for _ in 0..iters {
                            let guard = arc_swap.load();
                            for _ in 0..BATCH_SIZE {
                                black_box(&*guard);
                            }
                        }
                    });
                }
            });
            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// 基准测试 8: 内存分配压力下的读取
// ============================================================================
fn bench_read_under_memory_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_under_memory_pressure");
    group.sample_size(50);

    group.bench_function("smr_swap", |b| {
        let mut swap = smr_ops::new(LARGE_DATA_SIZE);

        // 预热：积累垃圾
        for i in 0..PRESSURE_WARMUP {
            smr_ops::write(&mut swap, i as u64, LARGE_DATA_SIZE);
        }

        b.iter_custom(|iters| {
            let reader = swap.local();

            let start = Instant::now();
            thread::scope(|s| {
                s.spawn(move || {
                    for _ in 0..iters {
                        smr_ops::read(&reader);
                    }
                });

                for i in 0..iters {
                    smr_ops::write(&mut swap, i, LARGE_DATA_SIZE);
                }
            });
            start.elapsed()
        });
    });

    group.bench_function("arc_swap", |b| {
        let arc_swap = arc_ops::new(LARGE_DATA_SIZE);

        // 预热
        for i in 0..PRESSURE_WARMUP {
            arc_ops::write(&arc_swap, i as u64, LARGE_DATA_SIZE);
        }

        b.iter_custom(|iters| {
            let start = Instant::now();
            thread::scope(|s| {
                let reader = arc_swap.clone();
                s.spawn(move || {
                    for _ in 0..iters {
                        arc_ops::read(&reader);
                    }
                });

                for i in 0..iters {
                    arc_ops::write(&arc_swap, i, LARGE_DATA_SIZE);
                }
            });
            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// 基准测试 9: 单写入者不同读写比例 (1 Writer + 2 Readers)
// ============================================================================
fn bench_swmr_read_write_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("swmr_read_write_ratio");
    group.sample_size(30);

    const NUM_READERS: usize = 2;
    const RATIOS: &[(usize, usize, &str)] = &[
        (100, 1, "100:1"),
        (10, 1, "10:1"),
        (1, 1, "1:1"),
        (1, 10, "1:10"),
        (1, 100, "1:100"),
    ];

    for &(read_mult, write_mult, ratio_name) in RATIOS {
        group.bench_with_input(
            BenchmarkId::new("smr_swap", ratio_name),
            &(read_mult, write_mult),
            |b, &(read_mult, write_mult)| {
                b.iter_custom(|iters| {
                    let mut swap = smr_ops::new(DATA_SIZE);
                    let readers = smr_ops::create_readers(&swap, NUM_READERS);

                    let read_iters = iters * read_mult as u64;
                    let write_iters = iters * write_mult as u64;

                    let start = Instant::now();
                    thread::scope(|s| {
                        s.spawn(|| {
                            for i in 0..write_iters {
                                smr_ops::write(&mut swap, i, DATA_SIZE);
                            }
                        });

                        for reader in readers {
                            s.spawn(move || {
                                for _ in 0..read_iters {
                                    smr_ops::read(&reader);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("arc_swap", ratio_name),
            &(read_mult, write_mult),
            |b, &(read_mult, write_mult)| {
                b.iter_custom(|iters| {
                    let arc_swap = arc_ops::new(DATA_SIZE);

                    let read_iters = iters * read_mult as u64;
                    let write_iters = iters * write_mult as u64;

                    let start = Instant::now();
                    thread::scope(|s| {
                        let writer = arc_swap.clone();
                        s.spawn(move || {
                            for i in 0..write_iters {
                                arc_ops::write(&writer, i, DATA_SIZE);
                            }
                        });

                        for _ in 0..NUM_READERS {
                            let reader = arc_swap.clone();
                            s.spawn(move || {
                                for _ in 0..read_iters {
                                    arc_ops::read(&reader);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("rwlock", ratio_name),
            &(read_mult, write_mult),
            |b, &(read_mult, write_mult)| {
                b.iter_custom(|iters| {
                    let rwlock = rwlock_ops::new(DATA_SIZE);

                    let read_iters = iters * read_mult as u64;
                    let write_iters = iters * write_mult as u64;

                    let start = Instant::now();
                    thread::scope(|s| {
                        let writer = rwlock.clone();
                        s.spawn(move || {
                            for i in 0..write_iters {
                                rwlock_ops::write_inplace(&writer, i);
                            }
                        });

                        for _ in 0..NUM_READERS {
                            let reader = rwlock.clone();
                            s.spawn(move || {
                                for _ in 0..read_iters {
                                    rwlock_ops::read(&reader);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("mutex", ratio_name),
            &(read_mult, write_mult),
            |b, &(read_mult, write_mult)| {
                b.iter_custom(|iters| {
                    let mutex = mutex_ops::new(DATA_SIZE);

                    let read_iters = iters * read_mult as u64;
                    let write_iters = iters * write_mult as u64;

                    let start = Instant::now();
                    thread::scope(|s| {
                        let writer = mutex.clone();
                        s.spawn(move || {
                            for i in 0..write_iters {
                                mutex_ops::write_inplace(&writer, i);
                            }
                        });

                        for _ in 0..NUM_READERS {
                            let reader = mutex.clone();
                            s.spawn(move || {
                                for _ in 0..read_iters {
                                    mutex_ops::read(&reader);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_handle_ops,
    bench_creation,
    bench_drop,
    bench_single_thread_read,
    bench_single_thread_write,
    bench_multi_thread_read,
    bench_mixed_read_write,
    bench_batch_read,
    bench_multi_writer_multi_reader,
    bench_read_latency_with_held_guard,
    bench_read_under_memory_pressure,
    bench_swmr_read_write_ratio,
);

criterion_main!(benches);
