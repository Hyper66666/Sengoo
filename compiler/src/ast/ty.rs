//! 类型

use super::{Ident, Node, Path, Span};

/// 类型
#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

impl Type {
    pub fn new(kind: TypeKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// 创建简单类型
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

    /// 是否是单元类型
    pub fn is_unit(&self) -> bool {
        matches!(&self.kind, TypeKind::Tuple(types) if types.is_empty())
    }

    /// 是否是 never 类型
    pub fn is_never(&self) -> bool {
        matches!(self.kind, TypeKind::Never)
    }

    /// 是否是引用类型
    pub fn is_ref(&self) -> bool {
        matches!(self.kind, TypeKind::Ref { .. })
    }

    /// 是否是可变引用
    pub fn is_mut_ref(&self) -> bool {
        matches!(self.kind, TypeKind::Ref { is_mut: true, .. })
    }
}

impl Node for Type {
    fn span(&self) -> Span {
        self.span
    }
}

/// 类型种类
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// 简单路径类型 `Name` 或 `module::Name`
    Path(Path),

    /// 元组类型 `(Type1, Type2)`
    Tuple(Vec<Type>),

    /// 数组类型 `[Type; N]`
    Array(Box<Type>, u64),

    /// 切片类型 `[Type]`
    Slice(Box<Type>),

    /// 指针类型 `*mut Type` 或 `*const Type`
    Ptr { base: Box<Type>, is_mut: bool },

    /// 引用类型 `&mut Type` 或 `&Type`
    Ref { base: Box<Type>, is_mut: bool },

    /// 函数类型 `fn(Type1, Type2) -> ReturnType`
    Fn {
        params: Vec<Type>,
        ret: Option<Box<Type>>,
    },

    /// Never 类型 `!`
    Never,

    /// Infer 类型 `_`
    Infer,

    /// 动态类型 `dyn Trait`
    Dyn(Vec<TraitBound>),

    /// Impl 占位符类型 `impl Trait`
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

    /// 是否是简单约束（无参数）
    pub fn is_simple(&self) -> bool {
        self.params.is_empty()
    }
}

impl Node for TraitBound {
    fn span(&self) -> Span {
        self.path.span
    }
}

/// 预定义的基本类型名称
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
