//! 类型定义

use super::{Node, Path, Span};

/// 类型节点
#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

impl Type {
    pub fn new(kind: TypeKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// 创建简单路径类型
    pub fn simple(name: impl Into<String>, span: Span) -> Self {
        Self::new(TypeKind::Path(Path::from_str(name, span)), span)
    }

    /// 创建路径类型
    pub fn path(path: Path) -> Self {
        let span = path.span();
        Self::new(TypeKind::Path(path), span)
    }

    /// 创建元组类型
    pub fn tuple(types: Vec<Type>, span: Span) -> Self {
        Self::new(TypeKind::Tuple(types), span)
    }

    /// 创建数组类型
    pub fn array(elem: Type, len: u64, span: Span) -> Self {
        Self::new(TypeKind::Array(Box::new(elem), len), span)
    }

    /// 创建切片类型
    pub fn slice(elem: Type, span: Span) -> Self {
        Self::new(TypeKind::Slice(Box::new(elem)), span)
    }

    /// 创建指针类型
    pub fn ptr(elem: Type, is_mut: bool, span: Span) -> Self {
        Self::new(
            TypeKind::Ptr {
                base: Box::new(elem),
                is_mut,
            },
            span,
        )
    }

    /// 创建引用类型
    pub fn ref_(elem: Type, is_mut: bool, span: Span) -> Self {
        Self::new(
            TypeKind::Ref {
                base: Box::new(elem),
                is_mut,
            },
            span,
        )
    }

    /// 创建函数类型
    pub fn fn_(params: Vec<Type>, ret: Option<Box<Type>>, span: Span) -> Self {
        Self::new(TypeKind::Fn { params, ret }, span)
    }

    /// 创建 never 类型
    pub fn never(span: Span) -> Self {
        Self::new(TypeKind::Never, span)
    }

    /// 创建单元类型
    pub fn unit(span: Span) -> Self {
        Self::new(TypeKind::Tuple(Vec::new()), span)
    }

    /// 判断是否是单元类型
    pub fn is_unit(&self) -> bool {
        matches!(&self.kind, TypeKind::Tuple(types) if types.is_empty())
    }

    /// 判断是否是 never 类型
    pub fn is_never(&self) -> bool {
        matches!(self.kind, TypeKind::Never)
    }

    /// 判断是否是引用类型
    pub fn is_ref(&self) -> bool {
        matches!(self.kind, TypeKind::Ref { .. })
    }

    /// 判断是否是可变引用类型
    pub fn is_mut_ref(&self) -> bool {
        matches!(self.kind, TypeKind::Ref { is_mut: true, .. })
    }
}

impl Node for Type {
    fn span(&self) -> Span {
        self.span
    }
}

/// 类型种类枚举
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// The implementing type inside a trait or impl declaration.
    SelfType,

    /// 简单路径类型，如  或
    Path(Path),

    /// 带泛型参数的路径类型，如
    PathWithArgs { path: Path, args: Vec<Type> },

    /// 元组类型，如
    Tuple(Vec<Type>),

    /// 数组类型，如
    Array(Box<Type>, u64),

    /// 切片类型，如
    Slice(Box<Type>),

    /// 裸指针类型，如  或
    Ptr { base: Box<Type>, is_mut: bool },

    /// 引用类型，如  或
    Ref { base: Box<Type>, is_mut: bool },

    /// 函数类型，如
    Fn {
        params: Vec<Type>,
        ret: Option<Box<Type>>,
    },

    /// Never 类型
    Never,

    /// 类型推断占位符
    Infer,

    /// 动态分发类型，如
    Dyn(Vec<TraitBound>),

    /// Impl Trait 类型，如
    ImplTrait(Vec<TraitBound>),
}

/// Trait 约束
#[derive(Debug, Clone, PartialEq)]
pub struct TraitBound {
    pub path: Path,
    pub params: Vec<Type>,
}

impl TraitBound {
    pub fn new(path: Path) -> Self {
        Self {
            path,
            params: Vec::new(),
        }
    }

    pub fn with_params(mut self, params: Vec<Type>) -> Self {
        self.params = params;
        self
    }

    /// 判断是否是简单约束（不含泛型参数）
    pub fn is_simple(&self) -> bool {
        self.params.is_empty()
    }
}

impl Node for TraitBound {
    fn span(&self) -> Span {
        self.path.span
    }
}

/// 内置基本类型名称常量
#[allow(dead_code)]
pub mod builtin {
    pub const BOOL: &str = "bool";
    pub const CHAR: &str = "char";
    pub const STR: &str = "str";
    pub const INT: &str = "int";
    pub const INT8: &str = "i8";
    pub const INT16: &str = "i16";
    pub const INT32: &str = "i32";
    pub const INT64: &str = "i64";
    pub const INT128: &str = "i128";
    pub const UINT: &str = "uint";
    pub const UINT8: &str = "u8";
    pub const UINT16: &str = "u16";
    pub const UINT32: &str = "u32";
    pub const UINT64: &str = "u64";
    pub const UINT128: &str = "u128";
    pub const FLOAT32: &str = "f32";
    pub const FLOAT64: &str = "f64";
    pub const BYTES: &str = "bytes";
    pub const SELF: &str = "Self";
    pub const SELF_LOWER: &str = "self";
}
