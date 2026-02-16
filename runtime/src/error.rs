//! 运行时错误类型定义

use thiserror::Error;

/// 运行时结果类型
pub type Result<T> = std::result::Result<T, RuntimeError>;

/// 运行时错误
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// 除零错误
    #[error("除零错误")]
    DivisionByZero,

    /// 索引越界
    #[error("索引越界: 索引 {index} 超出范围 0..{len}")]
    IndexOutOfBounds { index: usize, len: usize },

    /// 键不存在
    #[error("键不存在: {0}")]
    KeyNotFound(String),

    /// 空值解引用
    #[error("尝试解引用空值")]
    NullDereference,

    /// 类型错误
    #[error("运行时类型错误: {0}")]
    TypeError(String),

    /// 栈溢出
    #[error("栈溢出")]
    StackOverflow,

    /// 堆溢出
    #[error("堆溢出: 无法分配 {size} 字节")]
    HeapOverflow { size: usize },

    /// Python 错误
    #[error("Python 错误: {0}")]
    PythonError(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
}
