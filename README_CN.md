# SMR-Swap: 基于版本的单写多读交换容器

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)

一个高性能的 Rust 库，使用基于版本的内存回收机制，安全地在单个写入者和多个读取者之间共享可变数据。

[English](README.md) | [中文文档](README_CN.md)

## 特性

- **最小化锁设计**: 读取操作是 Wait-Free 的，写入操作仅在垃圾回收时需要同步
- **高性能**: 针对读写操作进行了优化
- **简洁 API**: 仅三个核心类型 `SmrSwap`、`LocalReader`、`ReadGuard`
- **内存安全**: 使用基于版本的回收机制（通过 `swmr-cell`）防止 Use-After-Free
- **零拷贝读取**: 读取者通过 RAII 守卫直接获得当前值的引用

## 快速开始

### 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
smr-swap = "0.7"
```

### 基本用法

```rust
use smr_swap::SmrSwap;
use std::thread;

fn main() {
    // 创建一个新的 SMR 容器
    let mut swap = SmrSwap::new(0);

    // 为当前线程获取本地读取句柄
    let local = swap.local();

    // 写入者更新值
    swap.update(1);

    // 在另一个线程中读取
    let local2 = swap.local();
    let handle = thread::spawn(move || {
        let guard = local2.load();
        assert_eq!(*guard, 1);
    });

    handle.join().unwrap();
}
```

### 多线程读取

```rust
use smr_swap::SmrSwap;
use std::thread;

fn main() {
    let mut swap = SmrSwap::new(vec![1, 2, 3]);
    
    // 为每个线程创建独立的 LocalReader
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

    // 写入者更新值
    swap.update(vec![4, 5, 6]);

    for handle in handles {
        handle.join().unwrap();
    }
}
```

### 使用闭包操作

```rust
use smr_swap::SmrSwap;

fn main() {
    let mut swap = SmrSwap::new(vec![1, 2, 3]);
    let local = swap.local();

    // 使用 map 转换值
    let sum: i32 = local.map(|v| v.iter().sum());
    println!("Sum: {}", sum);

    // 使用 filter 条件性获取守卫
    if let Some(guard) = local.filter(|v| v.len() > 2) {
        println!("Vector has more than 2 elements: {:?}", *guard);
    }

    // 使用 update_and_fetch 更新并获取新值
    let new_guard = swap.update_and_fetch(|v| {
        let mut new_vec = v.clone();
        new_vec.push(4);
        new_vec
    });
    println!("New value: {:?}", *new_guard);
}
```

## 核心概念

### 类型层次

| 类型 | 角色 | 关键方法 |
|------|------|--------|
| `SmrSwap<T>` | 主容器，持有数据和写入能力 | `new()`, `update()`, `load()`, `local()`, `swap()` |
| `LocalReader<T>` | 线程本地读取句柄 | `load()`, `map()`, `filter()` |
| `ReadGuard<'a, T>` | RAII 守卫，保护读取期间的数据 | `Deref` |

```
SmrSwap  ──local()──►  LocalReader  ──load()──►  ReadGuard
 (主容器)              (线程本地)                (RAII 守卫)
```

### 为什么需要 LocalReader？

`LocalReader` 是线程本地的读取句柄：
- 每个线程应该创建自己的 `LocalReader` 并重复使用
- `LocalReader` 是 `Send` 但不是 `Sync`，不应在线程间共享
- 包含线程本地的版本追踪信息，用于安全的内存回收

## API 概览

### `SmrSwap<T>`

主入口点，持有数据和写入能力。

| 方法 | 描述 |
|------|------|
| `new(initial: T)` | 创建新容器 |
| `local() -> LocalReader<T>` | 创建线程本地的读取句柄 |
| `update(new_value: T)` | 更新值，旧值会被安全回收 |
| `load() -> ReadGuard<T>` | 使用内部句柄读取当前值 |
| `swap(new_value: T) -> T` | 交换值并返回旧值（需要 `T: Clone`） |
| `update_and_fetch(f) -> ReadGuard<T>` | 应用闭包更新值并返回新值的守卫 |
| `collect()` | 手动触发垃圾回收 |

### `LocalReader<T>`

线程本地的读取句柄。

| 方法 | 描述 |
|------|------|
| `load() -> ReadGuard<T>` | 读取当前值，返回 RAII 守卫 |
| `map<F, U>(f: F) -> U` | 对值应用函数并返回结果 |
| `filter<F>(f: F) -> Option<ReadGuard<T>>` | 条件性返回守卫 |
| `clone()` | 创建新的 `LocalReader` |

### `ReadGuard<'a, T>`

RAII 守卫，实现 `Deref<Target = T>`，在守卫存活期间保护数据不被回收。

## 性能对比

与 `arc-swap` 的基准测试对比结果（测试环境：Windows，Bench 模式，Intel Core i9-13900KS）。

### 基准测试总结

| 场景 | SMR-Swap | ArcSwap | 性能提升 |
|------|----------|---------|---------|
| 单线程读取 | 0.91 ns | 9.15 ns | **快 90%** |
| 单线程写入 | 108.81 ns | 131.43 ns | **快 17%** |
| 多线程读取 (2 线程) | 0.90 ns | 9.36 ns | **快 90%** |
| 多线程读取 (4 线程) | 0.90 ns | 9.30 ns | **快 90%** |
| 多线程读取 (8 线程) | 0.96 ns | 9.72 ns | **快 90%** |
| 混合读写 (1写+2读) | 108.16 ns | 451.72 ns | **快 76%** |
| 混合读写 (1写+4读) | 110.58 ns | 453.31 ns | **快 76%** |
| 混合读写 (1写+8读) | 104.38 ns | 528.70 ns | **快 80%** |
| 批量读取 | 1.64 ns | 9.92 ns | **快 83%** |
| 持有守卫时读取 | 102.25 ns | 964.82 ns | **快 89%** |
| 内存压力下读取 | 825.30 ns | 1.70 µs | **快 51%** |

### 多写多读场景（SMR-Swap 使用 Mutex 包装）

| 配置 | SMR-Swap | Mutex | ArcSwap | 说明 |
|------|----------|-------|---------|------|
| 4写+4读 | 1.78 µs | 1.10 µs | 1.88 µs | SMR 比 ArcSwap 快 5% |
| 4写+8读 | 1.73 µs | 1.33 µs | 2.22 µs | SMR 比 ArcSwap 快 22% |
| 4写+16读 | 1.71 µs | 1.76 µs | 3.07 µs | SMR 比 ArcSwap 快 44% |

### 性能分析

- **读取性能卓越**：单线程和多线程读取都保持亚纳秒级延迟（~0.9 ns），比 ArcSwap 快约 10 倍
- **线性扩展**：多线程读取性能几乎不随线程数增加而下降
- **混合负载稳定**：在 1 写 + N 读场景下，SMR-Swap 保持稳定的 ~105 ns 延迟
- **多写场景**：虽然需要 Mutex 包装，但在高读取者数量时仍优于 ArcSwap
- **内存压力表现良好**：激进的 GC 策略确保内存压力下性能稳定

## 设计原理

### 单写多读模式

- **`SmrSwap<T>`** 持有写入能力，不可 `Clone`
  - 通过所有权系统保证单个写入者
  - 如需多写入者，可包装在 `Mutex<SmrSwap<T>>` 中

- **`LocalReader<T>`** 是 `Send` 但不是 `Sync`
  - 包含线程本地的版本信息
  - 每个线程应有自己的 `LocalReader`

### 内存管理

SMR-Swap 使用 `swmr-cell` 库进行基于版本的内存回收：
- 写入时自动将旧值加入待回收队列
- 当没有读取者引用旧值时，自动回收内存
- 可通过 `collect()` 手动触发回收

## 许可证

本项目采用以下任一许可证授权：

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

由你选择。
