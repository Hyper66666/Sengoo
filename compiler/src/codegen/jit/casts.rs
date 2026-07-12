use super::JITCodegen;
use crate::mir::MIRType;

impl JITCodegen {
    pub(super) fn emit_cast_value(
        &mut self,
        result: &str,
        src_value: &str,
        src_ty: &MIRType,
        target_ty: &MIRType,
    ) {
        let src_llvm = self.mir_type_to_llvm_str(src_ty);
        let dst_llvm = self.mir_type_to_llvm_str(target_ty);

        match (src_ty, target_ty) {
            (MIRType::Int(a), MIRType::Int(b) | MIRType::UInt(b)) if a < b => {
                self.ir.push_str(&format!(
                    "{} = sext {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::UInt(a), MIRType::Int(b) | MIRType::UInt(b)) if a < b => {
                self.ir.push_str(&format!(
                    "{} = zext {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Int(a) | MIRType::UInt(a), MIRType::Int(b) | MIRType::UInt(b)) if a > b => {
                self.ir.push_str(&format!(
                    "{} = trunc {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Int(a) | MIRType::UInt(a), MIRType::Int(b) | MIRType::UInt(b)) if a == b => {
                self.ir
                    .push_str(&format!("{} = add {} 0, {}\n", result, dst_llvm, src_value));
            }
            (MIRType::Float(a), MIRType::Float(b)) if a < b => {
                self.ir.push_str(&format!(
                    "{} = fpext {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Float(a), MIRType::Float(b)) if a > b => {
                self.ir.push_str(&format!(
                    "{} = fptrunc {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Int(_), MIRType::Float(_)) => {
                self.ir.push_str(&format!(
                    "{} = sitofp {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::UInt(_), MIRType::Float(_)) => {
                self.ir.push_str(&format!(
                    "{} = uitofp {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Float(float_width), MIRType::Int(int_width)) => {
                self.ir.push_str(&format!(
                    "{} = call {} @llvm.fptosi.sat.i{}.f{}({} {})\n",
                    result, dst_llvm, int_width, float_width, src_llvm, src_value
                ));
            }
            (MIRType::Float(float_width), MIRType::UInt(int_width)) => {
                self.ir.push_str(&format!(
                    "{} = call {} @llvm.fptoui.sat.i{}.f{}({} {})\n",
                    result, dst_llvm, int_width, float_width, src_llvm, src_value
                ));
            }
            (MIRType::Bool, MIRType::Int(_) | MIRType::UInt(_)) => {
                self.ir.push_str(&format!(
                    "{} = zext {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Int(_) | MIRType::UInt(_), MIRType::Bool) => {
                self.ir.push_str(&format!(
                    "{} = trunc {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Ptr(_), MIRType::Int(_) | MIRType::UInt(_))
            | (MIRType::Ref(_), MIRType::Int(_) | MIRType::UInt(_)) => {
                self.ir.push_str(&format!(
                    "{} = ptrtoint {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Int(_) | MIRType::UInt(_), MIRType::Ptr(_))
            | (MIRType::Int(_) | MIRType::UInt(_), MIRType::Ref(_)) => {
                self.ir.push_str(&format!(
                    "{} = inttoptr {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            _ => {
                self.ir.push_str(&format!(
                    "{} = bitcast {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
        }
    }
}
