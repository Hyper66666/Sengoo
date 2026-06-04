use super::*;

impl<'a> LoweringContext<'a> {
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
    ) {
        let discriminant = enum_pattern_discriminant(pat);
        match pattern_binding_plan(pat) {
            PatternBindingPlan::Ignore => {}
            PatternBindingPlan::BindWhole(name) => {
                let ty = self.get_local_type(enum_value).clone();
                let bound = self.add_local(Some(name), LocalKind::User, ty);
                self.push_inst(Instruction::Store {
                    destination: bound,
                    value: enum_value,
                });
            }
            PatternBindingPlan::BindTupleFields(fields) => {
                let scrutinee_ty = self.get_local_type(enum_value).clone();
                let payload_ty = enum_payload_ty(&scrutinee_ty, discriminant).unwrap_or(MIR_I64);
                let payload_local = self.add_local(None, LocalKind::Temp, payload_ty.clone());
                self.push_inst(Instruction::ExtractPayload {
                    destination: payload_local,
                    source: enum_value,
                });
                if fields.len() == 1 && !payload_has_fields(&payload_ty) {
                    let (_, name) = &fields[0];
                    let bound_local =
                        self.add_local(Some(name.clone()), LocalKind::User, payload_ty.clone());
                    self.push_inst(Instruction::Store {
                        destination: bound_local,
                        value: payload_local,
                    });
                } else {
                    for (index, name) in fields {
                        let field_ty = tuple_index_ty(&payload_ty, index).unwrap_or(MIR_I64);
                        let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
                        self.push_inst(Instruction::Extract {
                            destination: field_local,
                            value: payload_local,
                            index,
                        });
                        let bound_local = self.add_local(Some(name), LocalKind::User, field_ty);
                        self.push_inst(Instruction::Store {
                            destination: bound_local,
                            value: field_local,
                        });
                    }
                }
            }
            PatternBindingPlan::BindStructFields(fields) => {
                let scrutinee_ty = self.get_local_type(enum_value).clone();
                let payload_ty =
                    enum_payload_ty(&scrutinee_ty, discriminant).unwrap_or(scrutinee_ty.clone());
                let payload_local = self.add_local(None, LocalKind::Temp, payload_ty.clone());
                self.push_inst(Instruction::ExtractPayload {
                    destination: payload_local,
                    source: enum_value,
                });
                for (field_index, (field_name, bind_name)) in fields.iter().enumerate() {
                    let field_ty = struct_field_ty(&payload_ty, field_name).unwrap_or(MIR_I64);
                    let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
                    self.push_inst(Instruction::Extract {
                        destination: field_local,
                        value: payload_local,
                        index: field_index as u32,
                    });
                    let bound_local =
                        self.add_local(Some(bind_name.clone()), LocalKind::User, field_ty);
                    self.push_inst(Instruction::Store {
                        destination: bound_local,
                        value: field_local,
                    });
                }
            }
        }
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

fn payload_has_fields(ty: &MIRType) -> bool {
    matches!(ty, MIRType::Struct { .. } | MIRType::Tuple(_))
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
