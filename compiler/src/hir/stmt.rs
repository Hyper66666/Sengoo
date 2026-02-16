//! HIR 语句定义

use super::{HIRExpr, HIRType};

/// HIR 语句
#[derive(Debug, Clone)]
pub enum HIRStmt {
    /// Let 绑定
    Let {
        name: String,
        ty: HIRType,
        value: Option<HIRExpr>,
        is_mut: bool,
    },

    /// 表达式语句
    Expr(HIRExpr),

    /// 项声明（嵌套的函数、结构体等）
    Item, // TODO: 具体的嵌套项类型
}
