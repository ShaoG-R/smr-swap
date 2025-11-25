# SMR-Swap: Minimal-Locking Single-Writer Multiple-Reader Swap Container

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)

A high-performance, minimal-locking Rust library for safely sharing mutable data between a single writer and multiple readers using epoch-based memory reclamation.

[中文文档](README_CN.md) | [English](README.md)

## Features

- **Minimal-Locking**: Read operations are wait-free; write operations only require locks during garbage collection
- **High Performance**: Optimized for both read and write operations
- **Single-Writer Multiple-Reader Pattern**: Type-safe enforcement via `Swapper<T>` and `SwapReader<T>`
- **Memory Safe**: Uses epoch-based reclamation (via `swmr-epoch`) to prevent use-after-free
- **Zero-Copy Reads**: Readers get direct references to the current value via RAII guards
- **Thread Safe**: `SwapReader<T>` is `Send + Sync`, can be safely stored in structs and shared across threads

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
smr-swap = "0.6"
```

### Basic Usage

```rust
use smr_swap::SmrSwap;
use std::thread;

fn main() {
    // Create a new SMR container
    let mut swap = SmrSwap::new(vec![1, 2, 3]);

    // Get a shareable reader (Send + Sync)
    let reader = swap.reader().clone();

    let handle = thread::spawn(move || {
        // Create a thread-local handle to read
        let local = reader.handle();
        let guard = local.load();
        println!("Reader sees: {:?}", *guard);
    });

    // Writer updates the value
    swap.update(vec![4, 5, 6]);
    
    // Main thread reads directly
    println!("Main thread sees: {:?}", *swap.load());

    handle.join().unwrap();
}
```

### Storing Reader in Structs

`SwapReader<T>` is `Send + Sync`, so it can be safely stored in structs:

```rust
use smr_swap::{SmrSwap, SwapReader};
use std::thread;

struct MyService {
    reader: SwapReader<Config>,
}

impl MyService {
    fn get_config(&self) -> String {
        // Create a thread-local handle to read
        let handle = self.reader.handle();
        handle.map(|config| config.name.clone())
    }
}

struct Config {
    name: String,
}

fn main() {
    let (mut swapper, reader) = smr_swap::new_smr_pair(Config { name: "default".into() });
    let service = MyService { reader };
    
    // service can be safely shared across threads
    thread::scope(|s| {
        s.spawn(|| println!("{}", service.get_config()));
        s.spawn(|| println!("{}", service.get_config()));
    });
}
```

### Separating Writer and Reader

```rust
use std::thread;

fn main() {
    // Create Swapper and SwapReader pair directly
    let (mut swapper, reader) = smr_swap::new_smr_pair(42);
    
    // reader is Send + Sync, can be cloned and passed to multiple threads
    let reader_clone = reader.clone();
    
    thread::spawn(move || {
        let handle = reader_clone.handle();
        println!("Value: {}", *handle.load());
    });
    
    // Writer updates the value
    swapper.update(100);
}
```

## Core Concepts

### Type Hierarchy

| Type | Thread Safety | Role | Key Method |
|------|---------------|------|------------|
| `SwapReader<T>` | `Send + Sync` | Shareable reader, can be stored in structs | `handle()` |
| `ReaderHandle<T>` | `Send` only | Thread-local handle for actual reads | `load()`, `map()`, `filter()` |

```
SwapReader  ──handle()──►  ReaderHandle  ──load()──►  ReaderGuard
 (shared)                   (per-thread)              (RAII guard)
```

### Why This Design?

1. **`SwapReader<T>`** is `Send + Sync`
   - Can be safely stored in structs
   - Can be shared across threads via `&SwapReader`
   - Only exposes `handle()` method, cannot read directly

2. **`ReaderHandle<T>`** is `Send` but not `Sync`
   - Contains thread-local `LocalEpoch`
   - Provides actual read methods: `load()`, `map()`, `filter()`
   - Each thread needs its own `ReaderHandle`

## API Overview

### Global Functions
- `new_smr_pair<T>(initial: T) -> (Swapper<T>, SwapReader<T>)`: Create a new pair of Swapper and SwapReader directly.

### `SmrSwap<T>`
The main entry point. Holds writer, reader, and internal handle.
- `new(initial: T)`: Create a new container.
- `update(new_value: T)`: Update the value.
- `load()`: Read the current value (uses internal handle).
- `reader()`: Get `&SwapReader` (Send + Sync).
- `handle()`: Get `&ReaderHandle` (for direct reading).
- `swapper()`: Get a reference to the writer.

### `SwapReader<T>` (Send + Sync)
Shareable reader that can be stored in structs.
- `handle() -> ReaderHandle<T>`: Create a thread-local read handle.
- `clone()`: Clone the reader (implements `Clone`).

### `ReaderHandle<T>` (!Sync)
Thread-local read handle.
- `load() -> ReaderGuard<T>`: Returns a guard pointing to the current value.
- `map<F, U>(f: F) -> U`: Apply a function to the value and return the result.
- `filter<F>(f: F) -> Option<ReaderGuard<T>>`: Conditionally return a guard.
- `handle() -> ReaderHandle<T>`: Create a new handle.
- `reader() -> &SwapReader<T>`: Get a reference to the inner `SwapReader`.
- `clone()`: Clone the handle (implements `Clone`).

### `Swapper<T>`
The writer component.
- `update(new_value: T)`: Update the value.

## Performance Characteristics

Comprehensive benchmark results comparing SMR-Swap against `arc-swap` on modern hardware.

### Benchmark Summary Table

| Scenario | SMR-Swap | ArcSwap | Improvement | Notes |
|----------|----------|---------|-------------|-------|
| Single-Thread Read | 0.90 ns | 8.96 ns | **90% faster** | Pure read performance |
| Single-Thread Write | 87.90 ns | 130.23 ns | **32% faster** | Improved epoch management |
| Multi-Thread Read (2) | 0.90 ns | 9.37 ns | **90% faster** | No contention |
| Multi-Thread Read (4) | 0.91 ns | 9.33 ns | **90% faster** | Consistent scaling |
| Multi-Thread Read (8) | 0.93 ns | 9.63 ns | **90% faster** | Excellent scaling |
| Mixed R/W (2 readers) | 93.21 ns | 446.45 ns | **79% faster** | 1 writer + 2 readers |
| Mixed R/W (4 readers) | 92.89 ns | 451.09 ns | **79% faster** | 1 writer + 4 readers |
| Mixed R/W (8 readers) | 93.85 ns | 493.12 ns | **81% faster** | 1 writer + 8 readers |
| Batch Read | 1.62 ns | 9.91 ns | **84% faster** | Optimized batch reads |
| Multi-Writer (4 readers) | 664.63 ns | 1.92 µs | **65% faster** | 4 writers + 4 readers (Mutex) |
| Multi-Writer (8 readers) | 593.18 ns | 2.22 µs | **73% faster** | 4 writers + 8 readers (Mutex) |
| Multi-Writer (16 readers) | 652.44 ns | 2.93 µs | **78% faster** | 4 writers + 16 readers (Mutex) |
| Read with Held Guard | 89.91 ns | 908.69 ns | **90% faster** | Reader holds guard during write |
| Read Under Memory Pressure | 741.47 ns | 1.58 µs | **53% faster** | Aggressive GC collection |

### Detailed Performance Analysis

#### Single-Thread Read
```
smr-swap:  0.90 ns █
arc-swap:  8.96 ns ██████████
```
**Winner**: SMR-Swap (90% faster)
- Extremely fast read path with minimal overhead
- Direct pointer access without atomic operations
- Near-nanosecond latency

#### Single-Thread Write
```
smr-swap:  87.90 ns  ████████
arc-swap:  130.23 ns █████████████
```
**Winner**: SMR-Swap (32% faster)
- Improved epoch management efficiency
- Excellent write performance despite GC overhead

#### Multi-Thread Read Performance (Scaling)
```
Readers:   2         4         8
smr-swap:  0.90 ns   0.91 ns   0.93 ns
arc-swap:  9.37 ns   9.33 ns   9.63 ns
```
**Analysis**: 
- SMR-Swap maintains near-constant sub-nanosecond time regardless of thread count
- 90% faster than arc-swap across all thread counts
- Excellent scaling characteristics with virtually no contention

#### Mixed Read-Write (Most Realistic Scenario)
```
Readers:   2         4         8
smr-swap:  93 ns     93 ns     94 ns
arc-swap:  446 ns    451 ns    493 ns
```
**Winner**: SMR-Swap (79-81% faster)
- Consistent performance under load (93-94 ns across all thread counts)
- Minimal impact from concurrent writers
- ArcSwap shows increased latency with more readers (up to 493 ns)
- Aggressive GC ensures stable performance

#### Multi-Writer Multi-Reader Performance
```
Config:    4W+4R     4W+8R     4W+16R
smr-swap:  0.66 µs   0.59 µs   0.65 µs
mutex:     1.07 µs   1.44 µs   2.04 µs
arc-swap:  1.92 µs   2.22 µs   2.93 µs
```
**Winner**: SMR-Swap (65-78% faster than ArcSwap, 38-68% faster than Mutex)
- SMR-Swap (wrapped in Mutex) outperforms both pure Mutex and ArcSwap
- Pure Mutex performance degrades significantly as reader count increases (contention)
- ArcSwap is the slowest in this workload
- Demonstrates the efficiency of SMR-Swap's core mechanism even when wrapped in a lock

#### Read Under Memory Pressure
```
smr-swap:  741 ns   ████
arc-swap:  1580 ns  █████████
```
**Winner**: SMR-Swap (53% faster)
- **Improved**: Aggressive garbage collection prevents garbage accumulation
- Epoch-based reclamation is triggered immediately after each write
- Consistent performance even under memory pressure

#### Read Latency with Held Guard
```
smr-swap:  89.91 ns  ████
arc-swap:  908.69 ns ██████████████████
```
**Winner**: SMR-Swap (90% faster)
- Minimal overhead when readers hold guards
- Critical for applications requiring long-lived read access

## Design

### Type System Guarantees

- **`Swapper<T>`**: Not `Clone`
  - Guarantees single writer via type system
  - Can be wrapped in `Mutex<Swapper<T>>` for multiple writers (but introduces lock contention)

- **`SwapReader<T>`**: `Send + Sync`, implements `Clone`
  - Can be safely stored in structs
  - Can be shared across threads via `&SwapReader<T>`
  - Only exposes `handle()` method, ensuring each thread has its own `LocalEpoch`

- **`ReaderHandle<T>`**: `Send` but not `Sync`
  - Contains thread-local `LocalEpoch` for epoch protection
  - Provides actual read methods
  - Should not be shared across threads

### Memory Management

SMR-Swap uses a custom `swmr-epoch` library for memory reclamation, optimized for single-writer multiple-reader scenarios.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
