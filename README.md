# SMR-Swap: Version-Based Single-Writer Multiple-Reader Swap Container

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)
![no_std compatible](https://img.shields.io/badge/no_std-compatible-success.svg)

A high-performance Rust library for safely sharing mutable data between a single writer and multiple readers using version-based memory reclamation.

[中文文档](README_CN.md) | [English](README.md)

## Features

- **Minimal-Locking**: Read operations are wait-free; write operations only require synchronization during garbage collection
- **High Performance**: Optimized for both read and write operations
- **Simple API**: Only three core types: `SmrSwap`, `LocalReader`, `ReadGuard`
- **Memory Safe**: Uses version-based reclamation (via `swmr-cell`) to prevent use-after-free
- **Zero-Copy Reads**: Readers get direct references to the current value via RAII guards
- **no_std Compatible**: Supports `no_std` environments (requires `alloc`)

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
smr-swap = "0.9"
```

### no_std Usage

Use with `default-features = false` and enable `spin` feature (if you need the default spinlock-based mutex implementation in `swmr-cell`):

```toml
[dependencies]
smr-swap = { version = "0.9", default-features = false, features = ["spin"] }
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
| `share() -> SmrReader<T>` | Create a shareable reader factory |
| `into_swmr() -> SmrReader<T>` | Convert to a shareable reader factory |

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

Since smr-swap v0.9.0, the default strategy is **Write-Preferred**. The previous **Read-Preferred** strategy is available via the `read-preferred` feature.

Benchmark results comparing SMR-Swap against `arc-swap` (Windows, Bench mode, Intel Core i9-13900KS).

### Benchmark Summary

| Scenario | SMR-Swap (Write-Pref) | SMR-Swap (Read-Pref) | ArcSwap |
|----------|-----------------------|----------------------|---------|
| Single-Thread Read | **4.49 ns** | **0.90 ns** | 9.19 ns |
| Single-Thread Write | **54.84 ns** | 89.81 ns | 104.05 ns |
| Multi-Thread Read (2) | **4.81 ns** | **0.90 ns** | 9.23 ns |
| Multi-Thread Read (4) | **4.98 ns** | **0.92 ns** | 9.33 ns |
| Multi-Thread Read (8) | **5.10 ns** | **0.94 ns** | 9.42 ns |
| Mixed R/W (1W+2R) | **66.10 ns** | 86.01 ns | 428.14 ns |
| Mixed R/W (1W+4R) | **71.87 ns** | 86.03 ns | 429.16 ns |
| Mixed R/W (1W+8R) | **76.63 ns** | 86.75 ns | 470.58 ns |
| Batch Read | **5.52 ns** | **1.62 ns** | 9.61 ns |
| Read with Held Guard | **55.43 ns** | 84.96 ns | 891.63 ns |
| Read Under Memory Pressure | **816.01 ns** | 781.29 ns | 1.71 µs |

### Single-Writer Read/Write Ratio (1 Writer + 2 Readers)

| R/W Ratio | SMR-Swap (Write-Pref) | SMR-Swap (Read-Pref) | RwLock | Mutex |
|-----------|-----------------------|----------------------|--------|-------|
| 100:1 | 572.00 ns | 251.49 ns | 5.60 µs | 6.08 µs |
| 10:1 | 144.41 ns | 101.09 ns | 634.45 ns | 713.28 ns |
| 1:1 | 66.59 ns | 87.26 ns | 129.66 ns | 127.39 ns |
| 1:10 | 567.56 ns | 857.83 ns | 238.46 ns | 218.87 ns |
| 1:100 | 5.55 µs | 8.52 µs | 1.05 µs | 970.69 ns |

### Multi-Writer Multi-Reader (SMR-Swap wrapped in Mutex)

| Config | SMR-Swap (Write-Pref) | SMR-Swap (Read-Pref) | Mutex | ArcSwap |
|--------|-----------------------|----------------------|-------|---------|
| 4W+4R | 506.46 ns | 2.03 µs | 497.52 ns | 1.93 µs |
| 4W+8R | 516.46 ns | 2.10 µs | 818.02 ns | 2.24 µs |
| 4W+16R | 516.20 ns | 2.04 µs | 1.26 µs | 2.93 µs |

### Operational Overhead

| Operation | SMR-Swap (Write-Pref) | SMR-Swap (Read-Pref) | ArcSwap | Mutex |
|-----------|-----------------------|----------------------|---------|-------|
| Creation | ~152 ns | ~159 ns | ~131 ns | ~49 ns |
| Drop | ~81 ns | ~78 ns | ~108 ns | ~41 ns |
| Handle Clone | ~57 ns | ~57 ns | ~9 ns | ~9 ns |
| Local Check | ~0.18 ns | ~0.18 ns | N/A | N/A |

### Analysis

- **Write-Preferred (Default)**:
  - **Balanced Performance**: Better trade-off between read and write performance.
  - **Mechanism**: Uses **Symmetric Memory Barriers**, distributing synchronization overhead across both reads and writes.
  - **Fast Multi-Threaded Writes**: Significantly faster in mixed read/write and multi-writer scenarios (Avg ~500ns in 4W+4R vs ~2µs in Read-Preferred).
  - **Good Read Performance**: Read latency (~4.5ns) is higher than Read-Preferred (~0.9ns) but still ~2x faster than ArcSwap (~9.2ns).

- **Read-Preferred (Feature)**:
  - **Ultimate Read Performance**: Sub-nanosecond read latency (~0.9ns), ideal for read-heavy (>99% reads) workloads.
  - **Mechanism**: Uses **Asymmetric Memory Barriers** (Heavy-Write/Light-Read), shifting synchronization overhead almost entirely to the writer.
  - **Slower Writes**: Write operations are more expensive due to reader checking overhead and heavy barriers.
  - **Poor Multi-Writer Scaling**: Heavy contention in multi-writer scenarios.

- **Comparison**:
  - **Reads**: Read-Preferred > Write-Preferred > ArcSwap > Mutex
  - **Writes**: Mutex > Write-Preferred > Read-Preferred > ArcSwap
  - **Mixed**: Write-Preferred > Read-Preferred > ArcSwap

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
