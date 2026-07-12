use super::{common, IntegerOverflowMode, JITCodegen};
use crate::mir::{MIRType, MirBinOp};

impl JITCodegen {
    /// 浜屽厓鎿嶄綔杞?LLVM 鎸囦护 鈥?uses shared utility for opcode mapping
    pub(super) fn binary_op_to_llvm(
        &mut self,
        op: MirBinOp,
        ty: &MIRType,
        left: &str,
        right: &str,
    ) -> String {
        let llvm_ty = self.mir_type_to_llvm_str(ty);
        let res = "%result";
        if self.should_emit_integer_overflow_check(op, ty) {
            return self.checked_integer_binary_to_llvm(res, op, ty, &llvm_ty, left, right);
        }
        if self.should_emit_division_by_zero_check(op, ty) {
            let check = self.division_by_zero_check_to_llvm(ty, &llvm_ty, right);
            let opcode = common::binary_op_to_llvm(op, ty);
            return format!(
                "{check}\n{} = {} {} {}, {}",
                res, opcode, llvm_ty, left, right
            );
        }
        let opcode = common::binary_op_to_llvm(op, ty);
        format!("{} = {} {} {}, {}", res, opcode, llvm_ty, left, right)
    }

    fn should_emit_integer_overflow_check(&self, op: MirBinOp, ty: &MIRType) -> bool {
        self.integer_overflow_mode == IntegerOverflowMode::DebugChecked
            && matches!(op, MirBinOp::Add | MirBinOp::Sub | MirBinOp::Mul)
            && Self::integer_bit_width(ty).is_some_and(|width| width <= 64)
    }

    fn should_emit_division_by_zero_check(&self, op: MirBinOp, ty: &MIRType) -> bool {
        self.integer_overflow_mode == IntegerOverflowMode::DebugChecked
            && matches!(op, MirBinOp::Div | MirBinOp::Rem)
            && Self::integer_bit_width(ty).is_some_and(|width| width <= 64)
    }

    fn integer_bit_width(ty: &MIRType) -> Option<u8> {
        match ty {
            MIRType::Int(width) | MIRType::UInt(width) => Some(*width),
            _ => None,
        }
    }

    fn checked_integer_binary_to_llvm(
        &mut self,
        res: &str,
        op: MirBinOp,
        ty: &MIRType,
        llvm_ty: &str,
        left: &str,
        right: &str,
    ) -> String {
        let signedness = if matches!(ty, MIRType::UInt(_)) {
            "u"
        } else {
            "s"
        };
        let op_name = match op {
            MirBinOp::Add => "add",
            MirBinOp::Sub => "sub",
            MirBinOp::Mul => "mul",
            _ => unreachable!("checked integer overflow is only defined for + - *"),
        };
        let intrinsic = format!("llvm.{signedness}{op_name}.with.overflow.{llvm_ty}");
        let tuple_ty = format!("{{ {llvm_ty}, i1 }}");
        let index = self.overflow_check_counter;
        self.overflow_check_counter += 1;
        let check = format!("%jit.ov.check.{index}");
        let flag = format!("%jit.ov.flag.{index}");
        let flag_i64 = format!("%jit.ov.flag.i64.{index}");
        format!(
            "{check} = call {tuple_ty} @{intrinsic}({llvm_ty} {left}, {llvm_ty} {right})\n\
             {res} = extractvalue {tuple_ty} {check}, 0\n\
             {flag} = extractvalue {tuple_ty} {check}, 1\n\
             {flag_i64} = zext i1 {flag} to i64\n\
             call void @sengoo_panic_integer_overflow(i64 {flag_i64})"
        )
    }

    fn division_by_zero_check_to_llvm(
        &mut self,
        ty: &MIRType,
        llvm_ty: &str,
        divisor: &str,
    ) -> String {
        let divisor_i64 = if llvm_ty == "i64" {
            divisor.to_string()
        } else {
            let index = self.overflow_check_counter;
            self.overflow_check_counter += 1;
            let casted = format!("%jit.divisor.i64.{index}");
            let cast = if matches!(ty, MIRType::UInt(_)) {
                "zext"
            } else {
                "sext"
            };
            return format!(
                "{casted} = {cast} {llvm_ty} {divisor} to i64\n\
                 call void @sengoo_panic_division_by_zero(i64 {casted})"
            );
        };
        format!("call void @sengoo_panic_division_by_zero(i64 {divisor_i64})")
    }
}
