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
- **Concurrent**: Safe to use across multiple threads with `Send + Sync` bounds

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
smr-swap = "0.5"
```

### Basic Usage

```rust
use smr_swap::SmrSwap;
use std::thread;

fn main() {
    // Create a new SMR container with initial value
    let mut swap = SmrSwap::new(vec![1, 2, 3]);

    // Create a reader for a new thread (must use fork() for new threads)
    let reader = swap.reader().fork();

    let handle = thread::spawn(move || {
        // Read the value (lock-free)
        let guard = reader.load();
        println!("Reader sees: {:?}", *guard);
    });

    // Writer updates the value
    swap.update(vec![4, 5, 6]);
    
    // Main thread can also read
    println!("Main thread sees: {:?}", *swap.load());

    handle.join().unwrap();
}
```

### Using with Arc (for shared ownership)

While SMR-Swap works with any type `T`, you can wrap values in `Arc` for shared ownership:

```rust
use smr_swap::SmrSwap;
use std::sync::Arc;

fn main() {
    let mut swap = SmrSwap::new(Arc::new(vec![1, 2, 3]));
    
    // Update with a new Arc
    swap.update(Arc::new(vec![4, 5, 6]));
    
    // Or use swap() to get the old value back
    let old = swap.swap(Arc::new(vec![7, 8, 9]));
    println!("Old value: {:?}", old);
}
```

### Separating Writer and Reader

You can split the container into independent components:

```rust
use smr_swap::SmrSwap;

fn main() {
    // Option 1: Create SmrSwap first, then split
    let swap = SmrSwap::new(42);
    let (mut swapper, reader) = swap.into_components();
    
    // Option 2: Create components directly
    let (mut swapper, reader) = smr_swap::new_smr_pair(42);
    
    // Pass `swapper` to writer thread
    // Pass `reader` to reader threads (use reader.fork() for each thread)
}
```

## API Overview

### Global Functions
- `new_smr_pair<T>(initial: T) -> (Swapper<T>, SwapReader<T>)`: Create a new pair of Swapper and SwapReader directly.

### `SmrSwap<T>`
The main entry point. Holds both the writer and a reader.
- `new(initial: T)`: Create a new container.
- `update(new_value: T)`: Update the value.
- `load()`: Read the current value.
- `reader()`: Get a reference to the reader.
- `swapper()`: Get a reference to the writer.
- `into_components()`: Split into `Swapper` and `SwapReader`.

### `SwapReader<T>`
The reader component.
- `fork()`: Create a new reader for a different thread. **Important**: `SwapReader` is not `Clone` to enforce explicit forking for thread-local epoch registration.
- `load()`: Returns a `ReaderGuard` pointing to the current value.
- `map(|v| ...)`: Apply a function to the value and return the result.
- `filter(|v| ...)`: Conditionally return a guard.

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

- **`Swapper<T>`**: Not `Clone` (enforced via `Arc` single ownership)
  - Guarantees single writer via type system
  - Can be shared across threads if wrapped in `Arc` (but breaks single-writer guarantee)

- **`SwapReader<T>`**: Not `Clone` (Use `fork()` instead)
  - Use `fork()` to create a new reader for another thread.
  - Each reader independently sees the latest value.
  - Internally manages `LocalEpoch` registration.

### Memory Management

SMR-Swap uses a custom `swmr-epoch` library for memory reclamation, optimized for single-writer multiple-reader scenarios.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
