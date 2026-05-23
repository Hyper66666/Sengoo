use super::*;

impl<'a> LoweringContext<'a> {
    /// 生成运行时打印调用的指令（用于调试输出）。
    fn emit_runtime_print_call(&mut self, func: &str, arg_local: Local) {
        let call_local = self.add_local(None, LocalKind::Temp, MIR_UNIT);
        self.push_inst(Instruction::Call {
            destination: call_local,
            func: func.to_string(),
            args: vec![arg_local],
        });
    }

    fn emit_print_str_literal(&mut self, text: &str) {
        let str_local = self.lower_literal(&HIRLiteral::String(text.to_string()));
        self.emit_runtime_print_call("sengoo_print_str", str_local);
    }

    pub(super) fn emit_print_value(&mut self, value_local: Local, value_ty: &MIRType) {
        match value_ty {
            MIRType::Struct { name, fields } => {
                self.emit_print_str_literal(&format!("{} {{ ", name));

                for (index, (field_name, field_ty)) in fields.iter().enumerate() {
                    if index > 0 {
                        self.emit_print_str_literal(", ");
                    }
                    self.emit_print_str_literal(&format!("{}: ", field_name));

                    let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
                    self.push_inst(Instruction::Extract {
                        destination: field_local,
                        value: value_local,
                        index: index as u32,
                    });

                    self.emit_print_value(field_local, field_ty);
                }

                self.emit_print_str_literal(" }");
            }
            MIRType::Int(_) => self.emit_runtime_print_call("sengoo_print_i64", value_local),
            MIRType::Bool => self.emit_runtime_print_call("sengoo_print_bool", value_local),
            MIRType::Float(_) => self.emit_runtime_print_call("sengoo_print_f64", value_local),
            MIRType::Ptr(_) | MIRType::Ref(_) => {
                self.emit_runtime_print_call("sengoo_print_str", value_local)
            }
            _ => {
                self.errors.push(format!(
                    "print: unsupported MIR type for lowering: {:?}",
                    value_ty
                ));
            }
        }
    }
}
