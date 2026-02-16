//! 运算符

/// 二元运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    /// 算术运算
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    Mod, // %

    /// 位运算
    BitAnd, // &
    BitOr,  // |
    BitXor, // ^
    Shl,    // <<
    Shr,    // >>

    /// 逻辑运算
    And, // and
    Or, // or

    /// 比较运算
    Eq, // ==
    NotEq, // !=
    Lt,    // <
    Le,    // <=
    Gt,    // >
    Ge,    // >=

    /// 其他
    Pipe, // |>
    Compose, // .>

    /// 范围（保留，实际在 Expr 中处理）
    Range, // ..
    RangeInclusive, // ..=
}

impl BinOp {
    /// 是否是算术运算
    pub fn is_arithmetic(self) -> bool {
        matches!(
            self,
            Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Mod
        )
    }

    /// 是否是位运算
    pub fn is_bitwise(self) -> bool {
        matches!(
            self,
            Self::BitAnd | Self::BitOr | Self::BitXor | Self::Shl | Self::Shr
        )
    }

    /// 是否是逻辑运算
    pub fn is_logical(self) -> bool {
        matches!(self, Self::And | Self::Or)
    }

    /// 是否是比较运算
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::NotEq | Self::Lt | Self::Le | Self::Gt | Self::Ge
        )
    }

    /// 运算符的结合性
    pub fn associativity(self) -> Associativity {
        match self {
            Self::Pipe => Associativity::Left,
            _ => Associativity::Left,
        }
    }

    /// 运算符的优先级（数值越大优先级越高）
    pub fn precedence(self) -> u8 {
        match self {
            // 范围
            Self::Range | Self::RangeInclusive => 3,

            // 逻辑或
            Self::Or => 4,

            // 逻辑与
            Self::And => 5,

            // 比较
            Self::Eq | Self::NotEq | Self::Lt | Self::Le | Self::Gt | Self::Ge => 6,

            // 位或
            Self::BitOr => 7,

            // 位异或
            Self::BitXor => 8,

            // 位与
            Self::BitAnd => 9,

            // 移位
            Self::Shl | Self::Shr => 10,

            // 算术加减
            Self::Add | Self::Sub => 11,

            // 算术乘除
            Self::Mul | Self::Div | Self::Mod => 12,

            // 管道和组合
            Self::Pipe | Self::Compose => 2,
        }
    }
}

/// 结合性
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Associativity {
    Left,
    Right,
}

/// 一元运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnOp {
    /// 正号 `+`
    Plus,
    /// 负号 `-`
    Neg,
    /// 逻辑非 `not`
    Not,
    /// 位取反 `~`
    BitNot,
    /// 解引用 `*`
    Deref,
    /// 借用 `&`
    Ref,
    /// 可变借用 `&mut`
    RefMut,
    /// 可变解引用（保留）
    DerefMut,
}

impl UnOp {
    /// 是否是前缀运算符
    pub fn is_prefix(self) -> bool {
        !matches!(self, Self::Deref | Self::DerefMut)
    }

    /// 是否是后缀运算符
    pub fn is_postfix(self) -> bool {
        matches!(self, Self::Deref | Self::DerefMut)
    }
}

/// 赋值运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssignOp {
    /// 简单赋值 `=`
    Assign,

    /// 算术赋值
    AddAssign, // +=
    SubAssign, // -=
    MulAssign, // *=
    DivAssign, // /=
    ModAssign, // %=

    /// 位运算赋值
    BitAndAssign, // &=
    BitOrAssign,  // |=
    BitXorAssign, // ^=
    ShlAssign,    // <<=
    ShrAssign,    // >>=

    /// 其他
    PipeAssign, // |=>
}

impl AssignOp {
    /// 获取对应的二元运算符（如果有）
    pub fn to_binop(self) -> Option<BinOp> {
        match self {
            Self::AddAssign => Some(BinOp::Add),
            Self::SubAssign => Some(BinOp::Sub),
            Self::MulAssign => Some(BinOp::Mul),
            Self::DivAssign => Some(BinOp::Div),
            Self::ModAssign => Some(BinOp::Mod),
            Self::BitAndAssign => Some(BinOp::BitAnd),
            Self::BitOrAssign => Some(BinOp::BitOr),
            Self::BitXorAssign => Some(BinOp::BitXor),
            Self::ShlAssign => Some(BinOp::Shl),
            Self::ShrAssign => Some(BinOp::Shr),
            _ => None,
        }
    }

    /// 获取运算符的字符串表示
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Assign => "=",
            Self::AddAssign => "+=",
            Self::SubAssign => "-=",
            Self::MulAssign => "*=",
            Self::DivAssign => "/=",
            Self::ModAssign => "%=",
            Self::BitAndAssign => "&=",
            Self::BitOrAssign => "|=",
            Self::BitXorAssign => "^=",
            Self::ShlAssign => "<<=",
            Self::ShrAssign => ">>=",
            Self::PipeAssign => "|=>",
        }
    }
}

/// 实现完整的运算符字符串显示
impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::And => "and",
            Self::Or => "or",
            Self::Eq => "==",
            Self::NotEq => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Pipe => "|>",
            Self::Compose => ".>",
            Self::Range => "..",
            Self::RangeInclusive => "..=",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for UnOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Plus => "+",
            Self::Neg => "-",
            Self::Not => "not",
            Self::BitNot => "~",
            Self::Deref => "*",
            Self::Ref => "&",
            Self::RefMut => "&mut",
            Self::DerefMut => "*mut",
        };
        write!(f, "{}", s)
    }
}
