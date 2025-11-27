# SMR-Swap: Version-Based Single-Writer Multiple-Reader Swap Container

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)

A high-performance Rust library for safely sharing mutable data between a single writer and multiple readers using version-based memory reclamation.

[中文文档](README_CN.md) | [English](README.md)

## Features

- **Minimal-Locking**: Read operations are wait-free; write operations only require synchronization during garbage collection
- **High Performance**: Optimized for both read and write operations
- **Simple API**: Only three core types: `SmrSwap`, `LocalReader`, `ReadGuard`
- **Memory Safe**: Uses version-based reclamation (via `swmr-cell`) to prevent use-after-free
- **Zero-Copy Reads**: Readers get direct references to the current value via RAII guards

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
smr-swap = "0.7"
```

### Basic Usage

```rust
use smr_swap::SmrSwap;
use std::thread;

fn main() {
    // Create a new SMR container
    let mut swap = SmrSwap::new(0);

    // Get a thread-local reader
    let local = swap.local();

    // Writer updates the value
    swap.update(1);

    // Read in another thread
    let local2 = swap.local();
    let handle = thread::spawn(move || {
        let guard = local2.load();
        assert_eq!(*guard, 1);
    });

    handle.join().unwrap();
}
```

### Multi-Thread Reading

```rust
use smr_swap::SmrSwap;
use std::thread;

fn main() {
    let mut swap = SmrSwap::new(vec![1, 2, 3]);
    
    // Create independent LocalReader for each thread
    let readers: Vec<_> = (0..4).map(|_| swap.local()).collect();
    
    let handles: Vec<_> = readers
        .into_iter()
        .enumerate()
        .map(|(i, local)| {
            thread::spawn(move || {
                let guard = local.load();
                println!("Thread {} sees: {:?}", i, *guard);
            })
        })
        .collect();

    // Writer updates the value
    swap.update(vec![4, 5, 6]);

    for handle in handles {
        handle.join().unwrap();
    }
}
```

### Using Closures

```rust
use smr_swap::SmrSwap;

fn main() {
    let mut swap = SmrSwap::new(vec![1, 2, 3]);
    let local = swap.local();

    // Use map to transform value
    let sum: i32 = local.map(|v| v.iter().sum());
    println!("Sum: {}", sum);

    // Use filter to conditionally get guard
    if let Some(guard) = local.filter(|v| v.len() > 2) {
        println!("Vector has more than 2 elements: {:?}", *guard);
    }

    // Use update_and_fetch to update and get new value
    let new_guard = swap.update_and_fetch(|v| {
        let mut new_vec = v.clone();
        new_vec.push(4);
        new_vec
    });
    println!("New value: {:?}", *new_guard);
}
```

## Core Concepts

### Type Hierarchy

| Type | Role | Key Methods |
|------|------|-------------|
| `SmrSwap<T>` | Main container, holds data and write capability | `new()`, `update()`, `load()`, `local()`, `swap()` |
| `LocalReader<T>` | Thread-local read handle | `load()`, `map()`, `filter()` |
| `ReadGuard<'a, T>` | RAII guard, protects data during read | `Deref` |

```
SmrSwap  ──local()──►  LocalReader  ──load()──►  ReadGuard
 (main)                (per-thread)              (RAII guard)
```

### Why LocalReader?

`LocalReader` is a thread-local read handle:
- Each thread should create its own `LocalReader` and reuse it
- `LocalReader` is `Send` but not `Sync`, should not be shared across threads
- Contains thread-local version tracking for safe memory reclamation

## API Overview

### `SmrSwap<T>`

Main entry point, holds data and write capability.

| Method | Description |
|--------|-------------|
| `new(initial: T)` | Create a new container |
| `local() -> LocalReader<T>` | Create a thread-local read handle |
| `update(new_value: T)` | Update the value, old value will be safely reclaimed |
| `load() -> ReadGuard<T>` | Read current value using internal handle |
| `swap(new_value: T) -> T` | Swap value and return old value (requires `T: Clone`) |
| `update_and_fetch(f) -> ReadGuard<T>` | Apply closure to update and return guard to new value |
| `collect()` | Manually trigger garbage collection |

### `LocalReader<T>`

Thread-local read handle.

| Method | Description |
|--------|-------------|
| `load() -> ReadGuard<T>` | Read current value, returns RAII guard |
| `map<F, U>(f: F) -> U` | Apply function to value and return result |
| `filter<F>(f: F) -> Option<ReadGuard<T>>` | Conditionally return a guard |
| `clone()` | Create a new `LocalReader` |

### `ReadGuard<'a, T>`

RAII guard, implements `Deref<Target = T>`, protects data from reclamation while guard is alive.

## Performance

Benchmark results comparing SMR-Swap against `arc-swap` (Windows, Bench mode, Intel Core i9-13900KS).

### Benchmark Summary

| Scenario | SMR-Swap | ArcSwap | Improvement |
|----------|----------|---------|-------------|
| Single-Thread Read | 0.91 ns | 9.15 ns | **90% faster** |
| Single-Thread Write | 108.81 ns | 131.43 ns | **17% faster** |
| Multi-Thread Read (2) | 0.90 ns | 9.36 ns | **90% faster** |
| Multi-Thread Read (4) | 0.90 ns | 9.30 ns | **90% faster** |
| Multi-Thread Read (8) | 0.96 ns | 9.72 ns | **90% faster** |
| Mixed R/W (1W+2R) | 108.16 ns | 451.72 ns | **76% faster** |
| Mixed R/W (1W+4R) | 110.58 ns | 453.31 ns | **76% faster** |
| Mixed R/W (1W+8R) | 104.38 ns | 528.70 ns | **80% faster** |
| Batch Read | 1.64 ns | 9.92 ns | **83% faster** |
| Read with Held Guard | 102.25 ns | 964.82 ns | **89% faster** |
| Read Under Memory Pressure | 825.30 ns | 1.70 µs | **51% faster** |

### Multi-Writer Multi-Reader (SMR-Swap wrapped in Mutex)

| Config | SMR-Swap | Mutex | ArcSwap | Notes |
|--------|----------|-------|---------|-------|
| 4W+4R | 1.78 µs | 1.10 µs | 1.88 µs | SMR 5% faster than ArcSwap |
| 4W+8R | 1.73 µs | 1.33 µs | 2.22 µs | SMR 22% faster than ArcSwap |
| 4W+16R | 1.71 µs | 1.76 µs | 3.07 µs | SMR 44% faster than ArcSwap |

### Analysis

- **Excellent Read Performance**: Sub-nanosecond latency (~0.9 ns) for both single and multi-thread reads, ~10x faster than ArcSwap
- **Linear Scaling**: Multi-thread read performance remains nearly constant regardless of thread count
- **Stable Mixed Workload**: Maintains ~105 ns latency under 1 writer + N readers scenarios
- **Multi-Writer Scenarios**: Even with Mutex wrapping, outperforms ArcSwap with high reader counts
- **Good Under Memory Pressure**: Aggressive GC ensures stable performance under memory pressure

## Design

### Single-Writer Multiple-Reader Pattern

- **`SmrSwap<T>`** holds write capability, not `Clone`
  - Single writer guaranteed by ownership system
  - Wrap in `Mutex<SmrSwap<T>>` for multiple writers if needed

- **`LocalReader<T>`** is `Send` but not `Sync`
  - Contains thread-local version information
  - Each thread should have its own `LocalReader`

### Memory Management

SMR-Swap uses `swmr-cell` for version-based memory reclamation:
- Old values are automatically queued for reclamation on write
- Memory is reclaimed when no readers reference old values
- Use `collect()` to manually trigger reclamation

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
