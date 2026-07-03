use super::*;

impl Codegen {
    pub(super) fn codegen_instruction(
        &mut self,

        inst: &mir::Instruction,

        mir_fn: &MirFunction,
    ) -> Result<(), String> {
        let dbg = self.debug_location_suffix(&mir_fn.name);
        match inst {
            mir::Instruction::Nop => {}

            mir::Instruction::Assign { destination, value } => {
                let dest = self.local_name(*destination);
                let dest_ty = self.get_local_type(mir_fn, *destination).clone();
                let llvm_ty = self.mir_type_to_llvm_cached(&dest_ty);

                if self.local_uses_stack_slot(*destination, mir_fn) {
                    self.emit_indent();
                    match value {
                        mir::MirConstant::Int(n) => {
                            self.ir.push_str(&format!(
                                "store {} {}, {}* {}\n",
                                llvm_ty, n, llvm_ty, dest
                            ));
                        }
                        mir::MirConstant::Uint(n) => {
                            self.ir.push_str(&format!(
                                "store {} {}, {}* {}\n",
                                llvm_ty, n, llvm_ty, dest
                            ));
                        }
                        mir::MirConstant::Bool(b) => {
                            self.ir.push_str(&format!(
                                "store i1 {}, i1* {}\n",
                                if *b { 1 } else { 0 },
                                dest
                            ));
                        }
                        mir::MirConstant::Float(f) => {
                            let literal = Self::llvm_float_literal(*f);
                            self.ir.push_str(&format!(
                                "store {} {}, {}* {}\n",
                                llvm_ty, literal, llvm_ty, dest
                            ));
                        }
                        mir::MirConstant::Char(c) => {
                            self.ir.push_str(&format!(
                                "store {} {}, {}* {}\n",
                                llvm_ty, *c as u32, llvm_ty, dest
                            ));
                        }
                        mir::MirConstant::String(s) => {
                            let str_idx = self.strings.iter().position(|x| x == s).unwrap_or(0);
                            let tmp = format!("%assign.{}", self.load_counter);
                            self.load_counter += 1;
                            self.ir.push_str(&format!(
                                "{} = bitcast [{} x i8]* @.str.{} to i8*\n",
                                tmp,
                                s.len() + 1,
                                str_idx
                            ));
                            self.emit_indent();
                            self.ir.push_str(&format!(
                                "store {} {}, {}* {}\n",
                                llvm_ty, tmp, llvm_ty, dest
                            ));
                        }
                        mir::MirConstant::Bytes(_) => {
                            self.ir
                                .push_str(&format!("store {} 0, {}* {}\n", llvm_ty, llvm_ty, dest));
                        }
                        mir::MirConstant::GlobalRef(name) => {
                            let tmp = format!("%assign.{}", self.load_counter);
                            self.load_counter += 1;
                            if matches!(dest_ty, MIRType::Fn { .. }) {
                                self.ir.push_str(&format!(
                                    "{} = bitcast {} @{} to {}\n",
                                    tmp, llvm_ty, name, llvm_ty
                                ));
                            } else {
                                self.ir.push_str(&format!(
                                    "{} = bitcast i64* @{} to i64\n",
                                    tmp, name
                                ));
                            }
                            self.emit_indent();
                            self.ir.push_str(&format!(
                                "store {} {}, {}* {}\n",
                                llvm_ty, tmp, llvm_ty, dest
                            ));
                        }
                        mir::MirConstant::Unit => {
                            self.ir.push_str(&format!("store i8 0, i8* {}\n", dest));
                        }
                    }
                    return Ok(());
                }

                self.emit_indent();

                match value {
                    mir::MirConstant::Int(n) => {
                        self.ir
                            .push_str(&format!("{} = add {} 0, {}\n", dest, llvm_ty, n));
                    }

                    mir::MirConstant::Uint(n) => {
                        self.ir
                            .push_str(&format!("{} = add {} 0, {}\n", dest, llvm_ty, n));
                    }

                    mir::MirConstant::Bool(b) => {
                        self.ir.push_str(&format!(
                            "{} = add i1 0, {}\n",
                            dest,
                            if *b { 1 } else { 0 }
                        ));
                    }

                    mir::MirConstant::Float(f) => {
                        let literal = Self::llvm_float_literal(*f);
                        self.ir
                            .push_str(&format!("{} = fadd {} 0.0, {}\n", dest, llvm_ty, literal));
                    }

                    mir::MirConstant::Char(c) => {
                        self.ir
                            .push_str(&format!("{} = add {} 0, {}\n", dest, llvm_ty, *c as u32));
                    }

                    mir::MirConstant::String(s) => {
                        // String constants are stored in the module string table.
                        let str_idx = self.strings.iter().position(|x| x == s).unwrap_or(0);

                        let str_ref = format!("@.str.{}", str_idx);

                        self.ir.push_str(&format!(
                            "{} = bitcast [{} x i8]* {} to i8*\n",
                            dest,
                            s.len() + 1,
                            str_ref
                        ));
                    }

                    mir::MirConstant::Bytes(_) => {
                        self.ir.push_str(&format!("{} = add i64 0, 0\n", dest));
                    }

                    mir::MirConstant::GlobalRef(name) => {
                        let dest_ty = self.get_local_type(mir_fn, *destination);
                        let llvm_dest_ty = self.mir_type_to_llvm_cached(dest_ty);
                        if matches!(dest_ty, MIRType::Fn { .. }) {
                            self.ir.push_str(&format!(
                                "{} = bitcast {} @{} to {}
",
                                dest, llvm_dest_ty, name, llvm_dest_ty
                            ));
                        } else if matches!(dest_ty, MIRType::Ptr(_) | MIRType::Ref(_)) {
                            // Address of a global (e.g. a `dyn Trait` vtable),
                            // reinterpreted to the destination pointer type.
                            let src_ty = self
                                .global_llvm_type(name)
                                .map(|ty| format!("{}*", ty))
                                .unwrap_or_else(|| llvm_dest_ty.clone());
                            self.ir.push_str(&format!(
                                "{} = bitcast {} @{} to {}\n",
                                dest, src_ty, name, llvm_dest_ty
                            ));
                        } else {
                            self.ir.push_str(&format!(
                                "{} = bitcast i64* @{} to i64
",
                                dest, name
                            ));
                        }
                    }

                    mir::MirConstant::Unit => {
                        self.ir.push_str(&format!("{} = add i8 0, 0\n", dest));
                    }
                }
            }

            mir::Instruction::Unary {
                destination,

                op,

                operand,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*operand, mir_fn);

                self.emit_indent();

                match op {
                    mir::MirUnOp::Neg => {
                        self.ir
                            .push_str(&format!("{} = sub i64 0, {}\n", dest, src_val));
                    }

                    mir::MirUnOp::Not => {
                        self.ir
                            .push_str(&format!("{} = xor i1 {}, true\n", dest, src_val));
                    }

                    mir::MirUnOp::BitNot => {
                        self.ir
                            .push_str(&format!("{} = xor i64 {}, -1\n", dest, src_val));
                    }
                }
            }

            mir::Instruction::Binary {
                destination,

                op,

                left,

                right,
            } => {
                let dest = self.local_name(*destination);

                // Binary operands are resolved through operand_value, which loads stack slots.

                let left_val = self.operand_value(*left, mir_fn);

                let right_val = self.operand_value(*right, mir_fn);

                let left_ty = self.get_local_type(mir_fn, *left).clone();

                let llvm_ty = self.mir_type_to_llvm_cached(&left_ty);

                self.emit_indent();

                match op {
                    mir::MirBinOp::Add
                    | mir::MirBinOp::Sub
                    | mir::MirBinOp::Mul
                    | mir::MirBinOp::Div
                    | mir::MirBinOp::Rem
                    | mir::MirBinOp::Eq
                    | mir::MirBinOp::Ne
                    | mir::MirBinOp::Lt
                    | mir::MirBinOp::Le
                    | mir::MirBinOp::Gt
                    | mir::MirBinOp::Ge => {
                        let opcode = common::binary_op_to_llvm(*op, &left_ty);
                        self.ir.push_str(&format!(
                            "{} = {} {} {}, {}\n",
                            dest, opcode, llvm_ty, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::BitAnd => {
                        self.ir.push_str(&format!(
                            "{} = and {} {}, {}\n",
                            dest, llvm_ty, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::BitOr => {
                        self.ir.push_str(&format!(
                            "{} = or {} {}, {}\n",
                            dest, llvm_ty, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::BitXor => {
                        self.ir.push_str(&format!(
                            "{} = xor {} {}, {}\n",
                            dest, llvm_ty, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Shl => {
                        self.ir.push_str(&format!(
                            "{} = shl {} {}, {}\n",
                            dest, llvm_ty, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Shr => {
                        self.ir.push_str(&format!(
                            "{} = ashr {} {}, {}\n",
                            dest, llvm_ty, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::LogAnd => {
                        self.ir
                            .push_str(&format!("{} = and i1 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::LogOr => {
                        self.ir
                            .push_str(&format!("{} = or i1 {}, {}\n", dest, left_val, right_val));
                    }
                }
            }

            mir::Instruction::Load {
                destination,

                source,
            } => {
                let dest = self.local_name(*destination);

                let (local_info, src_ty) = &mir_fn.locals[source.index()];

                self.emit_indent();

                let src = self.local_name(*source);

                match (local_info.kind, src_ty) {
                    (LocalKind::User, MIRType::Ptr(inner) | MIRType::Ref(inner)) => {
                        let ptr_ty = self.mir_type_to_llvm_cached(src_ty);
                        let elem_ty = self.mir_type_to_llvm_cached(inner);
                        let ptr_value = format!("%ptr.load.{}", self.load_counter);
                        self.load_counter += 1;
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            ptr_value, ptr_ty, ptr_ty, src
                        ));
                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            dest, elem_ty, elem_ty, ptr_value
                        ));
                    }
                    (LocalKind::User, _) => {
                        let llvm_ty = self.mir_type_to_llvm_cached(src_ty);
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            dest, llvm_ty, llvm_ty, src
                        ));
                    }
                    (_, MIRType::Ptr(inner) | MIRType::Ref(inner)) => {
                        let load_ty = self.mir_type_to_llvm_cached(inner);
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            dest, load_ty, load_ty, src
                        ));
                    }
                    _ => {
                        let dst_ty = self.get_local_type(mir_fn, *destination).clone();
                        self.emit_value_copy(&dest, &dst_ty, &src)?;
                    }
                }
            }

            mir::Instruction::Store { destination, value } => {
                if destination == value {
                    // Redundant self-writeback (`store x -> x`) does not change program state.

                    return Ok(());
                }

                let dest = self.local_name(*destination);

                let val = self.operand_value(*value, mir_fn);

                let ty = self.get_local_type(mir_fn, *value);

                let llvm_ty = self.mir_type_to_llvm_cached(ty);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "store {} {}, {}* {}\n",
                    llvm_ty, val, llvm_ty, dest
                ));
            }

            mir::Instruction::IndexAddr {
                destination,

                base,

                index,
            } => {
                let dest = self.local_name(*destination);

                let base_reg = self.local_name(*base);

                // User index locals must be loaded before getelementptr.
                let idx_local_info = &mir_fn.locals[index.index()].0;

                self.emit_indent();

                if idx_local_info.kind == LocalKind::User {
                    // Load the user index local into an SSA value.
                    let idx_reg = self.local_name(*index);

                    let idx_temp = format!("%idx.{}", destination.id);

                    self.ir
                        .push_str(&format!("{} = load i64, i64* {}\n", idx_temp, idx_reg));

                    self.emit_indent();

                    self.ir.push_str(&format!(
                        "{} = getelementptr i64, i64* {}, i64 {}\n",
                        dest, base_reg, idx_temp
                    ));
                } else {
                    // Non-user index locals can be used directly in getelementptr.
                    let idx_reg = self.local_name(*index);

                    self.ir.push_str(&format!(
                        "{} = getelementptr i64, i64* {}, i64 {}\n",
                        dest, base_reg, idx_reg
                    ));
                }
            }

            mir::Instruction::Aggregate {
                destination,

                fields,

                ty,
            } => {
                // Aggregate initialization handles array and struct-like values.
                let dest = self.local_name(*destination);

                match ty {
                    MIRType::Array(elem_ty, _len) => {
                        let elem_llvm_ty = self.mir_type_to_llvm_cached(elem_ty);
                        let local_kind = mir_fn.locals[destination.index()].0.kind;

                        if local_kind == LocalKind::User {
                            for (i, field_local) in fields.iter().enumerate() {
                                let elem_ptr = format!("{}.elem.{}", dest, i);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = getelementptr {}, {}* {}, i64 {}\n",
                                    elem_ptr, elem_llvm_ty, elem_llvm_ty, dest, i
                                ));

                                let field_val = self.operand_value(*field_local, mir_fn);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "store {} {}, {}* {}\n",
                                    elem_llvm_ty, field_val, elem_llvm_ty, elem_ptr
                                ));
                            }
                        } else {
                            let llvm_ty = self.mir_type_to_llvm_cached(ty);
                            let mut current = "undef".to_string();

                            for (i, field_local) in fields.iter().enumerate() {
                                let field_val = self.operand_value(*field_local, mir_fn);
                                let temp = if i < fields.len() - 1 {
                                    format!("{}.f{}", dest, i)
                                } else {
                                    dest.clone()
                                };

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = insertvalue {} {}, {} {}, {}\n",
                                    temp, llvm_ty, current, elem_llvm_ty, field_val, i
                                ));

                                current = temp;
                            }
                        }
                    }

                    MIRType::Struct { .. } | MIRType::Tuple(_) => {
                        // Build struct values incrementally with insertvalue.
                        let llvm_ty = self.mir_type_to_llvm_cached(ty);

                        if fields.is_empty() {
                            self.emit_indent();

                            self.ir
                                .push_str(&format!("{} = alloca {}\n", dest, llvm_ty));
                        } else {
                            let mut current = "undef".to_string();

                            for (i, field_local) in fields.iter().enumerate() {
                                let field_val = self.operand_value(*field_local, mir_fn);

                                let field_ty = self.get_local_type(mir_fn, *field_local);

                                let field_llvm = self.mir_type_to_llvm_cached(field_ty);

                                let temp = if i < fields.len() - 1 {
                                    format!("{}.f{}", dest, i)
                                } else {
                                    dest.clone()
                                };

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = insertvalue {} {}, {} {}, {}\n",
                                    temp, llvm_ty, current, field_llvm, field_val, i
                                ));

                                current = temp;
                            }
                        }
                    }

                    MIRType::Enum { .. } => {
                        let llvm_ty = self.mir_type_to_llvm_cached(ty);
                        let payload_size = common::enum_payload_storage_size(ty);
                        let payload_storage_llvm = format!("[{payload_size} x i8]");
                        let discr_local = fields.first().copied().ok_or_else(|| {
                            "enum aggregate missing discriminant field".to_string()
                        })?;
                        let discr_val = self.operand_value(discr_local, mir_fn);
                        let slot = format!("{dest}.slot");
                        let discr_ptr = format!("{dest}.discr.ptr");

                        self.emit_indent();
                        self.ir.push_str(&format!("{slot} = alloca {llvm_ty}\n"));
                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "{discr_ptr} = getelementptr {llvm_ty}, {llvm_ty}* {slot}, i32 0, i32 0\n"
                        ));
                        self.emit_indent();
                        self.ir
                            .push_str(&format!("store i64 {discr_val}, i64* {discr_ptr}\n"));

                        if let Some(payload_local) = fields.get(1).copied() {
                            let payload_ty = self.get_local_type(mir_fn, payload_local).clone();
                            let payload_llvm = self.mir_type_to_llvm_cached(&payload_ty);
                            let payload_val = self.operand_value(payload_local, mir_fn);
                            let payload_bytes = format!("{dest}.payload.bytes");
                            let payload_ptr = format!("{dest}.payload.ptr");
                            self.emit_indent();
                            self.ir.push_str(&format!(
                                "{payload_bytes} = getelementptr {llvm_ty}, {llvm_ty}* {slot}, i32 0, i32 1\n"
                            ));
                            self.emit_indent();
                            self.ir.push_str(&format!(
                                "{payload_ptr} = bitcast {payload_storage_llvm}* {payload_bytes} to {payload_llvm}*\n"
                            ));
                            self.emit_indent();
                            self.ir.push_str(&format!(
                                "store {payload_llvm} {payload_val}, {payload_llvm}* {payload_ptr}\n"
                            ));
                        }
                        self.emit_indent();
                        self.ir
                            .push_str(&format!("{dest} = load {llvm_ty}, {llvm_ty}* {slot}\n"));
                    }

                    _ => {

                        // Other aggregate forms do not need extra emission here.
                    }
                }
            }

            mir::Instruction::AddrOf {
                destination,

                source,
            } => {
                let dest = self.local_name(*destination);

                let src = self.local_name(*source);
                let dest_ty = self.get_local_type(mir_fn, *destination).clone();
                let source_ty = self.get_local_type(mir_fn, *source).clone();
                let source_ptr_ty = format!("{}*", self.mir_type_to_llvm_cached(&source_ty));
                let dest_llvm = self.mir_type_to_llvm_cached(&dest_ty);

                self.emit_indent();

                match dest_ty {
                    MIRType::Ref(_) | MIRType::Ptr(_) | MIRType::Fn { .. } => {
                        self.emit_value_copy(&dest, &dest_ty, &src)?;
                    }
                    MIRType::Int(_) => {
                        self.ir.push_str(&format!(
                            "{} = ptrtoint {} {} to {}\n",
                            dest, source_ptr_ty, src, dest_llvm
                        ));
                    }
                    _ => {
                        return Err(format!("cannot take address into LLVM type {}", dest_llvm));
                    }
                }
            }

            mir::Instruction::Call {
                destination,

                func,

                args,
            } => {
                // Emit a direct function call.
                let dest = self.local_name(*destination);

                let dest_ty = self.get_local_type(mir_fn, *destination);

                let ret_ty = self.mir_type_to_llvm_cached(dest_ty);

                // Lower the built-in print function through puts when applicable.
                let is_print = func == "print";

                let actual_func = if is_print {
                    "puts"
                } else {
                    self.emitted_function_name(func)
                };

                let callee = if actual_func.starts_with('%') || actual_func.starts_with('@') {
                    actual_func.to_string()
                } else {
                    format!("@{}", actual_func)
                };

                // Resolve call operands through operand_value so loads happen consistently.
                let mut arg_strs: Vec<String> = Vec::new();

                for arg in args {
                    let arg_local = *arg;

                    let arg_ty = self.get_local_type(mir_fn, arg_local);

                    let llvm_arg_ty = self.mir_type_to_llvm_cached(arg_ty);

                    let val = self.operand_value(arg_local, mir_fn);

                    arg_strs.push(format!("{} {}", llvm_arg_ty, val));
                }

                self.emit_indent();

                if is_print {
                    // print lowers to puts and discards the C return code.
                    self.ir
                        .push_str(&format!("call i32 @puts({}){}\n", arg_strs.join(", "), dbg));

                    // Model print as returning unit in Sengoo.
                    self.ir.push_str(&format!("{} = add i8 0, 0\n", dest));
                } else if ret_ty == "void" {
                    self.ir.push_str(&format!(
                        "call void {}({}){}\n",
                        callee,
                        arg_strs.join(", "),
                        dbg
                    ));
                } else if self.uses_sret_async_result(func, dest_ty) {
                    let sret_slot = format!("{dest}.sret");
                    self.ir
                        .push_str(&format!("{sret_slot} = alloca {ret_ty}\n"));
                    self.emit_indent();
                    let mut sret_args =
                        vec![format!("{ret_ty}* sret({ret_ty}) align 8 {sret_slot}")];
                    sret_args.extend(arg_strs);
                    self.ir.push_str(&format!(
                        "call void {}({}){}\n",
                        callee,
                        sret_args.join(", "),
                        dbg
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = load {}, {}* {}\n",
                        dest, ret_ty, ret_ty, sret_slot
                    ));
                } else {
                    self.ir.push_str(&format!(
                        "{} = call {} {}({}){}\n",
                        dest,
                        ret_ty,
                        callee,
                        arg_strs.join(", "),
                        dbg
                    ));
                }
            }

            mir::Instruction::CallIndirect {
                destination,
                func_ptr,
                args,
            } => {
                let dest = self.local_name(*destination);
                let dest_ty = self.get_local_type(mir_fn, *destination).clone();
                let ret_ty = self.mir_type_to_llvm_cached(&dest_ty);

                let mut arg_tys: Vec<String> = Vec::new();
                let mut arg_strs: Vec<String> = Vec::new();
                for arg in args {
                    let arg_ty = self.get_local_type(mir_fn, *arg);
                    let llvm_arg_ty = self.mir_type_to_llvm_cached(arg_ty);
                    let val = self.operand_value(*arg, mir_fn);
                    arg_strs.push(format!("{} {}", llvm_arg_ty, val));
                    arg_tys.push(llvm_arg_ty);
                }

                // The function pointer arrives as a pointer-sized integer word
                // (loaded from a vtable slot); materialize a typed function
                // pointer before calling.
                let fn_ptr_ty = format!("{} ({})*", ret_ty, arg_tys.join(", "));
                let fn_word = self.operand_value(*func_ptr, mir_fn);
                let fn_word_ty = self.get_local_type(mir_fn, *func_ptr).clone();
                let fn_word_llvm = self.mir_type_to_llvm_cached(&fn_word_ty);
                let callee = format!("%callindirect.fn.{}", destination.id);
                let cast_op = if matches!(fn_word_ty, MIRType::Int(_)) {
                    "inttoptr"
                } else {
                    "bitcast"
                };
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{} = {} {} {} to {}\n",
                    callee, cast_op, fn_word_llvm, fn_word, fn_ptr_ty
                ));

                self.emit_indent();
                if ret_ty == "void" {
                    self.ir.push_str(&format!(
                        "call void {}({}){}\n",
                        callee,
                        arg_strs.join(", "),
                        dbg
                    ));
                } else {
                    self.ir.push_str(&format!(
                        "{} = call {} {}({}){}\n",
                        dest,
                        ret_ty,
                        callee,
                        arg_strs.join(", "),
                        dbg
                    ));
                }
            }

            mir::Instruction::Discriminant {
                destination,

                source,
            } => {
                let dest = self.local_name(*destination);
                let source_ty = self.get_local_type(mir_fn, *source).clone();
                let source_llvm = self.mir_type_to_llvm_cached(&source_ty);
                let src = self.operand_value(*source, mir_fn);
                self.ir.push_str(&format!(
                    "{} = extractvalue {} {}, 0\n",
                    dest, source_llvm, src
                ));
            }

            mir::Instruction::EnumConstruct {
                destination,

                discriminant,

                payload,

                enum_type,
            } => {
                let dest = self.local_name(*destination);
                let enum_llvm = self.mir_type_to_llvm_cached(enum_type);
                let payload_size = common::enum_payload_storage_size(enum_type);
                let payload_storage_llvm = format!("[{payload_size} x i8]");
                let slot = format!("{dest}.slot");
                let discr_ptr = format!("{dest}.discr.ptr");
                self.emit_indent();
                self.ir.push_str(&format!("{slot} = alloca {enum_llvm}\n"));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{discr_ptr} = getelementptr {enum_llvm}, {enum_llvm}* {slot}, i32 0, i32 0\n"
                ));
                self.emit_indent();
                self.ir
                    .push_str(&format!("store i64 {discriminant}, i64* {discr_ptr}\n"));

                if let Some(payload_local) = payload {
                    let payload_ty = self.get_local_type(mir_fn, *payload_local).clone();
                    let payload_llvm = self.mir_type_to_llvm_cached(&payload_ty);
                    let payload_val = self.operand_value(*payload_local, mir_fn);
                    let payload_bytes = format!("{dest}.payload.bytes");
                    let payload_ptr = format!("{dest}.payload.ptr");
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{payload_bytes} = getelementptr {enum_llvm}, {enum_llvm}* {slot}, i32 0, i32 1\n"
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{payload_ptr} = bitcast {payload_storage_llvm}* {payload_bytes} to {payload_llvm}*\n"
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "store {payload_llvm} {payload_val}, {payload_llvm}* {payload_ptr}\n"
                    ));
                }
                self.emit_indent();
                self.ir
                    .push_str(&format!("{dest} = load {enum_llvm}, {enum_llvm}* {slot}\n"));
            }

            mir::Instruction::ExtractPayload {
                destination,

                source,
            } => {
                let dest = self.local_name(*destination);
                let source_ty = self.get_local_type(mir_fn, *source).clone();
                let source_llvm = self.mir_type_to_llvm_cached(&source_ty);
                let payload_size = common::enum_payload_storage_size(&source_ty);
                let payload_storage_llvm = format!("[{payload_size} x i8]");
                let destination_ty = self.get_local_type(mir_fn, *destination).clone();
                let destination_llvm = self.mir_type_to_llvm_cached(&destination_ty);
                let src = self.operand_value(*source, mir_fn);
                let slot = format!("{dest}.enum.slot");
                let payload_bytes = format!("{dest}.payload.bytes");
                let payload_ptr = format!("{dest}.payload.ptr");
                self.emit_indent();
                self.ir
                    .push_str(&format!("{slot} = alloca {source_llvm}\n"));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "store {source_llvm} {src}, {source_llvm}* {slot}\n"
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{payload_bytes} = getelementptr {source_llvm}, {source_llvm}* {slot}, i32 0, i32 1\n"
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{payload_ptr} = bitcast {payload_storage_llvm}* {payload_bytes} to {destination_llvm}*\n"
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{dest} = load {destination_llvm}, {destination_llvm}* {payload_ptr}\n"
                ));
            }

            mir::Instruction::Cast {
                destination,

                value,

                to,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*value, mir_fn);

                let src_ty = self.get_local_type(mir_fn, *value);

                let src_llvm = self.mir_type_to_llvm_cached(src_ty);

                let dst_llvm = self.mir_type_to_llvm_cached(to);

                self.emit_indent();

                match (&src_ty, to) {
                    // Int-to-int casts use sext or trunc depending on destination width.
                    (MIRType::Int(a), MIRType::Int(b)) if a < b => {
                        self.ir.push_str(&format!(
                            "{} = sext {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    (MIRType::Int(a), MIRType::Int(b)) if a > b => {
                        self.ir.push_str(&format!(
                            "{} = trunc {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Float-to-float casts use fpext or fptrunc depending on destination width.
                    (MIRType::Float(a), MIRType::Float(b)) if a < b => {
                        self.ir.push_str(&format!(
                            "{} = fpext {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    (MIRType::Float(a), MIRType::Float(b)) if a > b => {
                        self.ir.push_str(&format!(
                            "{} = fptrunc {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Int-to-float casts use sitofp.
                    (MIRType::Int(_), MIRType::Float(_)) => {
                        self.ir.push_str(&format!(
                            "{} = sitofp {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Float-to-int casts use fptosi.
                    (MIRType::Float(_), MIRType::Int(_)) => {
                        self.ir.push_str(&format!(
                            "{} = fptosi {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Bool -> Int闂佹寧绋掗惌顔界箾閸ヮ剚鍋?zext闂佹寧绋戝? 闂佸湱顣介弲娑㈠春?iN闂佹寧绋戦ˇ顓㈠焵?
                    (MIRType::Bool, MIRType::Int(_)) => {
                        self.ir.push_str(&format!(
                            "{} = zext {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Int -> Bool闂佹寧绋掗惌顔界箾閸ヮ剚鍋?trunc闂佹寧绋戝鐑?闂佽鎯屾禍婊堝春?i1闂佹寧绋戦ˇ顓㈠焵?
                    (MIRType::Int(_), MIRType::Bool) => {
                        self.ir.push_str(&format!(
                            "{} = trunc {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    (MIRType::Ptr(_), MIRType::Int(_))
                    | (MIRType::Ref(_), MIRType::Int(_))
                    | (MIRType::Fn { .. }, MIRType::Int(_)) => {
                        self.ir.push_str(&format!(
                            "{} = ptrtoint {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    (MIRType::Int(_), MIRType::Ptr(_))
                    | (MIRType::Int(_), MIRType::Ref(_))
                    | (MIRType::Int(_), MIRType::Fn { .. }) => {
                        self.ir.push_str(&format!(
                            "{} = inttoptr {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    _ if src_ty == to => {
                        self.emit_value_copy(&dest, src_ty, &src_val)?;
                    }

                    // Same type or unsupported: bitcast as fallback
                    _ => {
                        self.ir.push_str(&format!(
                            "{} = bitcast {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }
                }
            }

            mir::Instruction::Bitcast {
                destination,

                value,

                to,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*value, mir_fn);

                let src_ty = self.get_local_type(mir_fn, *value);

                if !common::supports_mir_bitcast(src_ty, to) {
                    return Err(format!(
                        "invalid MIR bitcast from {} to {}",
                        self.mir_type_to_llvm_cached(src_ty),
                        self.mir_type_to_llvm_cached(to)
                    ));
                }

                let src_llvm = self.mir_type_to_llvm_cached(src_ty);

                let dst_llvm = self.mir_type_to_llvm_cached(to);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = bitcast {} {} to {}\n",
                    dest, src_llvm, src_val, dst_llvm
                ));
            }

            mir::Instruction::FieldAddr {
                destination,

                base,

                field,
            } => {
                let dest = self.local_name(*destination);

                let base_reg = self.local_name(*base);

                let base_ty = self.get_local_type(mir_fn, *base);

                let base_llvm = self.mir_type_to_llvm_cached(base_ty);

                self.emit_indent();

                // FieldAddr gets a pointer to a field within an aggregate type

                // Use getelementptr to compute the field address

                self.ir.push_str(&format!(
                    "{} = getelementptr inbounds {}, {}* {}, i32 0, i32 {}\n",
                    dest, base_llvm, base_llvm, base_reg, field
                ));
            }

            mir::Instruction::Extract {
                destination,

                value,

                index,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*value, mir_fn);

                let src_ty = self.get_local_type(mir_fn, *value);

                let src_llvm = self.mir_type_to_llvm_cached(src_ty);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = extractvalue {} {}, {}\n",
                    dest, src_llvm, src_val, index
                ));
            }

            mir::Instruction::Insert {
                destination,

                value,

                field,

                new_value,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*value, mir_fn);

                let src_ty = self.get_local_type(mir_fn, *value);

                let src_llvm = self.mir_type_to_llvm_cached(src_ty);

                let new_val = self.operand_value(*new_value, mir_fn);

                let new_ty = self.get_local_type(mir_fn, *new_value);

                let new_llvm = self.mir_type_to_llvm_cached(new_ty);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = insertvalue {} {}, {} {}, {}\n",
                    dest, src_llvm, src_val, new_llvm, new_val, field
                ));
            }

            mir::Instruction::Intrinsic {
                destination,

                intrinsic,

                args,
            } => {
                // Generate inline code for intrinsic operations

                match intrinsic {
                    mir::IntrinsicOp::AddWithOverflow => {
                        if args.len() >= 2 {
                            let left_val = self.operand_value(args[0], mir_fn);

                            let right_val = self.operand_value(args[1], mir_fn);

                            if let Some(dest) = destination {
                                let dest_name = self.local_name(*dest);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = add i64 {}, {}\n",
                                    dest_name, left_val, right_val
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::SubWithOverflow => {
                        if args.len() >= 2 {
                            let left_val = self.operand_value(args[0], mir_fn);

                            let right_val = self.operand_value(args[1], mir_fn);

                            if let Some(dest) = destination {
                                let dest_name = self.local_name(*dest);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = sub i64 {}, {}\n",
                                    dest_name, left_val, right_val
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::MulWithOverflow => {
                        if args.len() >= 2 {
                            let left_val = self.operand_value(args[0], mir_fn);

                            let right_val = self.operand_value(args[1], mir_fn);

                            if let Some(dest) = destination {
                                let dest_name = self.local_name(*dest);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = mul i64 {}, {}\n",
                                    dest_name, left_val, right_val
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::Copy { size, .. } => {
                        if args.len() >= 2 {
                            let dest_ptr = self.operand_value(args[0], mir_fn);

                            let src_ptr = self.operand_value(args[1], mir_fn);

                            self.emit_indent();

                            self.ir.push_str(&format!("call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false){}\n", dest_ptr, src_ptr, size, dbg));
                        }
                    }

                    mir::IntrinsicOp::Compare { size, .. } => {
                        if args.len() >= 2 {
                            let left_ptr = self.operand_value(args[0], mir_fn);

                            let right_ptr = self.operand_value(args[1], mir_fn);

                            if let Some(dest) = destination {
                                let dest_name = self.local_name(*dest);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = call i32 @memcmp(i8* {}, i8* {}, i64 {}){}\n",
                                    dest_name, left_ptr, right_ptr, size, dbg
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::MemMove { size, .. } => {
                        if args.len() >= 2 {
                            let dest_ptr = self.operand_value(args[0], mir_fn);

                            let src_ptr = self.operand_value(args[1], mir_fn);

                            self.emit_indent();

                            self.ir.push_str(&format!("call void @llvm.memmove.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false){}\n", dest_ptr, src_ptr, size, dbg));
                        }
                    }
                }
            }

            mir::Instruction::Phi {
                destination,

                incoming,
            } => {
                let dest = self.local_name(*destination);

                let ty = self.get_local_type(mir_fn, *destination);

                let is_void_like = match &ty {
                    MIRType::Unit | MIRType::Never => true,

                    MIRType::Tuple(fields) if fields.is_empty() => true,

                    _ => false,
                };

                if is_void_like {
                    // LLVM does not allow `phi void`.

                    return Ok(());
                }

                let llvm_ty = self.mir_type_to_llvm_cached(ty);

                let entries: Vec<String> = incoming
                    .iter()
                    .map(|(local, block_idx)| {
                        let val = self
                            .phi_incoming_values
                            .get(&(destination.index(), *block_idx, local.index()))
                            .cloned()
                            .unwrap_or_else(|| self.local_name(*local));

                        format!("[ {}, %bb_{} ]", val, block_idx)
                    })
                    .collect();

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = phi {} {}\n",
                    dest,
                    llvm_ty,
                    entries.join(", ")
                ));
            }
        }

        Ok(())
    }

    pub(super) fn targets_windows_msvc(&self) -> bool {
        self.target_triple
            .as_deref()
            .map_or(cfg!(target_os = "windows"), |triple| {
                triple.contains("windows-msvc")
            })
    }

    fn uses_sret_async_result(&self, func: &str, dest_ty: &MIRType) -> bool {
        matches!(dest_ty, MIRType::Struct { .. })
            && Self::async_result_uses_sret(self.targets_windows_msvc(), func)
    }
}
