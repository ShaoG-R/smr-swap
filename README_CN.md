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
- **零拷贝读取**: 读取者通过 RAII 守卫直接获得当前值的引用
- **并发安全**: 在多线程中安全使用，支持 `Send + Sync` 约束

## 快速开始

### 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
smr-swap = "0.5"
```

### 基本用法

```rust
use smr_swap::SmrSwap;
use std::thread;

fn main() {
    // 创建一个新的 SMR 容器，初始值为 vec![1, 2, 3]
    let mut swap = SmrSwap::new(vec![1, 2, 3]);

    // 为新线程创建一个读取者（必须使用 fork()）
    let reader = swap.reader().fork();

    let handle = thread::spawn(move || {
        // 读取值（无锁）
        let guard = reader.load();
        println!("Reader sees: {:?}", *guard);
    });

    // 写入者更新值
    swap.update(vec![4, 5, 6]);
    
    // 主线程也可以读取
    println!("Main thread sees: {:?}", *swap.load());

    handle.join().unwrap();
}
```

### 使用 Arc 进行共享所有权

SMR-Swap 可以与任何类型 `T` 一起使用，你也可以将值包装在 `Arc` 中以实现共享所有权：

```rust
use smr_swap::SmrSwap;
use std::sync::Arc;

fn main() {
    let mut swap = SmrSwap::new(Arc::new(vec![1, 2, 3]));
    
    // 更新为新的 Arc
    swap.update(Arc::new(vec![4, 5, 6]));
    
    // 或者使用 swap() 取回旧值
    let old = swap.swap(Arc::new(vec![7, 8, 9]));
    println!("Old value: {:?}", old);
}
```

### 分离写入者和读取者

你可以将容器拆分为独立的组件：

```rust
use smr_swap::SmrSwap;

fn main() {
    // 方式 1: 先创建 SmrSwap，然后拆分
    let swap = SmrSwap::new(42);
    let (mut swapper, reader) = swap.into_components();
    
    // 方式 2: 直接创建组件
    let (mut swapper, reader) = smr_swap::new_smr_pair(42);
    
    // 将 `swapper` 传递给写入线程
    // 将 `reader` 传递给读取线程（每个线程使用 reader.fork()）
}
```

## API 概览

### 全局函数
- `new_smr_pair<T>(initial: T) -> (Swapper<T>, SwapReader<T>)`: 直接创建一对 Swapper 和 SwapReader。

### `SmrSwap<T>`
主入口点。持有写入者和读取者。
- `new(initial: T)`: 创建新容器。
- `update(new_value: T)`: 更新值。
- `load()`: 读取当前值。
- `reader()`: 获取读取者引用。
- `swapper()`: 获取写入者引用。
- `into_components()`: 拆分为 `Swapper` 和 `SwapReader`。

### `SwapReader<T>`
读取者组件。
- `fork()`: 为另一个线程创建新的读取者。**重要**: `SwapReader` 不实现 `Clone`，以强制使用 `fork` 进行显式的线程本地 Epoch 注册。
- `load()`: 返回指向当前值的 `ReaderGuard`。
- `map(|v| ...)`: 对值应用函数并返回结果。
- `filter(|v| ...)`: 条件性返回守卫。

### `Swapper<T>`
写入者组件。
- `update(new_value: T)`: 更新值。

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
smr-swap:  89.91 ns  ████
arc-swap:  908.69 ns ██████████████████
```
**赢家**: SMR-Swap (快 90%)
- 读取者持有守卫时开销极小
- 对需要长时间保持读取访问的应用至关重要

## 设计原理

### 类型系统保证

- **`Swapper<T>`**: 不可 `Clone`（通过 `Arc` 单一所有权强制）
  - 通过类型系统保证单个写入者
  - 可以包装在 `Arc` 中在线程间共享（但会破坏单写保证）

- **`SwapReader<T>`**: 不可 `Clone`（使用 `fork()` 替代）
  - 使用 `fork()` 为另一个线程创建新的读取者。
  - 每个读取者独立看到最新值。
  - 内部管理 `LocalEpoch` 注册。

### 内存管理

SMR-Swap 使用自定义的 `swmr-epoch` 库进行内存回收，针对单写多读场景进行了优化。

## 许可证

在 Apache License 2.0 或 MIT 许可证下双重许可，任选其一。
