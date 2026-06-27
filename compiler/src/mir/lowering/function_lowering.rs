use super::*;
use crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_function(
    fn_item: &hir::HIRFunction,
    lambda_counter: &mut usize,
    known_functions: &HashSet<String>,
    known_function_sigs: &HashMap<String, FunctionSig>,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: ConcreteTypeRegistry,
    options: &MirLowerOptions,
    inherent_method_templates: &[InherentMethodTemplate],
    trait_method_templates: &[TraitMethodTemplate],
    inherited_known_overlay: HashSet<String>,
    inherited_sigs_overlay: HashMap<String, FunctionSig>,
) -> Result<(MirFunction, Vec<MirFunction>), String> {
    let params: Vec<MIRType> = fn_item
        .params
        .iter()
        .map(|p| {
            hir_type_to_mir_with_structs_and_enums(
                &p.ty,
                struct_defs,
                &options.enum_defs,
                &HashMap::new(),
            )
        })
        .collect();
    let return_type: MIRType = hir_type_to_mir_with_structs_and_enums(
        &fn_item.return_type,
        struct_defs,
        &options.enum_defs,
        &HashMap::new(),
    );
    let try_propagation = matches!(
        &return_type,
        MIRType::Struct { name, .. } if name == "Result" || name == "Option"
    );

    let mut mir_fn = MirFunction::new(fn_item.name.clone(), params, return_type);
    mir_fn.is_async = fn_item.is_async;
    let start_block = mir_fn.start_block;
    let mut ctx = LoweringContext::new(
        &mut mir_fn,
        lambda_counter,
        known_functions,
        known_function_sigs,
        struct_defs,
        concrete_type_registry,
        options.clone(),
        inherent_method_templates,
        trait_method_templates,
    );
    // Seed this context's private overlay with the caller's already-materialized
    // instances (empty at the top level; the parent's small overlay when this is
    // a nested generic/method materialization). Only the small overlay is moved
    // in — the program-global base tables stay borrowed and are never cloned.
    ctx.known_functions_overlay = inherited_known_overlay;
    ctx.function_sigs_overlay = inherited_sigs_overlay;

    // 为函数参数创建局部变量并绑定到符号。
    for (i, param) in fn_item.params.iter().enumerate() {
        let local = Local::new(i + 1, LocalKind::Param);
        ctx.local_names.insert(param.name.clone(), local);
        ctx.bind_local_symbol(param.symbol, local);
        if let Some((_, MIRType::Struct { name, .. })) = ctx.mir_fn.locals.get(i + 1) {
            ctx.type_names.insert(local, name.clone());
        }
        if !fn_item.name.ends_with("_Drop_drop") {
            ctx.record_drop_binding_if_needed(local);
        }
        ctx.contract_param_bindings
            .push((param.name.clone(), param.symbol, local));
    }

    // 设置函数体入口块，可能包含运行时合约检查。
    let body_entry = if options.runtime_contract_checks {
        if let Some(precondition) = fn_item.precondition.as_ref() {
            ctx.inject_precondition_check(precondition, start_block)
        } else {
            start_block
        }
    } else {
        start_block
    };
    if try_propagation {
        ctx.push_try_scope(try_expr_helpers::TryScope::Function);
    }
    ctx.lower_body_to_block(&fn_item.body, body_entry);
    if try_propagation {
        ctx.pop_try_scope();
    }
    if options.runtime_contract_checks {
        if let Some(postcondition) = fn_item.postcondition.as_ref() {
            ctx.inject_postcondition_checks(postcondition);
        }
    }

    // 降级结束后检查是否有错误需要报告。
    ctx.insert_drop_glue();

    if !ctx.errors.is_empty() {
        return Err(format!(
            "MIR lowering errors in function '{}':\n  {}",
            fn_item.name,
            ctx.errors.join("\n  ")
        ));
    }

    // 将lambda函数从上下文中取出并返回。
    let lambda_functions = ctx.lambda_functions;
    Ok((mir_fn, lambda_functions))
}
