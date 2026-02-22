//! HIR（高级中间表示 - High-Level Intermediate Representation）
//!
//! HIR 是类型检查后的 AST 简化版本，保留了高级语义。

mod body;
mod expr;
mod item;
mod lower;
mod pattern;
mod stmt;
mod ty;

pub use body::HIRBody;
pub use expr::{HIRBinaryOp, HIRExpr, HIRLiteral, HIRUnaryOp};
pub use item::{
    HIRConst, HIREnum, HIRExternBlock, HIRExternFunction, HIRExternItem, HIRExternStatic,
    HIRFunction, HIRImpl, HIRItem, HIRParam, HIRStatic, HIRStruct, HIRTrait, HIRTraitItem,
    HIRTypeParam, HIRTypeParamBound,
};
pub use lower::lower_ast;
pub use pattern::{HIRMatchArm, HIRPattern};
pub use stmt::HIRStmt;
pub use ty::{FloatKind, HIRType, HIRTypeKind, IntKind};

/// HIR 模块
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub items: Vec<HIRItem>,
}

impl Module {
    pub fn new(name: String) -> Self {
        Self {
            name,
            items: Vec::new(),
        }
    }

    pub fn add_item(&mut self, item: HIRItem) {
        self.items.push(item);
    }
}
