use super::*;

pub(super) struct SavedPatternBindings {
    names: Vec<(String, Option<Local>)>,
    symbols: Vec<(SymbolId, Option<Local>)>,
}

impl<'a> LoweringContext<'a> {
    pub(super) fn save_pattern_bindings(
        &self,
        pat: &crate::hir::HIRPattern,
    ) -> SavedPatternBindings {
        let names = pattern_binding_names(pat)
            .into_iter()
            .map(|name| {
                let previous = self.local_names.get(&name).copied();
                (name, previous)
            })
            .collect();
        let mut symbols = Vec::new();
        for symbol in pattern_binding_symbols(pat) {
            if !symbol.is_valid() || symbols.iter().any(|(saved, _)| *saved == symbol) {
                continue;
            }
            symbols.push((symbol, self.local_symbols.get(&symbol).copied()));
        }
        SavedPatternBindings { names, symbols }
    }

    pub(super) fn restore_pattern_bindings(&mut self, saved: SavedPatternBindings) {
        for (symbol, previous) in saved.symbols.into_iter().rev() {
            if let Some(local) = previous {
                self.local_symbols.insert(symbol, local);
            } else {
                self.local_symbols.remove(&symbol);
            }
        }
        for (name, previous) in saved.names.into_iter().rev() {
            if let Some(local) = previous {
                self.local_names.insert(name, local);
            } else {
                self.local_names.remove(&name);
            }
        }
    }

    pub(super) fn matches_pattern(&mut self, pat: &crate::hir::HIRPattern, value: Local) -> Local {
        if let crate::hir::HIRPattern::Or(lhs, rhs) = pat {
            let left_ok = self.matches_pattern(lhs, value);
            let right_ok = self.matches_pattern(rhs, value);
            let result = self.add_local(None, LocalKind::Temp, MIR_BOOL);
            self.push_inst(Instruction::Binary {
                destination: result,
                op: MirBinOp::LogOr,
                left: left_ok,
                right: right_ok,
            });
            return result;
        }

        let result = self.add_local(None, LocalKind::Temp, MIR_BOOL);

        match pattern_match_plan(pat) {
            PatternMatchPlan::AlwaysTrue => {
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
            PatternMatchPlan::EqLiteral(lit) => {
                let lit_local = self.lower_literal(&lit);
                self.push_inst(Instruction::Binary {
                    destination: result,
                    op: MirBinOp::Eq,
                    left: value,
                    right: lit_local,
                });
                result
            }
            PatternMatchPlan::EqDiscriminant(expected) => {
                let discr_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                self.push_inst(Instruction::Discriminant {
                    destination: discr_local,
                    source: value,
                });
                let expected_local = self.lower_literal(&HIRLiteral::Int(i64::from(expected)));
                self.push_inst(Instruction::Binary {
                    destination: result,
                    op: MirBinOp::Eq,
                    left: discr_local,
                    right: expected_local,
                });
                result
            }
        }
    }

    pub(super) fn lower_pattern_bindings(
        &mut self,
        pat: &crate::hir::HIRPattern,
        enum_value: Local,
        owns_value: bool,
    ) {
        let discriminant = enum_pattern_discriminant(pat);
        let scrutinee_ty = self.get_local_type(enum_value).clone();
        match pattern_binding_plan(pat) {
            PatternBindingPlan::Ignore => {
                if !owns_value {
                    return;
                }

                // A wildcard payload still owns the value selected by this
                // arm. Keep it in a temporary so the arm scope can release
                // it even when the scrutinee was itself an untracked temp.
                if let Some(discriminant) = discriminant {
                    let Some(payload_ty) = enum_payload_ty(&scrutinee_ty, Some(discriminant))
                    else {
                        return;
                    };
                    let payload_local = self.add_local(None, LocalKind::Temp, payload_ty);
                    self.push_inst(Instruction::ExtractPayload {
                        destination: payload_local,
                        source: enum_value,
                    });
                    self.record_drop_binding_if_needed(payload_local);
                } else if matches!(scrutinee_ty, MIRType::Enum { .. }) {
                    let owned = self.add_local(None, LocalKind::User, scrutinee_ty.clone());
                    self.push_inst(Instruction::Store {
                        destination: owned,
                        value: enum_value,
                    });
                    self.record_drop_binding_if_needed(owned);
                }
            }
            PatternBindingPlan::BindWhole(name) => {
                let ty = scrutinee_ty.clone();
                let symbol = pattern_binding_symbol(pat, &name);
                let bound = self.add_local(Some(name), LocalKind::User, ty);
                if let Some(symbol) = symbol {
                    self.bind_local_symbol(symbol, bound);
                }
                self.push_inst(Instruction::Store {
                    destination: bound,
                    value: enum_value,
                });
                if owns_value {
                    self.mark_drop_local_moved(enum_value);
                    self.record_drop_binding_if_needed(bound);
                }
            }
            PatternBindingPlan::BindTupleFields(fields) => {
                let payload_ty = enum_payload_ty(&scrutinee_ty, discriminant).unwrap_or(MIR_I64);
                let payload_local = self.add_local(None, LocalKind::Temp, payload_ty.clone());
                self.push_inst(Instruction::ExtractPayload {
                    destination: payload_local,
                    source: enum_value,
                });
                if owns_value {
                    self.record_drop_binding_if_needed(payload_local);
                }
                // A single-element tuple variant stores its element directly
                // as the payload (see `payload_to_mir`), even when that
                // element is itself a struct such as `String` — bind it
                // whole. Only a true multi-element `Tuple` payload is split
                // per index.
                if fields.len() == 1 && !matches!(payload_ty, MIRType::Tuple(_)) {
                    let (_, name) = &fields[0];
                    let symbol = pattern_binding_symbol(pat, name);
                    let bound_local =
                        self.add_local(Some(name.clone()), LocalKind::User, payload_ty.clone());
                    if let Some(symbol) = symbol {
                        self.bind_local_symbol(symbol, bound_local);
                    }
                    self.push_inst(Instruction::Store {
                        destination: bound_local,
                        value: payload_local,
                    });
                    if owns_value {
                        self.mark_drop_local_moved(payload_local);
                        self.record_drop_binding_if_needed(bound_local);
                    }
                } else {
                    for (index, name) in fields {
                        let symbol = pattern_binding_symbol(pat, &name);
                        let field_ty = tuple_index_ty(&payload_ty, index).unwrap_or(MIR_I64);
                        let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
                        self.push_inst(Instruction::Extract {
                            destination: field_local,
                            value: payload_local,
                            index,
                        });
                        let bound_local = self.add_local(Some(name), LocalKind::User, field_ty);
                        if let Some(symbol) = symbol {
                            self.bind_local_symbol(symbol, bound_local);
                        }
                        self.push_inst(Instruction::Store {
                            destination: bound_local,
                            value: field_local,
                        });
                        if owns_value {
                            self.mark_drop_field_moved(payload_local, vec![index]);
                            self.record_drop_binding_if_needed(bound_local);
                        }
                    }
                }
            }
            PatternBindingPlan::BindStructFields(fields) => {
                let payload_ty =
                    enum_payload_ty(&scrutinee_ty, discriminant).unwrap_or(scrutinee_ty.clone());
                let payload_local = self.add_local(None, LocalKind::Temp, payload_ty.clone());
                self.push_inst(Instruction::ExtractPayload {
                    destination: payload_local,
                    source: enum_value,
                });
                if owns_value {
                    self.record_drop_binding_if_needed(payload_local);
                }
                for (field_index, (field_name, bind_name)) in fields.iter().enumerate() {
                    let symbol = pattern_binding_symbol(pat, bind_name);
                    let field_ty = struct_field_ty(&payload_ty, field_name).unwrap_or(MIR_I64);
                    let field_index =
                        struct_field_index(&payload_ty, field_name).unwrap_or(field_index as u32);
                    let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
                    self.push_inst(Instruction::Extract {
                        destination: field_local,
                        value: payload_local,
                        index: field_index,
                    });
                    let bound_local =
                        self.add_local(Some(bind_name.clone()), LocalKind::User, field_ty);
                    if let Some(symbol) = symbol {
                        self.bind_local_symbol(symbol, bound_local);
                    }
                    self.push_inst(Instruction::Store {
                        destination: bound_local,
                        value: field_local,
                    });
                    if owns_value {
                        self.mark_drop_field_moved(payload_local, vec![field_index]);
                        self.record_drop_binding_if_needed(bound_local);
                    }
                }
            }
        }
    }
}

fn pattern_binding_names(pat: &crate::hir::HIRPattern) -> Vec<String> {
    match pattern_binding_plan(pat) {
        PatternBindingPlan::Ignore => Vec::new(),
        PatternBindingPlan::BindWhole(name) => vec![name],
        PatternBindingPlan::BindTupleFields(fields) => {
            fields.into_iter().map(|(_, name)| name).collect()
        }
        PatternBindingPlan::BindStructFields(fields) => {
            fields.into_iter().map(|(_, name)| name).collect()
        }
    }
}

fn pattern_binding_symbols(pat: &crate::hir::HIRPattern) -> Vec<SymbolId> {
    pattern_binding_names(pat)
        .iter()
        .filter_map(|name| pattern_binding_symbol(pat, name))
        .collect()
}

fn pattern_binding_symbol(pat: &crate::hir::HIRPattern, binding_name: &str) -> Option<SymbolId> {
    match pat {
        crate::hir::HIRPattern::Var { name, symbol, .. } if name == binding_name => Some(*symbol),
        crate::hir::HIRPattern::Struct { fields, .. }
        | crate::hir::HIRPattern::EnumVariant { fields, .. } => fields
            .iter()
            .filter_map(|(_, pattern)| pattern.as_ref())
            .find_map(|pattern| pattern_binding_symbol(pattern, binding_name)),
        crate::hir::HIRPattern::Tuple(patterns) => patterns
            .iter()
            .find_map(|pattern| pattern_binding_symbol(pattern, binding_name)),
        crate::hir::HIRPattern::Or(lhs, rhs) => pattern_binding_symbol(lhs, binding_name)
            .or_else(|| pattern_binding_symbol(rhs, binding_name)),
        crate::hir::HIRPattern::Slice {
            before,
            rest,
            after,
        } => before
            .iter()
            .chain(rest.iter().map(Box::as_ref))
            .chain(after.iter())
            .find_map(|pattern| pattern_binding_symbol(pattern, binding_name)),
        crate::hir::HIRPattern::Ref(pattern) | crate::hir::HIRPattern::RefMut(pattern) => {
            pattern_binding_symbol(pattern, binding_name)
        }
        _ => None,
    }
}

fn enum_pattern_discriminant(pat: &crate::hir::HIRPattern) -> Option<u32> {
    match pat {
        crate::hir::HIRPattern::EnumVariant { discriminant, .. } => Some(*discriminant),
        crate::hir::HIRPattern::Or(lhs, _) => enum_pattern_discriminant(lhs),
        _ => None,
    }
}

fn enum_payload_ty(ty: &MIRType, discriminant: Option<u32>) -> Option<MIRType> {
    let discriminant = discriminant?;
    match ty {
        MIRType::Enum { variants, .. } => variants.iter().find_map(|(discr, payload)| {
            if *discr == discriminant {
                payload.clone()
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn tuple_index_ty(ty: &MIRType, index: u32) -> Option<MIRType> {
    match ty {
        MIRType::Tuple(items) => items.get(index as usize).cloned(),
        _ => None,
    }
}

fn struct_field_ty(ty: &MIRType, field_name: &str) -> Option<MIRType> {
    match ty {
        MIRType::Struct { fields, .. } => fields
            .iter()
            .find(|(name, _)| name == field_name)
            .map(|(_, ty)| ty.clone()),
        _ => None,
    }
}

fn struct_field_index(ty: &MIRType, field_name: &str) -> Option<u32> {
    match ty {
        MIRType::Struct { fields, .. } => fields
            .iter()
            .position(|(name, _)| name == field_name)
            .map(|index| index as u32),
        _ => None,
    }
}
