//! 抽象语法树 (AST)
//!
//! 定义 Sengoo 语言的所有语法节点。

mod decl;
mod expr;
mod op;
mod param;
mod stmt;
mod ty;

pub use decl::{
    Class, ClassMember, Const, Decl, DeclKind, Enum, EnumVariant, Function, Impl, Import,
    ImportKind, Module, Static, Struct, StructField, Trait, TraitItem, TypeAlias, TypeParam,
    VariantField,
};
pub use expr::{Expr, ExprKind};
pub use op::{AssignOp, BinOp, UnOp};
pub use param::{Param, SelfParam};
pub use stmt::{Stmt, StmtKind};
pub use ty::{TraitBound, Type, TypeKind};

use crate::lexer::Span;
use crate::symbol::SymbolId;

/// AST 节点的通用 trait
pub trait Node {
    /// 获取节点的源代码位置
    fn span(&self) -> Span;
}

/// 标识符
#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub symbol: SymbolId,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self::with_symbol(name, SymbolId::INVALID, span)
    }

    pub fn with_symbol(name: impl Into<String>, symbol: SymbolId, span: Span) -> Self {
        Self {
            name: name.into(),
            symbol,
            span,
        }
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.span == other.span
    }
}

impl Eq for Ident {}

impl Node for Ident {
    fn span(&self) -> Span {
        self.span
    }
}

/// 程序根节点
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub decls: Vec<Decl>,
}

impl Program {
    pub fn new() -> Self {
        Self { decls: Vec::new() }
    }
}

impl Node for Program {
    fn span(&self) -> Span {
        if self.decls.is_empty() {
            return Span::new(0, 0);
        }
        Span::new(
            self.decls.first().unwrap().span().lo,
            self.decls.last().unwrap().span().hi,
        )
    }
}

/// 可见性修饰符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// 公共 (`pub`)
    Public,
    /// 私有 (`priv` 或无修饰符)
    Private,
}

impl Visibility {
    pub fn is_public(self) -> bool {
        matches!(self, Self::Public)
    }
}

/// 块 - 一系列语句
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

impl Block {
    pub fn new(stmts: Vec<Stmt>, span: Span) -> Self {
        Self { stmts, span }
    }
}

impl Node for Block {
    fn span(&self) -> Span {
        self.span
    }
}

/// 字面量
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// 整数
    Int(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 字符
    Char(char),
    /// 字节串
    Bytes(Vec<u8>),
    /// 布尔值
    Bool(bool),
    /// 空值
    Null,
    /// 单元值
    Unit,
}

impl Literal {
    pub fn span(&self) -> Span {
        // 字面量的 span 由创建它的 Token 决定
        Span::new(0, 0) // 占位符
    }

    pub fn int(value: i64) -> ExprKind {
        ExprKind::Literal(Literal::Int(value))
    }

    pub fn float(value: f64) -> ExprKind {
        ExprKind::Literal(Literal::Float(value))
    }

    pub fn string(value: impl Into<String>) -> ExprKind {
        ExprKind::Literal(Literal::String(value.into()))
    }

    pub fn char(value: char) -> ExprKind {
        ExprKind::Literal(Literal::Char(value))
    }

    pub fn bool(value: bool) -> ExprKind {
        ExprKind::Literal(Literal::Bool(value))
    }

    pub fn null() -> ExprKind {
        ExprKind::Literal(Literal::Null)
    }

    pub fn unit() -> ExprKind {
        ExprKind::Literal(Literal::Unit)
    }
}

/// 路径 (如 `std::collections::HashMap`)
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub segments: Vec<Ident>,
    pub span: Span,
}

impl Path {
    pub fn new(segments: Vec<Ident>, span: Span) -> Self {
        Self { segments, span }
    }

    /// 是否是简单路径（只有一个段）
    pub fn is_simple(&self) -> bool {
        self.segments.len() == 1
    }

    /// 获取简单路径的标识符
    pub fn as_simple(&self) -> Option<&Ident> {
        if self.is_simple() {
            self.segments.first()
        } else {
            None
        }
    }

    /// 从字符串创建简单路径
    pub fn from_str(name: impl Into<String>, span: Span) -> Self {
        Self {
            segments: vec![Ident::new(name, span)],
            span,
        }
    }
}

impl Node for Path {
    fn span(&self) -> Span {
        self.span
    }
}

/// 字段名（可以是标识符或字符串字面量）
#[derive(Debug, Clone, PartialEq)]
pub enum FieldName {
    Ident(Ident),
    String(String),
}

impl FieldName {
    pub fn span(&self) -> Span {
        match self {
            FieldName::Ident(ident) => ident.span,
            FieldName::String(_) => Span::new(0, 0),
        }
    }
}

/// 字段值
#[derive(Debug, Clone, PartialEq)]
pub struct FieldValue {
    pub name: FieldName,
    pub value: Expr,
    pub span: Span,
}

impl FieldValue {
    pub fn new(name: FieldName, value: Expr, span: Span) -> Self {
        Self { name, value, span }
    }

    /// 简写形式 (name 即为变量名)
    pub fn shorthand(ident: Ident, span: Span) -> Self {
        let name = FieldName::Ident(ident.clone());
        let value = Expr::new(ExprKind::Ident(ident.clone()), ident.span);
        Self { name, value, span }
    }
}

impl Node for FieldValue {
    fn span(&self) -> Span {
        self.span
    }
}

/// Arm of a match expression
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub patterns: Vec<super::pattern::Pattern>,
    pub guard: Option<Box<Expr>>,
    pub body: Expr,
    pub span: Span,
}

impl MatchArm {
    pub fn new(patterns: Vec<super::pattern::Pattern>, body: Expr, span: Span) -> Self {
        Self {
            patterns,
            guard: None,
            body,
            span,
        }
    }

    pub fn with_guard(mut self, guard: Expr) -> Self {
        self.guard = Some(Box::new(guard));
        self
    }
}

impl Node for MatchArm {
    fn span(&self) -> Span {
        self.span
    }
}

/// 模式定义
pub mod pattern {
    use super::{Ident, Node, Path, Span};

    /// 模式
    #[derive(Debug, Clone, PartialEq)]
    pub struct Pattern {
        pub kind: PatternKind,
        pub span: Span,
    }

    impl Pattern {
        pub fn new(kind: PatternKind, span: Span) -> Self {
            Self { kind, span }
        }
    }

    impl Node for Pattern {
        fn span(&self) -> Span {
            self.span
        }
    }

    /// 模式类型
    #[derive(Debug, Clone, PartialEq)]
    pub enum PatternKind {
        /// 通配符 `_`
        Wildcard,
        /// 字面量
        Literal(super::Literal),
        /// 标识符（变量绑定）
        Ident(Ident),
        /// 路径（枚举变体或结构体）
        Path(Path),
        /// 结构体模式 `Point { x, y }`
        Struct {
            path: Path,
            fields: Vec<StructPatternField>,
            rest: bool,
        },
        /// 元组结构体模式 `Some(x)`
        TupleStruct { path: Path, patterns: Vec<Pattern> },
        /// 元组模式 `(a, b, c)`
        Tuple(Vec<Pattern>),
        /// 切片模式 `[a, b, ..rest]`
        Slice(Vec<Pattern>, Option<Box<Pattern>>),
        /// 范围模式 `1..=100`
        Range(Box<Pattern>, Box<Pattern>, RangeEnd),
        /// 或模式 `A | B`
        Or(Vec<Pattern>),
    }

    /// 结构体模式字段
    #[derive(Debug, Clone, PartialEq)]
    pub struct StructPatternField {
        pub name: Ident,
        pub pattern: Pattern,
        pub shorthand: bool,
    }

    impl StructPatternField {
        pub fn new(name: Ident, pattern: Pattern, shorthand: bool) -> Self {
            Self {
                name,
                pattern,
                shorthand,
            }
        }

        /// 简写形式 `{ x }` 等价于 `{ x: x }`
        pub fn shorthand(ident: Ident) -> Self {
            let pattern = Pattern::new(PatternKind::Ident(ident.clone()), ident.span);
            Self {
                name: ident.clone(),
                pattern,
                shorthand: true,
            }
        }
    }

    /// 范围结束类型
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum RangeEnd {
        /// 包含 `..=`
        Inclusive,
        /// 不包含 `..`
        Exclusive,
        /// 半开 `..`
        HalfOpen,
    }
}
