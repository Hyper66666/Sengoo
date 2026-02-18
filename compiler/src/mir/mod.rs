//! MIR（中级中间表示 - Mid-Level Intermediate Representation）
//!
//! MIR 是接近 LLVM IR 的中间表示，使用 SSA（静态单赋值）形式。
//!
//! ## 特点
//!
//! - **SSA 形式**：每个变量只赋值一次
//! - **显式控制流**：使用基本块（Basic Block）和终止符（Terminator）
//! - **接近机器码**：易于映射到目标架构

mod bb;
mod inst;
mod lowering;
mod op;
pub mod opt;

pub use bb::{BasicBlock, CallArg, Terminator};
pub use inst::{InstId, Instruction, IntrinsicOp, Local, LocalKind};
pub use lowering::lower_hir;
pub use op::{MirBinOp, MirConstant, MirUnOp};

use crate::hir::HIRType;

/// MIR 函数
#[derive(Debug, Clone)]
pub struct MirFunction {
    /// 函数名
    pub name: String,
    /// 参数类型
    pub params: Vec<MIRType>,
    /// 返回类型
    pub return_type: MIRType,
    /// 局部变量
    pub locals: Vec<(Local, MIRType)>,
    /// 基本块
    pub basic_blocks: Vec<BasicBlock>,
    pub instructions: Vec<Instruction>,
    /// 起始基本块索引
    pub start_block: usize,
}

impl MirFunction {
    pub fn new(name: String, params: Vec<MIRType>, return_type: MIRType) -> Self {
        let mut locals = vec![(Local::new(0, LocalKind::Return), return_type.clone())];
        // 添加参数局部变量
        for (i, param_ty) in params.iter().enumerate() {
            locals.push((Local::new(i + 1, LocalKind::Param), param_ty.clone()));
        }

        let start_block = 0;
        let basic_blocks = vec![BasicBlock::new(start_block)];

        Self {
            name,
            params,
            return_type,
            locals,
            basic_blocks,
            instructions: Vec::new(),
            start_block,
        }
    }

    /// 添加新的局部变量
    pub fn add_local(&mut self, kind: LocalKind, ty: MIRType) -> Local {
        let id = self.locals.len();
        let local = Local::new(id, kind);
        self.locals.push((local, ty));
        local
    }

    /// 添加新的基本块
    pub fn add_block(&mut self) -> usize {
        let id = self.basic_blocks.len();
        self.basic_blocks.push(BasicBlock::new(id));
        id
    }

    pub fn alloc_inst(&mut self, inst: Instruction) -> InstId {
        let id = self.instructions.len();
        assert!(
            id <= u32::MAX as usize,
            "instruction arena exhausted (>{} instructions)",
            u32::MAX
        );
        self.instructions.push(inst);
        InstId(id as u32)
    }

    pub fn push_inst_to_block(&mut self, block_id: usize, inst: Instruction) {
        let inst_id = self.alloc_inst(inst);
        if let Some(block) = self.basic_blocks.get_mut(block_id) {
            block.push(inst_id);
        }
    }

    pub fn instruction(&self, id: InstId) -> &Instruction {
        &self.instructions[id.0 as usize]
    }

    pub fn instruction_mut(&mut self, id: InstId) -> &mut Instruction {
        &mut self.instructions[id.0 as usize]
    }

    pub fn block_instructions<'a>(
        &'a self,
        block: &'a BasicBlock,
    ) -> impl Iterator<Item = &'a Instruction> + 'a {
        block
            .instructions
            .iter()
            .map(move |id| self.instruction(*id))
    }

    /// 获取基本块的可变引用
    pub fn block_mut(&mut self, id: usize) -> Option<&mut BasicBlock> {
        self.basic_blocks.get_mut(id)
    }
}

/// MIR 类型（简化版）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MIRType {
    /// 单元类型
    Unit,
    /// Never 类型
    Never,
    /// 布尔类型
    Bool,
    /// 整数类型（位宽）
    Int(u8),
    /// 浮点类型（位宽）
    Float(u8),
    /// 引用类型
    Ref(Box<MIRType>),
    /// 指针类型
    Ptr(Box<MIRType>),
    /// 数组类型
    Array(Box<MIRType>, u64),
    /// 元组类型
    Tuple(Vec<MIRType>),
    /// 函数类型
    Fn {
        params: Vec<MIRType>,
        ret: Box<MIRType>,
    },
    /// 结构体类型
    Struct {
        name: String,
        fields: Vec<(String, MIRType)>,
    },
    /// 枚举类型
    /// 表示为 (判别值类型, [(变体索引, 变体类型)])
    Enum {
        /// 判别值类型（通常是整数）
        discr_type: Box<MIRType>,
        /// 变体信息：(判别值, 变体数据类型)
        /// 变体数据类型为 None 表示单元变体（如 None）
        /// Some(类型) 表示携带数据的变体（如 Some(T)）
        variants: Vec<(u32, Option<MIRType>)>,
    },
}

/// Common MIR type constants to avoid repeated construction
pub const MIR_I64: MIRType = MIRType::Int(64);
pub const MIR_BOOL: MIRType = MIRType::Bool;
pub const MIR_UNIT: MIRType = MIRType::Unit;
pub const MIR_F64: MIRType = MIRType::Float(64);

impl MIRType {
    pub fn unit() -> Self {
        MIRType::Unit
    }

    pub fn bool() -> Self {
        MIRType::Bool
    }

    pub fn int(bits: u8) -> Self {
        MIRType::Int(bits)
    }

    pub fn float(bits: u8) -> Self {
        MIRType::Float(bits)
    }

    pub fn pointer(inner: MIRType) -> Self {
        MIRType::Ptr(Box::new(inner))
    }

    pub fn tuple(types: Vec<MIRType>) -> Self {
        MIRType::Tuple(types)
    }

    /// 创建枚举类型
    pub fn enum_type(discr_type: MIRType, variants: Vec<(u32, Option<MIRType>)>) -> Self {
        MIRType::Enum {
            discr_type: Box::new(discr_type),
            variants,
        }
    }

    /// 检查是否为枚举类型
    pub fn is_enum(&self) -> bool {
        matches!(self, MIRType::Enum { .. })
    }

    /// 获取枚举变体数量
    pub fn enum_variant_count(&self) -> usize {
        match self {
            MIRType::Enum { variants, .. } => variants.len(),
            _ => 0,
        }
    }
}

impl From<HIRType> for MIRType {
    fn from(ty: HIRType) -> Self {
        use crate::hir::HIRTypeKind;
        match ty.kind {
            HIRTypeKind::Unit => MIRType::Unit,
            HIRTypeKind::Never => MIRType::Never,
            HIRTypeKind::Bool => MIRType::Bool,
            HIRTypeKind::Int(ik) => MIRType::Int(ik.bits() as u8),
            HIRTypeKind::Float(fk) => MIRType::Float(fk.bits() as u8),
            HIRTypeKind::Ref(_, inner) => MIRType::Ref(Box::new((*inner).into())),
            HIRTypeKind::Ptr(inner) => MIRType::Ptr(Box::new((*inner).into())),
            HIRTypeKind::Array(elem, len) => MIRType::Array(Box::new((*elem).into()), len as u64),
            HIRTypeKind::Tuple(types) => {
                MIRType::Tuple(types.into_iter().map(|t| t.into()).collect())
            }
            HIRTypeKind::Fn { params, ret } => MIRType::Fn {
                params: params.into_iter().map(|t| t.into()).collect(),
                ret: Box::new((*ret).into()),
            },
            HIRTypeKind::Named { .. } => {
                // 命名类型（结构体、枚举等）暂时映射为 i64
                // 实际实现需要从符号表中查找完整定义
                MIRType::Int(64)
            }
            _ => MIRType::Unit, // 其他类型暂时映射为单元类型
        }
    }
}
