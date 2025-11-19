# SMR-Swap: 最小化锁单写多读交换容器

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)

一个高性能的 Rust 库，使用基于 Epoch 的内存回收机制，安全地在单个写入者和多个读取者之间共享可变数据。

[English Documentation](README.md) | [中文文档](README_CN.md)

## 特性

- **最小化锁设计**: 读取操作是 Wait-Free 的，写入操作仅在垃圾回收时需要锁
- **高性能**: 针对读写操作进行了优化
- **单写多读模式**: 通过 `Swapper<T>` 和 `SwapReader<T>` 在类型系统层面强制执行
- **内存安全**: 使用基于 Epoch 的回收机制（通过 `swmr-epoch`）防止 Use-After-Free
- **零拷贝读取**: 读取者直接获得当前值的引用
- **并发安全**: 在多线程中安全使用，支持 `Send + Sync` 约束

## 快速开始

### 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
smr-swap = "0.4"
```

### 基本用法

```rust
use smr_swap;

fn main() {
    // 创建一个新的 SMR 容器，初始值为 vec![1, 2, 3]
    let (mut swapper, reader) = smr_swap::new(vec![1, 2, 3]);

    // 为当前线程注册读取者
    let reader_epoch = reader.register_reader();

    // 读取者可以克隆并在线程间共享
    let reader_clone = reader.clone();

    // 写入者更新值
    swapper.update(vec![4, 5, 6]);

    // 读取者看到新值
    let guard = reader_epoch.pin();
    let val = reader.read(&guard);
    println!("{:?}", *val); // [4, 5, 6]
}
```

### 使用 Arc 进行共享所有权

SMR-Swap 可以与任何类型 `T` 一起使用，你也可以将值包装在 `Arc` 中以实现共享所有权：

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

### 多写入者支持 (使用 Mutex)

由于 `Swapper<T>` 是单写入者的（不可 `Clone`），为了支持多写入者，你可以将其包装在 `Mutex` 中（并使用 `Arc` 进行共享）。SMR-Swap 高效的 `update` 操作通常使其比直接使用 `Mutex<T>` 或 `ArcSwap` 更快。

```rust
use smr_swap;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let (swapper, reader) = smr_swap::new(vec![1, 2, 3]);
    // 将 swapper 包装在 Mutex 中以支持多写入者
    let swapper = Arc::new(Mutex::new(swapper));
    let reader = Arc::new(reader);
    
    let mut handles = vec![];

    // 4 个写入者
    for i in 0..4 {
        let swapper_clone = swapper.clone();
        handles.push(thread::spawn(move || {
            // 加锁，更新，解锁
            swapper_clone.lock().unwrap().update(vec![i; 3]);
        }));
    }

    // 4 个读取者
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

**注意**: SMR-Swap 本身不需要 `Arc`。仅当你需要内部值的共享所有权时才使用 `Arc`。

## API 概览

### 创建容器

```rust
let (swapper, reader) = smr_swap::new(initial_value);
```

返回一个元组，包含：
- `Swapper<T>`: 写入者（不可 `Clone`，强制单写）
- `SwapReader<T>`: 读取者（可克隆以支持多个读取者）

### 注册读取者

在读取之前，每个线程必须注册自己以获得 `LocalEpoch`：

```rust
// 在读取者线程中
let local_epoch = reader.register_reader();

// 在写入者线程中
let writer_epoch = swapper.register_reader();
```

`LocalEpoch` 是 `!Sync` 的，必须存储在每个线程中（通常在线程本地存储中）。

### 写入者操作 (Swapper<T>)

#### `update(new_value: T)`
原子地替换当前值。

```rust
swapper.update(new_value);
```

#### `read<'a>(&self, guard: &'a PinGuard) -> &'a T`
获取当前值的只读引用。需要提供 `PinGuard` 以确保值不会被回收。

```rust
let guard = local_epoch.pin();
let val = swapper.read(&guard);
println!("当前值: {:?}", *val);
```

#### `map<F, U>(&self, local_epoch: &LocalEpoch, f: F) -> U where F: FnOnce(&T) -> U`
对当前值应用闭包并返回结果。

```rust
let len = swapper.map(&local_epoch, |v| v.len());
```

#### `update_and_fetch<'a, F>(&mut self, guard: &'a PinGuard, f: F) -> &'a T where F: FnOnce(&T) -> T`
使用提供的闭包原子地更新值，并返回新值的引用。

```rust
let guard = local_epoch.pin();
let val = swapper.update_and_fetch(&guard, |v| {
    let mut new_v = v.clone();
    new_v.push(42);
    new_v
});
```

#### `register_reader() -> LocalEpoch`
为当前线程注册一个读取者，并返回用于读取操作的 `LocalEpoch`。

```rust
let local_epoch = swapper.register_reader();
```

### Arc 专用的写入者操作 (Swapper<Arc<T>>)

以下方法仅在 `T` 被 `Arc` 包装时可用：

#### `swap(&mut self, local_epoch: &LocalEpoch, new_value: Arc<T>) -> Arc<T>`
原子地替换当前 `Arc` 包装的值并返回旧的 `Arc`。

```rust
use std::sync::Arc;

let (mut swapper, _) = smr_swap::new(Arc::new(42));
let writer_epoch = swapper.register_reader();

let old = swapper.swap(&writer_epoch, Arc::new(43));
println!("旧值: {:?}", *old); // 42
```

#### `update_and_fetch_arc<F>(&mut self, local_epoch: &LocalEpoch, f: F) -> Arc<T> where F: FnOnce(&Arc<T>) -> Arc<T>`
使用接收当前 `Arc` 并返回新 `Arc` 的闭包来更新值。

```rust
use std::sync::Arc;

let (mut swapper, _) = smr_swap::new(Arc::new(vec![1, 2, 3]));
let writer_epoch = swapper.register_reader();

let new_arc = swapper.update_and_fetch_arc(&writer_epoch, |current| {
    let mut vec = (**current).clone();
    vec.push(4);
    Arc::new(vec)
});
println!("新值: {:?}", *new_arc); // [1, 2, 3, 4]
```

### 读取者操作 (SwapReader<T>)

#### `read<'a>(&self, guard: &'a PinGuard) -> &'a T`
获取当前值的只读引用。

```rust
let guard = local_epoch.pin();
let val = reader.read(&guard);
println!("当前值: {:?}", *val);
```

#### `map<'a, F, U>(&self, local_epoch: &'a LocalEpoch, f: F) -> U where F: FnOnce(&T) -> U`
对当前值应用闭包并返回结果。

```rust
let len = reader.map(&local_epoch, |v| v.len());
```

#### `filter<'a, F>(&self, guard: &'a PinGuard, f: F) -> Option<&'a T> where F: FnOnce(&T) -> bool`
仅当谓词为真时返回当前值的引用。

```rust
let guard = local_epoch.pin();
if let Some(val) = reader.filter(&guard, |v| !v.is_empty()) {
    println!("非空: {:?}", *val);
}
```

#### `register_reader() -> LocalEpoch`
为当前线程注册一个读取者，并返回用于读取操作的 `LocalEpoch`。

```rust
let local_epoch = reader.register_reader();
```

## 性能特性

与 `arc-swap` 的全面基准测试对比结果。

### 基准测试总结表

| 场景 | SMR-Swap | ArcSwap | 性能提升 | 说明 |
|------|----------|---------|---------|------|
| 单线程读取 | 0.90 ns | 8.96 ns | **快 90%** | 纯读取性能 |
| 单线程写入 | 87.90 ns | 130.23 ns | **快 32%** | 改进的 Epoch 管理 |
| 多线程读取 (2) | 0.90 ns | 9.37 ns | **快 90%** | 无竞争 |
| 多线程读取 (4) | 0.91 ns | 9.33 ns | **快 90%** | 一致的扩展性 |
| 多线程读取 (8) | 0.93 ns | 9.63 ns | **快 90%** | 优秀的扩展性 |
| 混合读写 (2 读) | 93.21 ns | 446.45 ns | **快 79%** | 1 写 + 2 读 |
| 混合读写 (4 读) | 92.89 ns | 451.09 ns | **快 79%** | 1 写 + 4 读 |
| 混合读写 (8 读) | 93.85 ns | 493.12 ns | **快 81%** | 1 写 + 8 读 |
| 批量读取 | 1.62 ns | 9.91 ns | **快 84%** | 优化的批量读取 |
| 多写多读 (4 读) | 629.63 ns | 1.92 µs | **快 67%** | 4 写 + 4 读 (SMR 使用 Mutex) |
| 多写多读 (8 读) | 640.33 ns | 2.23 µs | **快 71%** | 4 写 + 8 读 (SMR 使用 Mutex) |
| 多写多读 (16 读) | 626.57 ns | 2.85 µs | **快 78%** | 4 写 + 16 读 (SMR 使用 Mutex) |
| 持有守卫的读取 | 89.91 ns | 908.69 ns | **快 90%** | 读取者持有守卫期间写入 |
| 内存压力下读取 | 741.47 ns | 1.58 µs | **快 53%** | 激进的 GC 回收 |

### 详细性能分析

#### 单线程读取
```
smr-swap:  0.90 ns █
arc-swap:  8.96 ns ██████████
```
**赢家**: SMR-Swap (快 90%)
- 极快的读取路径，开销极小
- 直接指针访问，无需原子操作
- 接近纳秒级延迟

#### 单线程写入
```
smr-swap:  87.90 ns ████████
arc-swap:  130.23 ns █████████████
```
**赢家**: SMR-Swap (快 32%)
- 改进的 Epoch 管理效率
- 即使有 GC 开销，写入速度依然极快

#### 多线程读取性能（扩展性）
```
读取者数:  2         4         8
smr-swap:  0.90 ns   0.91 ns   0.93 ns
arc-swap:  9.37 ns   9.33 ns   9.63 ns
```
**分析**:
- SMR-Swap 保持接近恒定的亚纳秒级读取时间，不受线程数影响
- 在所有线程数下都比 arc-swap 快 90%
- 扩展性极佳，几乎没有竞争

#### 混合读写（最现实的场景）
```
读取者数:  2         4         8
smr-swap:  93 ns     93 ns     94 ns
arc-swap:  446 ns    451 ns    493 ns
```
**赢家**: SMR-Swap (快 79-81%)
- 负载下性能稳定（93-94 ns 跨所有线程数）
- 并发写入影响极小
- ArcSwap 在读取者增加时延迟上升（最高 493 ns）
- 激进的 GC 策略确保稳定性能

#### 多写多读性能
```
配置:      4写4读    4写8读    4写16读
smr-swap:  0.63 µs   0.64 µs   0.63 µs
arc-swap:  1.92 µs   2.23 µs   2.85 µs
```
**赢家**: SMR-Swap (快 67-78%)
- 即使 SMR-Swap 需要 `Mutex` 来支持多写入者，其性能依然远超 ArcSwap
- ArcSwap 随着读取者增加，写入延迟显著增加
- 证明了 SMR-Swap 核心机制的高效性

#### 内存压力下的读取
```
smr-swap:  741 ns   ████
arc-swap:  1580 ns  █████████
```
**赢家**: SMR-Swap (快 53%)
- **改进**：激进的垃圾回收防止垃圾积累
- 在 `update()` 中立即触发 Epoch 回收
- 即使在内存压力下也保持一致的性能

#### 持有守卫时的读取延迟
```
smr-swap:  89.91 ns ████
arc-swap:  908.69 ns ██████████████████
```
**赢家**: SMR-Swap (快 90%)
- 读取者持有守卫时开销极小
- 对需要长时间保持读取访问的应用至关重要

### 性能选择建议

**选择 SMR-Swap 当：**
- 读取性能至关重要（读取速度快 90%）
- 多个读取者需要长时间持有守卫（快 90%）
- 混合读写模式很常见（快 79-81%）
- 需要在所有条件下稳定的低延迟读取
- 需要在内存压力下的可预测性能（快 53%）
- 需要亚纳秒级读取延迟
- 需要比 ArcSwap 更高的写入吞吐量（快 32%）

**选择 ArcSwap 当：**
- 需要最简单的实现
- 需要更成熟、经过充分验证的解决方案
- 优先考虑较低的写入延迟而非读取优化
- 读取模式简单，很少持有守卫

## 设计原理

### 类型系统保证

- **`Swapper<T>`**: 不可 `Clone`（通过 `Arc` 单一所有权强制）
  - 通过类型系统保证单个写入者
  - 可以包装在 `Arc` 中在线程间共享（但会破坏单写保证）

- **`SwapReader<T>`**: 可 `Clone`
  - 可以创建多个读取者并共享
  - 每个读取者独立看到最新值

- **`LocalEpoch`**: `!Sync`（由类型系统强制）
  - 必须存储在每个线程中（通常在线程本地存储中）
  - 确保每个线程有自己的 Epoch 追踪状态
  - 防止意外的跨线程共享

### API 设计：显式 LocalEpoch 管理

新的 API 设计要求显式的 `LocalEpoch` 注册：

```rust
// 读取者线程设置
let local_epoch = reader.register_reader();

// 所有读取操作都需要 PinGuard
let guard = local_epoch.pin();
let val = reader.read(&guard);
```

**优势**：
- **显式控制**：用户理解何时 Epoch 追踪处于活跃状态
- **类型安全**：编译器防止 LocalEpoch 跨线程误用
- **性能**：避免每次读取时隐藏的线程本地查询
- **灵活性**：用户可以缓存 LocalEpoch 以进行重复读取

### 内存管理

#### swmr-epoch 底层实现

SMR-Swap 使用自定义的 `swmr-epoch` 库进行内存回收，针对单写多读场景进行了优化：

**核心架构**：
- **全局 Epoch 计数器**：原子计数器，由 Writer 在垃圾回收期间推进
- **读取者槽位**：每个读取者维护一个 `ReaderSlot`，其中的 `AtomicUsize` 追踪其活跃 Epoch
- **共享状态**：`SharedState` 持有全局 Epoch 和 `Mutex<Vec<Weak<ReaderSlot>>>` 用于读取者追踪
- **垃圾桶**：Writer 维护 `VecDeque<(usize, Vec<RetiredObject>)>` 按 Epoch 分组垃圾

**关键机制**：

1. **Pin 操作** (`LocalEpoch::pin()`)：
   - 增加线程本地 `pin_count` 计数器
   - 首次 pin（计数 = 0）时：加载当前全局 Epoch 并存储在 `ReaderSlot`
   - 返回 `PinGuard` 保持线程被钉住
   - 支持可重入：多个嵌套 pin 通过 `pin_count` 追踪
   - 当 `PinGuard` 被 drop 时：递减 `pin_count`；若达到零则标记为 `INACTIVE_EPOCH`

2. **垃圾回收** (`GcHandle::collect()`)：
   - 步骤 1：通过 `fetch_add(1, Ordering::Acquire)` 推进全局 Epoch
   - 步骤 2：扫描所有活跃读取者（通过 `Weak` 引用）找出最小活跃 Epoch
   - 步骤 3：计算安全回收点：
     - 若无活跃读取者：回收所有垃圾
     - 否则：回收比 `min_active_epoch - 1` 更旧的 Epoch 中的垃圾
   - 步骤 4：从 `VecDeque` 前端弹出垃圾直到达到安全点
   - 步骤 5：清理读取者列表中的死 `Weak` 引用

3. **自动回收**：
   - 可配置阈值（默认：64 项）
   - 每次 `retire()` 后，若总垃圾超过阈值，自动触发 `collect()`
   - 可通过向 `new_with_threshold()` 传递 `None` 来禁用

4. **内存效率**：
   - 使用 `VecDeque` 实现 O(1) 前端移除已回收垃圾
   - Weak 引用防止读取者槽位被无限期保活
   - 回收周期中自动清理死读取者

**性能特点**：
- 单线程读：比 arc-swap 快 90%（最小原子操作）
- 单线程写：比 arc-swap 快 32%（直接所有权，无 Mutex 开销）
- 多线程读：比 arc-swap 快 90%（高效的 Epoch 追踪）
- 自动回收防止垃圾无限积累

**优化建议**：
- 对于读多写少的场景，使用 `read_with_guard()` 复用 Guard，避免重新 pin
- 在线程本地存储中缓存 `LocalEpoch`，避免重复调用 `register_reader()`
- 通过 `new_with_threshold()` 调整回收阈值以适应工作负载特性

### 线程安全

当 `T: 'static` 时，`Swapper<T>` 和 `SwapReader<T>` 都实现了 `Send + Sync`，允许在线程间安全共享。`LocalEpoch` 是 `!Sync` 的，防止意外的跨线程使用。

## 限制

- **不支持 `no_std`**: 需要 `std` 用于线程同步
- **仅支持单写**: 类型系统通过 `Swapper` 不可 `Clone` 来强制执行此限制
- **基于 Epoch 的回收**: 写入延迟取决于 Epoch 推进（通常为微秒级）
- **显式 LocalEpoch 管理**: 用户必须调用 `register_reader()` 并将 `LocalEpoch` 传递给读取操作

## 与其他方案对比

### vs. `arc-swap`
- **优势**: 读取性能快 90%，写入性能快 32%，持有守卫时写入快 90%
- **劣势**: 写入操作需要触发 GC，API 需要显式管理 LocalEpoch

### vs. `RwLock<T>`
- **优势**: 读操作 Wait-Free，无竞争，适合读密集型工作负载
- **劣势**: 仅支持单个写入者

### vs. `Mutex<T>`
- **优势**: 读操作 Wait-Free，无阻塞，性能更好
- **劣势**: 仅支持单个写入者

## 安全性

所有不安全代码都经过仔细记录和论证：
- 指针解引用由 Epoch Pin 保护
- 内存仅在守卫持有期间访问
- 延迟销毁确保不会发生 Use-After-Free

## 测试

运行测试：
```bash
cargo test
```

运行基准测试：
```bash
cargo bench
```

## 许可证

在 Apache License 2.0 或 MIT 许可证下双重许可，任选其一。

## 贡献

欢迎贡献！请确保所有测试通过且基准测试结果稳定后再提交。

## 基准测试详情

### 测试场景

基准测试涵盖了单写多读系统的典型工作负载：

1. **单线程读取**: 单个线程连续读取，测试纯读取性能
2. **单线程写入**: 单个线程连续写入，测试写入开销
3. **多线程读取** (2/4/8 线程): 并发读取扩展性测试
4. **混合读写**: 1 个写入线程 + N 个读取线程，最现实的场景
5. **批量读取**: 单个 Pin 内多次读取，测试 `read_with_guard()` 优化
6. **持有守卫读取**: 读取者持有守卫期间的写入延迟
7. **内存压力**: 频繁写入导致的垃圾积累，测试 GC 开销

### 关键发现

**读取性能**:
- 通过 `EpochPtr` 和 `PinGuard` 机制，SMR-Swap 在读取路径上比 arc-swap 快 **90%**
- 单线程读取达到 **0.90 ns**，接近硬件极限
- 多线程读取保持一致的亚纳秒级延迟，无竞争

**写入性能**:
- 单线程写入比 arc-swap 快 **32%**（87.90 ns vs 130.23 ns）
- 得益于 `VecDeque` 垃圾管理和激进的 GC 回收
- 混合工作负载中写入延迟稳定（92-94 ns），立即触发 GC
- 激进的 GC 策略在 `update()` 中确保可预测的性能

**扩展性**:
- 随着读取者数量增加，性能保持稳定，无竞争
- 多线程读取在 2/4/8 线程下都保持 0.90-0.93 ns
- 混合读写场景中 SMR-Swap 比 arc-swap 快 **79-81%**
- 即使在多写多读场景（需 Mutex），性能仍比 arc-swap 快 **67-78%**

**守卫持有**:
- 读取者持有守卫时，SMR-Swap 的写入延迟远低于 arc-swap（89.91 ns vs 908.69 ns）
- **快 90%**，在这个关键场景中表现优异
- 对长时间读取访问的应用至关重要

**内存压力**:
- **改进**：SMR-Swap 现在比 arc-swap 快 **53%**（741.47 ns vs 1580 ns）
- 激进的垃圾回收在 `update()` 中防止垃圾积累
- 基于 Epoch 的回收在每次写入后立即触发
- 即使在高频写入下，读取性能依然保持稳定

## 使用场景

SMR-Swap 特别适合以下场景，其中读取性能至关重要且写入相对不频繁：

### 理想场景

- **配置热更新**: 单个配置管理者，多个服务读取配置
  - 优势：配置读取延迟 < 1 ns，无锁等待
  - 适用：微服务架构中的动态配置分发

- **缓存管理**: 单个缓存更新线程，多个查询线程
  - 优势：缓存查询极快（0.90 ns），扩展性好
  - 适用：高并发查询场景

- **路由表**: 单个路由表管理者，多个转发线程
  - 优势：路由查询无竞争，支持长期持有引用
  - 适用：网络包转发、负载均衡

- **特征标志**: 单个管理员，多个检查线程
  - 优势：特征检查极快，无阻塞
  - 适用：A/B 测试、灰度发布

- **性能关键的读取路径**: 需要最小化读取延迟的系统
  - 优势：亚纳秒级读取延迟，90% 快于 arc-swap
  - 适用：高频交易、实时数据处理

### 不太适合的场景

- **频繁写入**: 如果写入频率接近读取频率，垃圾回收开销会增加
  - 建议：使用 `new_with_threshold(None)` 禁用自动回收，手动控制
  
- 内存极度紧张的环境：垃圾积累可能导致 GC 暂停
  - 建议：调整 `new_with_threshold()` 为更小的值，或使用 arc-swap


## 实现细节

### LocalEpoch 和 Pin 机制

- 每个读取者通过 `register_reader()` 获得一个 `LocalEpoch`（每个线程一次）
- `LocalEpoch` 包含：
  - `Arc<ReaderSlot>`：共享槽位追踪该读取者的活跃 Epoch
  - `Arc<SharedState>`：对全局状态的引用（Epoch 计数器和读取者列表）
  - `Cell<usize>`：线程本地 `pin_count` 用于可重入追踪
- 当调用 `read()` 并传递 `LocalEpoch` 时，它调用 `local_epoch.pin()`：
  - 若 `pin_count == 0`：加载当前全局 Epoch 并存储在 `ReaderSlot`
  - 增加 `pin_count` 并返回 `PinGuard`
  - 支持可重入：多个嵌套 pin 增加计数器
- 当 `PinGuard` 被 drop 时：
  - 递减 `pin_count`
  - 若 `pin_count` 达到零：标记线程为 `INACTIVE_EPOCH`（usize::MAX）

### 原子操作

- 使用 `EpochPtr<T>`（来自 `swmr-epoch`）进行原子指针管理
- `EpochPtr::load(&guard)` 安全地解引用指针，生命周期绑定到 guard
- `EpochPtr::store(new_value, &mut gc)` 原子地交换指针并退休旧值
- 使用 `Ordering::Acquire` 用于加载，`Ordering::Release` 用于存储以确保内存顺序

### 守卫机制

### 守卫机制

- `PinGuard<'a>` 保持 Epoch pin 状态
- `read` 返回 `&'a T`，其生命周期绑定到 `PinGuard`
- 确保值不能在守卫被 drop 后被访问
- `PinGuard` 支持 `Clone` 用于嵌套 pinning（增加 `pin_count`）

### 垃圾回收管道

1. **退休阶段**：当 Writer 调用 `store()` 时，旧值被包装在 `RetiredObject` 中并添加到垃圾桶
2. **积累**：垃圾按 Epoch 分组在 `VecDeque<(usize, Vec<RetiredObject>)>` 中
3. **自动触发**：每次 `retire()` 后，若总垃圾 > 阈值，自动调用 `collect()`
4. **回收阶段**：
   - 推进全局 Epoch
   - 扫描所有活跃读取者找出最小活跃 Epoch
   - 计算安全回收点（min_active_epoch - 1）
   - 从 deque 前端弹出垃圾直到达到安全点
   - 被 drop 的 `RetiredObject` 自动调用其析构函数
5. **清理**：死读取者槽位（通过 `Weak` 引用）在回收期间被清理

### 值的生命周期

- Writer 调用 `update()` 或 `swap()` 替换当前值
- 旧值立即被包装在 `RetiredObject` 中并存储在当前 Epoch 的垃圾桶中
- Writer 可选地调用 `gc.collect()` 来触发垃圾回收
- 当所有读取者离开该 Epoch 时，垃圾被安全回收并调用析构函数
- 这确保了不会发生 Use-After-Free，同时最小化了同步开销
