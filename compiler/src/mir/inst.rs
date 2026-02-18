//! MIR 指令定义

use super::{MIRType, MirBinOp, MirConstant, MirUnOp};

/// MIR instruction identifier into a function-local instruction arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstId(pub u32);

/// 局部变量
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Local {
    pub id: u32,
    pub kind: LocalKind,
}

impl Local {
    pub fn new(id: usize, kind: LocalKind) -> Self {
        assert!(
            id <= u32::MAX as usize,
            "local id overflow (>{})",
            u32::MAX
        );
        Self { id: id as u32, kind }
    }

    #[inline]
    pub fn index(self) -> usize {
        self.id as usize
    }

    /// 创建返回值局部变量
    pub fn return_local() -> Self {
        Self::new(0, LocalKind::Return)
    }
}

/// 局部变量种类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalKind {
    /// 返回值
    Return,
    /// 函数参数
    Param,
    /// 临时变量
    Temp,
    /// 用户定义的变量
    User,
}

/// MIR 指令
///
/// MIR 指令是无副作用的纯计算指令。
#[derive(Debug, Clone)]
pub enum Instruction {
    /// 赋值常量
    Assign {
        destination: Local,
        value: MirConstant,
    },

    /// 一元运算
    Unary {
        destination: Local,
        op: MirUnOp,
        operand: Local,
    },

    /// 二元运算
    Binary {
        destination: Local,
        op: MirBinOp,
        left: Local,
        right: Local,
    },

    /// 内存加载
    Load { destination: Local, source: Local },

    /// 内存存储
    Store { destination: Local, value: Local },

    /// 获取地址
    AddrOf { destination: Local, source: Local },

    /// 获取字段地址
    FieldAddr {
        destination: Local,
        base: Local,
        field: u32,
    },

    /// 获取索引地址
    IndexAddr {
        destination: Local,
        base: Local,
        index: Local,
    },

    /// 元组/数组提取
    Extract {
        destination: Local,
        value: Local,
        index: u32,
    },

    /// 元组/数组插入（返回新值）
    Insert {
        destination: Local,
        value: Local,
        field: u32,
        new_value: Local,
    },

    /// 转换类型（不改变值，只改变类型解释）
    Cast {
        destination: Local,
        value: Local,
        to: MIRType,
    },

    /// 聚合值初始化
    Aggregate {
        destination: Local,
        fields: Vec<Local>,
        ty: MIRType,
    },

    /// 函数调用（纯函数，无副作用）
    Call {
        destination: Local,
        func: String,
        args: Vec<Local>,
    },

    /// 内联函数调用
    Intrinsic {
        destination: Option<Local>,
        intrinsic: IntrinsicOp,
        args: Vec<Local>,
    },

    /// 获取枚举判别值
    /// 用于模式匹配时确定枚举的变体
    Discriminant { destination: Local, source: Local },

    /// 构造枚举变体
    /// 创建指定判别值的枚举实例
    EnumConstruct {
        destination: Local,
        /// 变体判别值
        discriminant: u32,
        /// 携带的数据（如果是单元变体则为空）
        payload: Option<Local>,
        /// 枚举类型
        enum_type: MIRType,
    },

    /// 从枚举中提取载荷数据
    /// 假设已经确认枚举是携带数据的变体
    ExtractPayload { destination: Local, source: Local },

    /// Phi 指令 — 在 SSA 合并点选择来自不同前驱块的值
    Phi {
        destination: Local,
        /// (值, 来源基本块索引) 对
        incoming: Vec<(Local, usize)>,
    },

    /// 空操作（用于调试或占位）
    Nop,
}

/// 内联操作
#[derive(Debug, Clone)]
pub enum IntrinsicOp {
    /// 整数相加并检查溢出
    AddWithOverflow,
    /// 整数相减并检查溢出
    SubWithOverflow,
    /// 整数相乘并检查溢出
    MulWithOverflow,
    /// 内存复制
    Copy { size: u64, align: u64 },
    /// 内存比较
    Compare { size: u64, align: u64 },
    /// 内存移动
    MemMove { size: u64, align: u64 },
}

impl Instruction {
    /// 获取指令的目标局部变量（如果有）
    pub fn destination(&self) -> Option<Local> {
        match self {
            Instruction::Assign { destination, .. }
            | Instruction::Unary { destination, .. }
            | Instruction::Binary { destination, .. }
            | Instruction::Load { destination, .. }
            | Instruction::AddrOf { destination, .. }
            | Instruction::FieldAddr { destination, .. }
            | Instruction::IndexAddr { destination, .. }
            | Instruction::Extract { destination, .. }
            | Instruction::Insert { destination, .. }
            | Instruction::Cast { destination, .. }
            | Instruction::Aggregate { destination, .. }
            | Instruction::Call { destination, .. }
            | Instruction::Discriminant { destination, .. }
            | Instruction::EnumConstruct { destination, .. }
            | Instruction::ExtractPayload { destination, .. }
            | Instruction::Phi { destination, .. } => Some(*destination),
            Instruction::Intrinsic { destination, .. } => *destination,
            Instruction::Store { .. } | Instruction::Nop => None,
        }
    }
}
