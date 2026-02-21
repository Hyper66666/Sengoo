//! 缁鐎?

use super::{Node, Path, Span};

/// 缁鐎?
#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

impl Type {
    pub fn new(kind: TypeKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// 閸掓稑缂撶粻鈧崡鏇犺閸?
    pub fn simple(name: impl Into<String>, span: Span) -> Self {
        Self::new(TypeKind::Path(Path::from_str(name, span)), span)
    }

    /// 閸掓稑缂撶捄顖氱窞缁鐎?
    pub fn path(path: Path) -> Self {
        let span = path.span();
        Self::new(TypeKind::Path(path), span)
    }

    /// 閸掓稑缂撻崗鍐矋缁鐎?
    pub fn tuple(types: Vec<Type>, span: Span) -> Self {
        Self::new(TypeKind::Tuple(types), span)
    }

    /// 閸掓稑缂撻弫鎵矋缁鐎?
    pub fn array(elem: Type, len: u64, span: Span) -> Self {
        Self::new(TypeKind::Array(Box::new(elem), len), span)
    }

    /// 閸掓稑缂撻崚鍥╁缁鐎?
    pub fn slice(elem: Type, span: Span) -> Self {
        Self::new(TypeKind::Slice(Box::new(elem)), span)
    }

    /// 閸掓稑缂撻幐鍥嫛缁鐎?
    pub fn ptr(elem: Type, is_mut: bool, span: Span) -> Self {
        Self::new(
            TypeKind::Ptr {
                base: Box::new(elem),
                is_mut,
            },
            span,
        )
    }

    /// 閸掓稑缂撳鏇犳暏缁鐎?
    pub fn ref_(elem: Type, is_mut: bool, span: Span) -> Self {
        Self::new(
            TypeKind::Ref {
                base: Box::new(elem),
                is_mut,
            },
            span,
        )
    }

    /// 閸掓稑缂撻崙鑺ユ殶缁鐎?
    pub fn fn_(params: Vec<Type>, ret: Option<Box<Type>>, span: Span) -> Self {
        Self::new(TypeKind::Fn { params, ret }, span)
    }

    /// 閸掓稑缂?never 缁鐎?
    pub fn never(span: Span) -> Self {
        Self::new(TypeKind::Never, span)
    }

    /// 閸掓稑缂撻崡鏇炲帗缁鐎?
    pub fn unit(span: Span) -> Self {
        Self::new(TypeKind::Tuple(Vec::new()), span)
    }

    /// 閺勵垰鎯侀弰顖氬礋閸忓啰琚崹?
    pub fn is_unit(&self) -> bool {
        matches!(&self.kind, TypeKind::Tuple(types) if types.is_empty())
    }

    /// 閺勵垰鎯侀弰?never 缁鐎?
    pub fn is_never(&self) -> bool {
        matches!(self.kind, TypeKind::Never)
    }

    /// 閺勵垰鎯侀弰顖氱穿閻劎琚崹?
    pub fn is_ref(&self) -> bool {
        matches!(self.kind, TypeKind::Ref { .. })
    }

    /// 閺勵垰鎯侀弰顖氬讲閸欐ê绱╅悽?
    pub fn is_mut_ref(&self) -> bool {
        matches!(self.kind, TypeKind::Ref { is_mut: true, .. })
    }
}

impl Node for Type {
    fn span(&self) -> Span {
        self.span
    }
}

/// 缁鐎风粔宥囪
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// 缁犫偓閸楁洝鐭惧鍕閸?`Name` 閹?`module::Name`
    Path(Path),

    /// 濞夋稑鐎风捄顖氱窞缁鐎?Name<T1, T2> 
    PathWithArgs { path: Path, args: Vec<Type> },

    /// 閸忓啰绮嶇猾璇茬€?`(Type1, Type2)`
    Tuple(Vec<Type>),

    /// 閺佹壆绮嶇猾璇茬€?`[Type; N]`
    Array(Box<Type>, u64),

    /// 閸掑洨澧栫猾璇茬€?`[Type]`
    Slice(Box<Type>),

    /// 閹稿洭鎷＄猾璇茬€?`*mut Type` 閹?`*const Type`
    Ptr { base: Box<Type>, is_mut: bool },

    /// 瀵洜鏁ょ猾璇茬€?`&mut Type` 閹?`&Type`
    Ref { base: Box<Type>, is_mut: bool },

    /// 閸戣姤鏆熺猾璇茬€?`fn(Type1, Type2) -> ReturnType`
    Fn {
        params: Vec<Type>,
        ret: Option<Box<Type>>,
    },

    /// Never 缁鐎?`!`
    Never,

    /// Infer 缁鐎?`_`
    Infer,

    /// 閸斻劍鈧胶琚崹?`dyn Trait`
    Dyn(Vec<TraitBound>),

    /// Impl 閸楃姳缍呯粭锔捐閸?`impl Trait`
    ImplTrait(Vec<TraitBound>),
}

/// Trait 缁撅附娼?
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

    /// 閺勵垰鎯侀弰顖滅暆閸楁洜瀹抽弶鐕傜礄閺冪姴寮弫甯礆
    pub fn is_simple(&self) -> bool {
        self.params.is_empty()
    }
}

impl Node for TraitBound {
    fn span(&self) -> Span {
        self.path.span
    }
}

/// 妫板嫬鐣炬稊澶屾畱閸╃儤婀扮猾璇茬€烽崥宥囆?
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
