# SMR-Swap: Lock-Free Single-Writer Multiple-Reader Swap Container

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)

A high-performance, lock-free Rust library for safely sharing mutable data between a single writer and multiple readers using epoch-based memory reclamation.

[中文文档](README_CN.md) | [English](README.md)

## Features

- **Lock-Free**: No mutexes or locks required for reads or writes
- **High Performance**: Optimized for both read and write operations
- **Single-Writer Multiple-Reader Pattern**: Type-safe enforcement via `Swapper<T>` and `SwapReader<T>`
- **Memory Safe**: Uses epoch-based reclamation (via `swmr-epoch`) to prevent use-after-free
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

### Arc-Specific Writer Operations

The following methods are only available when `T` is wrapped in an `Arc` (i.e., `Swapper<Arc<T>>`):

#### `swap(new_value: Arc<T>) -> Option<Arc<T>>`
Atomically replaces the current `Arc`-wrapped value and returns the old `Arc`.

```rust
use std::sync::Arc;

let (mut writer, _) = smr_swap::new(Arc::new(42));

// Swap the value and get the old one
if let Some(old) = writer.swap(Arc::new(43)) {
    println!("Old value: {:?}", *old); // 42
}
```

#### `update_and_fetch_arc<F>(f: F) -> Option<Arc<T>>`
Updates the value using a closure that receives the current `Arc` and returns a new `Arc`.

```rust
use std::sync::Arc;

let (mut writer, _) = smr_swap::new(Arc::new(vec![1, 2, 3]));

// Update the vector by adding an element
if let Some(new_arc) = writer.update_and_fetch_arc(|current| {
    let mut vec = current.to_vec();
    vec.push(4);
    Arc::new(vec)
}) {
    println!("New value: {:?}", *new_arc); // [1, 2, 3, 4]
}
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
| Single-Thread Read | 1.80 ns | 9.19 ns | **80% faster** | Pure read performance |
| Single-Thread Write | 137.13 ns | 129.20 ns | 6% slower | Epoch management overhead |
| Multi-Thread Read (2) | 1.82 ns | 9.29 ns | **80% faster** | No contention |
| Multi-Thread Read (4) | 1.85 ns | 9.25 ns | **80% faster** | Consistent scaling |
| Multi-Thread Read (8) | 2.05 ns (avg) | 9.38 ns | **78% faster** | Excellent scaling |
| Mixed R/W (2 readers) | 138.69 ns | 452.64 ns | **69% faster** | 1 writer + 2 readers |
| Mixed R/W (4 readers) | 139.54 ns | 455.19 ns | **69% faster** | 1 writer + 4 readers |
| Mixed R/W (8 readers) | 140.08 ns | 534.12 ns | **74% faster** | 1 writer + 8 readers |
| Batch Read | 2.53 ns | 9.67 ns | **74% faster** | Optimized batch reads |
| Read with Held Guard | 137.27 ns | 524.49 ns | **74% faster** | Reader holds guard during write |
| Read Under Memory Pressure | 860.54 ns | 1.18 μs | **27% faster** | Under memory pressure |

### Detailed Performance Analysis

#### Single-Thread Read
```
smr-swap:  1.80 ns █
arc-swap:  9.19 ns █████
```
**Winner**: SMR-Swap (80% faster)
- Optimized read path with minimal overhead
- Direct pointer access without atomic operations

#### Single-Thread Write
```
smr-swap:  137.13 ns █████████████████
arc-swap:  129.20 ns ████████████████
```
**Winner**: ArcSwap (6% faster)
- SMR-Swap's epoch management has minimal overhead
- Both show excellent write performance

#### Multi-Thread Read Performance (Scaling)
```
Readers:   2         4         8
smr-swap:  1.82 ns   1.85 ns   2.05 ns (avg)
arc-swap:  9.29 ns   9.25 ns   9.38 ns
```
**Analysis**: 
- SMR-Swap maintains near-constant time regardless of thread count
- 80% faster than arc-swap across all thread counts
- Excellent scaling characteristics

#### Mixed Read-Write (Most Realistic Scenario)
```
Readers:   2         4         8
smr-swap:  139 ns    140 ns    140 ns
arc-swap:  453 ns    455 ns    534 ns
```
**Winner**: SMR-Swap (69-74% faster)
- Consistent performance under load
- Minimal impact from concurrent writers
- ArcSwap shows increased latency with more readers

#### Read Under Memory Pressure
```
smr-swap:  860.54 ns █████
arc-swap:  1.18 μs   █████████
```
**Winner**: SMR-Swap (27% faster)
- More efficient memory management under pressure
- Better handling of system resource constraints

#### Read Latency with Held Guard
```
smr-swap:  137.27 ns █████
arc-swap:  524.49 ns ███████████████████
```
**Winner**: SMR-Swap (74% faster)
- Minimal overhead when readers hold guards
- Critical for applications requiring long-lived read access

### Performance Recommendations

**Use SMR-Swap when:**
- Read performance is critical (up to 80% faster reads)
- Multiple readers need to hold guards for extended periods
- Mixed read-write patterns are common
- Consistent low-latency reads are required
- Memory efficiency under pressure is important

**Use ArcSwap when:**
- You need maximum write performance (6% faster writes)
- Your workload is primarily single-threaded
- You need a simpler, more established solution
- You prefer slightly lower memory usage in exchange for slower reads

## Design

### Type System Guarantees

- **`Swapper<T>`**: Not `Clone` (enforced via `Arc` single ownership)
  - Guarantees single writer via type system
  - Can be shared across threads if wrapped in `Arc` (but breaks single-writer guarantee)

- **`SwapReader<T>`**: `Clone`
  - Multiple readers can be created and shared
  - Each reader independently sees the latest value

### Memory Management

#### swmr-epoch Implementation

SMR-Swap uses a custom `swmr-epoch` library for memory reclamation, optimized for single-writer multiple-reader scenarios compared to `crossbeam-epoch`:

**Core Design**:
- **Global Epoch Management**: Only the Writer can advance the global epoch (via `fetch_add`)
- **Reader Registration**: Each reader thread maintains a `ParticipantSlot` in TLS (thread-local storage) that records its current active epoch
- **Deferred Reclamation**: Writer maintains a garbage bin grouped by epoch (`BTreeMap<usize, Vec<ErasedGarbage>>`)

**Key Mechanisms**:

1. **Pin Operation** (`ReaderRegistry::pin()`):
   - Readers call `pin()` to obtain a `Guard`
   - On first pin, the current global epoch is recorded in the thread-local `active_epoch`
   - Supports reentrancy (via `pin_count` counter)
   - When Guard is dropped, if count reaches zero, the thread is marked inactive (`INACTIVE_EPOCH`)

2. **Garbage Reclamation** (`Writer::try_reclaim()`):
   - Writer triggers reclamation when garbage accumulates beyond threshold (default 64 items)
   - Step 1: Advance global epoch
   - Step 2: Scan all active readers to find minimum active epoch
   - Step 3: Calculate safe reclamation point = min_active_epoch - 1
   - Step 4: Use `BTreeMap::retain` to remove all garbage with epoch ≤ safe point

3. **Memory Optimization**:
   - Uses `BTreeMap::retain` instead of `split_off` to avoid new allocations, reducing global allocator contention
   - This prevents latency spikes on the first `pin()` operation

**Performance Characteristics**:
- Single-thread read: 42% faster (simpler Atomic operations)
- Single-thread write: 30% faster (Writer holds directly, no Mutex overhead)
- Multi-thread read: 104-128% slower than crossbeam-epoch (ThreadLocal lookup and atomic operation overhead per `pin()`)

**Optimization Suggestions**:
- For read-heavy scenarios, consider `read_with_guard()` method to reuse Guard
- Or cache Guard in SwapReader (requires thread-local)

**Each value is wrapped in an `Atomic<T>` pointer**:
- Readers safely dereference the pointer via Guard
- Old values are deferred for destruction until all readers have left the epoch

### Thread Safety

Both `Swapper<T>` and `SwapReader<T>` implement `Send + Sync` when `T: 'static`, allowing safe sharing across threads.

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

## Implementation Details

### Epoch Mechanism

- Each reader enters the current epoch via `ReaderRegistry::pin()`
- Writer uses deferred destruction to delay old value cleanup
- Old values are truly destroyed only when all readers have left the epoch

### Atomic Operations

- Uses `Atomic<T>` for atomic pointer swapping
- Uses `Ordering::Release` and `Ordering::Acquire` to ensure memory ordering
- Writer's `store()` method automatically hands old pointers to garbage collection

### Guard Mechanism

- `SwapGuard<T>` holds a `Guard` to maintain thread pin state
- Provides transparent access to the value via `Deref` trait
- When the guard is dropped, the pin count decrements, and if it reaches zero the thread is marked inactive
- Guard supports cloning to implement reentrant pin
