use super::*;

impl Codegen {
    pub(super) fn codegen_instruction(
        &mut self,

        inst: &mir::Instruction,

        mir_fn: &MirFunction,
    ) -> Result<(), String> {
        match inst {
            mir::Instruction::Nop => {}

            mir::Instruction::Assign { destination, value } => {
                let dest = self.local_name(*destination);

                self.emit_indent();

                match value {
                    mir::MirConstant::Int(n) => {
                        self.ir.push_str(&format!("{} = add i64 0, {}\n", dest, n));
                    }

                    mir::MirConstant::Uint(n) => {
                        self.ir.push_str(&format!("{} = add i64 0, {}\n", dest, n));
                    }

                    mir::MirConstant::Bool(b) => {
                        self.ir.push_str(&format!(
                            "{} = add i1 0, {}\n",
                            dest,
                            if *b { 1 } else { 0 }
                        ));
                    }

                    mir::MirConstant::Float(f) => {
                        self.ir
                            .push_str(&format!("{} = fadd double 0.0, {}\n", dest, f));
                    }

                    mir::MirConstant::Char(c) => {
                        self.ir
                            .push_str(&format!("{} = add i8 0, {}\n", dest, *c as i8));
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

                self.emit_indent();

                match op {
                    mir::MirBinOp::Add => {
                        self.ir
                            .push_str(&format!("{} = add i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Sub => {
                        self.ir
                            .push_str(&format!("{} = sub i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Mul => {
                        self.ir
                            .push_str(&format!("{} = mul i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Div => {
                        self.ir.push_str(&format!(
                            "{} = sdiv i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Rem => {
                        self.ir.push_str(&format!(
                            "{} = srem i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Eq => {
                        self.ir.push_str(&format!(
                            "{} = icmp eq i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Ne => {
                        self.ir.push_str(&format!(
                            "{} = icmp ne i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Lt => {
                        self.ir.push_str(&format!(
                            "{} = icmp slt i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Le => {
                        self.ir.push_str(&format!(
                            "{} = icmp sle i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Gt => {
                        self.ir.push_str(&format!(
                            "{} = icmp sgt i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Ge => {
                        self.ir.push_str(&format!(
                            "{} = icmp sge i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::BitAnd => {
                        self.ir
                            .push_str(&format!("{} = and i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::BitOr => {
                        self.ir
                            .push_str(&format!("{} = or i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::BitXor => {
                        self.ir
                            .push_str(&format!("{} = xor i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Shl => {
                        self.ir
                            .push_str(&format!("{} = shl i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Shr => {
                        self.ir.push_str(&format!(
                            "{} = ashr i64 {}, {}\n",
                            dest, left_val, right_val
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

                // Add one pointer level when storing into an alloca destination.

                let (local_info, src_ty) = &mir_fn.locals[source.index()];

                self.emit_indent();

                // User locals and pointer-like sources need an explicit load before assignment.
                // Other temporaries can be stored using their existing SSA/local name.
                let needs_load = local_info.kind == LocalKind::User
                    || matches!(src_ty, MIRType::Ptr(_) | MIRType::Ref(_));

                if needs_load {
                    let src = self.local_name(*source);

                    let llvm_ty = self.mir_type_to_llvm_cached(src_ty);

                    // Select the load type from the source pointer or reference element type.
                    let load_ty = match src_ty {
                        MIRType::Ptr(inner) | MIRType::Ref(inner) => {
                            self.mir_type_to_llvm_cached(inner)
                        }

                        _ => llvm_ty,
                    };

                    self.ir.push_str(&format!(
                        "{} = load {}, {}* {}\n",
                        dest, load_ty, load_ty, src
                    ));
                } else {
                    // Materialize a move by forwarding the SSA name directly.
                    let src = self.local_name(*source);

                    self.ir
                        .push_str(&format!("{} = add i64 0, {}\n", dest, src));
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
                        // Store each array element into its computed element slot.
                        let elem_llvm_ty = self.mir_type_to_llvm_cached(elem_ty);

                        for (i, field_local) in fields.iter().enumerate() {
                            // Compute the pointer for the current aggregate element.
                            let elem_ptr = format!("{}.elem.{}", dest, i);

                            self.emit_indent();

                            self.ir.push_str(&format!(
                                "{} = getelementptr {}, {}* {}, i64 {}\n",
                                elem_ptr, elem_llvm_ty, elem_llvm_ty, dest, i
                            ));

                            // Evaluate each field value before storing it into the aggregate slot.
                            let field_val = self.operand_value(*field_local, mir_fn);

                            self.emit_indent();

                            self.ir.push_str(&format!(
                                "store {} {}, {}* {}\n",
                                elem_llvm_ty, field_val, elem_llvm_ty, elem_ptr
                            ));
                        }
                    }

                    MIRType::Struct { .. } => {
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
                        let discr_local = fields.first().copied().ok_or_else(|| {
                            "enum aggregate missing discriminant field".to_string()
                        })?;
                        let discr_val = self.operand_value(discr_local, mir_fn);
                        let discr_temp = format!("{}.discr", dest);

                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "{} = insertvalue {} undef, i64 {}, 0\n",
                            discr_temp, llvm_ty, discr_val
                        ));

                        let payload_val = if let Some(payload_local) = fields.get(1).copied() {
                            self.operand_value(payload_local, mir_fn)
                        } else {
                            "0".to_string()
                        };

                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "{} = insertvalue {} {}, i64 {}, 1\n",
                            dest, llvm_ty, discr_temp, payload_val
                        ));
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
                // Bitcast keeps the same bits while changing only the LLVM view of the value.
                let dest = self.local_name(*destination);

                let src = self.local_name(*source);

                self.emit_indent();

                self.ir
                    .push_str(&format!("{} = bitcast i64* {} to i64\n", dest, src));
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

                let actual_func = if is_print { "puts" } else { func };

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
                        .push_str(&format!("call i32 @puts({})\n", arg_strs.join(", ")));

                    // Model print as returning unit in Sengoo.
                    self.ir.push_str(&format!("{} = add i8 0, 0\n", dest));
                } else if ret_ty == "void" {
                    self.ir
                        .push_str(&format!("call void {}({})\n", callee, arg_strs.join(", ")));
                } else {
                    self.ir.push_str(&format!(
                        "{} = call {} {}({})\n",
                        dest,
                        ret_ty,
                        callee,
                        arg_strs.join(", ")
                    ));
                }
            }

            mir::Instruction::Discriminant {
                destination,

                source,
            } => {
                // Construct an enum as `{ discr, payload }`, leaving payload undef when absent.
                let dest = self.local_name(*destination);

                let src = self.local_name(*source);

                // ExtractValue reads a field from an aggregate source value.

                self.ir.push_str(&format!(
                    "{} = extractvalue {{ i64, i64 }} {}, 0\n",
                    dest, src
                ));
            }

            mir::Instruction::EnumConstruct {
                destination,

                discriminant,

                payload,

                enum_type: _,
            } => {
                // Enum construction is represented as an LLVM aggregate literal.
                let dest = self.local_name(*destination);

                // Materialize the discriminant first.
                let discr_value = format!("{}.discr", dest);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0\n",
                    discr_value, discriminant
                ));

                // Fill payload slot 1 when the enum variant carries data.
                if let Some(payload_local) = payload {
                    let payload_val = self.operand_value(*payload_local, mir_fn);

                    self.emit_indent();

                    self.ir.push_str(&format!(
                        "{} = insertvalue {{ i64, i64 }} {}, i64 {}, 1\n",
                        dest, discr_value, payload_val
                    ));
                } else {
                    // Keep payload slot 1 as undef for payloadless variants.
                    self.emit_indent();

                    self.ir.push_str(&format!(
                        "{} = insertvalue {{ i64, i64 }} {}, i64 undef, 1\n",
                        dest, discr_value
                    ));
                }
            }

            mir::Instruction::ExtractPayload {
                destination,

                source,
            } => {
                // Bitcast reinterprets the source bits as the destination type.
                let dest = self.local_name(*destination);

                let src = self.local_name(*source);

                // The cast emits one conversion instruction into the destination local.

                self.ir.push_str(&format!(
                    "{} = extractvalue {{ i64, i64 }} {}, 1\n",
                    dest, src
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

                            self.ir.push_str(&format!("call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)\n", dest_ptr, src_ptr, size));
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
                                    "{} = call i32 @memcmp(i8* {}, i8* {}, i64 {})\n",
                                    dest_name, left_ptr, right_ptr, size
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::MemMove { size, .. } => {
                        if args.len() >= 2 {
                            let dest_ptr = self.operand_value(args[0], mir_fn);

                            let src_ptr = self.operand_value(args[1], mir_fn);

                            self.emit_indent();

                            self.ir.push_str(&format!("call void @llvm.memmove.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)\n", dest_ptr, src_ptr, size));
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
                        let val = self.local_name(*local);

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
}
