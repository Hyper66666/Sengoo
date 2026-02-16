//! 语句

use super::{Block, Expr, Ident, Node, Span, Type, Visibility};

/// 语句
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// 创建 let 语句
    pub fn let_stmt(name: Ident, ty: Option<Type>, value: Option<Expr>, span: Span) -> Self {
        Self::new(
            StmtKind::Let {
                name,
                ty,
                value: value.map(Box::new),
            },
            span,
        )
    }

    /// 创建 const 语句
    pub fn const_stmt(name: Ident, ty: Type, value: Expr, span: Span) -> Self {
        Self::new(
            StmtKind::Const {
                name,
                ty,
                value: Box::new(value),
            },
            span,
        )
    }

    /// 创建表达式语句
    pub fn expr(expr: Expr) -> Self {
        let span = expr.span();
        Self::new(StmtKind::Expr(Box::new(expr)), span)
    }

    /// 创建项声明语句
    pub fn item(decl: super::Decl) -> Self {
        let span = decl.span();
        Self::new(StmtKind::Item(Box::new(decl)), span)
    }

    /// 是否以分号结尾（不产生值）
    pub fn has_semi(&self) -> bool {
        !matches!(self.kind, StmtKind::Expr(_) | StmtKind::Item(_))
    }
}

impl Node for Stmt {
    fn span(&self) -> Span {
        self.span
    }
}

/// 语句类型
#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// Let 绑定 `let x: Type = expr;`
    Let {
        name: Ident,
        ty: Option<Type>,
        value: Option<Box<Expr>>,
    },

    /// Const 绑定 `const X: Type = expr;`
    Const {
        name: Ident,
        ty: Type,
        value: Box<Expr>,
    },

    /// 表达式语句（可能以分号结尾）
    Expr(Box<Expr>),

    /// 项声明（函数、结构体等）
    Item(Box<super::Decl>),
}

/// 局部变量定义（用于块内的 let 和 const）
#[derive(Debug, Clone, PartialEq)]
pub struct Local {
    pub is_const: bool,
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: Option<Expr>,
    pub span: Span,
}

impl Local {
    /// 创建 let 绑定
    pub fn let_(name: Ident, ty: Option<Type>, value: Option<Expr>, span: Span) -> Self {
        Self {
            is_const: false,
            name,
            ty,
            value,
            span,
        }
    }

    /// 创建 const 绑定
    pub fn const_(name: Ident, ty: Type, value: Expr, span: Span) -> Self {
        Self {
            is_const: true,
            name,
            ty: Some(ty),
            value: Some(value),
            span,
        }
    }

    /// 是否是 const
    pub fn is_const(&self) -> bool {
        self.is_const
    }

    /// 是否有显式类型注解
    pub fn has_type(&self) -> bool {
        self.ty.is_some()
    }

    /// 是否有初始值
    pub fn has_value(&self) -> bool {
        self.value.is_some()
    }
}

impl Node for Local {
    fn span(&self) -> Span {
        self.span
    }
}
