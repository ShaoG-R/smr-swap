use arc_swap::ArcSwap;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use smr_swap::SmrSwap;
use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// 基准测试 1: 单线程读取性能
// ============================================================================
fn bench_single_thread_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_read");
    group.sample_size(100);

    // SMR-Swap 单线程读取
    group.bench_function("smr_swap", |b| {
        let swap = SmrSwap::new(vec![1; 1000]);
        let reader = swap.reader();
        b.iter(|| {
            let guard = reader.load();
            black_box(&*guard);
        });
    });

    // ArcSwap 单线程读取
    group.bench_function("arc_swap", |b| {
        let arc_swap = ArcSwap::new(Arc::new(vec![1; 1000]));
        b.iter(|| {
            let guard = arc_swap.load();
            black_box(&*guard);
        });
    });

    group.finish();
}

// ============================================================================
// 基准测试 2: 单线程写入性能
// ============================================================================
fn bench_single_thread_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_write");
    group.sample_size(100);

    // SMR-Swap 单线程写入
    group.bench_function("smr_swap", |b| {
        let mut swap = SmrSwap::new(vec![1; 1000]);
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            swap.update(vec![counter; 1000]);
        });
    });

    // ArcSwap 单线程写入
    group.bench_function("arc_swap", |b| {
        let arc_swap = ArcSwap::new(Arc::new(vec![1; 1000]));
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            arc_swap.store(Arc::new(vec![counter; 1000]));
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

    for num_readers in [2, 4, 8].iter() {
        // SMR-Swap 多线程读取
        group.bench_with_input(
            BenchmarkId::new("smr_swap", num_readers),
            num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let swap = SmrSwap::new(vec![1; 1000]);
                    let reader = swap.reader();

                    let mut readers = Vec::with_capacity(num_readers);
                    for _ in 0..num_readers {
                        readers.push(reader.fork());
                    }

                    let start = std::time::Instant::now();
                    thread::scope(|s| {
                        for reader in readers {
                            s.spawn(move || {
                                for _ in 0..iters {
                                    let guard = reader.load();
                                    black_box(&*guard);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        // ArcSwap 多线程读取
        group.bench_with_input(
            BenchmarkId::new("arc_swap", num_readers),
            num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let arc_swap = Arc::new(ArcSwap::new(Arc::new(vec![1; 1000])));

                    let start = std::time::Instant::now();
                    thread::scope(|s| {
                        for _ in 0..num_readers {
                            let arc_swap_clone = arc_swap.clone();
                            s.spawn(move || {
                                for _ in 0..iters {
                                    let guard = arc_swap_clone.load();
                                    black_box(&*guard);
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

    for num_readers in [2, 4, 8].iter() {
        // SMR-Swap 混合读写
        group.bench_with_input(
            BenchmarkId::new("smr_swap", num_readers),
            num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let (mut swapper, reader) = smr_swap::new_smr_pair(vec![1; 1000]);

                    let mut readers = Vec::with_capacity(num_readers);
                    for _ in 0..num_readers {
                        readers.push(reader.fork());
                    }

                    let start = std::time::Instant::now();
                    thread::scope(|s| {
                        // 写入者线程
                        s.spawn(move || {
                            for i in 0..iters {
                                swapper.update(vec![i as u32; 1000]);
                            }
                        });

                        // 读取者线程
                        for reader in readers {
                            s.spawn(move || {
                                for _ in 0..iters {
                                    let guard = reader.load();
                                    black_box(&*guard);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        // ArcSwap 混合读写
        group.bench_with_input(
            BenchmarkId::new("arc_swap", num_readers),
            num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let arc_swap = Arc::new(ArcSwap::new(Arc::new(vec![1; 1000])));

                    let start = std::time::Instant::now();
                    thread::scope(|s| {
                        // 写入者线程
                        let arc_swap_clone = arc_swap.clone();
                        s.spawn(move || {
                            for i in 0..iters {
                                arc_swap_clone.store(Arc::new(vec![i as u32; 1000]));
                            }
                        });

                        // 读取者线程
                        for _ in 0..num_readers {
                            let arc_swap_clone = arc_swap.clone();
                            s.spawn(move || {
                                for _ in 0..iters {
                                    let guard = arc_swap_clone.load();
                                    black_box(&*guard);
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

    let num_writers = 4;

    for num_readers in [4, 8, 16].iter() {
        // SMR-Swap 多写多读 (使用 Mutex)
        group.bench_with_input(
            BenchmarkId::new("smr_swap", num_readers),
            num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let (swapper, reader) = smr_swap::new_smr_pair(vec![1; 1000]);
                    let swapper = Arc::new(Mutex::new(swapper));

                    let mut readers = Vec::with_capacity(num_readers);
                    for _ in 0..num_readers {
                        readers.push(reader.fork());
                    }

                    let start = std::time::Instant::now();
                    thread::scope(|s| {
                        // 写入者线程
                        for _ in 0..num_writers {
                            let swapper_clone = swapper.clone();
                            s.spawn(move || {
                                for i in 0..iters {
                                    swapper_clone.lock().unwrap().update(vec![i as u32; 1000]);
                                }
                            });
                        }

                        // 读取者线程
                        for reader in readers {
                            s.spawn(move || {
                                for _ in 0..iters {
                                    let guard = reader.load();
                                    black_box(&*guard);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        // Mutex 多写多读
        group.bench_with_input(
            BenchmarkId::new("mutex", num_readers),
            num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let mutex = Arc::new(Mutex::new(vec![1; 1000]));

                    let start = std::time::Instant::now();
                    thread::scope(|s| {
                        // 写入者线程
                        for _ in 0..num_writers {
                            let mutex_clone = mutex.clone();
                            s.spawn(move || {
                                for i in 0..iters {
                                    *mutex_clone.lock().unwrap() = vec![i as u32; 1000];
                                }
                            });
                        }

                        // 读取者线程
                        for _ in 0..num_readers {
                            let mutex_clone = mutex.clone();
                            s.spawn(move || {
                                for _ in 0..iters {
                                    let guard = mutex_clone.lock().unwrap();
                                    black_box(&*guard);
                                }
                            });
                        }
                    });
                    start.elapsed()
                });
            },
        );

        // ArcSwap 多写多读
        group.bench_with_input(
            BenchmarkId::new("arc_swap", num_readers),
            num_readers,
            |b, &num_readers| {
                b.iter_custom(|iters| {
                    let arc_swap = Arc::new(ArcSwap::new(Arc::new(vec![1; 1000])));

                    let start = std::time::Instant::now();
                    thread::scope(|s| {
                        // 写入者线程
                        for _ in 0..num_writers {
                            let arc_swap_clone = arc_swap.clone();
                            s.spawn(move || {
                                for i in 0..iters {
                                    arc_swap_clone.store(Arc::new(vec![i as u32; 1000]));
                                }
                            });
                        }

                        // 读取者线程
                        for _ in 0..num_readers {
                            let arc_swap_clone = arc_swap.clone();
                            s.spawn(move || {
                                for _ in 0..iters {
                                    let guard = arc_swap_clone.load();
                                    black_box(&*guard);
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

    // SMR-Swap: 读取者持有守卫，写入者写入
    group.bench_function("smr_swap", |b| {
        let (mut swapper, reader) = smr_swap::new_smr_pair(vec![1; 1000]);

        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            thread::scope(|s| {
                let reader_for_thread = reader.fork();
                s.spawn(move || {
                    // 读取者持有守卫
                    let _guard = reader_for_thread.load();
                    // 模拟长时间持有
                    std::thread::sleep(std::time::Duration::from_micros(10));
                    drop(_guard);
                });

                // 写入者尝试写入
                for i in 0..iters {
                    swapper.update(vec![i as u32; 1000]);
                }
            });
            start.elapsed()
        });
    });

    // ArcSwap: 读取者持有守卫，写入者写入
    group.bench_function("arc_swap", |b| {
        let arc_swap = Arc::new(ArcSwap::new(Arc::new(vec![1; 1000])));

        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            thread::scope(|s| {
                let arc_swap_clone = arc_swap.clone();
                s.spawn(move || {
                    // 读取者持有守卫
                    let _guard = arc_swap_clone.load();
                    // 模拟长时间持有
                    std::thread::sleep(std::time::Duration::from_micros(10));
                });

                // 写入者尝试写入
                for i in 0..iters {
                    arc_swap.store(Arc::new(vec![i as u32; 1000]));
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

    // SMR-Swap: 批量读取（单个 pin 内多次读取）
    group.bench_function("smr_swap_batch", |b| {
        let swap = SmrSwap::new(vec![1; 1000]);
        let reader = swap.reader();

        b.iter_custom(|iters| {
            let mut readers = Vec::with_capacity(4);
            for _ in 0..4 {
                readers.push(reader.fork());
            }

            let start = std::time::Instant::now();
            thread::scope(|s| {
                for reader in readers {
                    s.spawn(move || {
                        for _ in 0..iters {
                            // 批量读取：一个 pin 内多次读取
                            let guard = reader.load();
                            for _ in 0..10 {
                                black_box(&*guard);
                            }
                        }
                    });
                }
            });
            start.elapsed()
        });
    });

    // ArcSwap: 批量读取
    group.bench_function("arc_swap_batch", |b| {
        let arc_swap = Arc::new(ArcSwap::new(Arc::new(vec![1; 1000])));

        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            thread::scope(|s| {
                for _ in 0..4 {
                    let arc_swap_clone = arc_swap.clone();
                    s.spawn(move || {
                        for _ in 0..iters {
                            // 批量读取：一个 load 内多次使用
                            let guard = arc_swap_clone.load();
                            for _ in 0..10 {
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

    // SMR-Swap: 频繁写入导致内存压力
    group.bench_function("smr_swap", |b| {
        let (mut swapper, reader) = smr_swap::new_smr_pair(vec![0; 10000]);

        // 预先进行大量写入以积累垃圾
        for i in 0..1000 {
            swapper.update(vec![i; 10000]);
        }

        b.iter_custom(|iters| {
            let reader_for_thread = reader.fork();

            let start = std::time::Instant::now();
            thread::scope(|s| {
                s.spawn(move || {
                    for _ in 0..iters {
                        let guard = reader_for_thread.load();
                        black_box(&*guard);
                    }
                });

                // 同时进行写入
                for i in 0..iters {
                    swapper.update(vec![i as u32; 10000]);
                }
            });
            start.elapsed()
        });
    });

    // ArcSwap: 频繁写入导致内存压力
    group.bench_function("arc_swap", |b| {
        let arc_swap = Arc::new(ArcSwap::new(Arc::new(vec![0; 10000])));

        // 预先进行大量写入
        for i in 0..1000 {
            arc_swap.store(Arc::new(vec![i; 10000]));
        }

        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            thread::scope(|s| {
                let arc_swap_clone = arc_swap.clone();
                s.spawn(move || {
                    for _ in 0..iters {
                        let guard = arc_swap_clone.load();
                        black_box(&*guard);
                    }
                });

                // 同时进行写入
                for i in 0..iters {
                    arc_swap.store(Arc::new(vec![i as u32; 10000]));
                }
            });
            start.elapsed()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_thread_read,
    bench_single_thread_write,
    bench_multi_thread_read,
    bench_mixed_read_write,
    bench_batch_read,
    bench_multi_writer_multi_reader,
    bench_read_latency_with_held_guard,
    bench_read_under_memory_pressure,
);

criterion_main!(benches);
