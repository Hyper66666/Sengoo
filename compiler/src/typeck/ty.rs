//! 类型表示
//!
//! 定义类型检查过程中使用的类型系统

use crate::typeck::interner::{InternedTyId, TyInterner};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

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

    /// Whether values of this type use copy semantics instead of move/drop.
    ///
    /// This is the compiler-known baseline for the memory-management roadmap:
    /// integer and floating-point scalars, booleans, and references are `Copy`.
    /// User-defined `Copy` impls and derived Copy are introduced by the
    /// generics/trait roadmap, so ADTs remain non-Copy here.
    pub fn is_copy_value(&self) -> bool {
        matches!(
            self.kind,
            TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::Byte | TyKind::Ref(..)
        )
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

    /// 是否为 Future 类型
    pub fn is_future(&self) -> bool {
        matches!(self.kind, TyKind::Future(_))
    }

    /// 获取 Future 内部类型
    pub fn future_inner(&self) -> Option<&Ty> {
        match &self.kind {
            TyKind::Future(inner) => Some(inner),
            _ => None,
        }
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
    AssocProjection {
        base: Box<Ty>,
        trait_name: String,
        name: String,
    },
    /// impl Trait
    ImplTrait(Vec<String>),
    /// Future type (async function return type)
    Future(Box<Ty>),
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
            TyKind::AssocProjection { base, name, .. } => write!(f, "{}::{}", base, name),
            TyKind::ImplTrait(traits) => write!(f, "impl {}", traits.join(" + ")),
            TyKind::Future(inner) => write!(f, "Future<{}>", inner),
            TyKind::SelfType => write!(f, "Self"),
            TyKind::Inferred => write!(f, "_"),
        }
    }
}

/// 整数类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IntKind {
    I8,
    I16,
    I32,
    I64,
    I128,
    #[default]
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
            IntKind::ISize | IntKind::USize => 64,
        }
    }
}

/// 浮点类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FloatKind {
    F32,
    #[default]
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
    /// 获取位宽
    pub fn bits(&self) -> usize {
        match self {
            FloatKind::F32 => 32,
            FloatKind::F64 => 64,
        }
    }
}

/// 类型错误
///
/// **Phase 1 baseline stance** (Task 2.4, design.md Open Question 1):
/// 这些变体故意继续持有 owned `TyKind` 快照（如 `TypeMismatch { expected, found }`），
/// 而非 [`crate::typeck::interner::InternedTyId`]。理由：
/// 1. 诊断消息在错误发生时即被构造、消息文本在错误传播链中可读，不依赖 interner 是否还活着；
/// 2. `TyKind` 的 `fmt::Display` 已经覆盖全部变体，无需新建 interner-aware formatter；
/// 3. 错误路径本身就是「冷路径」，clone 一份 owned `TyKind` 的开销可忽略；
/// 4. 改为存 `InternedTyId` 需要 formatter 持有 interner 引用，复杂度远超收益。
///
/// 后续 phase 若决定迁移到 id 表示，应同时为 `TypeckError` 引入 interner 感知的 formatter
/// 并审查全部 `TypeMismatch::expected/found.kind.clone()` 构造点；当前 Phase 1 不做此改动。
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
    /// match 未覆盖所有变体/分支
    NonExhaustiveMatch {
        missing: Vec<String>,
        span_lo: u32,
        span_hi: u32,
    },
    /// match 中不可达分支
    UnreachableMatchArm { span_lo: u32, span_hi: u32 },
    /// match 守卫必须为 bool
    GuardNotBool { span_lo: u32, span_hi: u32 },
    /// or-pattern 绑定不一致
    OrPatternBindingMismatch { span_lo: u32, span_hi: u32 },
    /// `?` 使用位置或传播类型不合法
    InvalidQuestionMark {
        message: String,
        span_lo: u32,
        span_hi: u32,
    },
    FfiSignature {
        code: &'static str,
        message: String,
        span_lo: u32,
        span_hi: u32,
    },
    Diagnostic {
        code: &'static str,
        message: String,
        span_lo: u32,
        span_hi: u32,
    },
    /// 其他错误
    Other(String),
}

impl TypeckError {
    pub fn ffi_signature(
        code: &'static str,
        message: impl Into<String>,
        span_lo: u32,
        span_hi: u32,
    ) -> Self {
        Self::FfiSignature {
            code,
            message: message.into(),
            span_lo,
            span_hi,
        }
    }

    pub fn diagnostic(
        code: &'static str,
        message: impl Into<String>,
        span_lo: u32,
        span_hi: u32,
    ) -> Self {
        Self::Diagnostic {
            code,
            message: message.into(),
            span_lo,
            span_hi,
        }
    }

    pub fn stable_code(&self) -> Option<&'static str> {
        match self {
            Self::NonExhaustiveMatch { .. } => Some("non-exhaustive-match"),
            Self::UnreachableMatchArm { .. } => Some("unreachable-match-arm"),
            Self::GuardNotBool { .. } => Some("guard-not-bool"),
            Self::OrPatternBindingMismatch { .. } => Some("or-pattern-binding-mismatch"),
            Self::InvalidQuestionMark { .. } => Some("invalid-question-mark"),
            Self::FfiSignature { code, .. } => Some(code),
            Self::Diagnostic { code, .. } => Some(code),
            _ => None,
        }
    }

    pub fn span(&self) -> Option<(u32, u32)> {
        match self {
            Self::NonExhaustiveMatch {
                span_lo, span_hi, ..
            }
            | Self::UnreachableMatchArm { span_lo, span_hi }
            | Self::GuardNotBool { span_lo, span_hi }
            | Self::OrPatternBindingMismatch { span_lo, span_hi }
            | Self::InvalidQuestionMark {
                span_lo, span_hi, ..
            }
            | Self::FfiSignature {
                span_lo, span_hi, ..
            }
            | Self::Diagnostic {
                span_lo, span_hi, ..
            } => Some((*span_lo, *span_hi)),
            _ => None,
        }
    }
}

impl fmt::Display for TypeckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeckError::TypeMismatch { expected, found } => {
                write!(f, "type mismatch: expected {}, found {}", expected, found)
            }
            TypeckError::UndefinedType { name } => {
                write!(f, "undefined type: {}", name)
            }
            TypeckError::UndefinedVariable { name } => {
                write!(f, "undefined variable: {}", name)
            }
            TypeckError::UndefinedFunction { name } => {
                write!(f, "undefined function: {}", name)
            }
            TypeckError::ArgumentCountMismatch { expected, found } => {
                write!(
                    f,
                    "argument count mismatch: expected {}, found {}",
                    expected, found
                )
            }
            TypeckError::FieldNotFound {
                type_name,
                field_name,
            } => {
                write!(f, "type {} has no field {}", type_name, field_name)
            }
            TypeckError::MethodNotFound {
                type_name,
                method_name,
            } => {
                write!(f, "type {} has no method {}", type_name, method_name)
            }
            TypeckError::TypeInferenceFailed => {
                write!(f, "cannot infer type")
            }
            TypeckError::RecursiveType { name } => {
                write!(f, "recursive type: {}", name)
            }
            TypeckError::TypeTooLarge { ty } => {
                write!(f, "type is too large: {}", ty)
            }
            TypeckError::CyclicType => {
                write!(f, "cyclic type dependency")
            }
            TypeckError::NonExhaustiveMatch { missing, .. } => {
                write!(
                    f,
                    "[non-exhaustive-match] match is not exhaustive: missing {}",
                    missing.join(", ")
                )
            }
            TypeckError::UnreachableMatchArm { .. } => {
                write!(f, "[unreachable-match-arm] unreachable match arm")
            }
            TypeckError::GuardNotBool { .. } => {
                write!(f, "[guard-not-bool] match guard must be bool")
            }
            TypeckError::OrPatternBindingMismatch { .. } => {
                write!(
                    f,
                    "[or-pattern-binding-mismatch] or-pattern alternatives must bind the same names with compatible types"
                )
            }
            TypeckError::InvalidQuestionMark { message, .. } => {
                write!(f, "[invalid-question-mark] {}", message)
            }
            TypeckError::FfiSignature { code, message, .. } => {
                write!(f, "[{}] {}", code, message)
            }
            TypeckError::Diagnostic { code, message, .. } => {
                write!(f, "[{}] {}", code, message)
            }
            TypeckError::Other(msg) => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl std::error::Error for TypeckError {}

/// 类型替换（用于类型变量实例化）
///
/// Slice E / Task 3.3 迁移：存储从 `HashMap<TyVarId, Ty>` 改为
/// `HashMap<TyVarId, InternedTyId>`。原创意是让 unify checkpoint 中高频发生的
/// `self.subst.clone()`（见 `infer.rs::unify_in_place` 中多处）仅复制
/// `(usize, u32)` 键值对，避免之前递归 clone 全部嵌套 `Ty` 子树。
///
/// API 变动：`get` 回会从 `Option<&Ty>` 变为 `Option<Ty>`（materialize
/// 必需返回 owned 值）。仅需身份比较的调用方可改用 [`Self::get_id`]。
#[derive(Debug, Clone)]
pub struct Subst {
    map: HashMap<TyVarId, InternedTyId>,
    /// 会话级共享 interner；insert 时 intern owned `Ty`、get 时 materialize 回 owned `Ty`。
    interner: Rc<RefCell<TyInterner>>,
}

impl Subst {
    /// 创建与指定 session interner 绑定的空替换。
    ///
    /// 一般从 [`crate::typeck::env::TypeEnv::interner`] 获取同一 [`Rc`] 句柄，
    /// 以保证同一 type-check session 内 substitution 与 env / TypeInfer 共享同一 arena。
    pub fn new(interner: Rc<RefCell<TyInterner>>) -> Self {
        Self {
            map: HashMap::new(),
            interner,
        }
    }

    /// 绑定 `var` 到 `ty`。内部会 intern `ty` 并仅存储返回的 [`InternedTyId`]。
    pub fn insert(&mut self, var: TyVarId, ty: Ty) {
        let id = self.interner.borrow_mut().intern_ty(&ty);
        self.map.insert(var, id);
    }

    /// 返回 `var` 绑定的类型；未绑定返回 `None`。
    ///
    /// **API 变动**：Phase 1 之前返回 `Option<&Ty>`；Slice E 后存储为
    /// [`InternedTyId`]，需要 materialize 回 owned tree。同位置不需拥有权的
    /// 调用方可改用 [`Self::get_id`]。materialize 后的 `Ty` origin tag 为 `0`
    /// 哨兵值（存储阶段不保留 per-instance origin）。
    pub fn get(&self, var: TyVarId) -> Option<Ty> {
        self.map
            .get(&var)
            .map(|id| self.interner.borrow().materialize(*id))
    }

    /// 返回 `var` 绑定的结构性 id；不做 materialize。
    pub fn get_id(&self, var: TyVarId) -> Option<InternedTyId> {
        self.map.get(&var).copied()
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

    /// 合并两个替换；冲突时保留 `self` 中已有的绑定。
    pub fn union(mut self, other: Subst) -> Self {
        for (var, id) in other.map {
            self.map.entry(var).or_insert(id);
        }
        self
    }
}

impl Default for Subst {
    /// 作为澜中 substitution 创建：使用一个独立的空 interner，不与任何 session 共享。
    /// 生产代码请始终从 env 获取 [`Rc`] 句柄传入 [`Self::new`]。
    fn default() -> Self {
        Self::new(Rc::new(RefCell::new(TyInterner::new())))
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    #[test]
    fn scalar_and_reference_types_are_copy() {
        let int_ty = Ty::new(0, TyKind::Int(IntKind::I64));
        let float_ty = Ty::new(1, TyKind::Float(FloatKind::F64));
        let bool_ty = Ty::new(2, TyKind::Bool);
        let ref_ty = Ty::new(3, TyKind::Ref(false, Box::new(Ty::new(4, TyKind::Str))));

        assert!(int_ty.is_copy_value());
        assert!(float_ty.is_copy_value());
        assert!(bool_ty.is_copy_value());
        assert!(ref_ty.is_copy_value());
    }

    #[test]
    fn owning_and_unsized_values_are_not_copy_by_default() {
        let string_ty = Ty::new(
            0,
            TyKind::Adt {
                name: "String".to_string(),
                args: vec![],
            },
        );
        let str_ty = Ty::new(1, TyKind::Str);
        let bytes_ty = Ty::new(2, TyKind::Bytes);

        assert!(!string_ty.is_copy_value());
        assert!(!str_ty.is_copy_value());
        assert!(!bytes_ty.is_copy_value());
    }
}
