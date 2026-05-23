use super::{common, JITCodegen};
use crate::mir::{Local, MIRType, MirConstant, MirFunction};

impl JITCodegen {
    /// 鑾峰彇灞€閮ㄥ彉閲忕被鍨?
    pub(super) fn get_local_type(&self, mir_fn: &MirFunction, local: Local) -> MIRType {
        mir_fn
            .locals
            .iter()
            .find(|(l, _)| l.id == local.id)
            .map(|(_, ty)| ty.clone())
            .unwrap_or(MIRType::Int(64))
    }

    /// 鑾峰彇绫诲瀷鐨勫ぇ灏忥紙瀛楄妭锛?
    pub(super) fn get_type_size(&self, ty: &MIRType) -> u64 {
        match ty {
            MIRType::Bool => 1,
            MIRType::Int(n) => (*n as u64) / 8,
            MIRType::Float(n) => (*n as u64) / 8,
            MIRType::Ptr(_) | MIRType::Ref(_) => 8, // 鎸囬拡澶у皬
            MIRType::Array(elem, len) => self.get_type_size(elem) * len,
            _ => 8, // 默认大小
        }
    }

    /// 灞€閮ㄥ彉閲忓悕绉帮紙鐢ㄤ簬瀛樺偍锛?
    pub(super) fn local_name(&self, local: Local) -> String {
        format!("%local_{}", local.id)
    }

    /// 灞€閮ㄥ彉閲忓悕绉帮紙鐢ㄤ簬鍔犺浇锛?
    pub(super) fn local_reg(&self, local: Local) -> String {
        format!("%local_{}", local.id)
    }

    /// MIR 类型转 LLVM 类型字符串 — delegates to shared utility
    pub(super) fn mir_type_to_llvm_str(&self, ty: &MIRType) -> String {
        common::mir_type_to_llvm_str(ty)
    }

    /// 甯搁噺杞?LLVM 鍊煎瓧绗︿覆 鈥?delegates to shared utility
    pub(super) fn mir_constant_to_llvm_str(&self, constant: &MirConstant) -> String {
        common::mir_constant_to_llvm_str(constant)
    }

    /// 鍙戝皠缂╄繘 鈥?delegates to shared utility
    pub(super) fn emit_indent(&mut self) {
        common::emit_indent(&mut self.ir, self.indent);
    }
}
