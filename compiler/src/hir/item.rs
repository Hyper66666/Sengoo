//! HIR 项定义
//!
//! 定义模块级别的各种声明。

use super::{HIRBody, HIRExpr, HIRType};
use crate::symbol::SymbolId;

#[derive(Debug, Clone)]
pub struct HIRTypeParam {
    pub name: String,
    pub bounds: Vec<HIRTypeParamBound>,
    pub default: Option<HIRType>,
}

#[derive(Debug, Clone)]
pub struct HIRTypeParamBound {
    pub trait_path: String,
}

/// HIR 项
#[derive(Debug, Clone)]
pub enum HIRItem {
    Function(HIRFunction),
    Struct(HIRStruct),
    Enum(HIREnum),
    Trait(HIRTrait),
    Impl(HIRImpl),
    Const(HIRConst),
    Static(HIRStatic),
    TypeAlias(HIRTypeAlias),
}

/// HIR 函数
#[derive(Debug, Clone)]
pub struct HIRFunction {
    pub name: String,
    pub type_params: Vec<HIRTypeParam>,
    pub params: Vec<HIRParam>,
    pub return_type: HIRType,
    pub precondition: Option<HIRExpr>,
    pub postcondition: Option<HIRExpr>,
    pub body: HIRBody,
    pub is_async: bool,
    pub is_pub: bool,
}

/// HIR 函数参数
#[derive(Debug, Clone)]
pub struct HIRParam {
    pub name: String,
    pub symbol: SymbolId,
    pub ty: HIRType,
}

impl HIRParam {
    pub fn new(name: String, symbol: SymbolId, ty: HIRType) -> Self {
        Self { name, symbol, ty }
    }
}

/// HIR 结构体
#[derive(Debug, Clone)]
pub struct HIRStruct {
    pub name: String,
    pub type_params: Vec<HIRTypeParam>,
    pub fields: Vec<HIRField>,
    pub is_pub: bool,
}

/// HIR 结构体字段
#[derive(Debug, Clone)]
pub struct HIRField {
    pub name: String,
    pub ty: HIRType,
    pub is_pub: bool,
}

impl HIRField {
    pub fn new(name: String, ty: HIRType) -> Self {
        Self {
            name,
            ty,
            is_pub: false,
        }
    }

    pub fn public(mut self) -> Self {
        self.is_pub = true;
        self
    }
}

/// HIR 枚举
#[derive(Debug, Clone)]
pub struct HIREnum {
    pub name: String,
    pub type_params: Vec<HIRTypeParam>,
    pub variants: Vec<HIRVariant>,
    pub is_pub: bool,
}

/// HIR 枚举变体
#[derive(Debug, Clone)]
pub enum HIRVariant {
    /// 元组变体，如 `Some(T)`
    Tuple(String, Vec<HIRType>),
    /// 结构体变体，如 `Ok { value: T }`
    Struct(String, Vec<HIRField>),
    /// 单元变体，如 `None`
    Unit(String),
}

/// HIR Trait
#[derive(Debug, Clone)]
pub struct HIRTrait {
    pub name: String,
    pub type_params: Vec<HIRTypeParam>,
    pub items: Vec<HIRTraitItem>,
    pub is_pub: bool,
}

/// HIR Trait 项
#[derive(Debug, Clone)]
pub enum HIRTraitItem {
    Function(HIRFunction),
    Const(String, HIRType),
    Type(String),
}

/// HIR Impl
#[derive(Debug, Clone)]
pub struct HIRImpl {
    pub target_type: HIRType,
    pub trait_name: Option<String>,
    pub items: Vec<HIRFunction>,
}

/// HIR 常量
#[derive(Debug, Clone)]
pub struct HIRConst {
    pub name: String,
    pub ty: HIRType,
    pub value: HIRExpr,
    pub is_pub: bool,
}

/// HIR 静态变量
#[derive(Debug, Clone)]
pub struct HIRStatic {
    pub name: String,
    pub ty: HIRType,
    pub value: HIRExpr,
    pub is_mut: bool,
    pub is_pub: bool,
}

/// HIR 类型别名
#[derive(Debug, Clone)]
pub struct HIRTypeAlias {
    pub name: String,
    pub type_params: Vec<HIRTypeParam>,
    pub alias: HIRType,
    pub is_pub: bool,
}
