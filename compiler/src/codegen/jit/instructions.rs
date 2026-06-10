use super::{common, JITCodegen};
use crate::mir::{self, MIRType, MirConstant, MirFunction, MirUnOp};

impl JITCodegen {
    /// 鐢熸垚鎸囦护
    pub(super) fn codegen_instruction(
        &mut self,
        inst: &mir::Instruction,
        mir_fn: &MirFunction,
    ) -> Result<(), String> {
        match inst {
            mir::Instruction::Assign { destination, value } => {
                self.emit_indent();
                let dest = self.local_name(*destination);
                let ty = self.get_local_type(mir_fn, *destination);
                let llvm_ty = self.mir_type_to_llvm_str(&ty);

                match value {
                    MirConstant::String(s) => {
                        // 创建字符串常量
                        let str_ref = self.add_string(s);
                        let string_ty = format!("[{} x i8]", s.len() + 1);
                        self.ir.push_str(&format!(
                            "{} = bitcast {} {} to i8*\n",
                            dest, string_ty, str_ref
                        ));
                    }
                    _ => {
                        // 甯搁噺璧嬪€?
                        self.ir.push_str(&format!(
                            "store {} {}, {}* %local_{}\n",
                            llvm_ty,
                            self.mir_constant_to_llvm_str(value),
                            llvm_ty,
                            destination.id
                        ));
                    }
                }
            }
            mir::Instruction::AddrOf {
                destination,
                source,
            } => {
                self.emit_indent();
                let dest = self.local_name(*destination);
                let src = self.local_reg(*source);
                let src_ty = self.get_local_type(mir_fn, *source);
                let llvm_ty = self.mir_type_to_llvm_str(&src_ty);

                // AddrOf 鑾峰彇鍙橀噺鐨勫湴鍧€
                // 1. 浣跨敤 getelementptr 鑾峰彇鍦板潃
                let temp = format!("{}.addr", dest);
                self.ir.push_str(&format!(
                    "{} = getelementptr {}, {}* {}, i64 0\n",
                    temp, llvm_ty, llvm_ty, src
                ));

                // 2. 灏嗗湴鍧€瀛樺偍鍒?destination
                self.emit_indent();
                let dest_ty = self.get_local_type(mir_fn, *destination);
                let dest_llvm_ty = self.mir_type_to_llvm_str(&dest_ty);
                // store 鎸囦护鏍煎紡: store <type> <value>, <type>* <pointer>
                // alloca 杩斿洖鐨勭被鍨嬫槸 <type>*锛屾墍浠ヨ繖閲岄渶瑕?<type>**
                let dest_ptr_ty = format!("{}*", dest_llvm_ty);
                self.ir.push_str(&format!(
                    "store {} {}, {} {}\n",
                    dest_llvm_ty, temp, dest_ptr_ty, dest
                ));
            }
            mir::Instruction::Load {
                destination,
                source,
            } => {
                // Load 鎸囦护锛氫粠 source 鍔犺浇鍊煎埌 destination
                let dest = self.local_name(*destination);
                let dest_ty = self.get_local_type(mir_fn, *destination);
                let src = self.local_reg(*source);
                let src_ty = self.get_local_type(mir_fn, *source);

                // 鍔犺浇鐨勫€肩被鍨嬪簲璇ヤ娇鐢?destination 鐨勭被鍨?
                let llvm_value_ty = self.mir_type_to_llvm_str(&dest_ty);

                // 瀵逛簬 Ptr 绫诲瀷鐨?source锛岄渶瑕佸弻閲嶅姞杞?
                match &src_ty {
                    MIRType::Ptr(inner) => {
                        // source 是 Ptr(T)，在 LLVM 中会：
                        // - alloca 杩斿洖 T**
                        // - 第一次加载得到 T*
                        // - 第二次加载得到 T
                        let inner_ty = self.mir_type_to_llvm_str(inner);
                        let ptr_ty = format!("{}*", inner_ty);
                        let ptr_ptr_ty = format!("{}*", ptr_ty);

                        // 第一次加载：从 alloca 加载指针值
                        self.emit_indent();
                        let temp_ptr = format!("{}.ptr", dest);
                        self.ir.push_str(&format!(
                            "{} = load {}, {} {}\n",
                            temp_ptr, ptr_ty, ptr_ptr_ty, src
                        ));

                        // 第二次加载：从指针加载实际值
                        self.emit_indent();
                        let temp_val = format!("{}.val", dest);
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            temp_val, llvm_value_ty, inner_ty, temp_ptr
                        ));

                        // 灏嗗€煎瓨鍌ㄥ埌 destination锛坅lloca锛?
                        self.emit_indent();
                        let dest_ptr_ty = format!("{}*", llvm_value_ty);
                        self.ir.push_str(&format!(
                            "store {} {}, {} {}\n",
                            llvm_value_ty, temp_val, dest_ptr_ty, dest
                        ));
                    }
                    MIRType::Ref(inner) => {
                        // Ref 类型的处理类似 Ptr
                        let inner_ty = self.mir_type_to_llvm_str(inner);
                        let ptr_ty = format!("{}*", inner_ty);
                        let ptr_ptr_ty = format!("{}*", ptr_ty);

                        self.emit_indent();
                        let temp_ptr = format!("{}.ptr", dest);
                        self.ir.push_str(&format!(
                            "{} = load {}, {} {}\n",
                            temp_ptr, ptr_ty, ptr_ptr_ty, src
                        ));

                        self.emit_indent();
                        let temp_val = format!("{}.val", dest);
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            temp_val, llvm_value_ty, inner_ty, temp_ptr
                        ));

                        self.emit_indent();
                        let dest_ptr_ty = format!("{}*", llvm_value_ty);
                        self.ir.push_str(&format!(
                            "store {} {}, {} {}\n",
                            llvm_value_ty, temp_val, dest_ptr_ty, dest
                        ));
                    }
                    _ => {
                        // 普通类型，需要额外的指针层级（alloca 返回指针）
                        let llvm_ptr_ty = self.mir_type_to_llvm_str(&src_ty);
                        let llvm_src_ptr_ty = format!("{}*", llvm_ptr_ty);

                        // 鍔犺浇鍊煎埌涓存椂鍙橀噺
                        self.emit_indent();
                        let temp_val = format!("{}.val", dest);
                        self.ir.push_str(&format!(
                            "{} = load {}, {} {}\n",
                            temp_val, llvm_value_ty, llvm_src_ptr_ty, src
                        ));

                        // 灏嗗€煎瓨鍌ㄥ埌 destination锛坅lloca锛?
                        self.emit_indent();
                        let dest_ptr_ty = format!("{}*", llvm_value_ty);
                        self.ir.push_str(&format!(
                            "store {} {}, {} {}\n",
                            llvm_value_ty, temp_val, dest_ptr_ty, dest
                        ));
                    }
                }
            }
            mir::Instruction::Unary {
                destination,
                op,
                operand,
            } => {
                self.emit_indent();
                let _operand_name = self.local_name(*operand);
                let dest = self.local_name(*destination);
                let ty = self.get_local_type(mir_fn, *destination);
                let llvm_ty = self.mir_type_to_llvm_str(&ty);

                // 鍔犺浇鎿嶄綔鏁?
                let temp = format!("{}.temp", dest);
                self.ir.push_str(&format!(
                    "{} = load {}, {}* {}\n",
                    temp,
                    llvm_ty,
                    llvm_ty,
                    self.local_reg(*operand)
                ));

                // 执行一元操作
                let op_inst = match op {
                    MirUnOp::Neg => {
                        if matches!(ty, MIRType::Float(_)) {
                            format!("{}.neg = fneg float {}", dest, temp)
                        } else {
                            format!("{}.neg = sub {} 0, {}", dest, llvm_ty, temp)
                        }
                    }
                    MirUnOp::Not => {
                        format!(
                            "{}.not = xor {} {}, {}",
                            dest,
                            llvm_ty,
                            temp,
                            self.mir_constant_to_llvm_str(&MirConstant::Bool(true))
                        )
                    }
                    MirUnOp::BitNot => {
                        format!(
                            "{}.bitnot = xor {} {}, {}",
                            dest,
                            llvm_ty,
                            temp,
                            self.mir_constant_to_llvm_str(&MirConstant::Int(-1))
                        )
                    }
                };
                self.ir.push_str(&format!("{}\n", op_inst));

                // 瀛樺偍缁撴灉
                self.emit_indent();
                let result_reg = format!("{dest}.res");
                self.ir.push_str(&format!(
                    "store {} {}, {}* {}\n",
                    llvm_ty,
                    result_reg,
                    llvm_ty,
                    self.local_reg(*destination)
                ));
            }
            mir::Instruction::Cast {
                destination,
                value,
                to,
            } => {
                let dest = self.local_name(*destination);
                let src_ty = self.get_local_type(mir_fn, *value);
                let src_llvm = self.mir_type_to_llvm_str(&src_ty);
                let dst_llvm = self.mir_type_to_llvm_str(to);
                let src_reg = self.local_reg(*value);

                self.emit_indent();
                let src_temp = format!("{}.cast.in", dest);
                self.ir.push_str(&format!(
                    "{} = load {}, {}* {}\n",
                    src_temp, src_llvm, src_llvm, src_reg
                ));

                if src_ty == *to {
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "store {} {}, {}* {}\n",
                        dst_llvm, src_temp, dst_llvm, dest
                    ));
                } else {
                    self.emit_indent();
                    let casted = format!("{}.cast.out", dest);
                    self.emit_cast_value(&casted, &src_temp, &src_ty, to);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "store {} {}, {}* {}\n",
                        dst_llvm, casted, dst_llvm, dest
                    ));
                }
            }
            mir::Instruction::Bitcast {
                destination,
                value,
                to,
            } => {
                let dest = self.local_name(*destination);
                let src_ty = self.get_local_type(mir_fn, *value);
                let src_llvm = self.mir_type_to_llvm_str(&src_ty);
                let dst_llvm = self.mir_type_to_llvm_str(to);
                let src_reg = self.local_reg(*value);

                if !common::supports_mir_bitcast(&src_ty, to) {
                    return Err(format!(
                        "invalid MIR bitcast from {} to {}",
                        src_llvm, dst_llvm
                    ));
                }

                self.emit_indent();
                let src_temp = format!("{}.bitcast.in", dest);
                self.ir.push_str(&format!(
                    "{} = load {}, {}* {}\n",
                    src_temp, src_llvm, src_llvm, src_reg
                ));

                self.emit_indent();
                let casted = format!("{}.bitcast.out", dest);
                self.ir.push_str(&format!(
                    "{} = bitcast {} {} to {}\n",
                    casted, src_llvm, src_temp, dst_llvm
                ));

                self.emit_indent();
                self.ir.push_str(&format!(
                    "store {} {}, {}* {}\n",
                    dst_llvm, casted, dst_llvm, dest
                ));
            }
            mir::Instruction::Binary {
                destination,
                op,
                left,
                right,
            } => {
                self.emit_indent();
                let left_reg = self.local_reg(*left);
                let right_reg = self.local_reg(*right);
                let dest = self.local_name(*destination);
                let ty = self.get_local_type(mir_fn, *left);
                let llvm_ty = self.mir_type_to_llvm_str(&ty);

                // 鍔犺浇鎿嶄綔鏁?
                let left_temp = format!("{}.l", dest);
                let right_temp = format!("{}.r", dest);
                self.ir.push_str(&format!(
                    "{} = load {}, {}* {}\n",
                    left_temp, llvm_ty, llvm_ty, left_reg
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{} = load {}, {}* {}\n",
                    right_temp, llvm_ty, llvm_ty, right_reg
                ));

                // 执行二元操作
                let op_inst = self.binary_op_to_llvm(*op, &ty, &left_temp, &right_temp);
                self.ir.push_str(&format!("{}\n", op_inst));

                // 瀛樺偍缁撴灉 - 姣旇緝鎿嶄綔杩斿洖 i1锛屽叾浠栨搷浣滆繑鍥炴搷浣滄暟绫诲瀷
                self.emit_indent();
                let dest_ty = self.get_local_type(mir_fn, *destination);
                let llvm_dest_ty = self.mir_type_to_llvm_str(&dest_ty);
                self.ir.push_str(&format!(
                    "store {} %result, {}* {}\n",
                    llvm_dest_ty,
                    llvm_dest_ty,
                    self.local_reg(*destination)
                ));
            }
            mir::Instruction::Store { destination, value } => {
                self.codegen_store_instruction(*destination, *value, mir_fn);
            }
            mir::Instruction::FieldAddr {
                destination,
                base,
                field,
            } => {
                // 结构体字段地址计算: ptr = &base.field（字段索引是常量）
                let base_ty = self.get_local_type(mir_fn, *base);
                let llvm_base_ty = self.mir_type_to_llvm_str(&base_ty);
                let dest = self.local_name(*destination);
                let base_reg = self.local_reg(*base);

                // 使用 getelementptr 获取字段地址
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                    dest, llvm_base_ty, llvm_base_ty, base_reg, field
                ));
            }
            mir::Instruction::IndexAddr {
                destination,
                base,
                index,
            } => {
                self.codegen_index_addr_instruction(*destination, *base, *index, mir_fn);
            }
            mir::Instruction::Call {
                destination,
                func,
                args,
            } => {
                self.emit_indent();

                // 鐗规畩澶勭悊鍐呯疆鍑芥暟
                if func == "print" || func == "println" {
                    // print 鍑芥暟
                    let arg = args.first();
                    if let Some(arg_local) = arg {
                        let arg_ty = self.get_local_type(mir_fn, *arg_local);
                        let arg_val = self.local_reg(*arg_local);

                        match &arg_ty {
                            MIRType::Int(_) => {
                                self.ir.push_str(&format!(
                                    "call void @puts(i8* bitcast i64* inttoptr i64 {} to i8*)\n",
                                    arg_val
                                ));
                            }
                            MIRType::Ptr(inner) | MIRType::Ref(inner) => {
                                if matches!(inner.as_ref(), MIRType::Int(8)) {
                                    // 字符串指针
                                    self.ir
                                        .push_str(&format!("call void @puts(i8* {})\n", arg_val));
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    // 普通函数调用
                    // 获取目标函数的签名（如果存在）
                    let target_param_tys = self
                        .function_signatures
                        .get(func)
                        .map(|(params, _)| params.clone())
                        .unwrap_or_default();

                    let mut args_codegen = Vec::new();

                    for (i, local) in args.iter().enumerate() {
                        let ty = self.get_local_type(mir_fn, *local);
                        let llvm_ty = self.mir_type_to_llvm_str(&ty);
                        let reg = self.local_reg(*local);

                        // 鍔犺浇鍙傛暟鍊?
                        let arg_temp = format!("%.arg.{}.{}", func, i);
                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            arg_temp, llvm_ty, llvm_ty, reg
                        ));

                        // 如果目标函数参数类型与当前类型不同，进行转换
                        let final_arg = if let Some(target_ty) = target_param_tys.get(i) {
                            let llvm_target_ty = self.mir_type_to_llvm_str(target_ty);
                            if llvm_ty != llvm_target_ty {
                                let converted = format!("%.arg.{}.{}.conv", func, i);
                                self.emit_indent();
                                self.emit_cast_value(&converted, &arg_temp, &ty, target_ty);
                                format!("{} {}", llvm_target_ty, converted)
                            } else {
                                format!("{} {}", llvm_ty, arg_temp)
                            }
                        } else {
                            format!("{} {}", llvm_ty, arg_temp)
                        };

                        args_codegen.push(final_arg);
                    }

                    let dest = self.local_name(*destination);
                    // 获取被调用函数的返回类型
                    let return_ty = self
                        .function_signatures
                        .get(func)
                        .map(|(_, ret)| self.mir_type_to_llvm_str(ret))
                        .unwrap_or_else(|| self.mir_type_to_llvm_str(&mir_fn.return_type));

                    self.emit_indent();
                    if return_ty != "void" {
                        // 浣跨敤涓存椂鍚嶇О浣滀负 call 鐨勭粨鏋滐紝鐒跺悗瀛樺偍鍒?destination
                        let call_result = format!("{}.call", dest);
                        self.ir.push_str(&format!(
                            "{} = call {} @{}({})\n",
                            call_result,
                            return_ty,
                            func,
                            args_codegen.join(", ")
                        ));

                        // 灏嗙粨鏋滃瓨鍌ㄥ埌 destination锛坅lloca锛?
                        self.emit_indent();
                        let dest_ptr_ty = format!("{}*", return_ty);
                        self.ir.push_str(&format!(
                            "store {} {}, {} {}\n",
                            return_ty, call_result, dest_ptr_ty, dest
                        ));
                    } else {
                        self.ir.push_str(&format!(
                            "call void @{}({})\n",
                            func,
                            args_codegen.join(", ")
                        ));
                    }
                }
            }
            mir::Instruction::Nop => {}
            mir::Instruction::Aggregate {
                destination,
                fields,
                ty,
            } => {
                self.codegen_aggregate_instruction(*destination, fields, ty, mir_fn)?;
            }
            mir::Instruction::Discriminant {
                destination,
                source,
            } => {
                let source_ty = self.get_local_type(mir_fn, *source);
                let source_llvm = self.mir_type_to_llvm_str(&source_ty);
                let discr_ptr = format!("%.enum.discr.ptr.{}", destination.id);
                let discr_value = format!("%.enum.discr.{}", destination.id);
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{discr_ptr} = getelementptr {source_llvm}, {source_llvm}* {}, i32 0, i32 0\n",
                    self.local_reg(*source)
                ));
                self.emit_indent();
                self.ir
                    .push_str(&format!("{discr_value} = load i64, i64* {discr_ptr}\n"));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "store i64 {discr_value}, i64* {}\n",
                    self.local_reg(*destination)
                ));
            }
            mir::Instruction::EnumConstruct {
                destination,
                discriminant,
                payload,
                enum_type,
            } => {
                let enum_llvm = self.mir_type_to_llvm_str(enum_type);
                let discr_ptr = format!("%.enum.discr.ptr.{}", destination.id);
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{discr_ptr} = getelementptr {enum_llvm}, {enum_llvm}* {}, i32 0, i32 0\n",
                    self.local_reg(*destination)
                ));
                self.emit_indent();
                self.ir
                    .push_str(&format!("store i64 {discriminant}, i64* {discr_ptr}\n"));

                if let Some(payload_local) = payload {
                    let payload_ty = self.get_local_type(mir_fn, *payload_local);
                    let payload_llvm = self.mir_type_to_llvm_str(&payload_ty);
                    let payload_value = format!("%.enum.payload.{}", destination.id);
                    let payload_bytes = format!("%.enum.payload.bytes.{}", destination.id);
                    let payload_ptr = format!("%.enum.payload.ptr.{}", destination.id);
                    let payload_size = common::enum_payload_storage_size(enum_type);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{payload_value} = load {payload_llvm}, {payload_llvm}* {}\n",
                        self.local_reg(*payload_local)
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{payload_bytes} = getelementptr {enum_llvm}, {enum_llvm}* {}, i32 0, i32 1\n",
                        self.local_reg(*destination)
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{payload_ptr} = bitcast [{payload_size} x i8]* {payload_bytes} to {payload_llvm}*\n"
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "store {payload_llvm} {payload_value}, {payload_llvm}* {payload_ptr}\n"
                    ));
                }
            }
            mir::Instruction::ExtractPayload {
                destination,
                source,
            } => {
                let source_ty = self.get_local_type(mir_fn, *source);
                let source_llvm = self.mir_type_to_llvm_str(&source_ty);
                let destination_ty = self.get_local_type(mir_fn, *destination);
                let destination_llvm = self.mir_type_to_llvm_str(&destination_ty);
                let payload_size = common::enum_payload_storage_size(&source_ty);
                let payload_bytes = format!("%.enum.payload.bytes.{}", destination.id);
                let payload_ptr = format!("%.enum.payload.ptr.{}", destination.id);
                let payload_value = format!("%.enum.payload.{}", destination.id);
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{payload_bytes} = getelementptr {source_llvm}, {source_llvm}* {}, i32 0, i32 1\n",
                    self.local_reg(*source)
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{payload_ptr} = bitcast [{payload_size} x i8]* {payload_bytes} to {destination_llvm}*\n"
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{payload_value} = load {destination_llvm}, {destination_llvm}* {payload_ptr}\n"
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "store {destination_llvm} {payload_value}, {destination_llvm}* {}\n",
                    self.local_reg(*destination)
                ));
            }
            mir::Instruction::Phi {
                destination,
                incoming,
            } => {
                let ty = self.get_local_type(mir_fn, *destination);
                if matches!(ty, MIRType::Unit | MIRType::Never)
                    || matches!(&ty, MIRType::Tuple(fields) if fields.is_empty())
                {
                    return Ok(());
                }

                let llvm_ty = self.mir_type_to_llvm_str(&ty);
                let phi_value = format!("%.phi.{}", destination.id);
                let entries = incoming
                    .iter()
                    .map(|(local, block)| {
                        let value = self
                            .phi_incoming_values
                            .get(&(destination.index(), *block, local.index()))
                            .cloned()
                            .unwrap_or_else(|| self.local_reg(*local));
                        format!("[ {value}, %bb_{block} ]")
                    })
                    .collect::<Vec<_>>();
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{phi_value} = phi {llvm_ty} {}\n",
                    entries.join(", ")
                ));
                self.emit_indent();
                self.ir.push_str(&format!(
                    "store {llvm_ty} {phi_value}, {llvm_ty}* {}\n",
                    self.local_reg(*destination)
                ));
            }
            _ => {
                self.emit_indent();
                self.ir
                    .push_str(&format!("; unhandled instruction: {:?}\n", inst));
            }
        }
        Ok(())
    }
}
