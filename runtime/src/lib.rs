//! # Sengoo Runtime
//!
//! Sengoo 语言的运行时库。
//!
//! ## 模块组织
//!
//! - [`value`] - 值表示（待实现）
//! - [`memory`] - 内存管理（待实现）
//! - [`python`] - Python 互操作（可选，待实现）

pub mod error;

// TODO: 逐步添加模块
// pub mod value;
// pub mod memory;

#[cfg(feature = "python")]
pub mod python;

pub use error::{Result, RuntimeError};

/// 运行时版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
