use super::method_call_helpers::lower_method_call_from_locals;
use super::*;
use crate::format_template::{parse_format_template, FormatSegment};
use crate::hir::HIRLiteral;

/// MIR type of the stdlib owned `String` (`struct String { handle: i64 }`).
fn owned_string_mir_type() -> MIRType {
    MIRType::Struct {
        name: "String".to_string(),
        fields: vec![("handle".to_string(), MIR_I64)],
    }
}

impl<'a> LoweringContext<'a> {
    /// Lower a `format(template, args...)` call into runtime String building.
    ///
    /// The template literal is parsed at compile time into literal chunks and
    /// `{}` placeholders; each placeholder renders the next positional argument
    /// via the owned-`String` runtime, returning the assembled `String` value.
    pub(super) fn lower_format_call(&mut self, args: &[HIRExpr]) -> Local {
        let handle = self.emit_new_string_handle();

        let Some((template_expr, value_args)) = args.split_first() else {
            self.errors
                .push("format requires a string literal template".to_string());
            return self.wrap_string_handle(handle);
        };
        let HIRExpr::Lit(HIRLiteral::String(template)) = template_expr else {
            self.errors
                .push("format template must be a string literal".to_string());
            return self.wrap_string_handle(handle);
        };
        let segments = match parse_format_template(template) {
            Ok(segments) => segments,
            Err(err) => {
                self.errors.push(err.message());
                return self.wrap_string_handle(handle);
            }
        };

        let mut arg_index = 0;
        for segment in &segments {
            match segment {
                FormatSegment::Literal(text) => self.emit_push_str_literal(handle, text),
                FormatSegment::Placeholder(placeholder) => {
                    let selected_arg_index = placeholder.position.unwrap_or(arg_index);
                    if let Some(arg) = value_args.get(selected_arg_index) {
                        let value = self.lower_expr(arg);
                        let value_ty = self.get_local_type(value).clone();
                        self.emit_push_format_value(handle, value, &value_ty);
                    }
                    if placeholder.position.is_none() {
                        arg_index += 1;
                    }
                }
            }
        }

        self.wrap_string_handle(handle)
    }

    /// `sengoo_string_new()` → fresh owned-string handle.
    fn emit_new_string_handle(&mut self) -> Local {
        let handle = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: handle,
            func: "sengoo_string_new".to_string(),
            args: vec![],
        });
        handle
    }

    /// Build the `String { handle }` value and schedule it for scope-end drop.
    fn wrap_string_handle(&mut self, handle: Local) -> Local {
        let string = self.add_local(None, LocalKind::Temp, owned_string_mir_type());
        self.push_inst(Instruction::Aggregate {
            destination: string,
            fields: vec![handle],
            ty: owned_string_mir_type(),
        });
        self.record_drop_binding_if_needed(string);
        string
    }

    fn emit_call_i64(&mut self, func: &str, args: Vec<Local>) -> Local {
        let dest = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: dest,
            func: func.to_string(),
            args,
        });
        dest
    }

    /// Append a literal chunk of the template to the string under construction.
    fn emit_push_str_literal(&mut self, handle: Local, text: &str) {
        let str_local = self.lower_literal(&HIRLiteral::String(text.to_string()));
        self.emit_push_cstr(handle, str_local);
    }

    /// Append the text behind a `&str`/`i8*` pointer value.
    fn emit_push_cstr(&mut self, handle: Local, str_ptr: Local) {
        let ptr_i64 = self.emit_call_i64("sengoo_stdlib_str_ptr", vec![str_ptr]);
        self.emit_call_i64("sengoo_string_push_str_status", vec![handle, ptr_i64]);
    }

    /// Append the UTF-8 text of an owned `String` (`{ handle }`) to `handle`.
    fn emit_push_owned_string(&mut self, handle: Local, string_value: Local) {
        let value_handle = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Extract {
            destination: value_handle,
            value: string_value,
            index: 0,
        });
        let ptr_i64 = self.emit_call_i64("sengoo_string_as_str_ptr", vec![value_handle]);
        self.emit_call_i64("sengoo_string_push_str_status", vec![handle, ptr_i64]);
    }

    /// Render a single placeholder argument into the string being built.
    fn emit_push_format_value(&mut self, handle: Local, value: Local, value_ty: &MIRType) {
        if let MIRType::Struct { name, .. } = value_ty {
            let name = name.clone();
            if name == "String" {
                self.emit_push_owned_string(handle, value);
                return;
            }
            if self.has_display_to_string(&name) {
                let rendered = lower_method_call_from_locals(self, value, "to_string", &[]);
                self.emit_push_owned_string(handle, rendered);
                self.record_drop_binding_if_needed(rendered);
                return;
            }
        }
        match value_ty {
            MIRType::Int(_) => {
                self.emit_call_i64("sengoo_string_push_i64_status", vec![handle, value]);
            }
            MIRType::Bool => {
                self.emit_call_i64("sengoo_string_push_bool_status", vec![handle, value]);
            }
            MIRType::Ptr(_) | MIRType::Ref(_) => self.emit_push_cstr(handle, value),
            _ => {
                self.errors.push(format!(
                    "format: unsupported argument MIR type for lowering: {:?}",
                    value_ty
                ));
            }
        }
    }
}
