use super::{common, JITCodegen};
use crate::mir::{MIRType, MirBinOp};

impl JITCodegen {
    /// 浜屽厓鎿嶄綔杞?LLVM 鎸囦护 鈥?uses shared utility for opcode mapping
    pub(super) fn binary_op_to_llvm(
        &self,
        op: MirBinOp,
        ty: &MIRType,
        left: &str,
        right: &str,
    ) -> String {
        let llvm_ty = self.mir_type_to_llvm_str(ty);
        let res = "%result";
        let opcode = common::binary_op_to_llvm(op, ty);
        format!("{} = {} {} {}, {}", res, opcode, llvm_ty, left, right)
    }
}
