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
- **Zero-Copy Reads**: Readers get direct references to the current value
- **Concurrent**: Safe to use across multiple threads with `Send + Sync` bounds

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
smr-swap = "0.4"
```

### Basic Usage

```rust
use smr_swap;

fn main() {
    // Create a new SMR container with initial value
    let (mut swapper, reader) = smr_swap::new(vec![1, 2, 3]);

    // Register reader for current thread
    let reader_epoch = reader.register_reader();

    // Reader can clone and share across threads
    let reader_clone = reader.clone();

    // Writer updates the value
    swapper.update(vec![4, 5, 6]);

    // Reader sees the new value
    let guard = reader_epoch.pin();
    let val = reader.read(&guard);
    println!("{:?}", *val); // [4, 5, 6]
}
```

### Using with Arc (for shared ownership)

While SMR-Swap works with any type `T`, you can wrap values in `Arc` for shared ownership:

```rust
use smr_swap;
use std::sync::Arc;

fn main() {
    let (mut swapper, reader) = smr_swap::new(Arc::new(vec![1, 2, 3]));
    
    let reader_epoch = reader.register_reader();
    
    swapper.update(Arc::new(vec![4, 5, 6]));
    
    let guard = reader_epoch.pin();
    let val = reader.read(&guard);
    println!("{:?}", *val); // Arc<Vec<i32>>
}
```

### Multi-Writer Support (using Mutex)

Since `Swapper<T>` is single-writer (not `Clone`), to support multiple writers, you can wrap it in a `Mutex` (and `Arc` for sharing). SMR-Swap's efficient `update` often makes this faster than using `Mutex<T>` directly or `ArcSwap`.

```rust
use smr_swap;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let (swapper, reader) = smr_swap::new(vec![1, 2, 3]);
    // Wrap swapper in Mutex for multiple writers
    let swapper = Arc::new(Mutex::new(swapper));
    let reader = Arc::new(reader);
    
    let mut handles = vec![];

    // 4 Writers
    for i in 0..4 {
        let swapper_clone = swapper.clone();
        handles.push(thread::spawn(move || {
            // Lock, update, and unlock
            swapper_clone.lock().unwrap().update(vec![i; 3]);
        }));
    }

    // 4 Readers
    for _ in 0..4 {
        let reader_clone = reader.clone();
        handles.push(thread::spawn(move || {
            let local_epoch = reader_clone.register_reader();
            let guard = local_epoch.pin();
            let val = reader_clone.read(&guard);
            println!("{:?}", *val);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
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

### Registering Readers

Before reading, each thread must register itself to obtain a `LocalEpoch`:

```rust
// In reader thread
let local_epoch = reader.register_reader();

// In writer thread
let writer_epoch = swapper.register_reader();
```

The `LocalEpoch` is `!Sync` and must be stored per-thread (typically in thread-local storage).

### Writer Operations (Swapper<T>)

#### `update(new_value: T)`
Atomically replaces the current value.

```rust
swapper.update(new_value);
```

#### `read<'a>(&self, guard: &'a PinGuard) -> &'a T`
Gets a read-only reference to the current value. Requires a `PinGuard` to ensure the value is not reclaimed.

```rust
let guard = local_epoch.pin();
let val = swapper.read(&guard);
println!("Current: {:?}", *val);
```

#### `map<F, U>(&self, local_epoch: &LocalEpoch, f: F) -> U where F: FnOnce(&T) -> U`
Applies a closure to the current value and returns the result.

```rust
let len = swapper.map(&local_epoch, |v| v.len());
```

#### `update_and_fetch<'a, F>(&mut self, guard: &'a PinGuard, f: F) -> &'a T where F: FnOnce(&T) -> T`
Atomically updates the value using the provided closure and returns a reference to the new value.

```rust
let guard = local_epoch.pin();
let val = swapper.update_and_fetch(&guard, |v| {
    let mut new_v = v.clone();
    new_v.push(42);
    new_v
});
```

#### `register_reader() -> LocalEpoch`
Registers the current thread as a reader and returns a `LocalEpoch` for use in read operations.

```rust
let local_epoch = swapper.register_reader();
```

### Arc-Specific Writer Operations (Swapper<Arc<T>>)

The following methods are only available when `T` is wrapped in an `Arc`:

#### `swap(&mut self, local_epoch: &LocalEpoch, new_value: Arc<T>) -> Arc<T>`
Atomically replaces the current `Arc`-wrapped value and returns the old `Arc`.

```rust
use std::sync::Arc;

let (mut swapper, _) = smr_swap::new(Arc::new(42));
let writer_epoch = swapper.register_reader();

let old = swapper.swap(&writer_epoch, Arc::new(43));
println!("Old value: {:?}", *old); // 42
```

#### `update_and_fetch_arc<F>(&mut self, local_epoch: &LocalEpoch, f: F) -> Arc<T> where F: FnOnce(&Arc<T>) -> Arc<T>`
Updates the value using a closure that receives the current `Arc` and returns a new `Arc`.

```rust
use std::sync::Arc;

let (mut swapper, _) = smr_swap::new(Arc::new(vec![1, 2, 3]));
let writer_epoch = swapper.register_reader();

let new_arc = swapper.update_and_fetch_arc(&writer_epoch, |current| {
    let mut vec = (**current).clone();
    vec.push(4);
    Arc::new(vec)
});
println!("New value: {:?}", *new_arc); // [1, 2, 3, 4]
```

### Reader Operations (SwapReader<T>)

#### `read<'a>(&self, guard: &'a PinGuard) -> &'a T`
Gets a read-only reference to the current value.

```rust
let guard = local_epoch.pin();
let val = reader.read(&guard);
println!("Current: {:?}", *val);
```

#### `map<'a, F, U>(&self, local_epoch: &'a LocalEpoch, f: F) -> U where F: FnOnce(&T) -> U`
Applies a closure to the current value and returns the result.

```rust
let len = reader.map(&local_epoch, |v| v.len());
```

#### `filter<'a, F>(&self, guard: &'a PinGuard, f: F) -> Option<&'a T> where F: FnOnce(&T) -> bool`
Returns a reference to the current value if the closure returns true.

```rust
let guard = local_epoch.pin();
if let Some(val) = reader.filter(&guard, |v| !v.is_empty()) {
    println!("Non-empty: {:?}", *val);
}
```

#### `register_reader() -> LocalEpoch`
Registers the current thread as a reader and returns a `LocalEpoch` for use in read operations.

```rust
let local_epoch = reader.register_reader();
```

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
smr-swap:  87.90 ns ████████
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

### Performance Recommendations

**Use SMR-Swap when:**
- Read performance is critical (up to 90% faster reads)
- Multiple readers need to hold guards for extended periods (90% faster)
- Mixed read-write patterns are common (79-81% faster)
- Consistent low-latency reads are required under all conditions
- Predictable performance under memory pressure is needed (53% faster)
- Sub-nanosecond read latency is required
- Higher write throughput than ArcSwap is desired (32% faster)

**Use ArcSwap when:**
- You need the absolute simplest implementation
- You need a more established, battle-tested solution
- You prefer lower write latency over read optimization
- You have very simple read patterns with minimal guard holding

## Design

### Type System Guarantees

- **`Swapper<T>`**: Not `Clone` (enforced via `Arc` single ownership)
  - Guarantees single writer via type system
  - Can be shared across threads if wrapped in `Arc` (but breaks single-writer guarantee)

- **`SwapReader<T>`**: `Clone`
  - Multiple readers can be created and shared
  - Each reader independently sees the latest value

- **`LocalEpoch`**: `!Sync` (enforced by type system)
  - Must be stored per-thread (typically in thread-local storage)
  - Ensures each thread has its own epoch tracking state
  - Prevents accidental sharing across threads

### API Design: Explicit LocalEpoch Management

The new API design requires explicit `LocalEpoch` registration:

```rust
// Reader thread setup
let local_epoch = reader.register_reader();

// All read operations require a PinGuard
let guard = local_epoch.pin();
let val = reader.read(&guard);
```

**Benefits**:
- **Explicit control**: Users understand when epoch tracking is active
- **Type safety**: Compiler prevents misuse of LocalEpoch across threads
- **Performance**: Avoids hidden thread-local lookups on every read
- **Flexibility**: Users can cache LocalEpoch for repeated reads

### Memory Management

#### swmr-epoch Implementation

SMR-Swap uses a custom `swmr-epoch` library for memory reclamation, optimized for single-writer multiple-reader scenarios:

**Core Architecture**:
- **Global Epoch Counter**: Atomic counter advanced by writer during garbage collection
- **Reader Slots**: Each reader maintains a `ReaderSlot` with an `AtomicUsize` tracking its active epoch
- **Shared State**: `SharedState` holds the global epoch and a `Mutex<Vec<Weak<ReaderSlot>>>` for reader tracking
- **Garbage Bins**: Writer maintains a `VecDeque<(usize, Vec<RetiredObject>)>` grouping garbage by epoch

**Key Mechanisms**:

1. **Pin Operation** (`LocalEpoch::pin()`):
   - Increments thread-local `pin_count` counter
   - On first pin (count = 0), loads current global epoch and stores it in the `ReaderSlot`
   - Returns a `PinGuard` that keeps the thread pinned
   - Supports reentrancy: multiple nested pins via `pin_count` tracking
   - When `PinGuard` is dropped, decrements `pin_count`; if reaches zero, marks thread as `INACTIVE_EPOCH`

2. **Garbage Collection** (`GcHandle::collect()`):
   - Step 1: Advance global epoch via `fetch_add(1, Ordering::Acquire)`
   - Step 2: Scan all active readers (via `Weak` references) to find minimum active epoch
   - Step 3: Calculate safe reclamation point:
     - If no active readers: reclaim all garbage
     - Otherwise: reclaim garbage from epochs older than `min_active_epoch - 1`
   - Step 4: Pop garbage from front of `VecDeque` until reaching safe point
   - Step 5: Clean up dead `Weak` references in the readers list

3. **Automatic Reclamation**:
   - Configurable threshold (default: 64 items)
   - After each `retire()`, if total garbage exceeds threshold, `collect()` is automatically triggered
   - Can be disabled by passing `None` to `new_with_threshold()`

4. **Memory Efficiency**:
   - Uses `VecDeque` for O(1) front removal of reclaimed garbage
   - Weak references prevent reader slots from being kept alive indefinitely
   - Automatic cleanup of dead readers during collection cycles

**Performance Characteristics**:
- Single-thread read: 90% faster than arc-swap (minimal atomic operations)
- Single-thread write: 32% faster than arc-swap (direct ownership, no Mutex overhead)
- Multi-thread read: 90% faster than arc-swap (efficient epoch tracking)
- Automatic reclamation prevents unbounded garbage accumulation

**Optimization Suggestions**:
- For read-heavy scenarios, use `read_with_guard()` to reuse Guard without re-pinning
- Cache `LocalEpoch` in thread-local storage to avoid repeated `register_reader()` calls
- Adjust reclamation threshold via `new_with_threshold()` based on workload characteristics

### Thread Safety

Both `Swapper<T>` and `SwapReader<T>` implement `Send + Sync` when `T: 'static`, allowing safe sharing across threads. The `LocalEpoch` is `!Sync` to prevent accidental cross-thread usage.

## Limitations

- **No `no_std` support**: Requires `std` for thread synchronization
- **Single writer only**: The type system enforces this via `Swapper` not being `Clone`
- **Epoch-based reclamation**: Write latency depends on epoch advancement (typically microseconds)
- **Explicit LocalEpoch management**: Users must call `register_reader()` and pass `LocalEpoch` to read operations

## Comparison with Alternatives

### vs. `arc-swap`
- **Advantages**: 90% faster reads, 32% faster writes, 90% faster writes when guards are held
- **Disadvantages**: Writes trigger GC, API requires explicit LocalEpoch management

### vs. `RwLock<T>`
- **Advantages**: Wait-free reads, no contention, better for read-heavy workloads
- **Disadvantages**: Only supports single writer

### vs. `Mutex<T>`
- **Advantages**: Wait-free reads, no blocking, better performance
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

## Benchmark Details

### Test Scenarios

Benchmarks cover typical workloads for single-writer multiple-reader systems:

1. **Single-Thread Read**: Continuous reads from a single thread, tests pure read performance
2. **Single-Thread Write**: Continuous writes from a single thread, tests write overhead
3. **Multi-Thread Read** (2/4/8 threads): Concurrent read scalability testing
4. **Mixed Read-Write**: 1 writer thread + N reader threads, most realistic scenario
5. **Batch Read**: Multiple reads within a single pin, tests `read_with_guard()` optimization
6. **Read with Held Guard**: Write latency while readers hold guards
7. **Memory Pressure**: Frequent writes causing garbage accumulation, tests GC overhead

### Key Findings

**Read Performance**:
- Via `EpochPtr` and `PinGuard` mechanism, SMR-Swap is **90% faster** than arc-swap on reads
- Single-thread read achieves **0.90 ns**, approaching hardware limits
- Multi-thread reads maintain consistent sub-nanosecond latency with no contention

**Write Performance**:
- Single-thread write is **32% faster** than arc-swap (87.90 ns vs 130.23 ns)
- Benefits from `VecDeque` garbage management and aggressive GC collection
- Mixed workload write latency is stable (92-94 ns) with immediate GC
- Aggressive GC in `update()` ensures predictable performance

**Scalability**:
- Performance remains stable as reader count increases, with no contention
- Multi-thread reads maintain 0.90-0.93 ns across 2/4/8 threads
- Mixed read-write scenarios show SMR-Swap is **79-81% faster** than arc-swap
- Even in multi-writer scenarios (requiring Mutex), performance is **67-78% faster** than arc-swap

**Guard Holding**:
- When readers hold guards, SMR-Swap write latency is much lower than arc-swap (89.91 ns vs 908.69 ns)
- **90% faster** than arc-swap in this critical scenario
- Essential for applications requiring long-lived read access

**Memory Pressure**:
- **Improved**: SMR-Swap is now **53% faster** than arc-swap under memory pressure (741.47 ns vs 1580 ns)
- Aggressive garbage collection in `update()` prevents garbage accumulation
- Epoch-based reclamation is triggered immediately after each write
- Read performance remains stable even under high write frequency

## Use Cases

SMR-Swap is particularly well-suited for scenarios where read performance is critical and writes are relatively infrequent:

### Ideal Scenarios

- **Configuration Hot Updates**: Single configuration manager, multiple services reading config
  - Advantage: Config read latency < 1 ns, no lock contention
  - Suitable for: Microservice architectures with dynamic config distribution

- **Cache Management**: Single cache update thread, multiple query threads
  - Advantage: Cache queries extremely fast (0.90 ns), excellent scalability
  - Suitable for: High-concurrency query scenarios

- **Routing Tables**: Single routing table manager, multiple forwarding threads
  - Advantage: Route lookups have no contention, supports long-lived references
  - Suitable for: Network packet forwarding, load balancing

- **Feature Flags**: Single administrator, multiple checking threads
  - Advantage: Feature checks are extremely fast, non-blocking
  - Suitable for: A/B testing, canary deployments

- **Performance-Critical Read Paths**: Systems requiring minimal read latency
  - Advantage: Sub-nanosecond read latency, 90% faster than arc-swap
  - Suitable for: High-frequency trading, real-time data processing

### Less Suitable Scenarios

- **Frequent Writes**: If write frequency approaches read frequency, GC overhead increases
  - Recommendation: Use `new_with_threshold(None)` to disable auto-reclamation, control manually
  
- **Memory-Constrained Environments**: Garbage accumulation may cause GC pauses
  - Recommendation: Adjust `new_with_threshold()` to a smaller value, or use arc-swap

### Performance Optimization Tips

Choose optimization strategies based on workload characteristics:

1. **Read-Heavy** (Recommended):
   - Use default configuration (threshold 64)
   - Cache `LocalEpoch` in thread-local storage
   - Use `read_with_guard()` for batch reads

2. **Balanced Read-Write**:
   - Adjust threshold: `new_with_threshold(Some(128))` or higher
   - Call `gc.collect()` periodically to control GC timing

3. **Memory-Constrained**:
   - Lower threshold: `new_with_threshold(Some(32))`
   - Or disable auto-reclamation: `new_with_threshold(None)`, trigger `collect()` manually

## Implementation Details

### LocalEpoch and Pin Mechanism

- Each reader obtains a `LocalEpoch` via `register_reader()` (once per thread)
- `LocalEpoch` contains:
  - `Arc<ReaderSlot>`: Shared slot tracking this reader's active epoch
  - `Arc<SharedState>`: Reference to global state (epoch counter and reader list)
  - `Cell<usize>`: Thread-local `pin_count` for reentrancy tracking
- When `read()` is called with a `LocalEpoch`, it calls `local_epoch.pin()`:
  - If `pin_count == 0`: loads current global epoch and stores in `ReaderSlot`
  - Increments `pin_count` and returns `PinGuard`
  - Supports reentrancy: multiple nested pins increment counter
- When `PinGuard` is dropped:
  - Decrements `pin_count`
  - If `pin_count` reaches zero: marks thread as `INACTIVE_EPOCH` (usize::MAX)

### Atomic Operations

- Uses `EpochPtr<T>` (from `swmr-epoch`) for atomic pointer management
- `EpochPtr::load(&guard)` safely dereferences the pointer with lifetime bound to guard
- `EpochPtr::store(new_value, &mut gc)` atomically swaps pointer and retires old value
- Uses `Ordering::Acquire` for loads and `Ordering::Release` for stores to ensure memory ordering

### Guard Mechanism

- `PinGuard<'a>` maintains the epoch pin state
- `read` returns `&'a T` which is tied to the lifetime of `PinGuard`
- Ensures value cannot be accessed after guard is dropped
- `PinGuard` supports `Clone` for nested pinning (increments `pin_count`)

### Garbage Collection Pipeline

1. **Retire Phase**: When writer calls `store()`, old value is wrapped in `RetiredObject` and added to garbage bin
2. **Accumulation**: Garbage is grouped by epoch in `VecDeque<(usize, Vec<RetiredObject>)>`
3. **Automatic Trigger**: After each `retire()`, if total garbage > threshold, `collect()` is automatically invoked
4. **Collection Phase**:
   - Advance global epoch
   - Scan all active readers to find minimum active epoch
   - Calculate safe reclamation point (min_active_epoch - 1)
   - Pop garbage from front of deque until reaching safe point
   - Dropped `RetiredObject`s automatically invoke their destructors
5. **Cleanup**: Dead reader slots (via `Weak` references) are cleaned up during collection

### Value Lifecycle

- Writer calls `update()` or `swap()` to replace the current value
- Old value is immediately wrapped in `RetiredObject` and stored in garbage bin for current epoch
- Writer can optionally call `gc.collect()` to trigger garbage collection
- When all readers have left the epoch, garbage is safely reclaimed and destructors are invoked
- This ensures no use-after-free while minimizing synchronization overhead
