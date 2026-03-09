//! MIR 基本块定义

use super::{InstId, Local, MirConstant};

/// 基本块（Basic Block）
///
/// 基本块是只有一个入口点和一个出口点的指令序列。
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// 基本块索引
    pub id: usize,
    /// 指令列表
    pub instructions: Vec<InstId>,
    /// 终止符
    pub terminator: Option<Terminator>,
}

impl BasicBlock {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            instructions: Vec::new(),
            terminator: None,
        }
    }

    /// 添加指令
    pub fn push(&mut self, inst_id: InstId) {
        self.instructions.push(inst_id);
    }

    /// 设置终止符
    pub fn set_terminator(&mut self, term: Terminator) {
        self.terminator = Some(term);
    }
}

/// 终止符（Terminator）
///
/// 终止符决定基本块执行后的控制流。
#[derive(Debug, Clone)]
pub enum Terminator {
    /// 返回值
    Return(Option<Local>),
    /// 无条件跳转
    Goto(usize),
    /// 条件跳转
    If {
        /// 条件值
        cond: Local,
        /// 条件为真时跳转的目标
        then_block: usize,
        /// 条件为假时跳转的目标
        else_block: usize,
    },
    /// Switch 跳转（基于枚举变体）
    Switch {
        /// 判定的值
        discr: Local,
        /// 各变体对应的目标块
        targets: Vec<(u32, usize)>,
        /// 默认目标块
        otherwise: usize,
    },
    /// 调用后返回（用于需要特殊处理的函数调用）
    Call {
        /// 目标函数
        func: String,
        /// 参数
        args: Vec<CallArg>,
        /// 返回值位置
        destination: Local,
        /// 返回后跳转的目标块
        target: usize,
    },
    /// 跳出循环
    Break {
        /// 目标块（通常是循环的 exit_block）
        target: usize,
    },
    /// 继续下一次循环
    Continue {
        /// 目标块（通常是循环的 cond_block）
        target: usize,
    },
    /// 不可达
    Unreachable,
    /// Async suspend point: poll a child future
    Suspend {
        /// The poll function to call (e.g. "foo__poll")
        poll_func: String,
        /// The future handle local
        future_handle: Local,
        /// Where to store the poll result (0=pending, 1=ready)
        destination: Local,
        /// Block to jump to when ready
        ready_block: usize,
        /// Block to jump to when pending
        pending_block: usize,
    },
}

/// 调用参数
#[derive(Debug, Clone)]
pub enum CallArg {
    Local(Local),
    Constant(MirConstant),
}
