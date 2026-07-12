use super::*;

impl Codegen {
    pub(super) fn codegen_terminator(
        &mut self,

        terminator: &mir::Terminator,

        mir_fn: &MirFunction,
    ) -> Result<(), String> {
        let dbg = self.debug_terminator_location_suffix(mir_fn, terminator);
        match terminator {
            mir::Terminator::Return(value) => {
                if let Some(v) = value {
                    let ty = self.get_local_type(mir_fn, *v);

                    // `main` returning unit still lowers to exit code 0.
                    let is_main_returning_unit =
                        mir_fn.name == "main" && matches!(ty, MIRType::Unit);

                    if is_main_returning_unit {
                        self.emit_indent();

                        self.ir.push_str(&format!("ret i64 0{dbg}\n"));
                    } else if matches!(mir_fn.return_type, MIRType::Unit | MIRType::Never) {
                        self.emit_indent();

                        self.ir.push_str(&format!("ret void{dbg}\n"));
                    } else {
                        // Non-unit returns use operand_value to resolve the return register.
                        let reg = self.operand_value(*v, mir_fn);

                        let llvm_ty = self.mir_type_to_llvm_cached(ty);

                        self.emit_indent();

                        self.ir
                            .push_str(&format!("ret {} {}{}\n", llvm_ty, reg, dbg));
                    }
                } else {
                    // Functions without an explicit return emit a default return value.
                    if mir_fn.name == "main" {
                        self.emit_indent();

                        self.ir.push_str(&format!("ret i64 0{dbg}\n"));
                    } else {
                        self.emit_indent();

                        self.ir.push_str(&format!("ret void{dbg}\n"));
                    }
                }
            }

            mir::Terminator::Goto(target) => {
                self.emit_indent();

                self.ir
                    .push_str(&format!("br label %bb_{}{}\n", target, dbg));
            }

            mir::Terminator::If {
                cond: condition,

                then_block,

                else_block,
            } => {
                let cond_reg = self.operand_value(*condition, mir_fn);
                let cond_ty = self.get_local_type(mir_fn, *condition).clone();
                let cond_value = if matches!(cond_ty, MIRType::Bool) {
                    cond_reg
                } else {
                    let cond_llvm = self.mir_type_to_llvm_cached(&cond_ty);
                    let cmp_reg = format!("{}.as_bool", cond_reg);
                    let zero = Self::zero_literal_for_type(&cond_ty).ok_or_else(|| {
                        format!("if condition has unsupported LLVM type {}", cond_llvm)
                    })?;
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = icmp ne {} {}, {}\n",
                        cmp_reg, cond_llvm, cond_reg, zero
                    ));
                    cmp_reg
                };

                self.emit_indent();

                self.ir.push_str(&format!(
                    "br i1 {}, label %bb_{}, label %bb_{}{}\n",
                    cond_value, then_block, else_block, dbg
                ));
            }

            mir::Terminator::Switch {
                discr,

                targets,

                otherwise,
            } => {
                self.emit_indent();

                let discr_reg = self.operand_value(*discr, mir_fn);

                // 闂佹眹鍨婚崰鎰板垂?switch 闂佸湱顭堝ú锝夊箮?

                self.ir.push_str(&format!(
                    "switch i64 {}, label %bb_{} [",
                    discr_reg, otherwise
                ));

                // Emit each switch case target.

                for (value, target) in targets {
                    self.ir.push('\n');

                    self.emit_indent();

                    self.ir
                        .push_str(&format!("  i64 {}, label %bb_{}", value, target));
                }

                self.ir.push('\n');

                self.emit_indent();

                self.ir.push_str("]\n");
            }

            mir::Terminator::Break { target } => {
                self.emit_indent();

                self.ir
                    .push_str(&format!("br label %bb_{}{}\n", target, dbg));
            }

            mir::Terminator::Continue { target } => {
                self.emit_indent();

                self.ir
                    .push_str(&format!("br label %bb_{}{}\n", target, dbg));
            }

            mir::Terminator::Unreachable => {
                self.emit_indent();

                self.ir.push_str(&format!("unreachable{dbg}\n"));
            }

            mir::Terminator::Call {
                func,

                args,

                destination,

                target,
            } => {
                let dest = self.local_name(*destination);

                let dest_ty = self.get_local_type(mir_fn, *destination);

                let ret_ty = self.mir_type_to_llvm_cached(dest_ty);

                let arg_strs: Vec<String> = args
                    .iter()
                    .map(|arg| match arg {
                        mir::CallArg::Local(local) => {
                            let arg_ty = self.get_local_type(mir_fn, *local);

                            let llvm_arg_ty = self.mir_type_to_llvm_cached(arg_ty);

                            let val = self.operand_value(*local, mir_fn);

                            format!("{} {}", llvm_arg_ty, val)
                        }

                        mir::CallArg::Constant(constant) => match constant {
                            MirConstant::Int(n) => format!("i64 {}", n),

                            MirConstant::Bool(b) => format!("i1 {}", if *b { 1 } else { 0 }),

                            MirConstant::Float(f) => format!("double {}", f),

                            MirConstant::String(s) => format!(
                                "i8* @.str.{}",
                                self.strings.iter().position(|x| x == s).unwrap_or(0)
                            ),

                            _ => "i64 0".to_string(),
                        },
                    })
                    .collect();

                self.emit_indent();

                let callee = self.emitted_function_name(func);
                if ret_ty == "void" {
                    self.ir.push_str(&format!(
                        "call void @{}({}){}\n",
                        callee,
                        arg_strs.join(", "),
                        dbg
                    ));
                } else {
                    self.ir.push_str(&format!(
                        "{} = call {} @{}({}){}\n",
                        dest,
                        ret_ty,
                        callee,
                        arg_strs.join(", "),
                        dbg
                    ));
                }

                self.emit_indent();

                self.ir
                    .push_str(&format!("br label %bb_{}{}\n", target, dbg));
            }

            mir::Terminator::Suspend {
                poll_func,
                future_handle,
                destination,
                ready_block,
                pending_block,
            } => {
                let handle_val = self.operand_value(*future_handle, mir_fn);
                let dest = self.local_name(*destination);

                self.emit_indent();
                self.ir.push_str(&format!(
                    "{} = call i64 @{}(i64 {}){}\n",
                    dest, poll_func, handle_val, dbg
                ));

                let cmp = format!("{}_cmp", dest);
                self.emit_indent();
                self.ir
                    .push_str(&format!("{} = icmp eq i64 {}, 1\n", cmp, dest));

                self.emit_indent();
                self.ir.push_str(&format!(
                    "br i1 {}, label %bb_{}, label %bb_{}{}\n",
                    cmp, ready_block, pending_block, dbg
                ));
            }
        }

        Ok(())
    }
}
