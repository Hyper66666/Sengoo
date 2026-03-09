//! HIR 表达式定义

use super::{HIRBody, HIRMatchArm, HIRType};
use crate::symbol::SymbolId;

/// HIR 表达式
#[derive(Debug, Clone)]
pub enum HIRExpr {
    /// 字面量
    Lit(HIRLiteral),

    /// 变量引用
    Var {
        name: String,
        symbol: SymbolId,
    },

    /// 一元运算
    Unary(HIRUnaryOp, Box<HIRExpr>),

    /// 二元运算
    Binary(HIRBinaryOp, Box<HIRExpr>, Box<HIRExpr>),

    /// 逻辑与（短路）
    And(Box<HIRExpr>, Box<HIRExpr>),

    /// 逻辑或（短路）
    Or(Box<HIRExpr>, Box<HIRExpr>),

    /// 控制流 - if
    If {
        cond: Box<HIRExpr>,
        then_branch: Box<HIRBody>,
        else_branch: Option<Box<HIRBody>>,
    },

    /// 控制流 - match
    Match {
        scrutinee: Box<HIRExpr>,
        arms: Vec<HIRMatchArm>,
    },

    /// 控制流 - loop
    Loop(Box<HIRBody>),

    /// 控制流 - while
    While {
        cond: Box<HIRExpr>,
        body: Box<HIRBody>,
    },

    /// 控制流 - for
    For {
        var_name: String,
        var_symbol: SymbolId,
        iter: Box<HIRExpr>,
        body: Box<HIRBody>,
    },

    /// 函数调用
    Call {
        func: Box<HIRExpr>,
        args: Vec<HIRExpr>,
    },

    /// 方法调用
    MethodCall {
        receiver: Box<HIRExpr>,
        method: String,
        args: Vec<HIRExpr>,
    },

    /// 结构体实例化
    Struct {
        name: String,
        fields: Vec<(String, HIRExpr)>,
    },

    /// 数组字面量
    Array(Vec<HIRExpr>),

    /// 索引访问
    Index {
        base: Box<HIRExpr>,
        index: Box<HIRExpr>,
    },

    /// 字段访问
    Field {
        base: Box<HIRExpr>,
        field: String,
    },

    /// 赋值
    Assign {
        target: Box<HIRExpr>,
        value: Box<HIRExpr>,
    },

    /// 复合赋值
    AssignOp {
        target: Box<HIRExpr>,
        op: HIRBinaryOp,
        value: Box<HIRExpr>,
    },

    /// 返回
    Return(Option<Box<HIRExpr>>),

    /// 中断（break/continue）
    Break(Option<Box<HIRExpr>>),
    Continue,

    /// 块表达式
    Block(Box<HIRBody>),

    /// 类型转换
    Cast(Box<HIRExpr>, HIRType),

    /// 类型标注（用于类型转换）
    Ascribe(Box<HIRExpr>, HIRType),

    /// 引用
    Ref(bool, Box<HIRExpr>),

    /// 解引用
    Deref(Box<HIRExpr>),

    /// 范围
    Range {
        start: Option<Box<HIRExpr>>,
        end: Option<Box<HIRExpr>>,
        inclusive: bool,
    },

    /// 元组
    Tuple(Vec<HIRExpr>),

    /// Lambda / 闭包
    Lambda {
        params: Vec<String>,
        body: Box<HIRExpr>,
    },

    /// Await expression (consumes a Future<T> and yields T)
    Await(Box<HIRExpr>),

    /// Async block (currently rejected at typeck, reserved for future use)
    AsyncBlock(Box<HIRBody>),
}

/// HIR 字面量
#[derive(Debug, Clone, PartialEq)]
pub enum HIRLiteral {
    Int(i64),
    Uint(u64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Char(char),
    Bool(bool),
    Null,
}

/// HIR 一元运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HIRUnaryOp {
    /// 算术取反
    Neg,
    /// 逻辑取反
    Not,
    /// 位运算取反
    BitNot,
    /// 引用
    Ref,
    /// 可变引用
    RefMut,
    /// 解引用
    Deref,
}

/// HIR 二元运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HIRBinaryOp {
    /// 算术运算
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    /// 位运算
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,

    /// 逻辑运算
    LogAnd,
    LogOr,

    /// 比较运算
    Eq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,

    /// 赋值运算（仅用于类型检查）
    Assign,
}

impl HIRBinaryOp {
    /// 是否为比较运算符
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Self::Eq | Self::NotEq | Self::Lt | Self::Gt | Self::Le | Self::Ge
        )
    }

    /// 是否为算术运算符
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Mod
        )
    }

    /// 是否为位运算符
    pub fn is_bitwise(&self) -> bool {
        matches!(
            self,
            Self::BitAnd | Self::BitOr | Self::BitXor | Self::Shl | Self::Shr
        )
    }

    /// 获取运算符优先级（数值越大优先级越高）
    pub fn precedence(&self) -> u8 {
        match self {
            Self::Mul | Self::Div | Self::Mod => 12,
            Self::Add | Self::Sub => 11,
            Self::Shl | Self::Shr => 10,
            Self::Lt | Self::Gt | Self::Le | Self::Ge => 9,
            Self::Eq | Self::NotEq => 8,
            Self::BitAnd => 7,
            Self::BitXor => 6,
            Self::BitOr => 5,
            Self::LogAnd => 4,
            Self::LogOr => 3,
            Self::Assign => 2,
        }
    }
}
