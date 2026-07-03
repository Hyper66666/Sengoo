use super::*;
use crate::format_template::{
    parse_format_template, FormatAlign, FormatPlaceholder, FormatSegment, FormatStyle,
};
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
                        if placeholder.width.is_some() {
                            self.emit_push_padded_format_value(
                                handle,
                                value,
                                &value_ty,
                                placeholder,
                            );
                        } else {
                            self.emit_push_format_value(
                                handle,
                                value,
                                &value_ty,
                                placeholder.style,
                                placeholder.precision,
                            );
                        }
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

    fn emit_i64_const(&mut self, value: i64) -> Local {
        let dest = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: dest,
            value: MirConstant::Int(value),
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

    fn has_debug_to_string(&self, type_name: &str) -> bool {
        self.is_known_function(&format!("{}_Debug_to_string", type_name))
    }

    fn emit_push_trait_to_string(
        &mut self,
        handle: Local,
        value: Local,
        type_name: &str,
        trait_name: &str,
    ) {
        let rendered = self.add_local(None, LocalKind::Temp, owned_string_mir_type());
        self.type_names.insert(rendered, "String".to_string());
        self.push_inst(Instruction::Call {
            destination: rendered,
            func: format!("{}_{}_to_string", type_name, trait_name),
            args: vec![value],
        });
        self.emit_push_owned_string(handle, rendered);
        self.record_drop_binding_if_needed(rendered);
    }

    /// Render a single placeholder argument into the string being built.
    fn emit_push_format_value(
        &mut self,
        handle: Local,
        value: Local,
        value_ty: &MIRType,
        style: FormatStyle,
        precision: Option<usize>,
    ) {
        if precision.is_some() && !matches!(value_ty, MIRType::Float(_)) {
            self.errors.push(
                "format: precision is currently supported only for f64 arguments".to_string(),
            );
            return;
        }
        if let MIRType::Struct { name, .. } = value_ty {
            let name = name.clone();
            if name == "String" {
                self.emit_push_owned_string(handle, value);
                return;
            }
            if style == FormatStyle::Debug {
                if self.has_debug_to_string(&name) {
                    self.emit_push_trait_to_string(handle, value, &name, "Debug");
                    return;
                }
                self.emit_push_struct_debug_value(handle, value, value_ty);
                return;
            }
            if self.has_display_to_string(&name) {
                self.emit_push_trait_to_string(handle, value, &name, "Display");
                return;
            }
        }
        match value_ty {
            MIRType::Float(_) => {
                let precision_local = self.emit_i64_const(precision.unwrap_or(6) as i64);
                self.emit_call_i64(
                    "sengoo_string_push_f64_precision_status",
                    vec![handle, value, precision_local],
                );
            }
            MIRType::Int(_) => {
                self.emit_call_i64("sengoo_string_push_i64_status", vec![handle, value]);
            }
            MIRType::Bool => {
                self.emit_call_i64("sengoo_string_push_bool_status", vec![handle, value]);
            }
            MIRType::Ptr(_) | MIRType::Ref(_) => self.emit_push_cstr(handle, value),
            MIRType::Enum { .. } if style == FormatStyle::Debug => {
                if let Some(enum_name) = self.enum_name_for_mir_type(value_ty) {
                    if self.has_debug_to_string(&enum_name) {
                        self.emit_push_trait_to_string(handle, value, &enum_name, "Debug");
                        return;
                    }
                }
                self.emit_push_enum_debug_value(handle, value, value_ty);
            }
            _ => {
                self.errors.push(format!(
                    "format: unsupported argument MIR type for lowering: {:?}",
                    value_ty
                ));
            }
        }
    }

    fn enum_name_for_mir_type(&self, value_ty: &MIRType) -> Option<String> {
        self.options
            .enum_defs
            .values()
            .find(|def| def.mir_type() == *value_ty)
            .map(|def| def.name.clone())
    }

    fn emit_push_struct_debug_value(&mut self, handle: Local, value: Local, value_ty: &MIRType) {
        let MIRType::Struct { name, fields } = value_ty else {
            return;
        };

        self.emit_push_str_literal(handle, &format!("{} {{ ", name));
        for (index, (field_name, field_ty)) in fields.iter().enumerate() {
            if index > 0 {
                self.emit_push_str_literal(handle, ", ");
            }
            self.emit_push_str_literal(handle, &format!("{}: ", field_name));
            let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
            self.push_inst(Instruction::Extract {
                destination: field_local,
                value,
                index: index as u32,
            });
            self.emit_push_format_value(handle, field_local, field_ty, FormatStyle::Debug, None);
        }
        self.emit_push_str_literal(handle, " }");
    }

    fn emit_push_enum_debug_value(&mut self, handle: Local, value: Local, value_ty: &MIRType) {
        let Some(enum_def) = self
            .options
            .enum_defs
            .values()
            .find(|def| def.mir_type() == *value_ty)
            .cloned()
        else {
            self.errors
                .push("format: could not resolve enum metadata for Debug".to_string());
            return;
        };

        let discr = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Discriminant {
            destination: discr,
            source: value,
        });

        let join_block = self.new_block();
        let otherwise_block = self.new_block();
        let mut targets = Vec::with_capacity(enum_def.variants.len());
        let mut variant_blocks = Vec::with_capacity(enum_def.variants.len());
        for (discriminant, _, _) in &enum_def.variants {
            let block = self.new_block();
            targets.push((*discriminant, block));
            variant_blocks.push(block);
        }

        self.set_terminator(Terminator::Switch {
            discr,
            targets,
            otherwise: otherwise_block,
        });

        for ((_, variant_name, _), block) in enum_def.variants.iter().zip(variant_blocks) {
            self.set_current_block(block);
            self.emit_push_enum_variant_debug(handle, &enum_def.name, variant_name, value);
            self.set_terminator(Terminator::Goto(join_block));
        }

        self.set_current_block(otherwise_block);
        self.emit_push_str_literal(handle, &format!("{}::<unknown>", enum_def.name));
        self.set_terminator(Terminator::Goto(join_block));

        self.set_current_block(join_block);
    }

    fn emit_push_enum_variant_debug(
        &mut self,
        handle: Local,
        enum_name: &str,
        variant_name: &str,
        enum_value: Local,
    ) {
        let Some(enum_def) = self.options.enum_defs.get(enum_name).cloned() else {
            self.emit_push_str_literal(handle, &format!("{enum_name}::{variant_name}"));
            return;
        };
        let payload_ty = enum_def
            .variants
            .iter()
            .find(|(_, name, _)| name == variant_name)
            .and_then(|(_, _, payload)| payload.clone());

        self.emit_push_str_literal(handle, &format!("{enum_name}::{variant_name}"));
        let Some(payload_ty) = payload_ty else {
            return;
        };

        let payload = self.add_local(None, LocalKind::Temp, payload_ty.clone());
        self.push_inst(Instruction::ExtractPayload {
            destination: payload,
            source: enum_value,
        });
        match &payload_ty {
            MIRType::Tuple(fields) => {
                self.emit_push_str_literal(handle, "(");
                for (index, field_ty) in fields.iter().enumerate() {
                    if index > 0 {
                        self.emit_push_str_literal(handle, ", ");
                    }
                    let field = self.add_local(None, LocalKind::Temp, field_ty.clone());
                    self.push_inst(Instruction::Extract {
                        destination: field,
                        value: payload,
                        index: index as u32,
                    });
                    self.emit_push_format_value(handle, field, field_ty, FormatStyle::Debug, None);
                }
                self.emit_push_str_literal(handle, ")");
            }
            MIRType::Struct { fields, .. } => {
                self.emit_push_str_literal(handle, " { ");
                for (index, (field_name, field_ty)) in fields.iter().enumerate() {
                    if index > 0 {
                        self.emit_push_str_literal(handle, ", ");
                    }
                    self.emit_push_str_literal(handle, &format!("{field_name}: "));
                    let field = self.add_local(None, LocalKind::Temp, field_ty.clone());
                    self.push_inst(Instruction::Extract {
                        destination: field,
                        value: payload,
                        index: index as u32,
                    });
                    self.emit_push_format_value(handle, field, field_ty, FormatStyle::Debug, None);
                }
                self.emit_push_str_literal(handle, " }");
            }
            _ => {
                self.emit_push_str_literal(handle, "(");
                self.emit_push_format_value(handle, payload, &payload_ty, FormatStyle::Debug, None);
                self.emit_push_str_literal(handle, ")");
            }
        }
    }

    fn emit_push_padded_format_value(
        &mut self,
        handle: Local,
        value: Local,
        value_ty: &MIRType,
        placeholder: &FormatPlaceholder,
    ) {
        let temp_handle = self.emit_new_string_handle();
        self.emit_push_format_value(
            temp_handle,
            value,
            value_ty,
            placeholder.style,
            placeholder.precision,
        );
        let align_code = match placeholder.align {
            FormatAlign::None | FormatAlign::Right => 1,
        };
        let align_local = self.emit_i64_const(align_code);
        let width_local = self.emit_i64_const(placeholder.width.unwrap_or(0) as i64);
        self.emit_call_i64(
            "sengoo_string_push_padded_string_status",
            vec![handle, temp_handle, align_local, width_local],
        );
        self.emit_call_i64("sengoo_string_free_status", vec![temp_handle]);
    }
}
