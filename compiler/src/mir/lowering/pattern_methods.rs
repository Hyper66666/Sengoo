use super::*;

impl<'a> LoweringContext<'a> {
    /// 从枚举模式中提取判别值（discriminant）并生成匹配判断逻辑。
    /// 判断给定值是否匹配 HIR 模式，用于运行时合约检查。
    pub(super) fn matches_pattern(&mut self, pat: &crate::hir::HIRPattern, value: Local) -> Local {
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
        }
    }

    /// 将HIR模式绑定降级为MIR，生成对应的局部变量绑定指令。
    /// 将模式绑定降级到MIR，生成模式匹配的局部变量绑定指令。
    pub(super) fn lower_pattern_bindings(
        &mut self,
        pat: &crate::hir::HIRPattern,
        enum_value: Local,
    ) {
        match pattern_binding_plan(pat) {
            PatternBindingPlan::Ignore => {}
            PatternBindingPlan::BindWhole(name) => {
                let _ = self.add_local(Some(name), LocalKind::User, MIR_I64);
            }
            PatternBindingPlan::BindTupleFields(fields) => {
                let payload_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                self.push_inst(Instruction::ExtractPayload {
                    destination: payload_local,
                    source: enum_value,
                });
                for (index, name) in fields {
                    let field_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                    self.push_inst(Instruction::Extract {
                        destination: field_local,
                        value: payload_local,
                        index,
                    });
                    let bound_local = self.add_local(Some(name), LocalKind::User, MIR_I64);
                    self.push_inst(Instruction::Store {
                        destination: bound_local,
                        value: field_local,
                    });
                }
            }
        }
    }
}
