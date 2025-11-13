//! SwapGuard behavior and trait implementation tests
//! 
//! Tests for SwapGuard's Deref, Clone, Debug, Display implementations
//! and guard lifetime management

use crate::new;

/// Test SwapGuard Deref implementation
/// 测试 SwapGuard Deref 实现
#[test]
fn test_guard_deref() {
    let (_swapper, reader) = new(42);
    let guard = reader.read();
    
    // Deref should allow direct access to the value
    // Deref 应该允许直接访问值
    assert_eq!(*guard, 42);
}

/// Test SwapGuard Deref with strings
/// 测试 SwapGuard Deref（字符串）
#[test]
fn test_guard_deref_string() {
    let (_swapper, reader) = new(String::from("hello"));
    let guard = reader.read();
    
    assert_eq!(*guard, "hello");
    assert_eq!(guard.len(), 5);
}

/// Test SwapGuard Deref with vectors
/// 测试 SwapGuard Deref（向量）
#[test]
fn test_guard_deref_vector() {
    let (_swapper, reader) = new(vec![1, 2, 3, 4, 5]);
    let guard = reader.read();
    
    assert_eq!(*guard, vec![1, 2, 3, 4, 5]);
    assert_eq!(guard.len(), 5);
    assert_eq!(guard[0], 1);
}

/// Test SwapGuard clone_value method
/// 测试 SwapGuard clone_value 方法
#[test]
fn test_guard_clone_value() {
    let (_swapper, reader) = new(vec![1, 2, 3]);
    let guard = reader.read();
    
    let cloned = guard.clone_value();
    assert_eq!(cloned, vec![1, 2, 3]);
    assert_eq!(cloned, *guard);
}

/// Test SwapGuard clone_value with strings
/// 测试 SwapGuard clone_value（字符串）
#[test]
fn test_guard_clone_value_string() {
    let (_swapper, reader) = new(String::from("test"));
    let guard = reader.read();
    
    let cloned = guard.clone_value();
    assert_eq!(cloned, "test");
    assert_eq!(cloned, *guard);
}

/// Test SwapGuard Debug implementation
/// 测试 SwapGuard Debug 实现
#[test]
fn test_guard_debug() {
    let (_swapper, reader) = new(42);
    let guard = reader.read();
    
    let debug_str = format!("{:?}", guard);
    assert!(debug_str.contains("SwapGuard"));
    assert!(debug_str.contains("42"));
}

/// Test SwapGuard Debug with strings
/// 测试 SwapGuard Debug（字符串）
#[test]
fn test_guard_debug_string() {
    let (_swapper, reader) = new(String::from("debug_test"));
    let guard = reader.read();
    
    let debug_str = format!("{:?}", guard);
    assert!(debug_str.contains("SwapGuard"));
    assert!(debug_str.contains("debug_test"));
}

/// Test SwapGuard Debug with vectors
/// 测试 SwapGuard Debug（向量）
#[test]
fn test_guard_debug_vector() {
    let (_swapper, reader) = new(vec![1, 2, 3]);
    let guard = reader.read();
    
    let debug_str = format!("{:?}", guard);
    assert!(debug_str.contains("SwapGuard"));
}

/// Test SwapGuard Display implementation
/// 测试 SwapGuard Display 实现
#[test]
fn test_guard_display() {
    let (_swapper, reader) = new(42);
    let guard = reader.read();
    
    let display_str = format!("{}", guard);
    assert_eq!(display_str, "42");
}

/// Test SwapGuard Display with strings
/// 测试 SwapGuard Display（字符串）
#[test]
fn test_guard_display_string() {
    let (_swapper, reader) = new(String::from("display_test"));
    let guard = reader.read();
    
    let display_str = format!("{}", guard);
    assert_eq!(display_str, "display_test");
}

/// Test SwapGuard Display with floats
/// 测试 SwapGuard Display（浮点数）
#[test]
fn test_guard_display_float() {
    let (_swapper, reader) = new(3.14);
    let guard = reader.read();
    
    let display_str = format!("{}", guard);
    assert!(display_str.contains("3.14"));
}

/// Test multiple guards from same reader
/// 测试来自同一读取者的多个 guard
#[test]
fn test_multiple_guards_same_reader() {
    let (_swapper, reader) = new(100);
    
    let guard1 = reader.read();
    let guard2 = reader.read();
    let guard3 = reader.read();
    
    assert_eq!(*guard1, 100);
    assert_eq!(*guard2, 100);
    assert_eq!(*guard3, 100);
}

/// Test guard lifetime across scope
/// 测试 guard 在作用域中的生命周期
#[test]
fn test_guard_lifetime_scope() {
    let (_swapper, reader) = new(String::from("scope_test"));
    
    {
        let guard = reader.read();
        assert_eq!(*guard, "scope_test");
    } // guard dropped here
    
    // Reader should still work after guard is dropped
    // guard 被 drop 后，reader 仍应工作
    let guard2 = reader.read();
    assert_eq!(*guard2, "scope_test");
}

/// Test guard with nested scopes
/// 测试 guard 与嵌套作用域
#[test]
fn test_guard_nested_scopes() {
    let (_swapper, reader) = new(vec![1, 2, 3]);
    
    {
        let guard1 = reader.read();
        assert_eq!(guard1.len(), 3);
        
        {
            let guard2 = reader.read();
            assert_eq!(guard2.len(), 3);
        }
        
        // guard1 should still be valid
        // guard1 仍应有效
        assert_eq!(guard1.len(), 3);
    }
}

/// Test guard with explicit drop
/// 测试 guard 与显式 drop
#[test]
fn test_guard_explicit_drop() {
    let (_swapper, reader) = new(42);
    
    let guard = reader.read();
    assert_eq!(*guard, 42);
    
    drop(guard);
    
    // Reader should still work after explicit drop
    // 显式 drop 后，reader 仍应工作
    let guard2 = reader.read();
    assert_eq!(*guard2, 42);
}

/// Test guard with method calls
/// 测试 guard 与方法调用
#[test]
fn test_guard_method_calls() {
    let (_swapper, reader) = new(String::from("method_test"));
    let guard = reader.read();
    
    // Should be able to call methods on the guarded value
    // 应该能够在受保护的值上调用方法
    assert_eq!(guard.len(), 11);
    assert!(guard.starts_with("method"));
    assert!(guard.ends_with("test"));
}

/// Test guard with vector operations
/// 测试 guard 与向量操作
#[test]
fn test_guard_vector_operations() {
    let (_swapper, reader) = new(vec![10, 20, 30, 40, 50]);
    let guard = reader.read();
    
    assert_eq!(guard.len(), 5);
    assert_eq!(guard.first(), Some(&10));
    assert_eq!(guard.last(), Some(&50));
    assert!(guard.contains(&30));
}

/// Test guard comparison operations
/// 测试 guard 比较操作
#[test]
fn test_guard_comparison() {
    let (_swapper, reader) = new(42);
    let guard = reader.read();
    
    assert_eq!(*guard, 42);
    assert_ne!(*guard, 41);
    assert!(*guard > 40);
    assert!(*guard < 50);
}

/// Test guard with custom struct
/// 测试 guard 与自定义结构体
#[test]
fn test_guard_custom_struct() {
    #[derive(Debug, Clone, PartialEq)]
    struct Point {
        x: i32,
        y: i32,
    }

    let (_swapper, reader) = new(Point { x: 10, y: 20 });
    let guard = reader.read();
    
    assert_eq!(guard.x, 10);
    assert_eq!(guard.y, 20);
}

/// Test guard with tuple
/// 测试 guard 与元组
#[test]
fn test_guard_tuple() {
    let (_swapper, reader) = new((42, String::from("test")));
    let guard = reader.read();
    
    assert_eq!(guard.0, 42);
    assert_eq!(guard.1, "test");
}

/// Test guard with Option
/// 测试 guard 与 Option
#[test]
fn test_guard_option() {
    let (_swapper, reader) = new(Some(42));
    let guard = reader.read();
    
    assert_eq!(*guard, Some(42));
    assert!(guard.is_some());
    assert_eq!(guard.unwrap(), 42);
}

/// Test guard with Result
/// 测试 guard 与 Result
#[test]
fn test_guard_result() {
    let (_swapper, reader) = new(Ok::<i32, String>(42));
    let guard = reader.read();
    
    assert!(guard.is_ok());
    assert_eq!(guard.as_ref().unwrap(), &42);
}
