use super::JITCodegen;
use crate::mir::{self, MirFunction};

impl JITCodegen {
    /// 生成终止符
    pub(super) fn codegen_terminator(
        &mut self,
        terminator: &mir::Terminator,
        mir_fn: &MirFunction,
    ) -> Result<(), String> {
        self.emit_indent();
        match terminator {
            mir::Terminator::Return(value) => {
                if let Some(local) = value {
                    let ret_ty = &mir_fn.return_type;
                    let llvm_ret_ty = self.mir_type_to_llvm_str(ret_ty);
                    let local_ty = self.get_local_type(mir_fn, *local);
                    let llvm_local_ty = self.mir_type_to_llvm_str(&local_ty);

                    // 鎵€鏈?local 閮芥槸閫氳繃 alloca 鍒嗛厤鐨勶紝閮介渶瑕佸姞杞?
                    let reg = self.local_reg(*local);
                    let ret_temp = format!("%.ret.bb{}", self.current_block_id);
                    let local_ptr_ty = format!("{}*", llvm_local_ty);

                    if llvm_local_ty != llvm_ret_ty {
                        // 类型不匹配，使用 bitcast 转换指针
                        let bitcast_temp = format!("%ptr.{}", local.id);
                        let ret_ptr_ty = format!("{}*", llvm_ret_ty);
                        self.ir.push_str(&format!(
                            "{} = bitcast {} {} to {}\n",
                            bitcast_temp, local_ptr_ty, reg, ret_ptr_ty
                        ));
                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            ret_temp, llvm_ret_ty, llvm_ret_ty, bitcast_temp
                        ));
                    } else {
                        // 绫诲瀷鍖归厤锛岀洿鎺ュ姞杞?
                        self.ir.push_str(&format!(
                            "{} = load {}, {} {}\n",
                            ret_temp, llvm_ret_ty, local_ptr_ty, reg
                        ));
                    }

                    // main 鍑芥暟闇€瑕佽繑鍥?i32
                    if mir_fn.name == "main" && llvm_ret_ty != "i32" {
                        self.emit_indent();
                        let ret_i32 = format!("%.ret.i32.bb{}", self.current_block_id);
                        self.ir.push_str(&format!(
                            "{} = trunc {} {} to i32\n",
                            ret_i32, llvm_ret_ty, ret_temp
                        ));
                        self.emit_indent();
                        self.ir.push_str(&format!("ret i32 {}\n", ret_i32));
                    } else {
                        self.emit_indent();
                        self.ir
                            .push_str(&format!("ret {} {}\n", llvm_ret_ty, ret_temp));
                    }
                } else {
                    self.ir.push_str("ret void\n");
                }
            }
            mir::Terminator::Goto(target) => {
                self.ir.push_str(&format!("br label %bb_{}\n", target));
            }
            mir::Terminator::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_ty = self.get_local_type(mir_fn, *cond);
                let cond_val = self.local_reg(*cond);
                let cond_temp = format!("{}.cond", self.local_name(*cond));

                // 鍔犺浇鏉′欢鍊?
                self.ir.push_str(&format!(
                    "{} = load {}, {}* {}\n",
                    cond_temp,
                    self.mir_type_to_llvm_str(&cond_ty),
                    self.mir_type_to_llvm_str(&cond_ty),
                    cond_val
                ));

                // 濡傛灉涓嶆槸 i1锛岄渶瑕佽浆鎹?
                let cond_i1 = if self.mir_type_to_llvm_str(&cond_ty) != "i1" {
                    self.emit_indent();
                    let cond_i1_name = format!("{}.i1", cond_temp);
                    self.ir.push_str(&format!(
                        "{} = icmp ne {} {}, 0\n",
                        cond_i1_name,
                        self.mir_type_to_llvm_str(&cond_ty),
                        cond_temp
                    ));
                    Some(cond_i1_name)
                } else {
                    Some(cond_temp.clone())
                };

                self.emit_indent();
                self.ir.push_str(&format!(
                    "br i1 {}, label %bb_{}, label %bb_{}\n",
                    cond_i1.as_ref().unwrap_or(&cond_temp),
                    then_block,
                    else_block
                ));
            }
            mir::Terminator::Switch {
                discr,
                targets,
                otherwise,
            } => {
                let discr_ty = self.get_local_type(mir_fn, *discr);
                let discr_llvm = self.mir_type_to_llvm_str(&discr_ty);
                let discr_value = format!("{}.switch", self.local_name(*discr));
                self.ir.push_str(&format!(
                    "{discr_value} = load {discr_llvm}, {discr_llvm}* {}\n",
                    self.local_reg(*discr)
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "switch {discr_llvm} {discr_value}, label %bb_{otherwise} ["
                ));
                for (value, target) in targets {
                    self.ir.push('\n');
                    self.emit_indent();
                    self.ir
                        .push_str(&format!("  {discr_llvm} {value}, label %bb_{target}"));
                }
                self.ir.push('\n');
                self.emit_indent();
                self.ir.push_str("]\n");
            }
            mir::Terminator::Break { target } => {
                // break 璺宠浆鍒扮洰鏍囧潡
                self.ir.push_str(&format!("br label %bb_{}\n", target));
            }
            mir::Terminator::Continue { target } => {
                // continue 璺宠浆鍒扮洰鏍囧潡
                self.ir.push_str(&format!("br label %bb_{}\n", target));
            }
            mir::Terminator::Unreachable => {
                self.ir.push_str("unreachable\n");
            }
            _ => {
                self.ir
                    .push_str(&format!("; unhandled terminator: {:?}\n", terminator));
            }
        }
        Ok(())
    }
}
