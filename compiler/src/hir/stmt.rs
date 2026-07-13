//! HIR 语句定义。

use super::{HIRExpr, HIRType};
use crate::symbol::SymbolId;

/// HIR 语句。
#[derive(Debug, Clone)]
pub enum HIRStmt {
    /// Source statement boundary retained for MIR diagnostics and debug info.
    Source { site_lo: u32 },

    /// Coverage-only statement boundary. MIR lowering ignores this marker
    /// unless a coverage context is explicitly enabled by the toolchain.
    Coverage { site_lo: u32 },

    /// Let 绑定语句。
    Let {
        name: String,
        symbol: SymbolId,
        ty: HIRType,
        value: Option<HIRExpr>,
        is_mut: bool,
    },

    /// 表达式语句。
    Expr(HIRExpr),

    /// 嵌套条目声明占位符。
    Item,
}
