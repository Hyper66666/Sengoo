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
            (MIRType::Int(a), MIRType::Int(b)) if a < b => {
                self.ir.push_str(&format!(
                    "{} = sext {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Int(a), MIRType::Int(b)) if a > b => {
                self.ir.push_str(&format!(
                    "{} = trunc {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
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
            (MIRType::Float(_), MIRType::Int(_)) => {
                self.ir.push_str(&format!(
                    "{} = fptosi {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Bool, MIRType::Int(_)) => {
                self.ir.push_str(&format!(
                    "{} = zext {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Int(_), MIRType::Bool) => {
                self.ir.push_str(&format!(
                    "{} = trunc {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Ptr(_), MIRType::Int(_)) | (MIRType::Ref(_), MIRType::Int(_)) => {
                self.ir.push_str(&format!(
                    "{} = ptrtoint {} {} to {}\n",
                    result, src_llvm, src_value, dst_llvm
                ));
            }
            (MIRType::Int(_), MIRType::Ptr(_)) | (MIRType::Int(_), MIRType::Ref(_)) => {
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
