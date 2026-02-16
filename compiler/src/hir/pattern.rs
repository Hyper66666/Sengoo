//! HIR 模式定义

use super::{HIRExpr, HIRLiteral, HIRType};

/// HIR 模式
#[derive(Debug, Clone)]
pub enum HIRPattern {
    /// 通配符 `_`
    Wild,

    /// 字面量模式
    Lit(HIRLiteral),

    /// 变量绑定
    Var { name: String, mutability: bool },

    /// 结构体模式
    Struct {
        name: String,
        fields: Vec<(String, Option<HIRPattern>)>,
    },

    /// 元组模式
    Tuple(Vec<HIRPattern>),

    /// 或模式 `p1 | p2`
    Or(Box<HIRPattern>, Box<HIRPattern>),

    /// 切片模式
    Slice {
        before: Vec<HIRPattern>,
        rest: Option<Box<HIRPattern>>,
        after: Vec<HIRPattern>,
    },

    /// 范围模式
    Range {
        start: Option<Box<HIRExpr>>,
        end: Option<Box<HIRExpr>>,
    },

    /// 引用模式
    Ref(Box<HIRPattern>),

    /// 可变引用模式
    RefMut(Box<HIRPattern>),
}

/// HIR match 分支
#[derive(Debug, Clone)]
pub struct HIRMatchArm {
    pub pat: HIRPattern,
    pub guard: Option<Box<HIRExpr>>,
    pub body: Box<HIRExpr>,
}

impl HIRMatchArm {
    pub fn new(pat: HIRPattern, body: HIRExpr) -> Self {
        Self {
            pat,
            guard: None,
            body: Box::new(body),
        }
    }

    pub fn with_guard(mut self, guard: HIRExpr) -> Self {
        self.guard = Some(Box::new(guard));
        self
    }
}
