use super::method_call_helpers::lower_method_call_from_locals;
use super::*;

#[derive(Clone, Copy)]
enum PrintSink {
    Stdout,
    Stderr,
}

impl PrintSink {
    fn i64_func(self) -> &'static str {
        match self {
            Self::Stdout => "sengoo_print_i64",
            Self::Stderr => "sengoo_eprint_i64",
        }
    }

    fn bool_func(self) -> &'static str {
        match self {
            Self::Stdout => "sengoo_print_bool",
            Self::Stderr => "sengoo_eprint_bool",
        }
    }

    fn f64_func(self) -> &'static str {
        match self {
            Self::Stdout => "sengoo_print_f64",
            Self::Stderr => "sengoo_eprint_f64",
        }
    }

    fn str_func(self) -> &'static str {
        match self {
            Self::Stdout => "sengoo_print_str",
            Self::Stderr => "sengoo_eprint_str",
        }
    }

    fn string_func(self) -> &'static str {
        match self {
            Self::Stdout => "sengoo_print_string",
            Self::Stderr => "sengoo_eprint_string",
        }
    }
}

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

    fn emit_print_str_literal(&mut self, text: &str, sink: PrintSink) {
        let str_local = self.lower_literal(&HIRLiteral::String(text.to_string()));
        self.emit_runtime_print_call(sink.str_func(), str_local);
    }

    pub(super) fn emit_print_value(&mut self, value_local: Local, value_ty: &MIRType) {
        self.emit_print_value_to(value_local, value_ty, PrintSink::Stdout);
    }

    pub(super) fn emit_eprint_value(&mut self, value_local: Local, value_ty: &MIRType) {
        self.emit_print_value_to(value_local, value_ty, PrintSink::Stderr);
    }

    /// Whether `type_name` has a user `impl Display` whose `to_string` was
    /// lowered to the eager `<Type>_Display_to_string` runtime function.
    pub(super) fn has_display_to_string(&self, type_name: &str) -> bool {
        self.is_known_function(&format!("{}_Display_to_string", type_name))
    }

    /// Print the UTF-8 text of an owned `String` value (`{ handle: i64 }`).
    fn emit_print_owned_string(&mut self, string_local: Local, sink: PrintSink) {
        let handle = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Extract {
            destination: handle,
            value: string_local,
            index: 0,
        });
        self.emit_runtime_print_call(sink.string_func(), handle);
    }

    fn emit_print_value_to(&mut self, value_local: Local, value_ty: &MIRType, sink: PrintSink) {
        if let MIRType::Struct { name, .. } = value_ty {
            let name = name.clone();
            // The owned `String` prints its own UTF-8 text.
            if name == "String" {
                self.emit_print_owned_string(value_local, sink);
                return;
            }
            // Dispatch through a user `Display` impl when one exists, printing the
            // owned `String` it returns. The temporary is scheduled for the usual
            // scope-end drop so the produced text is not leaked.
            if self.has_display_to_string(&name) {
                let rendered = lower_method_call_from_locals(self, value_local, "to_string", &[]);
                self.emit_print_owned_string(rendered, sink);
                self.record_drop_binding_if_needed(rendered);
                return;
            }
        }
        match value_ty {
            MIRType::Struct { name, fields } => {
                self.emit_print_str_literal(&format!("{} {{ ", name), sink);

                for (index, (field_name, field_ty)) in fields.iter().enumerate() {
                    if index > 0 {
                        self.emit_print_str_literal(", ", sink);
                    }
                    self.emit_print_str_literal(&format!("{}: ", field_name), sink);

                    let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
                    self.push_inst(Instruction::Extract {
                        destination: field_local,
                        value: value_local,
                        index: index as u32,
                    });

                    self.emit_print_value_to(field_local, field_ty, sink);
                }

                self.emit_print_str_literal(" }", sink);
            }
            MIRType::Int(_) => self.emit_runtime_print_call(sink.i64_func(), value_local),
            MIRType::Bool => self.emit_runtime_print_call(sink.bool_func(), value_local),
            MIRType::Float(_) => self.emit_runtime_print_call(sink.f64_func(), value_local),
            MIRType::Ptr(_) | MIRType::Ref(_) => {
                self.emit_runtime_print_call(sink.str_func(), value_local)
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
