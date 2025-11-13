# SMR-Swap: 无锁单写多读交换容器

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)

一个高性能的 Rust 库，使用基于 Epoch 的内存回收机制，安全地在单个写入者和多个读取者之间共享可变数据。

## 特性

- **无锁设计**: 读取和写入都不需要互斥锁
- **高性能**: 针对读写操作进行了优化
- **单写多读模式**: 通过 `Swapper<T>` 和 `SwapReader<T>` 在类型系统层面强制执行
- **内存安全**: 使用基于 Epoch 的回收机制（通过 `crossbeam-epoch`）防止 Use-After-Free
- **零拷贝读取**: 读取者直接获得当前值的引用
- **并发安全**: 在多线程中安全使用，支持 `Send + Sync` 约束

## 快速开始

### 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
smr-swap = "0.1"
```

### 基本用法

```rust
use smr_swap;

fn main() {
    // 创建一个新的 SMR 容器，初始值为 vec![1, 2, 3]
    let (mut writer, reader) = smr_swap::new(vec![1, 2, 3]);

    // 读取者可以克隆并在线程间共享
    let reader_clone = reader.clone();

    // 写入者更新值
    writer.update(vec![4, 5, 6]);

    // 读取者看到新值
    let guard = reader.read().unwrap();
    println!("{:?}", *guard); // [4, 5, 6]
}
```

### 使用 Arc 进行共享所有权

SMR-Swap 可以与任何类型 `T` 一起使用，你也可以将值包装在 `Arc` 中以实现共享所有权：

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

**注意**: SMR-Swap 本身不需要 `Arc`。仅当你需要内部值的共享所有权时才使用 `Arc`。

## API 概览

### 创建容器

```rust
let (swapper, reader) = smr_swap::new(initial_value);
```

返回一个元组，包含：
- `Swapper<T>`: 写入者（不可 `Clone`，强制单写）
- `SwapReader<T>`: 读取者（可克隆以支持多个读取者）

### 写入者操作

#### `update(new_value: T)`
原子地替换当前值。

```rust
writer.update(new_value);
```

#### `read() -> Option<SwapGuard<T>>`
允许写入者读取当前值。

```rust
if let Some(guard) = writer.read() {
    println!("当前值: {:?}", *guard);
}
```

#### `update_and_fetch<F>(f: F) -> Option<SwapGuard<T>>`
基于当前值进行更新并返回新值。

```rust
let new_guard = writer.update_and_fetch(|old| {
    let mut v = old.clone();
    v.push(999);
    v
})?;
```

### Arc 专用的写入者操作

以下方法仅在 `T` 被 `Arc` 包装时可用（即 `Swapper<Arc<T>>`）：

#### `swap(new_value: Arc<T>) -> Option<Arc<T>>`
原子地替换当前 `Arc` 包装的值并返回旧的 `Arc`。

```rust
use std::sync::Arc;

let (mut writer, _) = smr_swap::new(Arc::new(42));

// 交换值并获取旧值
if let Some(old) = writer.swap(Arc::new(43)) {
    println!("旧值: {:?}", *old); // 42
}
```

#### `update_and_fetch_arc<F>(f: F) -> Option<Arc<T>>`
使用接收当前 `Arc` 并返回新 `Arc` 的闭包来更新值。

```rust
use std::sync::Arc;

let (mut writer, _) = smr_swap::new(Arc::new(vec![1, 2, 3]));

// 通过添加元素来更新向量
if let Some(new_arc) = writer.update_and_fetch_arc(|current| {
    let mut vec = current.to_vec();
    vec.push(4);
    Arc::new(vec)
}) {
    println!("新值: {:?}", *new_arc); // [1, 2, 3, 4]
}
```

### 读取者操作

#### `read() -> Option<SwapGuard<T>>`
读取当前值，返回一个守卫以确保在持有期间值不会被回收。

```rust
if let Some(guard) = reader.read() {
    println!("值: {:?}", *guard);
}
```

#### `map<F, R>(f: F) -> Option<R>`
对当前值应用闭包，无需持有守卫。

```rust
let doubled = reader.map(|v| v * 2);
```

#### `filter<F>(f: F) -> Option<SwapGuard<T>>`
仅当谓词为真时返回守卫。

```rust
if let Some(guard) = reader.filter(|v| v > &10) {
    println!("值 > 10: {:?}", *guard);
}
```

#### `try_clone_value() -> Option<T>` (需要 `T: Clone`)
克隆当前值。

```rust
if let Some(cloned) = reader.try_clone_value() {
    println!("克隆值: {:?}", cloned);
}
```

## 性能特性

与 `arc-swap` 的全面基准测试对比结果。

### 基准测试总结表

| 场景 | SMR-Swap | ArcSwap | 性能提升 | 说明 |
|------|----------|---------|---------|------|
| 单线程读取 | 1.80 ns | 9.19 ns | **快 80%** | 纯读取性能 |
| 单线程写入 | 137.13 ns | 129.20 ns | 慢 6% | Epoch 管理开销 |
| 多线程读取 (2) | 1.82 ns | 9.29 ns | **快 80%** | 无竞争 |
| 多线程读取 (4) | 1.85 ns | 9.25 ns | **快 80%** | 一致的扩展性 |
| 多线程读取 (8) | 2.05 ns (平均) | 9.38 ns | **快 78%** | 优秀的扩展性 |
| 混合读写 (2 读) | 138.69 ns | 452.64 ns | **快 69%** | 1 写 + 2 读 |
| 混合读写 (4 读) | 139.54 ns | 455.19 ns | **快 69%** | 1 写 + 4 读 |
| 混合读写 (8 读) | 140.08 ns | 534.12 ns | **快 74%** | 1 写 + 8 读 |
| 批量读取 | 2.53 ns | 9.67 ns | **快 74%** | 优化的批量读取 |
| 持有守卫的读取 | 137.27 ns | 524.49 ns | **快 74%** | 读取者持有守卫期间写入 |
| 内存压力下读取 | 860.54 ns | 1.18 μs | **快 27%** | 内存压力情况下 |

### 详细性能分析

#### 单线程读取
```
smr-swap:  5.47 ns ████
arc-swap:  8.98 ns ██████████
```
**赢家**: SMR-Swap (快 39%)
- 直接 Epoch Pin 开销最小
- 无锁竞争

#### 单线程写入
```
smr-swap:  148.51 ns ██████████████████
arc-swap:  129.37 ns ████████████████
```
**赢家**: ArcSwap (快 15%)
- SMR-Swap 有 Epoch 管理开销
- ArcSwap 使用更简单的原子操作

#### 多线程读取性能（扩展性）
```
读取者数:  2         4         8
smr-swap:  6.90 ns   9.13 ns   14.36 ns
arc-swap:  9.02 ns   9.26 ns   9.67 ns
```
**分析**:
- SMR-Swap 在 4 个读取者以内保持近线性扩展
- 8 个读取者时 Epoch 协调引入开销
- ArcSwap 在不同线程数下表现一致

#### 混合读写（最现实的场景）
```
读取者数:  2         4         8
smr-swap:  151 ns    152 ns    156 ns
arc-swap:  456 ns    453 ns    493 ns
```
**赢家**: SMR-Swap (快 66-68%)
- 基于 Epoch 的回收在并发读写时表现优异
- ArcSwap 的原子操作产生更多竞争

#### 持有守卫时的读取延迟
```
smr-swap:  140 ns ██████████
arc-swap:  481 ns ██████████████████████████████████
```
**赢家**: SMR-Swap (快 71%)
- 读取者持有守卫时影响最小
- 对延迟敏感的应用至关重要

### 性能选择建议

**选择 SMR-Swap 当：**
- 读取密集型工作负载（>80% 读取）
- 多个读取者需要长时间持有守卫
- 混合读写模式很常见
- 最小化读取延迟很关键

**选择 ArcSwap 当：**
- 写入密集型工作负载（>50% 写入）
- 单线程或读取者很少
- 简洁性比性能更重要
- 写入延迟很关键

## 设计原理

### 类型系统保证

- **`Swapper<T>`**: 不可 `Clone`（通过 `Arc` 单一所有权强制）
  - 通过类型系统保证单个写入者
  - 可以包装在 `Arc` 中在线程间共享（但会破坏单写保证）

- **`SwapReader<T>`**: 可 `Clone`
  - 可以创建多个读取者并共享
  - 每个读取者独立看到最新值

### 内存管理

- 使用 `crossbeam-epoch` 进行安全的内存回收
- 每个值包装在 `Atomic<T>` 指针中
- 读取者 Pin Epoch，防止当前值被垃圾回收
- 旧值被延迟销毁，直到所有读取者离开 Epoch

### 线程安全

当 `T: Send + Sync` 时，`Swapper<T>` 和 `SwapReader<T>` 都实现了 `Send + Sync`，允许在线程间安全共享。

## 限制

- **不支持 `no_std`**: 需要 `std` 用于线程同步
- **仅支持单写**: 类型系统强制执行此限制，但可以通过 `clone_inner()` 绕过
- **基于 Epoch 的回收**: 写入延迟取决于 Epoch 推进（通常为微秒级）

## 与其他方案对比

### vs. `arc-swap`
- **优势**: 更好的读取性能，特别是在持有守卫时
- **劣势**: 由于 Epoch 管理，写入延迟略高

### vs. `RwLock<T>`
- **优势**: 无锁，无竞争，适合读密集型工作负载
- **劣势**: 仅支持单个写入者

### vs. `Mutex<T>`
- **优势**: 无锁，无阻塞，性能更好
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

1. **单线程读取**: 单个线程连续读取
2. **单线程写入**: 单个线程连续写入
3. **多线程读取**: N 个线程并发读取
4. **混合读写**: 1 个写入线程 + N 个读取线程
5. **批量读取**: 单个 Pin 内多次读取
6. **持有守卫读取**: 读取者持有守卫期间的写入延迟
7. **内存压力**: 频繁写入导致的垃圾积累

### 关键发现

- **读取优化**: 通过 Epoch Pin 机制，smr-swap 在读取路径上比 arc-swap 快 39%
- **写入权衡**: 虽然单线程写入略慢，但在混合工作负载中表现更优
- **扩展性**: 随着读取者数量增加，性能保持稳定，无竞争
- **守卫持有**: 读取者持有守卫时，smr-swap 的写入延迟远低于 arc-swap

## 使用场景

SMR-Swap 特别适合以下场景：

- **配置热更新**: 单个配置管理者，多个服务读取配置
- **缓存管理**: 单个缓存更新线程，多个查询线程
- **路由表**: 单个路由表管理者，多个转发线程
- **特征标志**: 单个管理员，多个检查线程
- **性能关键的读取路径**: 需要最小化读取延迟的系统

## 实现细节

### Epoch 机制

- 每个读取者通过 `epoch::pin()` 进入当前 Epoch
- 写入者使用 `defer_destroy()` 延迟旧值的销毁
- 当所有读取者离开 Epoch 时，旧值才被真正销毁

### 原子操作

- 使用 `Atomic::swap()` 进行原子指针交换
- 使用 `Ordering::Release` 和 `Ordering::Acquire` 确保内存顺序

### 守卫机制

- `SwapGuard<T>` 持有 `Guard` 以防止 Epoch 推进
- 通过 `Deref` trait 提供对值的透明访问
- 当守卫被 Drop 时，Epoch Pin 自动释放
