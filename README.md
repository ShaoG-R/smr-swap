# SMR-Swap: Lock-Free Single-Writer Multiple-Reader Swap Container

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)

A high-performance, lock-free Rust library for safely sharing mutable data between a single writer and multiple readers using epoch-based memory reclamation.

## Features

- **Lock-Free**: No mutexes or locks required for reads or writes
- **High Performance**: Optimized for both read and write operations
- **Single-Writer Multiple-Reader Pattern**: Type-safe enforcement via `Swapper<T>` and `SwapReader<T>`
- **Memory Safe**: Uses epoch-based reclamation (via `crossbeam-epoch`) to prevent use-after-free
- **Zero-Copy Reads**: Readers get direct references to the current value
- **Concurrent**: Safe to use across multiple threads with `Send + Sync` bounds

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
smr-swap = "0.1"
```

### Basic Usage

```rust
use smr_swap;

fn main() {
    // Create a new SMR container with initial value
    let (mut writer, reader) = smr_swap::new(vec![1, 2, 3]);

    // Reader can clone and share across threads
    let reader_clone = reader.clone();

    // Writer updates the value
    writer.update(vec![4, 5, 6]);

    // Reader sees the new value
    let guard = reader.read().unwrap();
    println!("{:?}", *guard); // [4, 5, 6]
}
```

### Using with Arc (for shared ownership)

While SMR-Swap works with any type `T`, you can wrap values in `Arc` for shared ownership:

```rust
use smr_swap;
use std::sync::Arc;

fn main() {
    let (mut writer, reader) = smr_swap::new(Arc::new(vec![1, 2, 3]));
    
    writer.update(Arc::new(vec![4, 5, 6]));
    
    let guard = reader.read().unwrap();
    println!("{:?}", *guard); // Arc<Vec<i32>>
}
```

**Note**: SMR-Swap itself does not require `Arc`. Use `Arc` only if you need shared ownership of the inner value.

## API Overview

### Creating a Container

```rust
let (swapper, reader) = smr_swap::new(initial_value);
```

Returns a tuple of:
- `Swapper<T>`: The writer (not `Clone`, enforces single writer)
- `SwapReader<T>`: The reader (can be cloned for multiple readers)

### Writer Operations

#### `update(new_value: T)`
Atomically replaces the current value.

```rust
writer.update(new_value);
```

#### `swap(new_value: T) -> Option<T>`
Atomically replaces the current value and returns the old value.

```rust
if let Some(old) = writer.swap(new_value) {
    println!("Old value: {:?}", old);
}
```

#### `read() -> Option<SwapGuard<T>>`
Allows the writer to read the current value.

```rust
if let Some(guard) = writer.read() {
    println!("Current: {:?}", *guard);
}
```

#### `update_and_fetch<F>(f: F) -> Option<SwapGuard<T>>`
Updates based on the current value and returns the new value.

```rust
let new_guard = writer.update_and_fetch(|old| {
    let mut v = old.clone();
    v.push(999);
    v
})?;
```

### Reader Operations

#### `read() -> Option<SwapGuard<T>>`
Reads the current value, returning a guard that ensures the value won't be reclaimed while held.

```rust
if let Some(guard) = reader.read() {
    println!("Value: {:?}", *guard);
}
```

#### `map<F, R>(f: F) -> Option<R>`
Applies a closure to the current value without holding a guard.

```rust
let doubled = reader.map(|v| v * 2);
```

#### `filter<F>(f: F) -> Option<SwapGuard<T>>`
Returns a guard only if the predicate is true.

```rust
if let Some(guard) = reader.filter(|v| v > &10) {
    println!("Value > 10: {:?}", *guard);
}
```

#### `try_clone_value() -> Option<T>` (requires `T: Clone`)
Clones the current value.

```rust
if let Some(cloned) = reader.try_clone_value() {
    println!("Cloned: {:?}", cloned);
}
```

## Performance Characteristics

Comprehensive benchmark results comparing SMR-Swap against `arc-swap` on modern hardware.

### Benchmark Summary Table

| Scenario | SMR-Swap | ArcSwap | Improvement | Notes |
|----------|----------|---------|-------------|-------|
| Single-Thread Read | 5.47 ns | 8.98 ns | **39% faster** | Pure read performance |
| Single-Thread Write | 148.51 ns | 129.37 ns | 15% slower | Epoch management overhead |
| Multi-Thread Read (2) | 6.90 ns | 9.02 ns | **23% faster** | No contention |
| Multi-Thread Read (4) | 9.13 ns | 9.26 ns | **1% faster** | Consistent scaling |
| Multi-Thread Read (8) | 14.36 ns | 9.67 ns | 48% slower | Epoch coordination cost |
| Mixed R/W (2 readers) | 151.23 ns | 456.36 ns | **67% faster** | 1 writer + 2 readers |
| Mixed R/W (4 readers) | 152.27 ns | 452.69 ns | **66% faster** | 1 writer + 4 readers |
| Mixed R/W (8 readers) | 156.02 ns | 493.48 ns | **68% faster** | 1 writer + 8 readers |
| Batch Read (10x) | 9.89 ns | 10.14 ns | **2% faster** | Single pin, multiple reads |
| Read with Held Guard | 140.31 ns | 481.23 ns | **71% faster** | Reader holds guard during write |

### Detailed Performance Analysis

#### Single-Thread Read
```
smr-swap:  5.47 ns ████
arc-swap:  8.98 ns ██████████
```
**Winner**: SMR-Swap (39% faster)
- Direct epoch pin provides minimal overhead
- No lock contention

#### Single-Thread Write
```
smr-swap:  148.51 ns ██████████████████
arc-swap:  129.37 ns ████████████████
```
**Winner**: ArcSwap (15% faster)
- SMR-Swap has epoch management overhead
- ArcSwap uses simpler atomic operations

#### Multi-Thread Read Performance (Scaling)
```
Readers:   2         4         8
smr-swap:  6.90 ns   9.13 ns   14.36 ns
arc-swap:  9.02 ns   9.26 ns   9.67 ns
```
**Analysis**: 
- SMR-Swap maintains near-linear scaling up to 4 readers
- Epoch coordination introduces overhead at 8 readers
- ArcSwap shows better consistency across thread counts

#### Mixed Read-Write (Most Realistic Scenario)
```
Readers:   2         4         8
smr-swap:  151 ns    152 ns    156 ns
arc-swap:  456 ns    453 ns    493 ns
```
**Winner**: SMR-Swap (66-68% faster)
- Epoch-based reclamation excels with concurrent reads/writes
- ArcSwap's atomic operations create more contention

#### Read Latency with Held Guard
```
smr-swap:  140 ns ██████████
arc-swap:  481 ns ██████████████████████████████████
```
**Winner**: SMR-Swap (71% faster)
- Minimal impact when readers hold guards
- Critical for latency-sensitive applications

### Performance Recommendations

**Use SMR-Swap when:**
- Read-heavy workloads (>80% reads)
- Multiple readers need to hold guards for extended periods
- Mixed read-write patterns are common
- Minimizing read latency is critical

**Use ArcSwap when:**
- Write-heavy workloads (>50% writes)
- Single-threaded or very few readers
- Simplicity is more important than peak performance
- Write latency is critical

## Design

### Type System Guarantees

- **`Swapper<T>`**: Not `Clone` (enforced via `Arc` single ownership)
  - Guarantees single writer via type system
  - Can be shared across threads if wrapped in `Arc` (but breaks single-writer guarantee)

- **`SwapReader<T>`**: `Clone`
  - Multiple readers can be created and shared
  - Each reader independently sees the latest value

### Memory Management

- Uses `crossbeam-epoch` for safe memory reclamation
- Each value is wrapped in an `Atomic<T>` pointer
- Readers pin the epoch, preventing garbage collection of the current value
- Old values are deferred for destruction until all readers have left the epoch

### Thread Safety

Both `Swapper<T>` and `SwapReader<T>` implement `Send + Sync` when `T: Send + Sync`, allowing safe sharing across threads.

## Limitations

- **No `no_std` support**: Requires `std` for thread synchronization
- **Single writer only**: The type system enforces this, but can be bypassed via `clone_inner()`
- **Epoch-based reclamation**: Write latency depends on epoch advancement (typically microseconds)

## Comparison with Alternatives

### vs. `arc-swap`
- **Advantages**: Better read performance, especially with held guards
- **Disadvantages**: Slightly higher write latency due to epoch management

### vs. `RwLock<T>`
- **Advantages**: Lock-free, no contention, better for read-heavy workloads
- **Disadvantages**: Only supports single writer

### vs. `Mutex<T>`
- **Advantages**: Lock-free, no blocking, better performance
- **Disadvantages**: Single writer only

## Safety

All unsafe code is carefully documented and justified:
- Pointer dereferencing is guarded by epoch pins
- Memory is only accessed while guards are held
- Deferred destruction ensures no use-after-free

## Testing

Run tests with:
```bash
cargo test
```

Run benchmarks with:
```bash
cargo bench
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

## Contributing

Contributions are welcome! Please ensure all tests pass and benchmarks are stable before submitting.
