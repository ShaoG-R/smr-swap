# SMR-Swap: 基于版本的单写多读交换容器

[![Crates.io](https://img.shields.io/crates/v/smr-swap)](https://crates.io/crates/smr-swap)
[![Documentation](https://docs.rs/smr-swap/badge.svg)](https://docs.rs/smr-swap)
[![License](https://img.shields.io/crates/l/smr-swap)](LICENSE-MIT)
![no_std compatible](https://img.shields.io/badge/no_std-compatible-success.svg)

一个高性能的 Rust 库，使用基于版本的内存回收机制，安全地在单个写入者和多个读取者之间共享可变数据。

[English](README.md) | [中文文档](README_CN.md)

## 特性

- **最小化锁设计**: 读取操作是 Wait-Free 的，写入操作仅在垃圾回收时需要同步
- **高性能**: 针对读写操作进行了优化
- **简洁 API**: 仅三个核心类型 `SmrSwap`、`LocalReader`、`ReadGuard`
- **内存安全**: 使用基于版本的回收机制（通过 `swmr-cell`）防止 Use-After-Free
- **零拷贝读取**: 读取者通过 RAII 守卫直接获得当前值的引用
- **no_std 兼容**: 支持 `no_std` 环境（需要 `alloc`）

## 快速开始

### 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
smr-swap = "0.8"
```

### no_std 用法

使用 `default-features = false` 并启用 `spin` feature（如果你需要 `swmr-cell` 中默认的基于自旋锁的 mutex 实现）：

```toml
[dependencies]
smr-swap = { version = "0.8", default-features = false, features = ["spin"] }
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

    // 写入者存储新值
    swap.store(1);

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

    // 写入者存储新值
    swap.store(vec![4, 5, 6]);

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

### 使用 `SmrReader` 共享读取者创建能力

如果需要将创建读取者的能力分发给多个线程（例如，在线程动态变化的线程池中），可以使用 `SmrReader`。与 `LocalReader` 不同，`SmrReader` 是 `Sync` 和 `Clone` 的。

```rust
use smr_swap::SmrSwap;
use std::thread;

let mut swap = SmrSwap::new(0);

// 创建一个可以共享的 SmrReader 工厂
let reader_factory = swap.reader();

for i in 0..3 {
    // 为每个线程克隆工厂
    let factory = reader_factory.clone();
    
    thread::spawn(move || {
        // 使用工厂在线程上创建 LocalReader
        let local = factory.local();
        
        // ... 使用 local reader ...
    });
}
```

## 核心概念

### 类型层次

| 类型 | 角色 | 关键方法 |
|------|------|--------|
| `SmrSwap<T>` | 主容器，持有数据和写入能力 | `new()`, `store()`, `get()`, `load()`, `local()`, `swap()` |
| `LocalReader<T>` | 线程本地读取句柄 | `load()`, `map()`, `filter()`, `is_pinned()`, `version()` |
| `SmrReader<T>` | 线程间共享的读取者工厂 | `local()` |
| `ReadGuard<'a, T>` | RAII 守卫，保护读取期间的数据 | `Deref`, `AsRef`, `version()` |

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
| `reader() -> SmrReader<T>` | 创建可共享的读取者工厂 |
| `store(new_value: T)` | 存储新值，旧值会被安全回收 |
| `get() -> &T` | 获取当前值的引用（仅写者，无需 pin） |
| `update(f: FnOnce(&T) -> T)` | 使用闭包更新值 |
| `load() -> ReadGuard<T>` | 使用内部句柄读取当前值 |
| `load_cloned() -> T` | 加载并克隆当前值（需要 `T: Clone`） |
| `swap(new_value: T) -> T` | 交换值并返回旧值（需要 `T: Clone`） |
| `update_and_fetch(f) -> ReadGuard<T>` | 应用闭包更新值并返回新值的守卫 |
| `fetch_and_update(f) -> ReadGuard<T>` | 应用闭包更新值并返回旧值的守卫 |
| `version() -> usize` | 获取当前全局版本 |
| `garbage_count() -> usize` | 获取等待回收的垃圾数量 |
| `previous() -> Option<&T>` | 获取上一个存储值的引用 |
| `collect()` | 手动触发垃圾回收 |

### `LocalReader<T>`

线程本地的读取句柄。

| 方法 | 描述 |
|------|------|
| `load() -> ReadGuard<T>` | 读取当前值，返回 RAII 守卫 |
| `load_cloned() -> T` | 加载并克隆当前值（需要 `T: Clone`） |
| `map<F, U>(f: F) -> U` | 对值应用函数并返回结果 |
| `filter<F>(f: F) -> Option<ReadGuard<T>>` | 条件性返回守卫 |
| `is_pinned() -> bool` | 检查此读者是否当前被 pin |
| `version() -> usize` | 获取当前全局版本 |
| `clone()` | 创建新的 `LocalReader` |
| `share() -> SmrReader<T>` | 创建可共享的读取者工厂 |
| `into_swmr() -> SmrReader<T>` | 转换为可共享的读取者工厂 |

### `SmrReader<T>`

可以跨线程共享的 `LocalReader` 工厂。

| 方法 | 描述 |
|------|------|
| `local() -> LocalReader<T>` | 为当前线程创建 `LocalReader` |
| `clone()` | 克隆工厂（`Sync` + `Clone`） |

### `ReadGuard<'a, T>`

RAII 守卫，实现 `Deref<Target = T>` 和 `AsRef<T>`，在守卫存活期间保护数据不被回收。

| 方法 | 描述 |
|------|------|
| `version() -> usize` | 获取此守卫被 pin 到的版本 |
| `cloned() -> T` | 克隆内部值并返回（需要 `T: Clone`） |
| `into_inner() -> T` | 消耗守卫并返回克隆的值（需要 `T: Clone`） |
| `clone()` | 克隆守卫（增加 pin 计数） |

### 标准 Trait 实现

| 类型 | Trait |
|------|-------|
| `SmrSwap<T>` | `Default` (要求 `T: Default`), `From<T>`, `Debug` (要求 `T: Debug`) |
| `LocalReader<T>` | `Clone`, `Send`, `Debug` |
| `SmrReader<T>` | `Clone`, `Sync`, `Send`, `Debug` |
| `ReadGuard<'a, T>` | `Deref`, `AsRef`, `Clone`, `Debug` (要求 `T: Debug`) |

## 性能对比

自 smr-swap v0.9.0 起，默认策略调整为**写优先（Write-Preferred）**。原先的**读优先（Read-Preferred）**策略现在通过 `read-preferred` feature 开启。

与 `arc-swap` 的基准测试对比结果（测试环境：Windows，Bench 模式，Intel Core i9-13900KS）。

### 基准测试总结

| 场景 | SMR-Swap (写优先) | SMR-Swap (读优先) | ArcSwap |
|------|-------------------|-------------------|---------|
| 单线程读取 | **4.49 ns** | **0.90 ns** | 9.19 ns |
| 单线程写入 | **54.84 ns** | 89.81 ns | 104.79 ns |
| 多线程读取 (2 线程) | **4.81 ns** | **0.90 ns** | 9.19 ns |
| 多线程读取 (4 线程) | **4.98 ns** | **0.92 ns** | 9.30 ns |
| 多线程读取 (8 线程) | **5.10 ns** | **0.94 ns** | 9.60 ns |
| 混合读写 (1写+2读) | **66.10 ns** | 86.01 ns | 428.14 ns |
| 混合读写 (1写+4读) | **71.87 ns** | 86.03 ns | 429.16 ns |
| 混合读写 (1写+8读) | **76.63 ns** | 86.75 ns | 502.69 ns |
| 批量读取 | **5.52 ns** | **1.62 ns** | 9.58 ns |
| 持有守卫时读取 | **55.43 ns** | 84.96 ns | 886.14 ns |
| 内存压力下读取 | **816.01 ns** | 781.29 ns | 1.62 µs |

### 单写者不同读写比例场景（1 Writer + 2 Readers）

| 读写比例 | SMR-Swap (写优先) | SMR-Swap (读优先) | RwLock | Mutex |
|----------|-------------------|-------------------|--------|-------|
| 100:1 | 572.00 ns | 251.49 ns | 5.60 µs | 6.08 µs |
| 10:1 | 144.41 ns | 101.09 ns | 634.45 ns | 713.28 ns |
| 1:1 | 66.59 ns | 87.26 ns | 129.66 ns | 127.39 ns |
| 1:10 | 567.56 ns | 857.83 ns | 238.46 ns | 218.87 ns |
| 1:100 | 5.55 µs | 8.52 µs | 1.05 µs | 970.69 ns |

### 多写多读场景（SMR-Swap 使用 Mutex 包装）

| 配置 | SMR-Swap (写优先) | SMR-Swap (读优先) | Mutex | ArcSwap |
|------|-------------------|-------------------|-------|---------|
| 4写+4读 | 506.46 ns | 2.03 µs | 497.52 ns | 1.93 µs |
| 4写+8读 | 516.46 ns | 2.10 µs | 818.02 ns | 2.24 µs |
| 4写+16读 | 516.20 ns | 2.04 µs | 1.26 µs | 2.93 µs |

### 操作开销

| 操作 | SMR-Swap (写优先) | SMR-Swap (读优先) | ArcSwap | Mutex |
|------|-------------------|-------------------|---------|-------|
| 创建 (Creation) | ~152 ns | ~159 ns | ~131 ns | ~49 ns |
| 销毁 (Drop) | ~81 ns | ~78 ns | ~108 ns | ~41 ns |
| 句柄克隆 (Handle Clone) | ~57 ns | ~57 ns | ~9 ns | ~9 ns |
| 本地状态检查 | ~0.18 ns | ~0.18 ns | N/A | N/A |

### 性能分析

- **写优先 (默认)**:
  - **均衡性能**: 在读取和写入性能之间取得更好的平衡。
  - **技术原理**: 使用**对称内存屏障** (Symmetric Memory Barriers)，读写操作均承担常规同步开销。
  - **快速多线程写入**: 在混合读写和多写者场景中显著更快（4写4读平均 ~500ns，而读优先为 ~2µs）。
  - **良好的读取性能**: 读取延迟 (~4.5ns) 高于读优先 (~0.9ns)，但仍比 ArcSwap (~9.2ns) 快约 2 倍。

- **读优先 (Feature)**:
  - **极致读取性能**: 亚纳秒级读取延迟 (~0.9ns)，非常适合读极多（>99% 读取）的负载。
  - **技术原理**: 使用**非对称内存屏障** (Asymmetric Memory Barriers)，将同步开销几乎全部转移到写入端。
  - **较慢的写入**: 由于需要检查读取者状态和重量级屏障，写入操作开销较大。
  - **多写者扩展性差**: 在多写者场景下竞争严重。

- **对比总结**:
  - **读取**: 读优先 > 写优先 > ArcSwap > Mutex
  - **写入**: Mutex > 写优先 > 读优先 > ArcSwap
  - **混合**: 写优先 > 读优先 > ArcSwap

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
