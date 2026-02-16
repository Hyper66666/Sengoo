//! 函数参数

use super::{Ident, Node, Span, Type};

/// 函数参数
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
    pub is_mut: bool,
    pub span: Span,
}

impl Param {
    pub fn new(name: Ident, ty: Type, span: Span) -> Self {
        Self {
            name,
            ty,
            is_mut: false,
            span,
        }
    }

    pub fn with_mut(mut self) -> Self {
        self.is_mut = true;
        self
    }

    /// 是否是 self 参数
    pub fn is_self(&self) -> bool {
        self.name.name == "self"
    }
}

impl Node for Param {
    fn span(&self) -> Span {
        self.span
    }
}

/// self 参数类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelfParam {
    /// 借用 `self`
    Borrowed,
    /// 可变借用 `mut self`
    BorrowedMut,
    /// 移动 `self`
    Owned,
    /// 可变移动（很少见）
    OwnedMut,
}

impl SelfParam {
    /// 是否是可变的
    pub fn is_mut(&self) -> bool {
        matches!(self, Self::BorrowedMut | Self::OwnedMut)
    }

    /// 是否是借用的
    pub fn is_ref(&self) -> bool {
        matches!(self, Self::Borrowed | Self::BorrowedMut)
    }

    /// 是否是移动的
    pub fn is_owned(&self) -> bool {
        matches!(self, Self::Owned | Self::OwnedMut)
    }
}
