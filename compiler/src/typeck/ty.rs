//! 类型表示
//!
//! 定义类型检查过程中使用的类型系统

use std::collections::HashMap;
use std::fmt;

/// 类型 ID
pub type TyId = usize;

/// 类型变量 ID (用于类型推断)
pub type TyVarId = usize;

/// 类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ty {
    pub id: TyId,
    pub kind: TyKind,
}

impl Ty {
    /// 创建一个新的类型
    pub fn new(id: TyId, kind: TyKind) -> Self {
        Self { id, kind }
    }

    /// 创建错误类型
    pub fn error(id: TyId) -> Self {
        Self {
            id,
            kind: TyKind::Error,
        }
    }

    /// 是否为错误类型
    pub fn is_error(&self) -> bool {
        matches!(self.kind, TyKind::Error)
    }

    /// 是否为单元类型
    pub fn is_unit(&self) -> bool {
        matches!(self.kind, TyKind::Unit)
    }

    /// 是否为 Never 类型
    pub fn is_never(&self) -> bool {
        matches!(self.kind, TyKind::Never)
    }

    /// 是否为布尔类型
    pub fn is_bool(&self) -> bool {
        matches!(self.kind, TyKind::Bool)
    }

    /// 是否为整数类型
    pub fn is_int(&self) -> bool {
        matches!(self.kind, TyKind::Int(_))
    }

    /// 是否为浮点类型
    pub fn is_float(&self) -> bool {
        matches!(self.kind, TyKind::Float(_))
    }

    /// 是否为数值类型
    pub fn is_numeric(&self) -> bool {
        self.is_int() || self.is_float()
    }

    /// 是否为引用类型
    pub fn is_ref(&self) -> bool {
        matches!(self.kind, TyKind::Ref(..))
    }

    /// 是否为可变引用
    pub fn is_mut_ref(&self) -> bool {
        matches!(self.kind, TyKind::Ref(true, _))
    }

    /// 获取引用内部类型
    pub fn ref_inner(&self) -> Option<&Ty> {
        match &self.kind {
            TyKind::Ref(_, ty) => Some(ty),
            _ => None,
        }
    }

    /// 是否为函数类型
    pub fn is_fn(&self) -> bool {
        matches!(self.kind, TyKind::Fn { .. })
    }

    /// 是否为类型变量
    pub fn is_var(&self) -> bool {
        matches!(self.kind, TyKind::Var(_))
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

/// 类型种类
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TyKind {
    /// 错误类型（用于类型错误恢复）
    Error,
    /// 单元类型 `()`
    Unit,
    /// Never 类型 `!`
    Never,
    /// 布尔类型 `bool`
    Bool,
    /// 整数类型
    Int(IntKind),
    /// 浮点类型
    Float(FloatKind),
    /// 字符类型 `char`
    Char,
    /// 字符串类型 `str`
    Str,
    /// 字节类型 `u8`
    Byte,
    /// 字节数组 `[u8]`
    Bytes,
    /// 元组类型 `(T1, T2, ...)`
    Tuple(Vec<Ty>),
    /// 数组类型 `[T; N]`
    Array(Box<Ty>, usize),
    /// 切片类型 `[T]`
    Slice(Box<Ty>),
    /// 引用类型 `&T` 或 `&mut T`
    Ref(bool, Box<Ty>),
    /// 指针类型 `*T`
    Ptr(Box<Ty>),
    /// 函数类型 `fn(T1, T2, ...) -> T`
    Fn {
        params: Vec<Ty>,
        ret: Box<Ty>,
        is_variadic: bool,
    },
    /// 类型变量（用于类型推断）
    Var(TyVarId),
    /// 命名类型（结构体、枚举等）
    Adt { name: String, args: Vec<Ty> },
    /// Trait 对象 `dyn Trait`
    Dyn(Vec<String>),
    /// impl Trait
    ImplTrait(Vec<String>),
    /// Self 类型
    SelfType,
    /// 推断类型 `_`
    Inferred,
}

impl fmt::Display for TyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TyKind::Error => write!(f, "<error>"),
            TyKind::Unit => write!(f, "()"),
            TyKind::Never => write!(f, "!"),
            TyKind::Bool => write!(f, "bool"),
            TyKind::Int(kind) => write!(f, "{}", kind),
            TyKind::Float(kind) => write!(f, "{}", kind),
            TyKind::Char => write!(f, "char"),
            TyKind::Str => write!(f, "str"),
            TyKind::Byte => write!(f, "u8"),
            TyKind::Bytes => write!(f, "[u8]"),
            TyKind::Tuple(types) => {
                write!(f, "(")?;
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", ty)?;
                }
                write!(f, ")")
            }
            TyKind::Array(ty, n) => write!(f, "[{}; {}]", ty, n),
            TyKind::Slice(ty) => write!(f, "[{}]", ty),
            TyKind::Ref(false, ty) => write!(f, "&{}", ty),
            TyKind::Ref(true, ty) => write!(f, "&mut {}", ty),
            TyKind::Ptr(ty) => write!(f, "*{}", ty),
            TyKind::Fn { params, ret, .. } => {
                write!(f, "fn(")?;
                for (i, ty) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", ty)?;
                }
                write!(f, ") -> {}", ret)
            }
            TyKind::Var(id) => write!(f, "?{}", id),
            TyKind::Adt { name, args } if args.is_empty() => write!(f, "{}", name),
            TyKind::Adt { name, args } => {
                write!(f, "{}<", name)?;
                for (i, ty) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", ty)?;
                }
                write!(f, ">")
            }
            TyKind::Dyn(traits) => write!(f, "dyn {}", traits.join(" + ")),
            TyKind::ImplTrait(traits) => write!(f, "impl {}", traits.join(" + ")),
            TyKind::SelfType => write!(f, "Self"),
            TyKind::Inferred => write!(f, "_"),
        }
    }
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

impl fmt::Display for IntKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntKind::I8 => write!(f, "i8"),
            IntKind::I16 => write!(f, "i16"),
            IntKind::I32 => write!(f, "i32"),
            IntKind::I64 => write!(f, "i64"),
            IntKind::I128 => write!(f, "i128"),
            IntKind::ISize => write!(f, "isize"),
            IntKind::U8 => write!(f, "u8"),
            IntKind::U16 => write!(f, "u16"),
            IntKind::U32 => write!(f, "u32"),
            IntKind::U64 => write!(f, "u64"),
            IntKind::U128 => write!(f, "u128"),
            IntKind::USize => write!(f, "usize"),
        }
    }
}

impl IntKind {
    /// 获取默认的整数类型
    pub fn default() -> Self {
        IntKind::ISize
    }

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
            IntKind::I8 | IntKind::U8 => 8,
            IntKind::I16 | IntKind::U16 => 16,
            IntKind::I32 | IntKind::U32 => 32,
            IntKind::I64 | IntKind::U64 => 64,
            IntKind::I128 | IntKind::U128 => 128,
            IntKind::ISize | IntKind::USize => 32,
        }
    }
}

/// 浮点类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatKind {
    F32,
    F64,
}

impl fmt::Display for FloatKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FloatKind::F32 => write!(f, "f32"),
            FloatKind::F64 => write!(f, "f64"),
        }
    }
}

impl FloatKind {
    /// 获取默认的浮点类型
    pub fn default() -> Self {
        FloatKind::F64
    }

    /// 获取位宽
    pub fn bits(&self) -> usize {
        match self {
            FloatKind::F32 => 32,
            FloatKind::F64 => 64,
        }
    }
}

/// 类型错误
#[derive(Debug, Clone, PartialEq)]
pub enum TypeckError {
    /// 类型不匹配
    TypeMismatch { expected: TyKind, found: TyKind },
    /// 未定义的类型
    UndefinedType { name: String },
    /// 未定义的变量
    UndefinedVariable { name: String },
    /// 未定义的函数
    UndefinedFunction { name: String },
    /// 参数数量错误
    ArgumentCountMismatch { expected: usize, found: usize },
    /// 字段不存在
    FieldNotFound {
        type_name: String,
        field_name: String,
    },
    /// 方法不存在
    MethodNotFound {
        type_name: String,
        method_name: String,
    },
    /// 无法推断类型
    TypeInferenceFailed,
    /// 递归类型
    RecursiveType { name: String },
    /// 类型太大
    TypeTooLarge { ty: TyKind },
    /// 循环依赖
    CyclicType,
    /// 其他错误
    Other(String),
}

impl fmt::Display for TypeckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeckError::TypeMismatch { expected, found } => {
                write!(f, "类型不匹配: 期望 {}, 找到 {}", expected, found)
            }
            TypeckError::UndefinedType { name } => {
                write!(f, "未定义的类型: {}", name)
            }
            TypeckError::UndefinedVariable { name } => {
                write!(f, "未定义的变量: {}", name)
            }
            TypeckError::UndefinedFunction { name } => {
                write!(f, "未定义的函数: {}", name)
            }
            TypeckError::ArgumentCountMismatch { expected, found } => {
                write!(f, "参数数量错误: 期望 {} 个, 找到 {} 个", expected, found)
            }
            TypeckError::FieldNotFound {
                type_name,
                field_name,
            } => {
                write!(f, "类型 {} 没有字段 {}", type_name, field_name)
            }
            TypeckError::MethodNotFound {
                type_name,
                method_name,
            } => {
                write!(f, "类型 {} 没有方法 {}", type_name, method_name)
            }
            TypeckError::TypeInferenceFailed => {
                write!(f, "无法推断类型")
            }
            TypeckError::RecursiveType { name } => {
                write!(f, "递归类型: {}", name)
            }
            TypeckError::TypeTooLarge { ty } => {
                write!(f, "类型太大: {}", ty)
            }
            TypeckError::CyclicType => {
                write!(f, "循环类型依赖")
            }
            TypeckError::Other(msg) => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl std::error::Error for TypeckError {}

/// 类型替换（用于类型变量实例化）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subst {
    map: HashMap<TyVarId, Ty>,
}

impl Subst {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, var: TyVarId, ty: Ty) {
        self.map.insert(var, ty);
    }

    pub fn get(&self, var: TyVarId) -> Option<&Ty> {
        self.map.get(&var)
    }

    pub fn contains_key(&self, var: TyVarId) -> bool {
        self.map.contains_key(&var)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// 合并两个替换
    pub fn union(mut self, other: Subst) -> Self {
        for (var, ty) in other.map {
            if !self.map.contains_key(&var) {
                self.map.insert(var, ty);
            }
        }
        self
    }
}

impl Default for Subst {
    fn default() -> Self {
        Self::new()
    }
}
