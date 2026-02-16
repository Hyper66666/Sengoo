//! MIR 操作符定义

/// MIR 一元操作符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirUnOp {
    /// 算术取反
    Neg,
    /// 逻辑取反
    Not,
    /// 位取反
    BitNot,
}

/// MIR 二元操作符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirBinOp {
    // 算术运算
    /// 加法
    Add,
    /// 减法
    Sub,
    /// 乘法
    Mul,
    /// 除法
    Div,
    /// 取余
    Rem,

    // 位运算
    /// 位与
    BitAnd,
    /// 位或
    BitOr,
    /// 位异或
    BitXor,
    /// 左移
    Shl,
    /// 右移
    Shr,

    // 逻辑运算
    /// 逻辑与
    LogAnd,
    /// 逻辑或
    LogOr,

    // 比较运算
    /// 相等
    Eq,
    /// 不等
    Ne,
    /// 小于
    Lt,
    /// 大于
    Gt,
    /// 小于等于
    Le,
    /// 大于等于
    Ge,
}

impl MirBinOp {
    /// 是否为比较运算符
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Lt | Self::Gt | Self::Le | Self::Ge
        )
    }

    /// 是否为算术运算符
    pub fn is_arithmetic(self) -> bool {
        matches!(
            self,
            Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Rem
        )
    }

    /// 是否为位运算符
    pub fn is_bitwise(self) -> bool {
        matches!(
            self,
            Self::BitAnd | Self::BitOr | Self::BitXor | Self::Shl | Self::Shr
        )
    }

    /// 获取运算符的 MIPS/LLVM 风格名称
    pub fn as_str(self) -> &'static str {
        match self {
            MirBinOp::Add => "add",
            MirBinOp::Sub => "sub",
            MirBinOp::Mul => "mul",
            MirBinOp::Div => "div",
            MirBinOp::Rem => "rem",
            MirBinOp::BitAnd => "and",
            MirBinOp::BitOr => "or",
            MirBinOp::BitXor => "xor",
            MirBinOp::Shl => "shl",
            MirBinOp::Shr => "shr",
            MirBinOp::LogAnd => "and",
            MirBinOp::LogOr => "or",
            MirBinOp::Eq => "eq",
            MirBinOp::Ne => "ne",
            MirBinOp::Lt => "lt",
            MirBinOp::Gt => "gt",
            MirBinOp::Le => "le",
            MirBinOp::Ge => "ge",
        }
    }
}

/// MIR 常量
#[derive(Debug, Clone, PartialEq)]
pub enum MirConstant {
    /// 单元值
    Unit,
    /// 布尔值
    Bool(bool),
    /// 整数
    Int(i64),
    /// 无符号整数
    Uint(u64),
    /// 浮点数
    Float(f64),
    /// 字符
    Char(char),
    /// 字符串（存储为全局符号引用）
    String(String),
    /// 字节数组
    Bytes(Vec<u8>),
    /// 全局变量/函数引用
    GlobalRef(String),
}

impl MirConstant {
    /// 获取常量的类型
    pub fn ty(&self) -> super::MIRType {
        match self {
            MirConstant::Unit => super::MIRType::Unit,
            MirConstant::Bool(_) => super::MIRType::Bool,
            MirConstant::Int(_) => super::MIRType::Int(64),
            MirConstant::Uint(_) => super::MIRType::Int(64),
            MirConstant::Float(_) => super::MIRType::Float(64),
            MirConstant::Char(_) => super::MIRType::Int(32),
            MirConstant::String(_) => super::MIRType::pointer(super::MIRType::Int(8)),
            MirConstant::Bytes(_) => super::MIRType::Array(Box::new(super::MIRType::Int(8)), 0),
            MirConstant::GlobalRef(_) => super::MIRType::pointer(super::MIRType::Unit),
        }
    }
}
