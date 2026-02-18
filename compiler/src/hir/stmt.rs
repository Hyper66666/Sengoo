//! HIR 璇彞瀹氫箟

use super::{HIRExpr, HIRType};
use crate::symbol::SymbolId;

/// HIR 璇彞
#[derive(Debug, Clone)]
pub enum HIRStmt {
    /// Let 缁戝畾
    Let {
        name: String,
        symbol: SymbolId,
        ty: HIRType,
        value: Option<HIRExpr>,
        is_mut: bool,
    },

    /// 琛ㄨ揪寮忚鍙?
    Expr(HIRExpr),

    /// Nested item declaration placeholder.
    Item,
}
