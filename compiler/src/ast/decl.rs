//! 声明

use super::param::{Param, SelfParam};
use super::ty::TraitBound;
use super::{Block, Ident, Node, Path, Span, Type, Visibility};

/// 声明
#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    pub kind: DeclKind,
    pub span: Span,
}

impl Decl {
    pub fn new(kind: DeclKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// 创建函数声明
    pub fn fn_decl(func: Function) -> Self {
        let span = func.span;
        Self::new(DeclKind::Function(func), span)
    }

    /// 创建结构体声明
    pub fn struct_decl(struct_: Struct) -> Self {
        let span = struct_.span;
        Self::new(DeclKind::Struct(struct_), span)
    }

    /// 创建枚举声明
    pub fn enum_decl(enum_: Enum) -> Self {
        let span = enum_.span;
        Self::new(DeclKind::Enum(enum_), span)
    }

    /// 创建类声明
    pub fn class_decl(class: Class) -> Self {
        let span = class.span;
        Self::new(DeclKind::Class(class), span)
    }

    /// 创建 trait 声明
    pub fn trait_decl(trait_: Trait) -> Self {
        let span = trait_.span;
        Self::new(DeclKind::Trait(trait_), span)
    }

    /// 创建 impl 块
    pub fn impl_decl(impl_: Impl) -> Self {
        let span = impl_.span;
        Self::new(DeclKind::Impl(impl_), span)
    }

    /// 创建类型别名
    pub fn type_alias(type_: TypeAlias) -> Self {
        let span = type_.span;
        Self::new(DeclKind::TypeAlias(type_), span)
    }

    /// 创建常量声明
    pub fn const_decl(const_: Const) -> Self {
        let span = const_.span;
        Self::new(DeclKind::Const(const_), span)
    }

    /// 创建静态变量声明
    pub fn static_decl(static_: Static) -> Self {
        let span = static_.span;
        Self::new(DeclKind::Static(static_), span)
    }

    /// 创建 import 声明
    pub fn import(import: Import) -> Self {
        let span = import.span;
        Self::new(DeclKind::Import(import), span)
    }

    pub fn extern_block(extern_block: ExternBlock) -> Self {
        let span = extern_block.span;
        Self::new(DeclKind::ExternBlock(extern_block), span)
    }

    /// 创建模块声明
    pub fn module(module: Module) -> Self {
        let span = module.span;
        Self::new(DeclKind::Module(module), span)
    }

    /// 获取声明名称
    pub fn name(&self) -> Option<&Ident> {
        match &self.kind {
            DeclKind::Function(f) => Some(&f.name),
            DeclKind::Struct(s) => Some(&s.name),
            DeclKind::Enum(e) => Some(&e.name),
            DeclKind::Class(c) => Some(&c.name),
            DeclKind::Trait(t) => Some(&t.name),
            DeclKind::Impl(_) => None,
            DeclKind::TypeAlias(t) => Some(&t.name),
            DeclKind::Const(c) => Some(&c.name),
            DeclKind::Static(s) => Some(&s.name),
            DeclKind::Import(_) => None,
            DeclKind::ExternBlock(_) => None,
            DeclKind::Module(m) => Some(&m.name),
        }
    }

    /// 是否有 pub 可见性
    pub fn is_public(&self) -> bool {
        match &self.kind {
            DeclKind::Function(f) => f.vis.is_public(),
            DeclKind::Struct(s) => s.vis.is_public(),
            DeclKind::Enum(e) => e.vis.is_public(),
            DeclKind::Class(c) => c.vis.is_public(),
            DeclKind::Trait(t) => t.vis.is_public(),
            DeclKind::Impl(i) => i.vis.is_public(),
            DeclKind::TypeAlias(t) => t.vis.is_public(),
            DeclKind::Const(c) => c.vis.is_public(),
            DeclKind::Static(s) => s.vis.is_public(),
            DeclKind::Import(_) => true,
            DeclKind::ExternBlock(_) => true,
            DeclKind::Module(m) => m.vis.is_public(),
        }
    }
}

impl Node for Decl {
    fn span(&self) -> Span {
        self.span
    }
}

/// 声明类型
#[derive(Debug, Clone, PartialEq)]
pub enum DeclKind {
    /// 函数 `fn name(params) -> Type { body }`
    Function(Function),

    /// 结构体 `struct Name { fields }` 或 `struct Name(Type0, Type1);`
    Struct(Struct),

    /// 枚举 `enum Name { variants }`
    Enum(Enum),

    /// 类 `class Name { members }`
    Class(Class),

    /// Trait `trait Name { items }`
    Trait(Trait),

    /// Impl 块 `impl Type { methods }` 或 `impl Trait for Type { methods }`
    Impl(Impl),

    /// 类型别名 `type Name = Type;`
    TypeAlias(TypeAlias),

    /// 常量 `const NAME: Type = value;`
    Const(Const),

    /// 静态变量 `static NAME: Type = value;`
    Static(Static),

    /// 导入 `import ...`
    Import(Import),

    /// extern block `extern "C" { ... }`
    ExternBlock(ExternBlock),

    /// 模块 `mod name { ... }`
    Module(Module),
}

/// 函数声明
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub self_param: Option<SelfParam>,
    pub return_type: Option<Type>,
    pub precondition: Option<Box<super::Expr>>,
    pub postcondition: Option<Box<super::Expr>>,
    pub body: Block,
    pub is_async: bool,
    pub abi: Option<String>,
    pub is_unsafe: bool,
    pub no_mangle: bool,
    pub export_name: Option<String>,
    pub span: Span,
}

impl Function {
    pub fn new(name: Ident, body: Block, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name,
            type_params: Vec::new(),
            params: Vec::new(),
            self_param: None,
            return_type: None,
            precondition: None,
            postcondition: None,
            body,
            is_async: false,
            abi: None,
            is_unsafe: false,
            no_mangle: false,
            export_name: None,
            span,
        }
    }

    pub fn with_visibility(mut self, vis: Visibility) -> Self {
        self.vis = vis;
        self
    }

    pub fn with_type_params(mut self, params: Vec<TypeParam>) -> Self {
        self.type_params = params;
        self
    }

    pub fn with_params(mut self, params: Vec<Param>) -> Self {
        self.params = params;
        self
    }

    pub fn with_self_param(mut self, self_param: SelfParam) -> Self {
        self.self_param = Some(self_param);
        self
    }

    pub fn with_return_type(mut self, return_type: Type) -> Self {
        self.return_type = Some(return_type);
        self
    }

    pub fn with_async(mut self) -> Self {
        self.is_async = true;
        self
    }
}

impl Node for Function {
    fn span(&self) -> Span {
        self.span
    }
}

/// extern 声明块
#[derive(Debug, Clone, PartialEq)]
pub struct ExternBlock {
    pub abi: String,
    pub link_name: Option<String>,
    pub items: Vec<ExternItem>,
    pub span: Span,
}

impl ExternBlock {
    pub fn new(abi: impl Into<String>, span: Span) -> Self {
        Self {
            abi: abi.into(),
            link_name: None,
            items: Vec::new(),
            span,
        }
    }
}

impl Node for ExternBlock {
    fn span(&self) -> Span {
        self.span
    }
}

/// extern 块条目
#[derive(Debug, Clone, PartialEq)]
pub enum ExternItem {
    Function(ExternFunction),
    Static(ExternStatic),
}

/// extern 函数声明
#[derive(Debug, Clone, PartialEq)]
pub struct ExternFunction {
    pub vis: Visibility,
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub is_unsafe: bool,
    pub span: Span,
}

impl Node for ExternFunction {
    fn span(&self) -> Span {
        self.span
    }
}

/// extern 静态变量声明
#[derive(Debug, Clone, PartialEq)]
pub struct ExternStatic {
    pub vis: Visibility,
    pub is_mut: bool,
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

impl Node for ExternStatic {
    fn span(&self) -> Span {
        self.span
    }
}

/// 结构体声明
#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<StructField>,
    pub span: Span,
}

impl Struct {
    pub fn new(name: Ident, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name,
            type_params: Vec::new(),
            fields: Vec::new(),
            span,
        }
    }

    /// 是否是元组结构体
    pub fn is_tuple_struct(&self) -> bool {
        self.fields.iter().any(|f| f.name.is_none())
    }

    /// 是否是单元结构体
    pub fn is_unit_struct(&self) -> bool {
        self.fields.is_empty()
    }
}

impl Node for Struct {
    fn span(&self) -> Span {
        self.span
    }
}

/// 结构体字段
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub vis: Visibility,
    pub name: Option<Ident>,
    pub ty: Type,
    pub span: Span,
}

impl StructField {
    pub fn named(name: Ident, ty: Type, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name: Some(name),
            ty,
            span,
        }
    }

    pub fn unnamed(_index: usize, ty: Type, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name: None,
            ty,
            span,
        }
    }
}

impl Node for StructField {
    fn span(&self) -> Span {
        self.span
    }
}

/// 枚举声明
#[derive(Debug, Clone, PartialEq)]
pub struct Enum {
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

impl Enum {
    pub fn new(name: Ident, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name,
            type_params: Vec::new(),
            variants: Vec::new(),
            span,
        }
    }
}

impl Node for Enum {
    fn span(&self) -> Span {
        self.span
    }
}

/// 枚举变体
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Ident,
    pub fields: Vec<VariantField>,
    pub discriminant: Option<Box<super::Expr>>,
    pub span: Span,
}

impl EnumVariant {
    pub fn unit(name: Ident, span: Span) -> Self {
        Self {
            name,
            fields: Vec::new(),
            discriminant: None,
            span,
        }
    }

    pub fn tuple(name: Ident, types: Vec<Type>, span: Span) -> Self {
        Self {
            name,
            fields: types
                .into_iter()
                .map(|ty| VariantField::Unnamed(ty))
                .collect(),
            discriminant: None,
            span,
        }
    }

    pub fn struct_(name: Ident, fields: Vec<StructField>, span: Span) -> Self {
        Self {
            name,
            fields: fields
                .into_iter()
                .map(|f| match f.name {
                    Some(name) => VariantField::Named(name, f.ty),
                    None => VariantField::Unnamed(f.ty),
                })
                .collect(),
            discriminant: None,
            span,
        }
    }
}

impl Node for EnumVariant {
    fn span(&self) -> Span {
        self.span
    }
}

/// 变体字段
#[derive(Debug, Clone, PartialEq)]
pub enum VariantField {
    Named(Ident, Type),
    Unnamed(Type),
}

/// 类声明
#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub extends: Option<Path>,
    pub implements: Vec<TraitBound>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

impl Class {
    pub fn new(name: Ident, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name,
            type_params: Vec::new(),
            extends: None,
            implements: Vec::new(),
            members: Vec::new(),
            span,
        }
    }
}

impl Node for Class {
    fn span(&self) -> Span {
        self.span
    }
}

/// 类成员
#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    Field(StructField),
    Method(Function),
}

/// Trait 声明
#[derive(Debug, Clone, PartialEq)]
pub struct Trait {
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub bounds: Vec<TraitBound>,
    pub items: Vec<TraitItem>,
    pub span: Span,
}

impl Trait {
    pub fn new(name: Ident, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name,
            type_params: Vec::new(),
            bounds: Vec::new(),
            items: Vec::new(),
            span,
        }
    }
}

impl Node for Trait {
    fn span(&self) -> Span {
        self.span
    }
}

/// Trait 项
#[derive(Debug, Clone, PartialEq)]
pub enum TraitItem {
    Function(Function),
    Const(Const),
    Type(TypeAlias),
}

/// Impl 块
#[derive(Debug, Clone, PartialEq)]
pub struct Impl {
    pub vis: Visibility,
    pub type_params: Vec<TypeParam>,
    pub target_type: Type,
    pub trait_path: Option<Path>,
    pub items: Vec<Function>,
    pub span: Span,
}

impl Impl {
    pub fn new(target_type: Type, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            type_params: Vec::new(),
            target_type,
            trait_path: None,
            items: Vec::new(),
            span,
        }
    }

    /// 是否是 trait impl（如 `impl Trait for Type`）
    pub fn is_trait_impl(&self) -> bool {
        self.trait_path.is_some()
    }

    /// 是否是固有 impl（如 `impl Type`）
    pub fn is_inherent_impl(&self) -> bool {
        self.trait_path.is_none()
    }
}

impl Node for Impl {
    fn span(&self) -> Span {
        self.span
    }
}

/// 类型别名
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAlias {
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub ty: Type,
    pub span: Span,
}

impl TypeAlias {
    pub fn new(name: Ident, ty: Type, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name,
            type_params: Vec::new(),
            ty,
            span,
        }
    }
}

impl Node for TypeAlias {
    fn span(&self) -> Span {
        self.span
    }
}

/// 常量声明
#[derive(Debug, Clone, PartialEq)]
pub struct Const {
    pub vis: Visibility,
    pub name: Ident,
    pub ty: Type,
    pub value: Box<super::Expr>,
    pub span: Span,
}

impl Const {
    pub fn new(name: Ident, ty: Type, value: super::Expr, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name,
            ty,
            value: Box::new(value),
            span,
        }
    }
}

impl Node for Const {
    fn span(&self) -> Span {
        self.span
    }
}

/// 静态变量声明
#[derive(Debug, Clone, PartialEq)]
pub struct Static {
    pub vis: Visibility,
    pub is_mut: bool,
    pub name: Ident,
    pub ty: Type,
    pub value: Box<super::Expr>,
    pub span: Span,
}

impl Static {
    pub fn new(name: Ident, ty: Type, value: super::Expr, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            is_mut: false,
            name,
            ty,
            value: Box::new(value),
            span,
        }
    }

    pub fn with_mut(mut self) -> Self {
        self.is_mut = true;
        self
    }
}

impl Node for Static {
    fn span(&self) -> Span {
        self.span
    }
}

/// 导入声明
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub path: Path,
    pub alias: Option<Ident>,
    pub kind: ImportKind,
    pub span: Span,
}

impl Import {
    pub fn new(path: Path, kind: ImportKind, span: Span) -> Self {
        Self {
            path,
            alias: None,
            kind,
            span,
        }
    }

    pub fn with_alias(mut self, alias: Ident) -> Self {
        self.alias = Some(alias);
        self
    }

    /// 是否是通配符导入（如 `import * from module`）
    pub fn is_wildcard(&self) -> bool {
        matches!(self.kind, ImportKind::Wildcard)
    }
}

impl Node for Import {
    fn span(&self) -> Span {
        self.span
    }
}

/// 导入类型
#[derive(Debug, Clone, PartialEq)]
pub enum ImportKind {
    /// 简单导入 `import path`
    Simple,
    /// 通配符导入 `import * from path`
    Wildcard,
    /// 选择性导入 `import { a, b, c } from path`
    Selective(Vec<Ident>),
}

/// 模块声明
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub vis: Visibility,
    pub name: Ident,
    pub items: Vec<Decl>,
    pub span: Span,
}

impl Module {
    pub fn new(name: Ident, span: Span) -> Self {
        Self {
            vis: Visibility::Private,
            name,
            items: Vec::new(),
            span,
        }
    }
}

impl Node for Module {
    fn span(&self) -> Span {
        self.span
    }
}

/// 类型参数
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name: Ident,
    pub bounds: Vec<TraitBound>,
    pub default: Option<Type>,
}

impl TypeParam {
    pub fn new(name: Ident) -> Self {
        Self {
            name,
            bounds: Vec::new(),
            default: None,
        }
    }

    pub fn with_bounds(mut self, bounds: Vec<TraitBound>) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn with_default(mut self, default: Type) -> Self {
        self.default = Some(default);
        self
    }
}
