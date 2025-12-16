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
smr-swap = "0.8"
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

    // Writer stores a new value
    swap.store(1);

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

    // Writer stores a new value
    swap.store(vec![4, 5, 6]);

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

### Shared Reader Creation with `SmrReader`

If you need to distribute the ability to create readers to multiple threads (e.g., in a dynamic thread pool), use `SmrReader`. Unlike `LocalReader`, `SmrReader` is `Sync` and `Clone`.

```rust
use smr_swap::SmrSwap;
use std::thread;

let mut swap = SmrSwap::new(0);

// Create a shareable SmrReader factory
let reader_factory = swap.reader();

for i in 0..3 {
    // Clone the factory for each thread
    let factory = reader_factory.clone();
    
    thread::spawn(move || {
        // Use factory to create a LocalReader on the thread
        let local = factory.local();
        
        // ... use local reader ...
    });
}
```

## Core Concepts

### Type Hierarchy

| Type | Role | Key Methods |
|------|------|-------------|
| `SmrSwap<T>` | Main container, holds data and write capability | `new()`, `store()`, `get()`, `load()`, `local()`, `swap()` |
| `LocalReader<T>` | Thread-local read handle | `load()`, `map()`, `filter()`, `is_pinned()`, `version()` |
| `SmrReader<T>` | Cross-thread reader factory | `local()` |
| `ReadGuard<'a, T>` | RAII guard, protects data during read | `Deref`, `AsRef`, `version()` |

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
| `reader() -> SmrReader<T>` | Create a shareable reader factory |
| `store(new_value: T)` | Store a new value, old value will be safely reclaimed |
| `get() -> &T` | Get reference to current value (writer-only, no pin required) |
| `update(f: FnOnce(&T) -> T)` | Update value using a closure |
| `load() -> ReadGuard<T>` | Read current value using internal handle |
| `load_cloned() -> T` | Load and clone the current value (requires `T: Clone`) |
| `swap(new_value: T) -> T` | Swap value and return old value (requires `T: Clone`) |
| `update_and_fetch(f) -> ReadGuard<T>` | Apply closure to update and return guard to new value |
| `fetch_and_update(f) -> ReadGuard<T>` | Apply closure to update and return guard to old value |
| `version() -> usize` | Get current global version |
| `garbage_count() -> usize` | Get number of objects waiting for garbage collection |
| `previous() -> Option<&T>` | Get reference to previously stored value |
| `collect()` | Manually trigger garbage collection |

### `LocalReader<T>`

Thread-local read handle.

| Method | Description |
|--------|-------------|
| `load() -> ReadGuard<T>` | Read current value, returns RAII guard |
| `load_cloned() -> T` | Load and clone the current value (requires `T: Clone`) |
| `map<F, U>(f: F) -> U` | Apply function to value and return result |
| `filter<F>(f: F) -> Option<ReadGuard<T>>` | Conditionally return a guard |
| `is_pinned() -> bool` | Check if this reader is currently pinned |
| `version() -> usize` | Get current global version |
| `clone()` | Create a new `LocalReader` |

### `SmrReader<T>`

A sharable factory for `LocalReader`.

| Method | Description |
|--------|-------------|
| `local() -> LocalReader<T>` | Create a `LocalReader` for the current thread |
| `clone()` | Clone the factory (`Sync` + `Clone`) |

### `ReadGuard<'a, T>`

RAII guard, implements `Deref<Target = T>` and `AsRef<T>`, protects data from reclamation while guard is alive.

| Method | Description |
|--------|-------------|
| `version() -> usize` | Get the version this guard is pinned to |
| `cloned() -> T` | Clone the inner value and return it (requires `T: Clone`) |
| `into_inner() -> T` | Consume the guard and return cloned value (requires `T: Clone`) |
| `clone()` | Clone the guard (increments pin count) |

### Standard Trait Implementations

| Type | Traits |
|------|--------|
| `SmrSwap<T>` | `Default` (requires `T: Default`), `From<T>`, `Debug` (requires `T: Debug`) |
| `LocalReader<T>` | `Clone`, `Send`, `Debug` |
| `SmrReader<T>` | `Clone`, `Sync`, `Send`, `Debug` |
| `ReadGuard<'a, T>` | `Deref`, `AsRef`, `Clone`, `Debug` (requires `T: Debug`) |

## Performance

Benchmark results comparing SMR-Swap against `arc-swap` (Windows, Bench mode, Intel Core i9-13900KS).

### Benchmark Summary

| Scenario | SMR-Swap | ArcSwap | Improvement |
|----------|----------|---------|-------------|
| Single-Thread Read | 0.90 ns | 9.20 ns | **90% faster** |
| Single-Thread Write | 66.61 ns | 104.97 ns | **37% faster** |
| Multi-Thread Read (2) | 0.90 ns | 9.24 ns | **90% faster** |
| Multi-Thread Read (4) | 0.94 ns | 9.47 ns | **90% faster** |
| Multi-Thread Read (8) | 0.93 ns | 9.75 ns | **90% faster** |
| Mixed R/W (1W+2R) | 56.36 ns | 426.24 ns | **87% faster** |
| Mixed R/W (1W+4R) | 56.52 ns | 428.05 ns | **87% faster** |
| Mixed R/W (1W+8R) | 58.21 ns | 509.31 ns | **89% faster** |
| Batch Read | 1.63 ns | 10.19 ns | **84% faster** |
| Read with Held Guard | 54.57 ns | 891.63 ns | **94% faster** |
| Read Under Memory Pressure | 873.52 ns | 1.63 µs | **46% faster** |

### Single-Writer Read/Write Ratio (1 Writer + 2 Readers)

| R/W Ratio | SMR-Swap | Mutex | Notes |
|-----------|----------|-------|-------|
| 100:1 | 171.54 ns | 6.15 µs | SMR 97% faster than Mutex |
| 10:1 | 67.64 ns | 739.97 ns | SMR 91% faster than Mutex |
| 1:1 | 55.96 ns | 145.27 ns | SMR 61% faster than Mutex |
| 1:10 | 554.74 ns | 231.97 ns | Mutex 58% faster than SMR |
| 1:100 | 5.90 µs | 1.13 µs | Mutex 81% faster than SMR |

### Multi-Writer Multi-Reader (SMR-Swap wrapped in Mutex)

| Config | SMR-Swap | Mutex | ArcSwap | Notes |
|--------|----------|-------|---------|-------|
| 4W+4R | 559.87 ns | 503.32 ns | 1.85 µs | SMR 70% faster than ArcSwap |
| 4W+8R | 564.80 ns | 825.97 ns | 2.17 µs | SMR 74% faster than ArcSwap |
| 4W+16R | 563.60 ns | 1.20 µs | 2.81 µs | SMR 80% faster than ArcSwap |

### Analysis

- **Excellent Read Performance**: Sub-nanosecond latency (~0.9 ns) for both single and multi-thread reads, ~10x faster than ArcSwap
- **Linear Scaling**: Multi-thread read performance remains nearly constant regardless of thread count
- **Stable Mixed Workload**: Maintains ~57 ns latency under 1 writer + N readers scenarios
- **Optimal for Read-Heavy Workloads**: At 100:1 and 10:1 R/W ratios, SMR-Swap dominates, 91-97% faster than Mutex
- **Write-Heavy Workloads**: At higher write ratios (1:10, 1:100), Mutex has better write efficiency
- **Excellent Multi-Writer Performance**: Even with Mutex wrapping, SMR-Swap is 70-80% faster than ArcSwap in multi-writer multi-reader scenarios
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
