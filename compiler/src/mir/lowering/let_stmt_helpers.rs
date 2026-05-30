use super::*;

pub(super) fn lower_let_stmt(
    ctx: &mut LoweringContext<'_>,
    name: &str,
    symbol: SymbolId,
    ty: &HIRType,
    value: Option<&HIRExpr>,
    is_mut: bool,
) {
    let _ = is_mut;
    let kind = LocalKind::User;
    let mir_ty = ty.clone().into();

    if let Some(value_expr) = value {
        let value_local = if let HIRExpr::Lambda { params, body } = value_expr {
            lower_lambda_expr_with_expected(
                ctx,
                params,
                body,
                Some(hir_type_to_mir_with_structs(ty, ctx.struct_defs)),
            )
        } else {
            ctx.lower_expr(value_expr)
        };
        let lambda_name = ctx.lambda_names.get(&value_local).cloned();

        if let Some(ln) = lambda_name {
            let env_vars = ctx
                .lambda_environments
                .get(&ln)
                .map(|env| env.vars.clone())
                .unwrap_or_default();

            if env_vars.is_empty() {
                ctx.local_names.insert(name.to_string(), value_local);
                ctx.bind_local_symbol(symbol, value_local);
            } else {
                let local = ctx.add_local(Some(name.to_string()), kind, mir_ty);
                ctx.bind_local_symbol(symbol, local);
                ctx.lambda_names.insert(local, ln.clone());

                let env_elem_ty = MIR_I64;
                let env_ty = MIRType::Array(Box::new(env_elem_ty.clone()), env_vars.len() as u64);
                let env_local = ctx.mir_fn.add_local(LocalKind::User, env_ty);

                for (i, (var_name, _var_local)) in env_vars.iter().enumerate() {
                    if let Some(&captured_local) = ctx.local_names.get(var_name) {
                        let elem_addr_local = ctx.add_local(
                            None,
                            LocalKind::Temp,
                            MIRType::Ptr(Box::new(env_elem_ty.clone())),
                        );
                        let index_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
                        ctx.push_inst(Instruction::Assign {
                            destination: index_local,
                            value: MirConstant::Int(i as i64),
                        });
                        ctx.push_inst(Instruction::IndexAddr {
                            destination: elem_addr_local,
                            base: env_local,
                            index: index_local,
                        });

                        let captured_value_local =
                            ctx.add_local(None, LocalKind::Temp, env_elem_ty.clone());
                        ctx.push_inst(Instruction::Load {
                            destination: captured_value_local,
                            source: captured_local,
                        });
                        ctx.push_inst(Instruction::Store {
                            destination: elem_addr_local,
                            value: captured_value_local,
                        });
                    }
                }

                let env_ptr_local = ctx
                    .mir_fn
                    .add_local(LocalKind::Temp, MIRType::Ptr(Box::new(env_elem_ty)));
                ctx.push_inst(Instruction::AddrOf {
                    destination: env_ptr_local,
                    source: env_local,
                });

                if let Some(env_mut) = ctx.lambda_environments.get_mut(&ln) {
                    env_mut.env_ptr_local = Some(env_ptr_local);
                } else {
                    ctx.errors.push(format!(
                        "MIR lowering: lambda environment not found for '{}' in Let binding",
                        ln
                    ));
                }
            }
        } else {
            let value_ty = ctx.get_local_type(value_local).clone();
            let value_info_opt = ctx
                .mir_fn
                .locals
                .iter()
                .find(|(l, _)| l == &value_local)
                .map(|(l, _t)| *l);

            let value_info = match value_info_opt {
                Some(info) => info,
                None => {
                    ctx.errors.push(format!(
                        "MIR lowering: local info not found for local {:?} in Let binding for '{}'",
                        value_local, name
                    ));
                    let local = ctx.add_local(Some(name.to_string()), kind, mir_ty);
                    ctx.bind_local_symbol(symbol, local);
                    if let Some(type_name) = ctx.type_names.get(&value_local).cloned() {
                        ctx.type_names.insert(local, type_name);
                    }
                    ctx.push_inst(Instruction::Store {
                        destination: local,
                        value: value_local,
                    });
                    if let Some(origin) = ctx.future_origins.get(&value_local).cloned() {
                        ctx.future_origins.insert(local, origin);
                    }
                    return;
                }
            };

            if matches!(value_ty, MIRType::Array(_, _)) && value_info.kind == LocalKind::User {
                ctx.local_names.insert(name.to_string(), value_local);
                ctx.bind_local_symbol(symbol, value_local);
            } else {
                let actual_ty = value_ty.clone();
                let local = ctx.add_local(Some(name.to_string()), kind, actual_ty);
                ctx.bind_local_symbol(symbol, local);
                if let Some(type_name) = ctx.type_names.get(&value_local).cloned() {
                    ctx.type_names.insert(local, type_name);
                }
                ctx.push_inst(Instruction::Store {
                    destination: local,
                    value: value_local,
                });
                if let Some(origin) = ctx.future_origins.get(&value_local).cloned() {
                    ctx.future_origins.insert(local, origin);
                }
            }
        }
    } else {
        let local = ctx.add_local(Some(name.to_string()), kind, mir_ty);
        ctx.bind_local_symbol(symbol, local);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{HIRExpr, HIRType, IntKind};
    use crate::symbol::SymbolId;

    type TestCtxParts = (
        MirFunction,
        usize,
        HashSet<String>,
        HashMap<String, FunctionSig>,
        HashMap<String, &'static hir::HIRStruct>,
        Vec<InherentMethodTemplate>,
        Vec<TraitMethodTemplate>,
    );

    fn make_ctx() -> TestCtxParts {
        (
            MirFunction::new("test".to_string(), vec![], MIR_UNIT),
            0usize,
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn lower_let_stmt_propagates_future_origin_to_new_local() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
        let start_block = mir_fn.start_block;
        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let future_local = ctx.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_I64)));
        ctx.local_names.insert("f".to_string(), future_local);
        ctx.bind_local_symbol(SymbolId::new(1), future_local);
        ctx.future_origins
            .insert(future_local, "worker".to_string());

        let value_expr = HIRExpr::Var {
            name: "f".to_string(),
            symbol: SymbolId::new(1),
        };
        lower_let_stmt(
            &mut ctx,
            "bound",
            SymbolId::new(2),
            &HIRType::int(IntKind::I64),
            Some(&value_expr),
            false,
        );

        let bound_local = *ctx
            .local_symbols
            .get(&SymbolId::new(2))
            .expect("bound symbol should exist");
        assert_eq!(
            ctx.future_origins.get(&bound_local).map(String::as_str),
            Some("worker")
        );
    }

    #[test]
    fn lower_let_stmt_reuses_user_array_local_binding() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
        let start_block = mir_fn.start_block;
        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let array_local =
            ctx.add_local(None, LocalKind::User, MIRType::Array(Box::new(MIR_I64), 2));
        ctx.local_names.insert("arr".to_string(), array_local);
        ctx.bind_local_symbol(SymbolId::new(1), array_local);

        let value_expr = HIRExpr::Var {
            name: "arr".to_string(),
            symbol: SymbolId::new(1),
        };
        lower_let_stmt(
            &mut ctx,
            "arr2",
            SymbolId::new(2),
            &HIRType::array(HIRType::int(IntKind::I64), 2),
            Some(&value_expr),
            false,
        );

        assert_eq!(ctx.local_names.get("arr2"), Some(&array_local));
        assert_eq!(ctx.local_symbols.get(&SymbolId::new(2)), Some(&array_local));
    }

    #[test]
    fn lower_let_stmt_materializes_env_ptr_for_capturing_lambda_binding() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
        let start_block = mir_fn.start_block;
        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let captured_local = ctx.add_local(Some("cap".to_string()), LocalKind::User, MIR_I64);
        ctx.local_names.insert("cap".to_string(), captured_local);

        let lambda_local = ctx.add_local(
            None,
            LocalKind::Temp,
            MIRType::Fn {
                params: vec![MIR_I64],
                ret: Box::new(MIR_I64),
            },
        );
        ctx.local_names.insert("lam".to_string(), lambda_local);
        ctx.bind_local_symbol(SymbolId::new(1), lambda_local);
        ctx.lambda_names
            .insert(lambda_local, "lambda$0".to_string());
        ctx.lambda_environments.insert(
            "lambda$0".to_string(),
            LambdaEnv {
                vars: vec![("cap".to_string(), captured_local)],
                env_type: MIRType::Ptr(Box::new(MIR_I64)),
                env_ptr_local: None,
            },
        );

        let value_expr = HIRExpr::Var {
            name: "lam".to_string(),
            symbol: SymbolId::new(1),
        };
        lower_let_stmt(
            &mut ctx,
            "bound_lambda",
            SymbolId::new(2),
            &HIRType::function(
                vec![HIRType::int(IntKind::I64)],
                Box::new(HIRType::int(IntKind::I64)),
            ),
            Some(&value_expr),
            false,
        );

        let bound_local = *ctx
            .local_symbols
            .get(&SymbolId::new(2))
            .expect("bound lambda symbol should exist");
        assert_eq!(
            ctx.lambda_names.get(&bound_local).map(String::as_str),
            Some("lambda$0")
        );
        assert!(ctx
            .lambda_environments
            .get("lambda$0")
            .and_then(|env| env.env_ptr_local)
            .is_some());
    }
}
