//! HIR 类型定义

/// HIR 类型
///
/// HIR 类型是类型检查后的类型表示，所有类型变量已被解析。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HIRType {
    pub kind: HIRTypeKind,
}

impl HIRType {
    pub fn new(kind: HIRTypeKind) -> Self {
        Self { kind }
    }

    /// 单元类型 `()`
    pub fn unit() -> Self {
        Self::new(HIRTypeKind::Unit)
    }

    /// Never 类型 `!`
    pub fn never() -> Self {
        Self::new(HIRTypeKind::Never)
    }

    /// 布尔类型 `bool`
    pub fn bool() -> Self {
        Self::new(HIRTypeKind::Bool)
    }

    /// 字符类型 `char`
    pub fn char() -> Self {
        Self::new(HIRTypeKind::Char)
    }

    /// 字符串类型 `str`
    pub fn str() -> Self {
        Self::new(HIRTypeKind::Str)
    }

    /// 整数类型
    pub fn int(kind: IntKind) -> Self {
        Self::new(HIRTypeKind::Int(kind))
    }

    /// 浮点类型
    pub fn float(kind: FloatKind) -> Self {
        Self::new(HIRTypeKind::Float(kind))
    }

    /// 引用类型
    pub fn reference(mutability: bool, inner: HIRType) -> Self {
        Self::new(HIRTypeKind::Ref(mutability, Box::new(inner)))
    }

    /// 指针类型
    pub fn pointer(inner: HIRType) -> Self {
        Self::new(HIRTypeKind::Ptr(Box::new(inner)))
    }

    /// 数组类型 `[T; N]`
    pub fn array(elem: HIRType, len: usize) -> Self {
        Self::new(HIRTypeKind::Array(Box::new(elem), len))
    }

    /// 切片类型 `[T]`
    pub fn slice(elem: HIRType) -> Self {
        Self::new(HIRTypeKind::Slice(Box::new(elem)))
    }

    /// 元组类型
    pub fn tuple(types: Vec<HIRType>) -> Self {
        Self::new(HIRTypeKind::Tuple(types))
    }

    /// 函数类型
    pub fn function(params: Vec<HIRType>, ret: Box<HIRType>) -> Self {
        Self::new(HIRTypeKind::Fn { params, ret })
    }

    /// 命名类型（结构体、枚举等）
    pub fn named(name: String, args: Vec<HIRType>) -> Self {
        Self::new(HIRTypeKind::Named { name, args })
    }

    /// 是否为单元类型
    pub fn is_unit(&self) -> bool {
        matches!(self.kind, HIRTypeKind::Unit)
    }

    /// 是否为 Never 类型
    pub fn is_never(&self) -> bool {
        matches!(self.kind, HIRTypeKind::Never)
    }

    /// 是否为布尔类型
    pub fn is_bool(&self) -> bool {
        matches!(self.kind, HIRTypeKind::Bool)
    }

    /// 是否为数值类型
    pub fn is_numeric(&self) -> bool {
        matches!(self.kind, HIRTypeKind::Int(_) | HIRTypeKind::Float(_))
    }

    /// 是否为引用类型
    pub fn is_ref(&self) -> bool {
        matches!(self.kind, HIRTypeKind::Ref(..))
    }
}

/// HIR 类型种类
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HIRTypeKind {
    /// 单元类型 `()`
    Unit,
    /// Never 类型 `!`
    Never,
    /// 布尔类型 `bool`
    Bool,
    /// 字符类型 `char`
    Char,
    /// 字符串类型 `str`
    Str,
    /// 字节类型 `u8`
    Byte,
    /// 字节数组 `[u8]`
    Bytes,

    /// 整数类型
    Int(IntKind),
    /// 浮点类型
    Float(FloatKind),

    /// 元组类型 `(T1, T2, ...)`
    Tuple(Vec<HIRType>),
    /// 数组类型 `[T; N]`
    Array(Box<HIRType>, usize),
    /// 切片类型 `[T]`
    Slice(Box<HIRType>),

    /// 引用类型 `&T` 或 `&mut T`
    Ref(bool, Box<HIRType>),
    /// 指针类型 `*T`
    Ptr(Box<HIRType>),

    /// 函数类型 `fn(T1, T2, ...) -> T`
    Fn {
        params: Vec<HIRType>,
        ret: Box<HIRType>,
    },

    /// 命名类型（结构体、枚举、类等）
    Named { name: String, args: Vec<HIRType> },

    /// Trait 对象 `dyn Trait`
    TraitObject(Vec<String>),

    /// 类型推断失败时的错误类型
    Error,
}

/// 整数类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntKind {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
}

impl IntKind {
    /// 是否为有符号整数
    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128 | Self::ISize
        )
    }

    /// 获取位宽
    pub fn bits(&self) -> usize {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::U64 => 64,
            Self::I128 | Self::U128 => 128,
            Self::ISize | Self::USize => 32, // 假设 32 位指针
        }
    }
}

impl std::fmt::Display for IntKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I8 => write!(f, "i8"),
            Self::I16 => write!(f, "i16"),
            Self::I32 => write!(f, "i32"),
            Self::I64 => write!(f, "i64"),
            Self::I128 => write!(f, "i128"),
            Self::ISize => write!(f, "isize"),
            Self::U8 => write!(f, "u8"),
            Self::U16 => write!(f, "u16"),
            Self::U32 => write!(f, "u32"),
            Self::U64 => write!(f, "u64"),
            Self::U128 => write!(f, "u128"),
            Self::USize => write!(f, "usize"),
        }
    }
}

/// 浮点类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatKind {
    F32,
    F64,
}

impl FloatKind {
    /// 获取位宽
    pub fn bits(&self) -> usize {
        match self {
            Self::F32 => 32,
            Self::F64 => 64,
        }
    }
}

impl std::fmt::Display for FloatKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::F32 => write!(f, "f32"),
            Self::F64 => write!(f, "f64"),
        }
    }
}
