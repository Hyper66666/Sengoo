//! HIR到MIR的降级器，将高级中间表示转换为低级中间表示。

use crate::hir::{
    self, HIRBody, HIRExpr, HIRItem, HIRLiteral, HIRStmt, HIRType, HIRTypeKind,
};
use crate::hir::HIRTrait;
use crate::method_resolution::{
    ambiguous_method_error, explicit_hir_method_param_count, explicit_hir_method_params,
    select_method_candidate, MethodCandidate, MethodCandidateMatch,
};
use crate::mir::lowering_helpers::{
    collect_free_vars, collect_free_vars_in_body, collect_named_symbols,
};
use crate::mir::async_origin_helpers::{
    infer_async_base_name_from_instructions, infer_last_async_start_base,
};
use crate::mir::concrete_type_helpers::{
    collect_concrete_named_types_from_impl, collect_concrete_named_types_from_items,
};
use crate::mir::direct_call_helpers::collect_direct_call_names;
use crate::mir::hir_specialization_helpers::{
    hir_type_is_concrete, hir_type_is_placeholder_name, substitute_hir_function,
    substitute_hir_type,
};
use crate::mir::pattern_helpers::{
    build_match_switch_plan, pattern_binding_plan, pattern_match_plan, PatternBindingPlan,
    PatternMatchPlan,
};
use crate::mir::type_helpers::is_void_like;
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirBinOp, MirConstant, MirFunction, MirUnOp,
    Terminator, MIR_BOOL, MIR_I64, MIR_UNIT,
};
use crate::type_naming::{
    hir_type_instance_name as hir_type_to_instance_name,
    hir_type_prefix as hir_type_to_prefix,
    mir_type_instance_name as mir_type_to_instance_name,
};
use super::generic_methods::{
    collect_inherent_method_templates, collect_trait_method_templates_for_impl,
    ConcreteTypeRegistry, InherentMethodTemplate, TraitMethodTemplate,
};
use super::async_lowering::{async_spawn_kind_id, select_runtime_function_name};
use crate::symbol::SymbolId;
use std::collections::{HashMap, HashSet};

/// MirLowerOptions用于配置HIR到MIR的降级过程的选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirLowerOptions {
    pub runtime_contract_checks: bool,
    pub lazy_generic_mono: bool,
    pub async_functions: HashSet<String>,
}

impl Default for MirLowerOptions {
    fn default() -> Self {
        Self {
            runtime_contract_checks: false,
            lazy_generic_mono: true,
            async_functions: HashSet::new(),
        }
    }
}

fn mir_local_name(local: Local) -> String {
    match local.kind {
        LocalKind::Param => format!("%l_{}", local.id),
        LocalKind::Temp => format!("%t_{}", local.id),
        LocalKind::User => format!("%u_{}", local.id),
        LocalKind::Return => format!("%ret_{}", local.id),
    }
}

fn collect_concrete_named_types_closure(
    items: &[HIRItem],
    known_named_types: &HashSet<String>,
) -> HashMap<String, HIRType> {
    let mut out = collect_concrete_named_types_from_items(items, known_named_types);

    loop {
        let before_len = out.len();
        for item in items {
            if let HIRItem::Impl(impl_item) = item {
                for expanded_impl in expand_impl_variants(impl_item, &out, known_named_types) {
                    collect_concrete_named_types_from_impl(
                        &expanded_impl,
                        known_named_types,
                        &mut out,
                    );
                }
            }
        }

        if out.len() == before_len {
            break;
        }
    }

    out
}

fn match_generic_impl_target(
    template: &HIRType,
    concrete: &HIRType,
    known_named_types: &HashSet<String>,
    subst: &mut HashMap<String, HIRType>,
) -> bool {
    if let Some(name) = hir_type_is_placeholder_name(template, known_named_types) {
        match subst.get(&name) {
            Some(existing) => existing == concrete,
            None => {
                subst.insert(name, concrete.clone());
                true
            }
        }
    } else {
        match (&template.kind, &concrete.kind) {
            (HIRTypeKind::Unit, HIRTypeKind::Unit)
            | (HIRTypeKind::Never, HIRTypeKind::Never)
            | (HIRTypeKind::Bool, HIRTypeKind::Bool)
            | (HIRTypeKind::Char, HIRTypeKind::Char)
            | (HIRTypeKind::Str, HIRTypeKind::Str)
            | (HIRTypeKind::Byte, HIRTypeKind::Byte)
            | (HIRTypeKind::Bytes, HIRTypeKind::Bytes) => true,
            (HIRTypeKind::Int(lhs), HIRTypeKind::Int(rhs)) => lhs == rhs,
            (HIRTypeKind::Float(lhs), HIRTypeKind::Float(rhs)) => lhs == rhs,
            (HIRTypeKind::Ref(lhs_mut, lhs), HIRTypeKind::Ref(rhs_mut, rhs)) => {
                lhs_mut == rhs_mut && match_generic_impl_target(lhs, rhs, known_named_types, subst)
            }
            (HIRTypeKind::Ptr(lhs), HIRTypeKind::Ptr(rhs))
            | (HIRTypeKind::Slice(lhs), HIRTypeKind::Slice(rhs)) => {
                match_generic_impl_target(lhs, rhs, known_named_types, subst)
            }
            (HIRTypeKind::Array(lhs, lhs_len), HIRTypeKind::Array(rhs, rhs_len)) => {
                lhs_len == rhs_len && match_generic_impl_target(lhs, rhs, known_named_types, subst)
            }
            (HIRTypeKind::Tuple(lhs), HIRTypeKind::Tuple(rhs)) => {
                lhs.len() == rhs.len()
                    && lhs.iter().zip(rhs.iter()).all(|(lhs, rhs)| {
                        match_generic_impl_target(lhs, rhs, known_named_types, subst)
                    })
            }
            (
                HIRTypeKind::Fn { params: lhs_params, ret: lhs_ret },
                HIRTypeKind::Fn { params: rhs_params, ret: rhs_ret },
            ) => {
                lhs_params.len() == rhs_params.len()
                    && lhs_params.iter().zip(rhs_params.iter()).all(|(lhs, rhs)| {
                        match_generic_impl_target(lhs, rhs, known_named_types, subst)
                    })
                    && match_generic_impl_target(lhs_ret, rhs_ret, known_named_types, subst)
            }
            (
                HIRTypeKind::Named {
                    name: lhs_name,
                    args: lhs_args,
                },
                HIRTypeKind::Named {
                    name: rhs_name,
                    args: rhs_args,
                },
            ) => {
                lhs_name == rhs_name
                    && lhs_args.len() == rhs_args.len()
                    && lhs_args.iter().zip(rhs_args.iter()).all(|(lhs, rhs)| {
                        match_generic_impl_target(lhs, rhs, known_named_types, subst)
                    })
            }
            _ => false,
        }
    }
}

fn impl_type_prefix(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Named { args, .. } if !args.is_empty() => hir_type_to_instance_name(ty),
        _ => hir_type_to_prefix(ty),
    }
}

fn instantiate_impl_method(
    method: &hir::HIRFunction,
    legacy_prefix: &str,
    concrete_prefix: &str,
    subst: &HashMap<String, HIRType>,
) -> hir::HIRFunction {
    let mut method = substitute_hir_function(method, subst);
    let suffix = method
        .name
        .strip_prefix(&format!("{}_", legacy_prefix))
        .unwrap_or(&method.name)
        .to_string();
    method.name = format!("{}_{}", concrete_prefix, suffix);
    method
}

fn expand_impl_variants(
    impl_item: &hir::HIRImpl,
    concrete_named_types: &HashMap<String, HIRType>,
    known_named_types: &HashSet<String>,
) -> Vec<hir::HIRImpl> {
    let legacy_prefix = hir_type_to_prefix(&impl_item.target_type);
    if hir_type_is_concrete(&impl_item.target_type, known_named_types) {
        let concrete_prefix = impl_type_prefix(&impl_item.target_type);
        return vec![hir::HIRImpl {
            target_type: impl_item.target_type.clone(),
            trait_name: impl_item.trait_name.clone(),
            items: impl_item
                .items
                .iter()
                .map(|method| {
                    instantiate_impl_method(method, &legacy_prefix, &concrete_prefix, &HashMap::new())
                })
                .collect(),
        }];
    }

    let mut variants = Vec::new();
    let mut seen = HashSet::new();
    for concrete in concrete_named_types.values() {
        let mut subst = HashMap::new();
        if match_generic_impl_target(
            &impl_item.target_type,
            concrete,
            known_named_types,
            &mut subst,
        ) {
            let concrete_prefix = impl_type_prefix(concrete);
            if seen.insert(concrete_prefix.clone()) {
                variants.push(hir::HIRImpl {
                    target_type: concrete.clone(),
                    trait_name: impl_item.trait_name.clone(),
                    items: impl_item
                        .items
                        .iter()
                        .map(|method| {
                            instantiate_impl_method(method, &legacy_prefix, &concrete_prefix, &subst)
                        })
                        .collect(),
                });
            }
        }
    }
    variants
}

fn hir_type_to_mir_with_structs_and_subst(
    ty: &HIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    subst: &HashMap<String, MIRType>,
) -> MIRType {
    match &ty.kind {
        HIRTypeKind::Named { name, args } => {
            if args.is_empty() {
                if let Some(replacement) = subst.get(name) {
                    return replacement.clone();
                }
            }

            if let Some(def) = struct_defs.get(name) {
                let mut nested_subst = subst.clone();
                for (type_param, arg) in def.type_params.iter().zip(args.iter()) {
                    nested_subst.insert(
                        type_param.name.clone(),
                        hir_type_to_mir_with_structs_and_subst(arg, struct_defs, subst),
                    );
                }
                let instance_name = if args.is_empty() {
                    name.clone()
                } else {
                    let parts: Vec<String> = args
                        .iter()
                        .map(|arg| {
                            mir_type_to_instance_name(&hir_type_to_mir_with_structs_and_subst(
                                arg,
                                struct_defs,
                                subst,
                            ))
                        })
                        .collect();
                    format!("{}_{}", name, parts.join("_"))
                };
                MIRType::Struct {
                    name: instance_name,
                    fields: def
                        .fields
                        .iter()
                        .map(|field| {
                            (
                                field.name.clone(),
                                hir_type_to_mir_with_structs_and_subst(
                                    &field.ty,
                                    struct_defs,
                                    &nested_subst,
                                ),
                            )
                        })
                        .collect(),
                }
            } else {
                ty.clone().into()
            }
        }
        HIRTypeKind::Str => MIRType::Ptr(Box::new(MIRType::Int(8))),
        HIRTypeKind::Ref(_, inner) if matches!(inner.kind, HIRTypeKind::Str) => {
            MIRType::Ptr(Box::new(MIRType::Int(8)))
        }
        HIRTypeKind::Ref(_, inner) => MIRType::Ref(Box::new(
            hir_type_to_mir_with_structs_and_subst(inner, struct_defs, subst),
        )),
        HIRTypeKind::Ptr(inner) => MIRType::Ptr(Box::new(
            hir_type_to_mir_with_structs_and_subst(inner, struct_defs, subst),
        )),
        HIRTypeKind::Array(elem, len) => MIRType::Array(
            Box::new(hir_type_to_mir_with_structs_and_subst(elem, struct_defs, subst)),
            *len as u64,
        ),
        HIRTypeKind::Tuple(types) => MIRType::Tuple(
            types
                .iter()
                .map(|item| hir_type_to_mir_with_structs_and_subst(item, struct_defs, subst))
                .collect(),
        ),
        HIRTypeKind::Fn { params, ret } => MIRType::Fn {
            params: params
                .iter()
                .map(|item| hir_type_to_mir_with_structs_and_subst(item, struct_defs, subst))
                .collect(),
            ret: Box::new(hir_type_to_mir_with_structs_and_subst(ret, struct_defs, subst)),
        },
        _ => ty.clone().into(),
    }
}

fn hir_type_to_mir_with_structs(
    ty: &HIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
) -> MIRType {
    hir_type_to_mir_with_structs_and_subst(ty, struct_defs, &HashMap::new())
}

fn bind_mir_subst_from_hir_type(
    template: &HIRType,
    actual: &MIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    subst: &mut HashMap<String, MIRType>,
) {
    match &template.kind {
        HIRTypeKind::Named { name, args } if args.is_empty() && !struct_defs.contains_key(name) => {
            match subst.get(name) {
                Some(existing) if existing != actual => {}
                Some(_) => {}
                None => {
                    subst.insert(name.clone(), actual.clone());
                }
            }
        }
        HIRTypeKind::Named { name, args } => {
            if let (Some(def), MIRType::Struct { fields, .. }) = (struct_defs.get(name), actual) {
                let mut field_subst = HashMap::new();
                for (type_param, arg) in def.type_params.iter().zip(args.iter()) {
                    field_subst.insert(type_param.name.clone(), arg.clone());
                }
                for field in &def.fields {
                    if let Some((_, actual_field_ty)) =
                        fields.iter().find(|(field_name, _)| field_name == &field.name)
                    {
                        let template_field_ty = substitute_hir_type(&field.ty, &field_subst);
                        bind_mir_subst_from_hir_type(
                            &template_field_ty,
                            actual_field_ty,
                            struct_defs,
                            subst,
                        );
                    }
                }
            }
        }
        HIRTypeKind::Ref(_, inner) => {
            if let MIRType::Ref(actual_inner) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Ptr(inner) => {
            if let MIRType::Ptr(actual_inner) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Array(inner, _) => {
            if let MIRType::Array(actual_inner, _) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Tuple(items) => {
            if let MIRType::Tuple(actual_items) = actual {
                for (template_item, actual_item) in items.iter().zip(actual_items.iter()) {
                    bind_mir_subst_from_hir_type(template_item, actual_item, struct_defs, subst);
                }
            }
        }
        _ => {}
    }
}

pub fn lower_hir(items: &[HIRItem]) -> Result<Vec<MirFunction>, String> {
    lower_hir_with_options(items, MirLowerOptions::default())
}

pub fn lower_hir_with_options(
    items: &[HIRItem],
    options: MirLowerOptions,
) -> Result<Vec<MirFunction>, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut lambda_counter = 0;
    let direct_calls = if options.lazy_generic_mono {
        collect_direct_call_names(items)
    } else {
        HashSet::new()
    };

    let mut trait_defs: HashMap<String, &HIRTrait> = HashMap::new();
    let mut struct_defs: HashMap<String, &hir::HIRStruct> = HashMap::new();
    let mut known_named_types: HashSet<String> = HashSet::new();
    for item in items {
        match item {
            HIRItem::Trait(trait_item) => {
                trait_defs.insert(trait_item.name.clone(), trait_item);
            }
            HIRItem::Struct(struct_item) => {
                known_named_types.insert(struct_item.name.clone());
                struct_defs.insert(struct_item.name.clone(), struct_item);
            }
            _ => {}
        }
    }
    let concrete_named_types = collect_concrete_named_types_closure(items, &known_named_types);
    let concrete_type_registry = ConcreteTypeRegistry::new(&struct_defs, &concrete_named_types);
    let inherent_method_templates = collect_inherent_method_templates(items);
    let mut trait_method_templates: Vec<TraitMethodTemplate> = Vec::new();
    let mut eager_trait_functions: Vec<hir::HIRFunction> = Vec::new();

    let mut known_functions: HashSet<String> = HashSet::new();
    let mut known_function_sigs: HashMap<String, FunctionSig> = HashMap::new();
    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                known_functions.insert(fn_item.name.clone());
                known_function_sigs.insert(
                    fn_item.name.clone(),
                    FunctionSig {
                        ret_type: hir_type_to_mir_with_structs(&fn_item.return_type, &struct_defs),
                        param_count: fn_item.params.len(),
                        env: vec![],
                    },
                );
            }
            HIRItem::ExternBlock(extern_block) => {
                for extern_item in &extern_block.items {
                    if let hir::HIRExternItem::Function(extern_fn) = extern_item {
                        known_functions.insert(extern_fn.name.clone());
                        known_function_sigs.insert(
                            extern_fn.name.clone(),
                            FunctionSig {
                                ret_type: hir_type_to_mir_with_structs(&extern_fn.return_type, &struct_defs),
                                param_count: extern_fn.params.len(),
                                env: vec![],
                            },
                        );
                    }
                }
            }
            HIRItem::Impl(impl_item) => {
                for impl_item in expand_impl_variants(
                    impl_item,
                    &concrete_named_types,
                    &known_named_types,
                ) {
                    let type_prefix = impl_type_prefix(&impl_item.target_type);
                    if let Some(trait_name) = &impl_item.trait_name {
                        let collected = collect_trait_method_templates_for_impl(
                            &impl_item,
                            trait_defs.get(trait_name.as_str()).copied(),
                            &type_prefix,
                        );
                        for registration in collected.eager_registrations() {
                            known_function_sigs.insert(
                                registration.name.clone(),
                                FunctionSig {
                                    ret_type: hir_type_to_mir_with_structs(
                                        &registration.return_type,
                                        &struct_defs,
                                    ),
                                    param_count: registration.explicit_param_count,
                                    env: vec![],
                                },
                            );
                            known_functions.insert(registration.name);
                        }
                        eager_trait_functions
                            .extend(collected.eager_methods.into_iter().map(|method| method.function));
                        trait_method_templates.extend(collected.templates);
                    } else {
                        for method in &impl_item.items {
                            if !method.type_params.is_empty() {
                                continue;
                            }
                            known_functions.insert(method.name.clone());
                            known_function_sigs.insert(
                                method.name.clone(),
                                FunctionSig {
                                    ret_type: hir_type_to_mir_with_structs(
                                        &method.return_type,
                                        &struct_defs,
                                    ),
                                    param_count: explicit_hir_method_param_count(method),
                                    env: vec![],
                                },
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                if options.lazy_generic_mono
                    && !fn_item.type_params.is_empty()
                    && !direct_calls.contains(&fn_item.name)
                {
                    continue;
                }
                match lower_function(
                    fn_item,
                    &mut lambda_counter,
                    &known_functions,
                    &known_function_sigs,
                    &struct_defs,
                    concrete_type_registry.clone(),
                    &options,
                    &inherent_method_templates,
                    &trait_method_templates,
                ) {
                    Ok((mir_fn, lambdas)) => {
                        results.push(mir_fn);
                        results.extend(lambdas);
                    }
                    Err(e) => errors.push(e),
                }
            }
            HIRItem::Impl(impl_item) => {
                for impl_item in expand_impl_variants(
                    impl_item,
                    &concrete_named_types,
                    &known_named_types,
                ) {
                    if impl_item.trait_name.is_some() {
                        continue;
                    }
                    for method in &impl_item.items {
                        if !method.type_params.is_empty() {
                            continue;
                        }
                        match lower_function(
                            method,
                            &mut lambda_counter,
                            &known_functions,
                            &known_function_sigs,
                            &struct_defs,
                            concrete_type_registry.clone(),
                            &options,
                            &inherent_method_templates,
                            &trait_method_templates,
                        ) {
                            Ok((mir_fn, lambdas)) => {
                                results.push(mir_fn);
                                results.extend(lambdas);
                            }
                            Err(e) => errors.push(e),
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for function in eager_trait_functions {
        match lower_function(
            &function,
            &mut lambda_counter,
            &known_functions,
            &known_function_sigs,
            &struct_defs,
            concrete_type_registry.clone(),
            &options,
            &inherent_method_templates,
            &trait_method_templates,
        ) {
            Ok((mir_fn, lambdas)) => {
                results.push(mir_fn);
                results.extend(lambdas);
            }
            Err(e) => errors.push(e),
        }
    }

    if !errors.is_empty() {
        return Err(format!("MIR lowering failed:\n{}", errors.join("\n")));
    }

    Ok(results)
}

fn lower_function(
    fn_item: &hir::HIRFunction,
    lambda_counter: &mut usize,
    known_functions: &HashSet<String>,
    known_function_sigs: &HashMap<String, FunctionSig>,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: ConcreteTypeRegistry,
    options: &MirLowerOptions,
    inherent_method_templates: &[InherentMethodTemplate],
    trait_method_templates: &[TraitMethodTemplate],
) -> Result<(MirFunction, Vec<MirFunction>), String> {
    let params: Vec<MIRType> = fn_item
        .params
        .iter()
        .map(|p| hir_type_to_mir_with_structs(&p.ty, struct_defs))
        .collect();
    let return_type: MIRType = hir_type_to_mir_with_structs(&fn_item.return_type, struct_defs);

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

    // 为函数参数创建局部变量并绑定到符号。
    for (i, param) in fn_item.params.iter().enumerate() {
        let local = Local::new(i + 1, LocalKind::Param);
        ctx.local_names.insert(param.name.clone(), local);
        ctx.bind_local_symbol(param.symbol, local);
        if let Some((_, MIRType::Struct { name, .. })) = ctx.mir_fn.locals.get(i + 1) {
            ctx.type_names.insert(local, name.clone());
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
    ctx.lower_body_to_block(&fn_item.body, body_entry);
    if options.runtime_contract_checks {
        if let Some(postcondition) = fn_item.postcondition.as_ref() {
            ctx.inject_postcondition_checks(postcondition);
        }
    }

    // 降级结束后检查是否有错误需要报告。
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

/// 循环上下文，记录 `break/continue` 目标基本块。
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    /// break目标基本块的索引。
    break_block: usize,
    /// continue目标基本块的索引。
    continue_block: usize,
}

/// 函数签名信息，存储函数名、参数数量和参数类型。
#[derive(Clone)]
struct FunctionSig {
    ret_type: MIRType,
    param_count: usize,
    /// 函数参数数量（不含环境指针参数）。
    #[allow(dead_code)]
    env: Vec<(String, MIRType)>,
}

/// Lambda 捕获环境。
struct LambdaEnv {
    /// 捕获变量列表，保存变量名及其对应的局部变量 `Local`。
    vars: Vec<(String, Local)>,
    /// 自由变量列表，用于lambda捕获分析。
    #[allow(dead_code)]
    env_type: MIRType,
    /// 捕获环境数组对应的MIR局部变量（Local句柄）。
    env_ptr_local: Option<Local>,
}

/// MIR lowering 上下文。
struct LoweringContext<'a> {
    mir_fn: &'a mut MirFunction,
    /// 当前正在降级的MIR函数的可变引用。
    local_names: HashMap<String, Local>,
    local_symbols: HashMap<SymbolId, Local>,
    contract_param_bindings: Vec<(String, SymbolId, Local)>,
    /// 当前基本块的索引（None表示未设置）。
    current_block: Option<usize>,
    /// 错误信息列表，记录降级过程中遇到的错误。
    errors: Vec<String>,
    /// 循环上下文栈，记录嵌套循环的 `break/continue` 目标。
    loop_stack: Vec<LoopContext>,
    /// 循环嵌套栈，支持多层循环的break/continue。
    lambda_counter: &'a mut usize,
    /// 存储lambda上下文中Lambda函数计数器的引用。
    lambda_functions: Vec<MirFunction>,
    /// lambda名称到Local的映射，用于lambda引用解析。
    lambda_names: HashMap<Local, String>,
    /// lambda函数集合，存储生成的所有lambda MIR函数。
    function_sigs: HashMap<String, FunctionSig>,
    /// lambda环境信息表，按名称索引。
    lambda_environments: HashMap<String, LambdaEnv>,
    /// 局部变量名与MIR类型的映射表。
    type_names: HashMap<Local, String>,
    /// 已知函数名集合，用于快速判断标识符是否表示函数调用。
    known_functions: HashSet<String>,
    struct_defs: &'a HashMap<String, &'a hir::HIRStruct>,
    concrete_type_registry: ConcreteTypeRegistry,
    options: MirLowerOptions,
    inherent_method_templates: &'a [InherentMethodTemplate],
    trait_method_templates: &'a [TraitMethodTemplate],
    /// Maps a Local → async function base name when that local holds a future
    /// handle produced by a `foo__start(...)` call. Propagated through let
    /// bindings so that `let f = async_fn(); await f` resolves correctly.
    future_origins: HashMap<Local, String>,
}

impl<'a> LoweringContext<'a> {
    fn new(
        mir_fn: &'a mut MirFunction,
        lambda_counter: &'a mut usize,
        known_functions: &'a HashSet<String>,
        known_function_sigs: &HashMap<String, FunctionSig>,
        struct_defs: &'a HashMap<String, &'a hir::HIRStruct>,
        concrete_type_registry: ConcreteTypeRegistry,
        options: MirLowerOptions,
        inherent_method_templates: &'a [InherentMethodTemplate],
        trait_method_templates: &'a [TraitMethodTemplate],
    ) -> Self {
        Self {
            mir_fn,
            local_names: HashMap::new(),
            local_symbols: HashMap::new(),
            contract_param_bindings: Vec::new(),
            current_block: None,
            errors: Vec::new(),
            loop_stack: Vec::new(),
            lambda_counter,
            lambda_functions: Vec::new(),
            lambda_names: HashMap::new(),
            function_sigs: known_function_sigs.clone(),
            lambda_environments: HashMap::new(),
            type_names: HashMap::new(),
            known_functions: known_functions.clone(),
            struct_defs,
            concrete_type_registry,
            options,
            inherent_method_templates,
            trait_method_templates,
            future_origins: HashMap::new(),
        }
    }

    fn receiver_type_prefix(&self, receiver_ty: &MIRType) -> String {
        match receiver_ty {
            MIRType::Int(bits) => format!("i{}", bits),
            MIRType::Float(bits) => format!("f{}", bits),
            MIRType::Bool => "bool".to_string(),
            MIRType::Array(_, _) => "array".to_string(),
            MIRType::Tuple(_) => "tuple".to_string(),
            MIRType::Ptr(inner) | MIRType::Ref(inner) => match inner.as_ref() {
                MIRType::Int(bits) => format!("i{}_ptr", bits),
                MIRType::Float(bits) => format!("f{}_ptr", bits),
                MIRType::Bool => "bool_ptr".to_string(),
                _ => "ptr".to_string(),
            },
            MIRType::Struct { name, .. } => name.clone(),
            MIRType::Enum { .. } => "enum".to_string(),
            _ => "i64".to_string(),
        }
    }

    fn method_dispatch_name(
        &self,
        receiver_local: Local,
        receiver_ty: &MIRType,
        method: &str,
    ) -> String {
        if let Some(type_name) = self.type_names.get(&receiver_local) {
            format!("{}_{}", type_name, method)
        } else {
            match receiver_ty {
                MIRType::Int(bits) => format!("i{}_{}", bits, method),
                MIRType::Float(bits) => format!("f{}_{}", bits, method),
                MIRType::Bool => format!("bool_{}", method),
                MIRType::Array(_, _) => format!("array_{}", method),
                MIRType::Tuple(_) => format!("tuple_{}", method),
                MIRType::Ptr(inner) | MIRType::Ref(inner) => match inner.as_ref() {
                    MIRType::Int(bits) => format!("i{}_ptr_{}", bits, method),
                    MIRType::Float(bits) => format!("f{}_ptr_{}", bits, method),
                    MIRType::Bool => format!("bool_ptr_{}", method),
                    _ => format!("ptr_{}", method),
                },
                MIRType::Struct { name, .. } => format!("{}_{}", name, method),
                MIRType::Enum { .. } => format!("enum_{}", method),
                _ => format!("i64_{}", method),
            }
        }
    }

    fn receiver_type_display(&self, receiver_local: Local, receiver_ty: &MIRType) -> String {
        if let Some(type_name) = self.type_names.get(&receiver_local) {
            type_name.clone()
        } else {
            match receiver_ty {
                MIRType::Int(bits) => format!("i{}", bits),
                MIRType::Float(bits) => format!("f{}", bits),
                MIRType::Bool => "bool".to_string(),
                MIRType::Array(_, _) => "array".to_string(),
                MIRType::Tuple(_) => "tuple".to_string(),
                MIRType::Ptr(_) | MIRType::Ref(_) => "ptr".to_string(),
                MIRType::Struct { name, .. } => name.clone(),
                MIRType::Enum { .. } => "enum".to_string(),
                _ => format!("{:?}", receiver_ty),
            }
        }
    }

    fn resolve_method_call_target(
        &mut self,
        receiver_local: Local,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
    ) -> Result<String, String> {
        let method_func_name = self.method_dispatch_name(receiver_local, receiver_ty, method);
        let type_display = self.receiver_type_display(receiver_local, receiver_ty);

        if self.known_functions.contains(&method_func_name) {
            return Ok(method_func_name);
        }
        if let Some(generated_name) =
            self.try_materialize_inherent_method(receiver_ty, method, arg_locals)
        {
            return Ok(generated_name);
        }
        if let Some(generated_name) =
            self.try_materialize_trait_method(receiver_ty, method, arg_locals, &type_display)?
        {
            return Ok(generated_name);
        }

        let type_prefix = if let Some(type_name) = self.type_names.get(&receiver_local) {
            type_name.clone()
        } else {
            self.receiver_type_prefix(receiver_ty)
        };

        match self.select_known_trait_method_candidate(
            &type_prefix,
            method,
            &method_func_name,
            arg_locals.len(),
        ) {
            MethodCandidateMatch::None | MethodCandidateMatch::WrongArity { .. } => Err(format!(
                "method '{}' not found for type '{}'",
                method, type_display
            )),
            MethodCandidateMatch::One(name) => Ok(name),
            MethodCandidateMatch::Ambiguous { labels } => {
                Err(ambiguous_method_error(method, &type_display, &labels))
            }
        }
    }

    fn bind_method_specialization_subst(
        &self,
        target_type: &HIRType,
        method: &hir::HIRFunction,
        receiver_ty: &MIRType,
        arg_locals: &[Local],
    ) -> Option<HashMap<String, MIRType>> {
        let mut mir_subst = HashMap::new();
        bind_mir_subst_from_hir_type(target_type, receiver_ty, self.struct_defs, &mut mir_subst);

        let actual_arg_types: Vec<MIRType> = arg_locals
            .iter()
            .map(|local| self.get_local_type(*local).clone())
            .collect();
        let explicit_params = explicit_hir_method_params(&method.params);
        if explicit_params.len() != actual_arg_types.len() {
            return None;
        }
        for (param, actual_ty) in explicit_params.iter().zip(actual_arg_types.iter()) {
            bind_mir_subst_from_hir_type(&param.ty, actual_ty, self.struct_defs, &mut mir_subst);
        }

        Some(mir_subst)
    }

    fn realize_method_specialization(
        &mut self,
        target_type: &HIRType,
        method: &hir::HIRFunction,
        receiver_ty: &MIRType,
        mir_subst: HashMap<String, MIRType>,
    ) -> Option<(HashMap<String, HIRType>, String)> {
        let receiver_prefix = self.receiver_type_prefix(receiver_ty);
        let mut hir_subst = HashMap::new();
        for (name, mir_ty) in &mir_subst {
            hir_subst.insert(name.clone(), self.concrete_type_registry.hir_type_for_mir(mir_ty)?);
        }
        if !method
            .type_params
            .iter()
            .all(|param| hir_subst.contains_key(&param.name))
        {
            return None;
        }

        let concrete_target = substitute_hir_type(target_type, &hir_subst);
        let concrete_prefix = impl_type_prefix(&concrete_target);
        self.concrete_type_registry
            .register_instance(concrete_prefix.clone(), concrete_target.clone());
        for ty in hir_subst.values() {
            if matches!(ty.kind, HIRTypeKind::Named { .. }) {
                self.concrete_type_registry
                    .register_instance(hir_type_to_instance_name(ty), ty.clone());
            }
        }
        if concrete_prefix != receiver_prefix {
            return None;
        }

        Some((hir_subst, concrete_prefix))
    }

    fn prepare_method_specialization(
        &mut self,
        target_type: &HIRType,
        method: &hir::HIRFunction,
        receiver_ty: &MIRType,
        arg_locals: &[Local],
    ) -> Option<(HashMap<String, HIRType>, String)> {
        let mir_subst =
            self.bind_method_specialization_subst(target_type, method, receiver_ty, arg_locals)?;
        self.realize_method_specialization(target_type, method, receiver_ty, mir_subst)
    }

    fn lower_materialized_method(&mut self, specialized: hir::HIRFunction) -> Option<String> {
        if self.known_functions.contains(&specialized.name) {
            return Some(specialized.name);
        }

        self.function_sigs.insert(
            specialized.name.clone(),
            FunctionSig {
                ret_type: hir_type_to_mir_with_structs(&specialized.return_type, self.struct_defs),
                param_count: explicit_hir_method_param_count(&specialized),
                env: vec![],
            },
        );
        self.known_functions.insert(specialized.name.clone());

        match lower_function(
            &specialized,
            self.lambda_counter,
            &self.known_functions,
            &self.function_sigs,
            self.struct_defs,
            self.concrete_type_registry.clone(),
            &self.options,
            self.inherent_method_templates,
            self.trait_method_templates,
        ) {
            Ok((mir_fn, nested)) => {
                self.lambda_functions.push(mir_fn);
                self.lambda_functions.extend(nested);
                Some(specialized.name)
            }
            Err(error) => {
                self.errors.push(error);
                None
            }
        }
    }

    fn select_known_trait_method_candidate(
        &self,
        type_prefix: &str,
        method: &str,
        excluded_name: &str,
        expected_param_count: usize,
    ) -> MethodCandidateMatch<String> {
        let suffix = format!("_{}", method);
        let prefix = format!("{}_", type_prefix);
        let matches = self
            .known_functions
            .iter()
            .filter(|name| {
                name.starts_with(&prefix)
                    && name.ends_with(&suffix)
                    && *name != excluded_name
                    && {
                        let middle = &name[prefix.len()..name.len() - suffix.len()];
                        !middle.is_empty()
                    }
            })
            .map(|name| MethodCandidate {
                label: name.clone(),
                param_count: self
                    .function_sigs
                    .get(name)
                    .map(|sig| sig.param_count)
                    .unwrap_or(0),
                value: name.clone(),
            })
            .collect();
        select_method_candidate(matches, expected_param_count)
    }

    fn try_materialize_inherent_method(
        &mut self,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
    ) -> Option<String> {
        for template in self.inherent_method_templates {
            let legacy_prefix = hir_type_to_prefix(&template.target_type);
            let original_method_name = template
                .method
                .name
                .strip_prefix(&format!("{}_", legacy_prefix))
                .unwrap_or(&template.method.name);
            if original_method_name != method {
                continue;
            }

            let (hir_subst, concrete_prefix) = self.prepare_method_specialization(
                &template.target_type,
                &template.method,
                receiver_ty,
                arg_locals,
            )?;

            let mut specialized = instantiate_impl_method(
                &template.method,
                &legacy_prefix,
                &concrete_prefix,
                &hir_subst,
            );
            specialized.type_params.clear();
            if !template.method.type_params.is_empty() {
                let suffixes: Vec<String> = template
                    .method
                    .type_params
                    .iter()
                    .filter_map(|param| hir_subst.get(&param.name))
                    .map(hir_type_to_instance_name)
                    .collect();
                specialized.name = format!("{}_{}", specialized.name, suffixes.join("_"));
            }

            return self.lower_materialized_method(specialized);
        }
        None
    }

    fn specialize_trait_method_candidate(
        &mut self,
        template: &TraitMethodTemplate,
        receiver_ty: &MIRType,
        arg_locals: &[Local],
    ) -> Option<MethodCandidate<hir::HIRFunction>> {
        let (hir_subst, concrete_prefix) = self.prepare_method_specialization(
            &template.target_type,
            &template.method,
            receiver_ty,
            arg_locals,
        )?;

        let mut specialized = substitute_hir_function(&template.method, &hir_subst);
        specialized.type_params.clear();
        if !template.method.type_params.is_empty() {
            let suffixes: Vec<String> = template
                .method
                .type_params
                .iter()
                .filter_map(|param| hir_subst.get(&param.name))
                .map(hir_type_to_instance_name)
                .collect();
            specialized.name = format!(
                "{}_{}_{}_{}",
                concrete_prefix,
                template.trait_name,
                template.method.name,
                suffixes.join("_")
            );
        } else {
            specialized.name = format!(
                "{}_{}_{}",
                concrete_prefix,
                template.trait_name,
                template.method.name,
            );
        }

        Some(MethodCandidate {
            label: format!("{} ({})", specialized.name, template.trait_name),
            param_count: explicit_hir_method_param_count(&specialized),
            value: specialized,
        })
    }

    fn try_materialize_trait_method(
        &mut self,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
        type_display: &str,
    ) -> Result<Option<String>, String> {
        let mut candidates = Vec::new();
        for template in self.trait_method_templates {
            if template.method.name != method {
                continue;
            }

            if let Some(candidate) =
                self.specialize_trait_method_candidate(template, receiver_ty, arg_locals)
            {
                candidates.push(candidate);
            }
        }

        match select_method_candidate(candidates, arg_locals.len()) {
            MethodCandidateMatch::None | MethodCandidateMatch::WrongArity { .. } => Ok(None),
            MethodCandidateMatch::One(specialized) => Ok(self.lower_materialized_method(specialized)),
            MethodCandidateMatch::Ambiguous { labels } => {
                Err(ambiguous_method_error(method, type_display, &labels))
            }
        }
    }

    fn infer_struct_literal_type(
        &mut self,
        name: &str,
        field_locals: &HashMap<String, Local>,
    ) -> Option<MIRType> {
        let def = self.struct_defs.get(name)?;
        let mut subst: HashMap<String, MIRType> = HashMap::new();
        for field in &def.fields {
            let local = field_locals.get(&field.name)?;
            let actual_ty = self.get_local_type(*local).clone();
            bind_mir_subst_from_hir_type(&field.ty, &actual_ty, self.struct_defs, &mut subst);
        }

        if !def.type_params.is_empty()
            && !def
                .type_params
                .iter()
                .all(|type_param| subst.contains_key(&type_param.name))
        {
            return None;
        }

        let instance_name = if def.type_params.is_empty() {
            name.to_string()
        } else {
            let parts: Vec<String> = def
                .type_params
                .iter()
                .map(|type_param| {
                    mir_type_to_instance_name(
                        subst
                            .get(&type_param.name)
                            .expect("generic struct literal type param should be inferred"),
                    )
                })
                .collect();
            format!("{}_{}", name, parts.join("_"))
        };

        let concrete_hir_ty = HIRType::named(
            name.to_string(),
            def.type_params
                .iter()
                .map(|type_param| {
                    self.concrete_type_registry
                        .hir_type_for_mir(
                            subst
                                .get(&type_param.name)
                                .expect("generic struct literal type param should be inferred"),
                        )
                        .expect("concrete struct literal arg should resolve to HIR type")
                })
                .collect(),
        );
        self.concrete_type_registry
            .register_instance(instance_name.clone(), concrete_hir_ty);

        Some(MIRType::Struct {
            name: instance_name,
            fields: def
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        hir_type_to_mir_with_structs_and_subst(&field.ty, self.struct_defs, &subst),
                    )
                })
                .collect(),
        })
    }

    fn lambda_name(&mut self) -> String {
        let name = format!("$__lambda{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }

    fn async_block_name(&mut self) -> String {
        let name = format!("$__async_block{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }

    /// 将循环上下文压入循环嵌套栈。
    fn push_loop(&mut self, break_block: usize, continue_block: usize) {
        self.loop_stack.push(LoopContext {
            break_block,
            continue_block,
        });
    }

    /// 返回当前上下文的指令列表的可变引用。
    /// 根据参数列表收集表达式中的自由变量及其对应 `Local`。
    fn collect_free_vars(
        &self,
        params: &[String],
        body: &crate::hir::HIRExpr,
    ) -> Vec<(String, Local)> {
        collect_free_vars(body, params, &self.local_names)
    }

    fn collect_async_block_free_vars(&self, body: &crate::hir::HIRBody) -> Vec<(String, Local)> {
        collect_free_vars_in_body(body, &self.local_names)
    }


    fn lower_async_block(&mut self, body: &HIRBody) -> Local {
        let async_block_name = self.async_block_name();
        let free_vars = self.collect_async_block_free_vars(body);
        let capture_types: Vec<MIRType> = free_vars
            .iter()
            .map(|(_, local)| self.get_local_type(*local).clone())
            .collect();
        let capture_args: Vec<Local> = free_vars.iter().map(|(_, local)| *local).collect();

        let mut async_fn = MirFunction::new(async_block_name.clone(), capture_types.clone(), MIR_UNIT);
        async_fn.is_async = true;
        let async_start = async_fn.start_block;

        let mut async_ctx = LoweringContext::new(
            &mut async_fn,
            self.lambda_counter,
            &self.known_functions,
            &self.function_sigs,
            self.struct_defs,
            self.concrete_type_registry.clone(),
            self.options.clone(),
            self.inherent_method_templates,
            self.trait_method_templates,
        );
        async_ctx.current_block = Some(async_start);

        for (index, (var_name, outer_local)) in free_vars.iter().enumerate() {
            let param_local = Local::new(index + 1, LocalKind::Param);
            async_ctx.local_names.insert(var_name.clone(), param_local);
            if let Some(type_name) = self.type_names.get(outer_local).cloned() {
                async_ctx.type_names.insert(param_local, type_name);
            }
            if let Some(origin) = self.future_origins.get(outer_local).cloned() {
                async_ctx.future_origins.insert(param_local, origin);
            }
        }

        let result_local = async_ctx.lower_body_to_block_val(body, async_start);
        let result_ty = async_ctx.get_local_type(result_local).clone();
        async_ctx.mir_fn.return_type = result_ty.clone();
        if let Some((_, slot_ty)) = async_ctx.mir_fn.locals.get_mut(0) {
            *slot_ty = result_ty.clone();
        }

        let cur = async_ctx.current_block();
        let already_terminated = async_ctx
            .mir_fn
            .block_mut(cur)
            .is_some_and(|block| block.terminator.is_some());
        if !already_terminated {
            if matches!(result_ty, MIRType::Unit) {
                async_ctx.set_terminator(Terminator::Return(None));
            } else {
                async_ctx.set_terminator(Terminator::Return(Some(result_local)));
            }
        }

        let async_errors = std::mem::take(&mut async_ctx.errors);
        let nested_functions = std::mem::take(&mut async_ctx.lambda_functions);
        drop(async_ctx);

        if !async_errors.is_empty() {
            self.errors.push(format!(
                "async block lowering failed for '{}':\n  {}",
                async_block_name,
                async_errors.join("\n  ")
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        self.known_functions.insert(async_block_name.clone());
        self.options.async_functions.insert(async_block_name.clone());
        self.function_sigs.insert(
            async_block_name.clone(),
            FunctionSig {
                ret_type: result_ty.clone(),
                param_count: capture_types.len(),
                env: vec![],
            },
        );

        self.lambda_functions.push(async_fn);
        self.lambda_functions.extend(nested_functions);

        let future_local = self.add_local(None, LocalKind::Temp, result_ty);
        self.push_inst(Instruction::Call {
            destination: future_local,
            func: format!("{}__start", async_block_name),
            args: capture_args,
        });
        self.future_origins
            .insert(future_local, async_block_name);
        future_local
    }


    /// 弹出当前循环的break/continue目标。
    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    /// 获取当前循环的break目标块索引。
    fn get_break_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.break_block)
    }

    /// 获取当前循环的continue目标块索引。
    fn get_continue_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.continue_block)
    }

    /// 添加一个新的局部变量并返回其Local句柄。
    fn add_local(&mut self, name: Option<String>, kind: LocalKind, ty: MIRType) -> Local {
        let local = self.mir_fn.add_local(kind, ty);
        if let Some(name) = name {
            self.local_names.insert(name, local);
        }
        local
    }

    fn bind_local_symbol(&mut self, symbol: SymbolId, local: Local) {
        if symbol.is_valid() {
            self.local_symbols.insert(symbol, local);
        }
    }

    /// 获取局部变量的MIR类型。
    fn get_local_type(&self, local: Local) -> &MIRType {
        if let Some((_, ty)) = self.mir_fn.locals.get(local.index()) {
            ty
        } else {
            &MIR_UNIT
        }
    }

    /// 获取局部变量的类型信息。
    /// 解析名称和符号ID对应的局部变量，或创建新的局部变量。
    fn resolve_local(&mut self, name: &str, symbol: SymbolId) -> Local {
        if symbol.is_valid() {
            if let Some(&local) = self.local_symbols.get(&symbol) {
                return local;
            }
        }
        match self.local_names.get(name) {
            Some(&local) => local,
            None => {
                // 变量未定义时报告错误并返回临时变量。
                self.errors.push(format!("undefined variable: '{}'", name));
                // 错误处理：返回一个unit类型的临时local。
                self.mir_fn.add_local(LocalKind::Temp, MIR_UNIT)
            }
        }
    }

    /// 创建一个新的基本块并返回其索引。
    fn new_block(&mut self) -> usize {
        self.mir_fn.add_block()
    }

    /// 设置当前基本块为指定块。
    fn set_current_block(&mut self, block: usize) {
        self.current_block = Some(block);
    }

    /// 返回当前正在生成的基本块索引。
    fn current_block(&self) -> usize {
        self.current_block.expect("no current block set")
    }

    fn propagate_future_origin_through_phi(
        &mut self,
        destination: Local,
        incoming: &[(Local, usize)],
    ) {
        if !matches!(self.get_local_type(destination), MIRType::Future(_)) {
            return;
        }

        let mut resolved = Vec::with_capacity(incoming.len());
        for (local, _) in incoming {
            let Some(origin) = self.future_origins.get(local).cloned() else {
                return;
            };
            resolved.push(origin);
        }

        let Some(first) = resolved.first().cloned() else {
            return;
        };
        if resolved.iter().all(|origin| origin == &first) {
            self.future_origins.insert(destination, first);
        }
    }

    /// Check if two types are compatible for binary operations and, if not,
    /// try to insert Cast instructions to reconcile them.  Returns the
    /// (possibly cast) left and right locals whose types now match, or pushes
    /// an error and returns the originals unchanged.
    fn reconcile_binary_operand_types(&mut self, left: Local, right: Local) -> (Local, Local) {
        let left_ty = self.get_local_type(left).clone();
        let right_ty = self.get_local_type(right).clone();

        // 若两侧类型已经相同，无需调和。
        if left_ty == right_ty {
            return (left, right);
        }

        // Determine if a cast between two types is valid and, if so,
        // which direction to cast (returns the common target type).
        match (&left_ty, &right_ty) {
        // 对整数和浮点数类型进行隐式类型提升（宽化）。
            (MIRType::Int(a), MIRType::Int(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::Int(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }

        // 两个浮点数操作数：选择较大位宽的类型。
            (MIRType::Float(a), MIRType::Float(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::Float(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }

        // 整数与浮点数混合：将整数转为浮点数。
            (MIRType::Int(_), MIRType::Float(b)) => {
                let target_ty = MIRType::Float(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Float(a), MIRType::Int(_)) => {
                let target_ty = MIRType::Float(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }

        // 布尔与整数混合：将bool转为对应位宽的整数。
            (MIRType::Bool, MIRType::Int(b)) => {
                let target_ty = MIRType::Int(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Int(a), MIRType::Bool) => {
                let target_ty = MIRType::Int(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }

        // 其他类型组合：无需自动转换，直接返回左侧类型。
            _ => {
                self.errors.push(format!(
                    "type mismatch in binary operation: left operand has type {:?}, right operand has type {:?}",
                    left_ty, right_ty
                ));
                (left, right)
            }
        }
    }

    /// Insert a Cast instruction that converts `source` to `target_ty`,
    /// returning the new local that holds the cast result.
    fn insert_cast(&mut self, source: Local, target_ty: MIRType) -> Local {
        let dest = self.add_local(None, LocalKind::Temp, target_ty.clone());
        self.push_inst(Instruction::Cast {
            destination: dest,
            value: source,
            to: target_ty,
        });
        dest
    }

    /// 向当前基本块追加一条MIR指令。
    fn push_inst(&mut self, inst: Instruction) {
        let block_id = self.current_block();
        self.mir_fn.push_inst_to_block(block_id, inst);
    }

    /// 向当前基本块追加terminator终止指令。
    fn set_terminator(&mut self, term: Terminator) {
        let block_id = self.current_block();
        if let Some(block) = self.mir_fn.block_mut(block_id) {
            block.set_terminator(term);
        }
    }

    fn inject_precondition_check(&mut self, precondition: &HIRExpr, entry_block: usize) -> usize {
        self.set_current_block(entry_block);
        let cond_local = self.lower_contract_condition(precondition, None);
        let pass_block = self.new_block();
        let fail_block = self.new_block();
        self.set_terminator(Terminator::If {
            cond: cond_local,
            then_block: pass_block,
            else_block: fail_block,
        });
        self.set_current_block(fail_block);
        self.set_terminator(Terminator::Unreachable);
        pass_block
    }

    fn inject_postcondition_checks(&mut self, postcondition: &HIRExpr) {
        let return_sites = self
            .mir_fn
            .basic_blocks
            .iter()
            .enumerate()
            .filter_map(|(block_id, block)| match block.terminator.clone() {
                Some(Terminator::Return(value)) => Some((block_id, value)),
                _ => None,
            })
            .collect::<Vec<_>>();

        for (return_block, return_value) in return_sites {
            let Some(return_local) = return_value else {
                continue;
            };

            let check_block = self.new_block();
            let success_block = self.new_block();
            let fail_block = self.new_block();

            if let Some(block) = self.mir_fn.block_mut(return_block) {
                block.set_terminator(Terminator::Goto(check_block));
            }

            self.set_current_block(check_block);
            let cond_local = self.lower_contract_condition(postcondition, Some(return_local));
            self.set_terminator(Terminator::If {
                cond: cond_local,
                then_block: success_block,
                else_block: fail_block,
            });

            self.set_current_block(success_block);
            self.set_terminator(Terminator::Return(Some(return_local)));

            self.set_current_block(fail_block);
            self.set_terminator(Terminator::Unreachable);
        }
    }

    fn lower_contract_condition(
        &mut self,
        condition: &HIRExpr,
        result_local: Option<Local>,
    ) -> Local {
        let mut saved_name_bindings = Vec::<(String, Option<Local>)>::new();
        let mut saved_symbol_bindings = Vec::<(SymbolId, Option<Local>)>::new();

        for (name, symbol, local) in &self.contract_param_bindings {
            let previous_name = self.local_names.insert(name.clone(), *local);
            saved_name_bindings.push((name.clone(), previous_name));
            if symbol.is_valid() {
                let previous_symbol = self.local_symbols.insert(*symbol, *local);
                saved_symbol_bindings.push((*symbol, previous_symbol));
            }
        }

        if let Some(result_local) = result_local {
            let result_name = "result".to_string();
            let previous_result_name = self.local_names.insert(result_name.clone(), result_local);
            saved_name_bindings.push((result_name, previous_result_name));

            let mut result_symbols = Vec::new();
            collect_named_symbols(condition, "result", &mut result_symbols);
            for symbol in result_symbols {
                if symbol.is_valid() {
                    let previous_symbol = self.local_symbols.insert(symbol, result_local);
                    saved_symbol_bindings.push((symbol, previous_symbol));
                }
            }
        }

        let cond_local = self.lower_expr(condition);

        for (symbol, previous) in saved_symbol_bindings.into_iter().rev() {
            if let Some(local) = previous {
                self.local_symbols.insert(symbol, local);
            } else {
                self.local_symbols.remove(&symbol);
            }
        }
        for (name, previous) in saved_name_bindings.into_iter().rev() {
            if let Some(local) = previous {
                self.local_names.insert(name, local);
            } else {
                self.local_names.remove(&name);
            }
        }

        cond_local
    }


    /// 将HIR函数体降级为基本块（不计算返回值）。
    fn lower_body_to_block(&mut self, body: &HIRBody, target_block: usize) {
        self.lower_body_to_block_with_return(body, target_block, true);
    }

    /// 将HIR函数体降级为基本块，计算块值（返回最后一个表达式）。
    fn lower_body_to_block_val(&mut self, body: &HIRBody, target_block: usize) -> Local {
        self.set_current_block(target_block);

        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }

        if let Some(expr) = &body.expr {
            self.lower_expr(expr)
        } else {
            self.add_local(None, LocalKind::Temp, MIR_UNIT)
        }
    }

    /// 将HIR函数体降级为基本块，并在末尾插入return指令。
    fn lower_body_to_block_with_return(
        &mut self,
        body: &HIRBody,
        target_block: usize,
        add_return: bool,
    ) {
        self.set_current_block(target_block);

        // 降级函数体的所有语句到当前基本块。
        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }

        // 若块尾存在表达式，则先降级该表达式并视情况插入 return。
        if let Some(expr) = &body.expr {
            let result_local = self.lower_expr(expr);
            if add_return {
                // Only add return if the current block doesn't already have a
                // terminator (e.g. set by break/continue/return inside the expr).
                let cur = self.current_block();
                let already_terminated = self
                    .mir_fn
                    .block_mut(cur)
                    .map_or(false, |b| b.terminator.is_some());
                if !already_terminated {
                    // 为函数体末尾生成隐式return指令。
                    // 检查是否为main函数的隐式返回情况。
                    let is_main_with_unit_body = self.mir_fn.name == "main"
                        && matches!(self.mir_fn.return_type, MIRType::Int(_))
                        && matches!(*self.get_local_type(result_local), MIRType::Unit);

                    if is_main_with_unit_body {
                        self.set_terminator(Terminator::Return(None));
                    } else {
                        self.set_terminator(Terminator::Return(Some(result_local)));
                    }
                }
            }
        // 若需要添加return终止符则插入return指令。
        } else if add_return {
            // 当需要添加return且最后一个块未终止时，插入return指令。
            // Only set return if the current block doesn't already have a
            // terminator (e.g. set by break/continue/return in a statement).
            let cur = self.current_block();
            let already_terminated = self
                .mir_fn
                .block_mut(cur)
                .map_or(false, |b| b.terminator.is_some());
            if !already_terminated {
                self.set_terminator(Terminator::Return(None));
            }
        }
    }

    /// 将HIR函数体降级为新基本块并返回块索引。
    fn lower_body(&mut self, body: &HIRBody) -> usize {
        let entry_block = self.new_block();
        self.lower_body_to_block(body, entry_block);
        entry_block
    }

    /// 将单条HIR语句降级为MIR指令序列。
    fn lower_stmt(&mut self, stmt: &HIRStmt) {
        match stmt {
            HIRStmt::Let {
                name,
                symbol,
                ty,
                value,
                is_mut,
            } => {
                let kind = if *is_mut {
                    LocalKind::User
                } else {
                    LocalKind::User
                };
                let mir_ty = ty.clone().into();

                if let Some(value_expr) = value {
                    // 先降级 `let` 初始化表达式，再决定绑定策略。
                    let value_local = self.lower_expr(value_expr);

                // 处理let绑定，确定变量种类并降级初始值。
                    let lambda_name = self.lambda_names.get(&value_local).cloned();

                    if let Some(ln) = lambda_name {
                        let env_vars = self
                            .lambda_environments
                            .get(&ln)
                            .map(|env| env.vars.clone())
                            .unwrap_or_default();

                        if env_vars.is_empty() {
                            self.local_names.insert(name.clone(), value_local);
                            self.bind_local_symbol(*symbol, value_local);
                        } else {
                            let local = self.add_local(Some(name.clone()), kind, mir_ty);
                            self.bind_local_symbol(*symbol, local);
                            self.lambda_names.insert(local, ln.clone());

                // 若初始值为lambda，需要为其生成捕获环境。
                            // 为lambda捕获环境分配数组局部变量。
                            let env_elem_ty = MIR_I64;
                            let env_ty = MIRType::Array(
                                Box::new(env_elem_ty.clone()),
                                env_vars.len() as u64,
                            );

                            // 创建lambda捕获环境数组local。
                            let env_local = self.mir_fn.add_local(LocalKind::User, env_ty);

                            // 遍历lambda环境变量，存入捕获数组。
                            for (i, (var_name, _var_local)) in env_vars.iter().enumerate() {
                                // 若变量已在当前上下文中绑定，则把它加入 lambda 捕获环境。
                                if let Some(&captured_local) = self.local_names.get(var_name) {
                                // 计算被捕获变量的元素地址并存入环境。
                                    let elem_addr_local = self.add_local(
                                        None,
                                        LocalKind::Temp,
                                        MIRType::Ptr(Box::new(env_elem_ty.clone())),
                                    );
                                    let index_local =
                                        self.add_local(None, LocalKind::Temp, MIR_I64);
                                    self.push_inst(Instruction::Assign {
                                        destination: index_local,
                                        value: MirConstant::Int(i as i64),
                                    });
                                    self.push_inst(Instruction::IndexAddr {
                                        destination: elem_addr_local,
                                        base: env_local,
                                        index: index_local,
                                    });

                                // 从外层上下文加载被捕获的变量值。
                                    let captured_value_local =
                                        self.add_local(None, LocalKind::Temp, env_elem_ty.clone());
                                    self.push_inst(Instruction::Load {
                                        destination: captured_value_local,
                                        source: captured_local,
                                    });

                // 将捕获变量的值逐一存入环境数组中。
                                    self.push_inst(Instruction::Store {
                                        destination: elem_addr_local,
                                        value: captured_value_local,
                                    });
                                }
                            }

                            // 分配指向lambda捕获环境的指针。
                            // 分配指向lambda捕获环境的指针局部变量。
                            let env_ptr_local = self
                                .mir_fn
                                .add_local(LocalKind::Temp, MIRType::Ptr(Box::new(env_elem_ty)));
                            self.push_inst(Instruction::AddrOf {
                                destination: env_ptr_local,
                                source: env_local,
                            });

                            // 更新lambda环境中的env_ptr_local字段。
                            if let Some(env_mut) = self.lambda_environments.get_mut(&ln) {
                                env_mut.env_ptr_local = Some(env_ptr_local);
                            } else {
                                self.errors.push(format!(
                                    "MIR lowering: lambda environment not found for '{}' in Let binding",
                                    ln
                                ));
                            }
                        }
                    } else {
                    // 若为lambda引用，处理捕获环境传递。
                        // 若捕获变量为数组类型，直接存储引用。
                        let value_ty = self.get_local_type(value_local).clone();
                        let value_info_opt = self
                            .mir_fn
                            .locals
                            .iter()
                            .find(|(l, _)| l == &value_local)
                            .map(|(l, _t)| l.clone());

                        let value_info = match value_info_opt {
                            Some(info) => info,
                            None => {
                                self.errors.push(format!(
                                    "MIR lowering: local info not found for local {:?} in Let binding for '{}'",
                                    value_local, name
                                ));
                                // Fall through to the normal path with a new local
                                let local = self.add_local(Some(name.clone()), kind, mir_ty);
                                self.bind_local_symbol(*symbol, local);
                                if let Some(type_name) = self.type_names.get(&value_local).cloned()
                                {
                                    self.type_names.insert(local, type_name);
                                }
                                self.push_inst(Instruction::Store {
                                    destination: local,
                                    value: value_local,
                                });
                                // Propagate future origin through the let binding.
                                if let Some(origin) = self.future_origins.get(&value_local).cloned() {
                                    self.future_origins.insert(local, origin);
                                }
                                return;
                            }
                        };

                        if matches!(value_ty, MIRType::Array(_, _))
                            && value_info.kind == LocalKind::User
                        {
                            // 将数组类型变量直接加入local_names映射。
                            // 数组类型直接复用local，加入名称映射。
                            self.local_names.insert(name.clone(), value_local);
                            self.bind_local_symbol(*symbol, value_local);
                            // 绑定符号到local并传播类型名。
                        } else {
                            // 普通类型：创建新local并生成Store指令。
                            // 否则创建新局部变量并生成Store指令。
                            let actual_ty = value_ty.clone();
                            let local = self.add_local(Some(name.clone()), kind, actual_ty);
                            self.bind_local_symbol(*symbol, local);
                            // 将类型名传播给新创建的局部变量。
                            if let Some(type_name) = self.type_names.get(&value_local).cloned() {
                                self.type_names.insert(local, type_name);
                            }
                            self.push_inst(Instruction::Store {
                                destination: local,
                                value: value_local,
                            });
                            // Propagate future origin through the let binding.
                            if let Some(origin) = self.future_origins.get(&value_local).cloned() {
                                self.future_origins.insert(local, origin);
                            }
                        }
                    }
                } else {
                    // 普通（非异步）let 绑定，直接添加局部变量。
                    let local = self.add_local(Some(name.clone()), kind, mir_ty);
                    self.bind_local_symbol(*symbol, local);
                }
            }
            HIRStmt::Expr(expr) => {
                self.lower_expr(expr);
            }
            HIRStmt::Item => {}
        }
    }

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

    fn emit_print_value(&mut self, value_local: Local, value_ty: &MIRType) {
        match value_ty {
            MIRType::Struct { name, fields } => {
                self.emit_print_str_literal(&format!("{} {{ ", name));

                let fields = fields.clone();
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

    fn lower_builtin_print(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "print expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let arg_local = arg_locals[0];
        let arg_ty = self.get_local_type(arg_local).clone();
        self.emit_print_value(arg_local, &arg_ty);
        self.add_local(None, LocalKind::Temp, MIR_UNIT)
    }

    fn lower_builtin_spawn(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "spawn expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let future_handle = arg_locals[0];
        let base_name = self.resolve_async_base_name(future_handle);
        if base_name == "unknown" {
            self.errors.push(
                "spawn requires a future produced by an async function or async block".to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let kind_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: kind_local,
            value: MirConstant::Int(async_spawn_kind_id(&base_name)),
        });

        let task_id = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: task_id,
            func: "sengoo_async_spawn_raw".to_string(),
            args: vec![kind_local, future_handle],
        });

        future_handle
    }

    fn lower_builtin_spawn_task(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "spawn_task expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let future_handle = arg_locals[0];
        let base_name = self.resolve_async_base_name(future_handle);
        if base_name == "unknown" {
            self.errors.push(
                "spawn_task requires a future produced by an async function or async block"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let kind_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: kind_local,
            value: MirConstant::Int(async_spawn_kind_id(&base_name)),
        });

        let task_id = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: task_id,
            func: "sengoo_async_spawn_raw".to_string(),
            args: vec![kind_local, future_handle],
        });

        task_id
    }

    fn lower_builtin_sleep(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "sleep expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let duration_local = arg_locals[0];
        let future_local =
            self.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_UNIT)));
        self.push_inst(Instruction::Call {
            destination: future_local,
            func: "sengoo_async_sleep__start".to_string(),
            args: vec![duration_local],
        });
        self.future_origins
            .insert(future_local, "sengoo_async_sleep".to_string());
        future_local
    }

    fn lower_builtin_timeout(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 2 {
            self.errors.push(format!(
                "timeout expects exactly two arguments, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let future_handle = arg_locals[0];
        let duration_local = arg_locals[1];
        let base_name = self.resolve_async_base_name(future_handle);
        if base_name == "unknown" {
            self.errors.push(
                "timeout requires a future produced by an async function or async block"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let kind_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: kind_local,
            value: MirConstant::Int(async_spawn_kind_id(&base_name)),
        });

        let future_local =
            self.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_BOOL)));
        self.push_inst(Instruction::Call {
            destination: future_local,
            func: "sengoo_async_timeout_bool__start".to_string(),
            args: vec![kind_local, future_handle, duration_local],
        });
        self.future_origins
            .insert(future_local, "sengoo_async_timeout_bool".to_string());
        future_local
    }

    fn async_await_result_type(&self, future_handle: Local) -> MIRType {
        match self.get_local_type(future_handle) {
            MIRType::Future(inner) => (**inner).clone(),
            _ => MIR_I64,
        }
    }

    fn lower_async_wait(&mut self, future_handle: Local) -> Local {
        let func_name = self.resolve_async_base_name(future_handle);
        if func_name == "unknown" {
            self.errors.push(
                "unable to resolve async future origin during MIR lowering".to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_ty = self.async_await_result_type(future_handle);
        let result_local = self.add_local(None, LocalKind::Temp, result_ty);
        let poll_result = self.add_local(None, LocalKind::Temp, MIR_I64);
        let ready_block = self.new_block();
        let pending_block = self.new_block();

        self.set_terminator(Terminator::Suspend {
            poll_func: format!("{}__poll", func_name),
            future_handle,
            destination: poll_result,
            ready_block,
            pending_block,
        });

        self.set_current_block(pending_block);
        self.set_terminator(Terminator::Goto(self.current_block()));

        self.set_current_block(ready_block);
        self.push_inst(Instruction::Call {
            destination: result_local,
            func: format!("{}__result", func_name),
            args: vec![future_handle],
        });
        result_local
    }

    fn lower_builtin_join(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 2 {
            self.errors.push(format!(
                "join expects exactly two arguments, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let _first_result = self.lower_async_wait(arg_locals[0]);
        let _second_result = self.lower_async_wait(arg_locals[1]);
        self.add_local(None, LocalKind::Temp, MIR_UNIT)
    }

    fn lower_builtin_cancel_task(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "cancel_task expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
        self.push_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_cancel_task".to_string(),
            args: vec![arg_locals[0]],
        });
        result_local
    }

    fn lower_builtin_task_status(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "task_status expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_task_status".to_string(),
            args: vec![arg_locals[0]],
        });
        result_local
    }

    fn lower_builtin_select(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 2 {
            self.errors.push(format!(
                "select expects exactly two arguments, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let first_handle = arg_locals[0];
        let second_handle = arg_locals[1];
        let first_name = self.resolve_async_base_name(first_handle);
        let second_name = self.resolve_async_base_name(second_handle);
        if first_name == "unknown" || second_name == "unknown" {
            self.errors.push(
                "select requires futures produced by async functions or async blocks".to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_ty = self.async_await_result_type(first_handle);
        let Some(select_runtime) = select_runtime_function_name(&result_ty) else {
            self.errors.push(
                "select currently only supports Future values whose results are bool, integer, or float scalars during MIR lowering"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        };

        let second_result_ty = self.async_await_result_type(second_handle);
        if second_result_ty != result_ty {
            self.errors.push(
                "select requires futures with matching result types during MIR lowering"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let first_kind = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: first_kind,
            value: MirConstant::Int(async_spawn_kind_id(&first_name)),
        });

        let second_kind = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: second_kind,
            value: MirConstant::Int(async_spawn_kind_id(&second_name)),
        });

        let result_local = self.add_local(None, LocalKind::Temp, result_ty);
        self.push_inst(Instruction::Call {
            destination: result_local,
            func: select_runtime,
            args: vec![first_kind, first_handle, second_kind, second_handle],
        });
        result_local
    }

    fn lower_expr(&mut self, expr: &HIRExpr) -> Local {
        match expr {
            HIRExpr::Lit(lit) => self.lower_literal(lit),
            HIRExpr::Var { name, symbol } => self.resolve_local(name, *symbol),
            HIRExpr::Unary(op, operand) => {
                // 一元表达式先区分取引用这类需要地址语义的特殊分支。
                match op {
                    hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut => {
                    // 取引用操作：获取操作数的地址。
                        let expr_local = self.lower_expr(operand);
                        let expr_ty = self.get_local_type(expr_local).clone();

                        // 取引用操作：生成指针类型的局部变量。
                        let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
                        let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);

                        // 生成AddrOf指令，创建指针类型local。
                        self.push_inst(Instruction::AddrOf {
                            destination: ptr_local,
                            source: expr_local,
                        });

                        ptr_local
                    }
                    hir::HIRUnaryOp::Deref => {
                    // 解引用操作：通过Load指令读取指针指向的值。
                        let ptr_local = self.lower_expr(operand);
                        let ptr_ty = self.get_local_type(ptr_local).clone();

                        let elem_ty = match ptr_ty {
                            MIRType::Ptr(inner) | MIRType::Ref(inner) => (*inner).clone(),
                            _ => MIR_I64,
                        };

                        let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                        self.push_inst(Instruction::Load {
                            destination: result_local,
                            source: ptr_local,
                        });

                        result_local
                    }
                    _ => {
                        // 其余一元运算按普通值语义降级。
                        let operand_local = self.lower_expr(operand);
                        let mir_op = self.lower_un_op(op);
                        let local = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Unary {
                            destination: local,
                            op: mir_op,
                            operand: operand_local,
                        });
                        local
                    }
                }
            }
            HIRExpr::Binary(op, left, right) => {
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let mir_op = self.lower_bin_op(op);

                // String concatenation: when both operands are string type
                // (Ptr(Int(8))) and the operation is Add, generate a call to
                // sengoo_str_concat instead of a binary add instruction.
                if mir_op == MirBinOp::Add {
                    let is_string_concat = {
                        let left_ty = self.get_local_type(left_local);
                        let right_ty = self.get_local_type(right_local);
                        let is_string = |ty: &MIRType| matches!(ty, MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)));
                        is_string(left_ty) && is_string(right_ty)
                    };
                    if is_string_concat {
                        let result_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
                        let result_local = self.add_local(None, LocalKind::Temp, result_ty);
                        self.push_inst(Instruction::Call {
                            destination: result_local,
                            func: "sengoo_str_concat".to_string(),
                            args: vec![left_local, right_local],
                        });
                        return result_local;
                    }
                }

                // String comparison: when both operands are string type
                // (Ptr(Int(8))) and the operation is Eq or Ne, generate a call
                // to sengoo_str_eq instead of a binary comparison instruction.
                // sengoo_str_eq returns i64 (1=equal, 0=not equal), so we
                // convert to bool by comparing the result with 0.
                if mir_op == MirBinOp::Eq || mir_op == MirBinOp::Ne {
                    let is_string_cmp = {
                        let left_ty = self.get_local_type(left_local);
                        let right_ty = self.get_local_type(right_local);
                        let is_string = |ty: &MIRType| matches!(ty, MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)));
                        is_string(left_ty) && is_string(right_ty)
                    };
                    if is_string_cmp {
                        // Call sengoo_str_eq(left, right) -> i64
                        let call_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Call {
                            destination: call_result,
                            func: "sengoo_str_eq".to_string(),
                            args: vec![left_local, right_local],
                        });

                        // Create constant 0 for comparison
                        let zero = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Assign {
                            destination: zero,
                            value: MirConstant::Int(0),
                        });

                        // Convert i64 result to bool:
                        // 字符串比较：将比较结果转换为bool。
                        // Eq时非零表示相等，Ne时零表示不等。
                        let cmp_op = if mir_op == MirBinOp::Eq {
                            MirBinOp::Ne
                        } else {
                            MirBinOp::Eq
                        };
                        let bool_result = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                        self.push_inst(Instruction::Binary {
                            destination: bool_result,
                            op: cmp_op,
                            left: call_result,
                            right: zero,
                        });

                        return bool_result;
                    }
                }

                // 在生成二元指令前，调和两个操作数的类型。
                // Before generating the binary instruction, reconcile operand
                // types: insert Cast instructions for compatible mismatches or
                // record an error for incompatible types (Requirement 7.4).
                let (left_local, right_local) =
                    self.reconcile_binary_operand_types(left_local, right_local);

                // Determine the result type based on the (possibly cast) operand type.
                let operand_ty = self.get_local_type(left_local).clone();
                let result_ty = match mir_op {
                    MirBinOp::Eq
                    | MirBinOp::Ne
                    | MirBinOp::Lt
                    | MirBinOp::Le
                    | MirBinOp::Gt
                    | MirBinOp::Ge
                    | MirBinOp::LogAnd
                    | MirBinOp::LogOr => MIR_BOOL,
                    _ => operand_ty,
                };
                let local = self.add_local(None, LocalKind::Temp, result_ty);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: mir_op,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Block(body) => {
                self.lower_body(body);
                Local::new(0, LocalKind::Return)
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let then_block = self.new_block();
                let else_block = self.new_block();
                let join_block = self.new_block();

                let cond_local = self.lower_expr(cond);
                self.set_terminator(Terminator::If {
                    cond: cond_local,
                    then_block,
                    else_block,
                });

                // 降级then分支到新基本块，并获取其值。
                let then_val = self.lower_body_to_block_val(then_branch, then_block);
                let then_end = self.current_block();
                if let Some(block) = self.mir_fn.block_mut(then_end) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(join_block));
                    }
                }

                // 降级else分支（如存在）到新基本块。
                if let Some(e) = else_branch {
                    let else_val = self.lower_body_to_block_val(e, else_block);
                    let else_end = self.current_block();
                    if let Some(block) = self.mir_fn.block_mut(else_end) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(join_block));
                        }
                    }

                    // if表达式结果合并：插入Phi或直接赋值。
                    // 合并if-else分支到join块，插入Phi节点。
                    self.set_current_block(join_block);
                    let then_ty = self.get_local_type(then_val).clone();
                    if is_void_like(&then_ty) {
                        self.add_local(None, LocalKind::Temp, MIR_UNIT)
                    } else {
                        let result = self.add_local(None, LocalKind::Temp, then_ty);
                        let incoming = vec![(then_val, then_end), (else_val, else_end)];
                        self.push_inst(Instruction::Phi {
                            destination: result,
                            incoming: incoming.clone(),
                        });
                        self.propagate_future_origin_through_phi(result, &incoming);
                        result
                    }
                } else {
                    // 更新else分支的末尾块并插入跳转指令。
                    if let Some(block) = self.mir_fn.block_mut(else_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(join_block));
                        }
                    }
                    self.set_current_block(join_block);
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Loop(body) => {
                let loop_block = self.new_block();
                let exit_block = self.new_block();

                self.set_terminator(Terminator::Goto(loop_block));

                // 压入循环上下文，约定 `break -> exit_block`，`continue -> loop_block`。
                self.push_loop(exit_block, loop_block);

                // 将loop循环体降级为基本块，设置到loop_block。
                self.lower_body_to_block_with_return(body, loop_block, false);

                // 弹出循环栈，更新break目标信息。
                self.pop_loop();

                // After lowering the body, the current block may differ from
                // loop_block (e.g. when the body contains `if` or other control
                // flow that creates new blocks).  We need to ensure that every
                // block reachable at the end of the body that lacks a terminator
                // unconditionally branches back to loop_block.
                let end_block = self.current_block();
                if end_block != loop_block {
                    // The body introduced extra blocks; make sure the final
                    // block loops back.
                    if let Some(block) = self.mir_fn.block_mut(end_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(loop_block));
                        }
                    }
                }

                // Also ensure loop_block itself loops back when it has no
                // terminator (simple body with no control flow).
                if let Some(block) = self.mir_fn.block_mut(loop_block) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(loop_block));
                    }
                }

                self.set_current_block(exit_block);
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::While { cond, body } => {
                let cond_block = self.new_block();
                let body_block = self.new_block();
                let exit_block = self.new_block();

                self.set_terminator(Terminator::Goto(cond_block));

                // 设置当前基本块为while的条件判断块。
                self.set_current_block(cond_block);
                let cond_local = self.lower_expr(cond);
                self.set_terminator(Terminator::If {
                    cond: cond_local,
                    then_block: body_block,
                    else_block: exit_block,
                });

                // 压入循环上下文，约定 `break -> exit_block`，`continue -> cond_block`。
                self.push_loop(exit_block, cond_block);

                // 将while循环体降级，设置到body_block。
                self.lower_body_to_block_with_return(body, body_block, false);

                // 弹出循环栈，更新break目标信息。
                self.pop_loop();

                // 若 body 末尾块尚未终止，则回跳到 `cond_block`。
                // 当前循环体结束，收集body末尾块的信息。
                // 降级结束后将body末尾块跳转回循环条件块。
                let body_end_block = self.current_block();
                if body_end_block != body_block {
                // 循环体结束后若未终止，添加跳回条件块的跳转。
                    if let Some(block) = self.mir_fn.block_mut(body_end_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(cond_block));
                        }
                    }
                }
                // 若循环体块未被终止，添加到back-edge跳转。
                if let Some(block) = self.mir_fn.block_mut(body_block) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(cond_block));
                    }
                }

                self.set_current_block(exit_block);
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::For {
                var_name,
                iter,
                body,
                ..
            } => {
                // 根据迭代对象形态分别处理 `for` lowering。
                match iter.as_ref() {
                    HIRExpr::Range {
                        start,
                        end,
                        inclusive,
                    } => {
                    // for x in start..end语句，生成对应的范围循环MIR。
                        let cond_block = self.new_block();
                        let body_block = self.new_block();
                        let inc_block = self.new_block();
                        // 生成for x in 0..N形式的范围循环。
                        let exit_block = self.new_block();

                        // 降级start和end表达式并存入局部变量。
                        let start_local = if let Some(s) = start {
                            self.lower_expr(s)
                        } else {
                            // 缺省起点时使用 `0` 作为范围起始。
                            let zero = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Assign {
                                destination: zero,
                                value: MirConstant::Int(0),
                            });
                            zero
                        };

                        let end_local = if let Some(e) = end {
                            self.lower_expr(e)
                        } else {
                            // 计算范围上界（未指定时使用数组长度）。
                            let max = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Assign {
                                destination: max,
                                value: MirConstant::Int(i64::MAX),
                            });
                            max
                        };

                        // 为范围循环生成循环变量和步进逻辑。
                        let loop_var =
                            self.add_local(Some(var_name.clone()), LocalKind::User, MIR_I64);
                        self.push_inst(Instruction::Store {
                            destination: loop_var,
                            value: start_local,
                        });

                        // 条件块循环回while条件判断。
                        self.set_terminator(Terminator::Goto(cond_block));

                        // 生成范围循环的步进增量指令。
                        self.set_current_block(cond_block);
                        let loop_var_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: loop_var_loaded,
                            source: loop_var,
                        });

                        let end_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: end_loaded,
                            source: end_local,
                        });

                        // 生成范围比较条件。
                        let cond_local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                        let compare_op = if *inclusive {
                            MirBinOp::Le
                        } else {
                            MirBinOp::Lt
                        };
                        self.push_inst(Instruction::Binary {
                            destination: cond_local,
                            op: compare_op,
                            left: loop_var_loaded,
                            right: end_loaded,
                        });

                        self.set_terminator(Terminator::If {
                            cond: cond_local,
                            then_block: body_block,
                            else_block: exit_block,
                        });

                        // 压入循环上下文，约定 `break -> exit_block`，`continue -> inc_block`。
                        self.push_loop(exit_block, inc_block);

                        // 降级for-in（迭代器）循环体到body_block。
                        self.lower_body_to_block_with_return(body, body_block, false);

                        // 弹出循环栈，更新break目标信息。
                        self.pop_loop();

                        // 若 body_block 尚未终止，则跳到 `inc_block`。
                        if let Some(block) = self.mir_fn.block_mut(body_block) {
                            if block.terminator.is_none() {
                                block.set_terminator(Terminator::Goto(inc_block));
                            }
                        }

                        // 设置步进块并增加索引变量。
                        self.set_current_block(inc_block);
                        let inc_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: inc_loaded,
                            source: loop_var,
                        });

                        let one = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Assign {
                            destination: one,
                            value: MirConstant::Int(1),
                        });

                        let inc_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Binary {
                            destination: inc_result,
                            op: MirBinOp::Add,
                            left: inc_loaded,
                            right: one,
                        });

                        self.push_inst(Instruction::Store {
                            destination: loop_var,
                            value: inc_result,
                        });

                        // 设置条件块跳转回for条件判断块。
                        self.set_terminator(Terminator::Goto(cond_block));

                        self.set_current_block(exit_block);
                        self.add_local(None, LocalKind::Temp, MIR_UNIT)
                    }
                    _ => {
                    // 处理非范围类型的for-in循环（迭代器模式）。
                        let iter_local = self.lower_expr(iter);
                        let iter_ty = self.get_local_type(iter_local).clone();

                        match iter_ty {
                            MIRType::Array(elem_ty, len) => {
                                // 处理数组形式的 `for x in arr { body }`。
                                let cond_block = self.new_block();
                                let body_block = self.new_block();
                                let inc_block = self.new_block();
                                let exit_block = self.new_block();

                                // 初始化for循环的索引变量。
                                // 为for循环创建索引变量和初始值。
                                let index_var = self.add_local(None, LocalKind::User, MIR_I64);
                                let init_val = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: init_val,
                                    value: MirConstant::Int(0),
                                });
                                self.push_inst(Instruction::Store {
                                    destination: index_var,
                                    value: init_val,
                                });

                                // 为for循环创建循环变量并绑定。
                                let loop_var = self.add_local(
                                    Some(var_name.clone()),
                                    LocalKind::User,
                                    (*elem_ty).clone(),
                                );

                                // 获取数组/切片的长度用于边界检查。
                                let len_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: len_local,
                                    value: MirConstant::Int(len as i64),
                                });

                                // 设置循环条件块并添加跳转。
                                self.set_terminator(Terminator::Goto(cond_block));

                                // 生成数组for循环的条件检查跳转。
                                self.set_current_block(cond_block);
                                let index_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: index_loaded,
                                    source: index_var,
                                });

                                let len_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: len_loaded,
                                    source: len_local,
                                });

                                // 生成边界检查条件 `index < len`。
                                let cond_local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                                self.push_inst(Instruction::Binary {
                                    destination: cond_local,
                                    op: MirBinOp::Lt,
                                    left: index_loaded,
                                    right: len_loaded,
                                });

                                self.set_terminator(Terminator::If {
                                    cond: cond_local,
                                    then_block: body_block,
                                    else_block: exit_block,
                                });

                                // 压入循环上下文，约定 `break -> exit_block`，`continue -> inc_block`。
                                self.push_loop(exit_block, inc_block);

                                // 设置循环体块并绑定当前元素。
                                self.set_current_block(body_block);

                                // 从循环索引变量加载当前值。
                                // 加载索引变量后，计算元素地址。
                                let index_for_addr = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: index_for_addr,
                                    source: index_var,
                                });
                                let elem_addr_local = self.add_local(
                                    None,
                                    LocalKind::Temp,
                                    MIRType::Ptr(elem_ty.clone()),
                                );
                                self.push_inst(Instruction::IndexAddr {
                                    destination: elem_addr_local,
                                    base: iter_local,
                                    index: index_for_addr,
                                });

                                // 通过Load指令从数组加载当前元素。
                                let elem_loaded =
                                    self.add_local(None, LocalKind::Temp, (*elem_ty).clone());
                                self.push_inst(Instruction::Load {
                                    destination: elem_loaded,
                                    source: elem_addr_local,
                                });

                                // 将元素值存入循环变量local。
                                self.push_inst(Instruction::Store {
                                    destination: loop_var,
                                    value: elem_loaded,
                                });

                                // 降级for循环体到新基本块。
                                self.lower_body_to_block_with_return(body, body_block, false);

                                // 弹出循环栈，更新break目标信息。
                                self.pop_loop();

                                // 若 body_block 尚未终止，则跳到 `inc_block`。
                                if let Some(block) = self.mir_fn.block_mut(body_block) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(inc_block));
                                    }
                                }

                                // 生成索引自增逻辑 `index++`。
                                self.set_current_block(inc_block);
                                let inc_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: inc_loaded,
                                    source: index_var,
                                });

                                let one = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: one,
                                    value: MirConstant::Int(1),
                                });

                                let inc_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Binary {
                                    destination: inc_result,
                                    op: MirBinOp::Add,
                                    left: inc_loaded,
                                    right: one,
                                });

                                self.push_inst(Instruction::Store {
                                    destination: index_var,
                                    value: inc_result,
                                });

                                // 跳回条件检查块继续循环。
                                self.set_terminator(Terminator::Goto(cond_block));

                                self.set_current_block(exit_block);
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            }
                            _ => {
                            // 捕获变量解析失败，生成错误信息。
                                self.errors.push(format!(
                                    "for loop: unsupported iterator type: {:?}",
                                    iter_ty
                                ));
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            }
                        }
                    }
                }
            }
            HIRExpr::Call { func, args } => {
                let arg_locals: Vec<Local> = args.iter().map(|a| self.lower_expr(a)).collect();

                // 解析函数表达式、函数名、返回类型和环境指针。
                let (func_name, ret_type, env_ptr_local) = match func.as_ref() {
                    HIRExpr::Var { name, .. } => {
                        // Prefer local function-valued variables (e.g. lambdas) over builtins.
                        if let Some(&var_local) = self.local_names.get(name) {
                            if let Some(lambda_name) = self.lambda_names.get(&var_local) {
                                let ret = self
                                    .function_sigs
                                    .get(lambda_name)
                                    .map(|sig| sig.ret_type.clone())
                                    .unwrap_or(MIR_I64);

                                let env_ptr = self
                                    .lambda_environments
                                    .get(lambda_name)
                                    .and_then(|env| env.env_ptr_local);

                                (lambda_name.clone(), ret, env_ptr)
                            } else {
                                let local_ty = self.get_local_type(var_local).clone();
                                if let MIRType::Fn { ret, .. } = &local_ty {
                                    (mir_local_name(var_local), (**ret).clone(), None)
                                } else {
                                    let ret = self
                                        .function_sigs
                                        .get(name)
                                        .map(|sig| sig.ret_type.clone())
                                        .unwrap_or(MIR_I64);
                                    (name.clone(), ret, None)
                                }
                            }
                        } else if name == "print" {
                            return self.lower_builtin_print(&arg_locals);
                        } else if name == "spawn_task" {
                            return self.lower_builtin_spawn_task(&arg_locals);
                        } else if name == "sleep" {
                            return self.lower_builtin_sleep(&arg_locals);
                        } else if name == "timeout" {
                            return self.lower_builtin_timeout(&arg_locals);
                        } else if name == "spawn" {
                            return self.lower_builtin_spawn(&arg_locals);
                        } else if name == "join" {
                            return self.lower_builtin_join(&arg_locals);
                        } else if name == "cancel_task" {
                            return self.lower_builtin_cancel_task(&arg_locals);
                        } else if name == "task_status" {
                            return self.lower_builtin_task_status(&arg_locals);
                        } else if name == "select" {
                            return self.lower_builtin_select(&arg_locals);
                        } else {
                            let ret = self
                                .function_sigs
                                .get(name)
                                .map(|sig| sig.ret_type.clone())
                                .unwrap_or(MIR_I64);
                            (name.clone(), ret, None)
                        }
                    }
                    _ => (String::new(), MIR_UNIT, None),
                };

                let is_async_call = self.options.async_functions.contains(&func_name);
                let local_ty = if is_async_call {
                    MIRType::Future(Box::new(ret_type.clone()))
                } else {
                    ret_type.clone()
                };

                let local: Local = self.add_local(None, LocalKind::Temp, local_ty);
                if !is_async_call {
                    if let MIRType::Struct { name, .. } = &ret_type {
                        self.type_names.insert(local, name.clone());
                    }
                }

                // 调用lambda时将环境指针作为第一个参数传入。
                let mut final_args = Vec::new();
                if let Some(env_ptr) = env_ptr_local {
                    final_args.push(env_ptr);
                }
                final_args.extend(arg_locals);

                let actual_func = if is_async_call {
                    format!("{}__start", func_name.clone())
                } else {
                    func_name.clone()
                };
                self.push_inst(Instruction::Call {
                    destination: local,
                    func: actual_func,
                    args: final_args,
                });
                // Track which async function produced this future handle.
                if is_async_call {
                    self.future_origins.insert(local, func_name);
                }
                local
            }
            HIRExpr::And(left, right) => {
                // 短路逻辑AND：左侧为false时直接跳过右侧。
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: MirBinOp::LogAnd,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Or(left, right) => {
                // 短路逻辑OR：左侧为true时直接跳过右侧。
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: MirBinOp::LogOr,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Break(value) => {
                // 处理 `break` 表达式。
                if let Some(target) = self.get_break_target() {
                    // 若有break目标，生成到目标块的跳转指令。
                    if let Some(v) = value {
                        self.lower_expr(v);
                    }
                    self.set_terminator(Terminator::Break { target });
                    // break语句：设置跳转目标并返回unit。
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                } else {
                    self.errors.push("break outside of loop".to_string());
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Continue => {
                // 处理 `continue` 表达式。
                if let Some(target) = self.get_continue_target() {
                    self.set_terminator(Terminator::Continue { target });
                    // continue语句：设置跳转目标并返回unit。
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                } else {
                    self.errors.push("continue outside of loop".to_string());
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Assign { target, value } => {
                // 普通赋值：降级目标和值，生成Store或Assign指令。
                // 降级赋值语句，获取目标局部变量。
                let value_local = self.lower_expr(value);

                // 根据赋值目标类型分别生成Store或Assign指令。
                match target.as_ref() {
                    HIRExpr::Var { name, symbol } => {
                        let target_local = self.resolve_local(name, *symbol);
                        if value_local == target_local {
                            // Skip no-op self-assignment (`x = x`) to reduce temp churn.
                            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                        }
                        // 复制类型名到目标局部变量的映射中。
                        if let Some(type_name) = self.type_names.get(&value_local).cloned() {
                            self.type_names.insert(target_local, type_name);
                        }
                        self.push_inst(Instruction::Store {
                            destination: target_local,
                            value: value_local,
                        });
                    }
                    HIRExpr::Index { base, index } => {
                        // 处理索引赋值：`arr[i] = value`。
                        let base_local = self.lower_expr(base);
                        let index_local = self.lower_expr(index);

                        // 结构体字段赋值：计算字段偏移并Store。
                        let base_ty = self.get_local_type(base_local).clone();
                        let elem_ty = match &base_ty {
                            MIRType::Array(elem, _) => (**elem).clone(),
                            _ => {
                                self.errors
                                    .push("index assignment on non-array type".to_string());
                                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                            }
                        };

                        let addr_local =
                            self.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(elem_ty)));
                        self.push_inst(Instruction::IndexAddr {
                            destination: addr_local,
                            base: base_local,
                            index: index_local,
                        });

                        // 数组索引赋值：计算元素地址并Store。
                        self.push_inst(Instruction::Store {
                            destination: addr_local,
                            value: value_local,
                        });
                    }
                    _ => {
                        self.errors.push(format!("unsupported assignment target"));
                    }
                }
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::AssignOp { target, op, value } => {
                // 处理复合赋值：`target op= value`，例如 `x += 1`。
                // 降级赋值语句，获取目标局部变量。
                let value_local = self.lower_expr(value);

                match target.as_ref() {
                    HIRExpr::Var { name, symbol } => {
                        let target_local = self.resolve_local(name, *symbol);
                        // 解析赋值目标并获取目标local类型。
                        let target_ty = self.get_local_type(target_local).clone();
                        let current_val = self.add_local(None, LocalKind::Temp, target_ty.clone());
                        self.push_inst(Instruction::Load {
                            destination: current_val,
                            source: target_local,
                        });
                        // 生成复合赋值的二元运算并Store。
                        let mir_op = self.lower_bin_op(op);
                        let result = self.add_local(None, LocalKind::Temp, target_ty);
                        self.push_inst(Instruction::Binary {
                            destination: result,
                            op: mir_op,
                            left: current_val,
                            right: value_local,
                        });
                        // 将数组元素类型名传播到被赋值的局部变量。
                        self.push_inst(Instruction::Store {
                            destination: target_local,
                            value: result,
                        });
                    }
                    HIRExpr::Index { base, index } => {
                        // 处理索引复合赋值：`arr[i] += value`。
                        let base_local = self.lower_expr(base);
                        let index_local = self.lower_expr(index);

                        // 元组字段赋值：计算字段偏移并Store。
                        let base_ty = self.get_local_type(base_local).clone();
                        let elem_ty = match &base_ty {
                            MIRType::Array(elem, _) => (**elem).clone(),
                            _ => {
                                self.errors.push(
                                    "index compound assignment on non-array type".to_string(),
                                );
                                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                            }
                        };

                        let addr_local = self.add_local(
                            None,
                            LocalKind::Temp,
                            MIRType::Ptr(Box::new(elem_ty.clone())),
                        );
                        self.push_inst(Instruction::IndexAddr {
                            destination: addr_local,
                            base: base_local,
                            index: index_local,
                        });

                        // 复合赋值操作：先加载当前值再计算新值。
                        let current_val = self.add_local(None, LocalKind::Temp, elem_ty.clone());
                        self.push_inst(Instruction::Load {
                            destination: current_val,
                            source: addr_local,
                        });

                        // 生成索引赋值的二元运算并Store到元素地址。
                        let mir_op = self.lower_bin_op(op);
                        let result = self.add_local(None, LocalKind::Temp, elem_ty);
                        self.push_inst(Instruction::Binary {
                            destination: result,
                            op: mir_op,
                            left: current_val,
                            right: value_local,
                        });

                        // 将值存入元组字段对应的内存地址。
                        self.push_inst(Instruction::Store {
                            destination: addr_local,
                            value: result,
                        });
                    }
                    _ => {
                        self.errors
                            .push(format!("unsupported compound assignment target"));
                    }
                }
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::Array(elems) => {
                // 处理数组字面量 `[a, b, c]`。
                // 降级数组字面量，为每个元素生成局部变量。
                let elem_locals: Vec<Local> = elems.iter().map(|e| self.lower_expr(e)).collect();

                // 确定数组元素类型（从第一个元素推断）。
                let elem_ty = if let Some(first_local) = elem_locals.first() {
                    self.get_local_type(*first_local).clone()
                } else {
                    MIR_UNIT
                };
                let array_ty = MIRType::Array(Box::new(elem_ty), elems.len() as u64);

                // 将数组元素local列表打包成MIR Array指令。
                let array_local = self.add_local(None, LocalKind::User, array_ty.clone());
                self.push_inst(Instruction::Aggregate {
                    destination: array_local,
                    fields: elem_locals,
                    ty: array_ty,
                });

                array_local
            }
            HIRExpr::Index { base, index } => {
                // 处理索引表达式 `arr[i]`。
                let base_local = self.lower_expr(base);
                let index_local = self.lower_expr(index);

                // 处理索引表达式：获取数组元素地址并加载。
                let base_ty = self.get_local_type(base_local).clone();
                let elem_ty = match base_ty {
                    MIRType::Array(elem, _) => *elem,
                    _ => MIR_UNIT,
                };

                // 计算元素地址，用GEP（地址偏移）方式索引。
                let addr_local = self.add_local(
                    None,
                    LocalKind::Temp,
                    MIRType::Ptr(Box::new(elem_ty.clone())),
                );
                self.push_inst(Instruction::IndexAddr {
                    destination: addr_local,
                    base: base_local,
                    index: index_local,
                });

                // 生成加法运算用于指针偏移计算。
                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Load {
                    destination: result_local,
                    source: addr_local,
                });

                result_local
            }
            HIRExpr::Struct { name, fields } => {
                let lowered_fields: Vec<(String, Local)> = fields
                    .iter()
                    .map(|(field_name, expr)| (field_name.clone(), self.lower_expr(expr)))
                    .collect();
                let field_locals_by_name: HashMap<String, Local> = lowered_fields
                    .iter()
                    .map(|(field_name, local)| (field_name.clone(), *local))
                    .collect();

                let struct_ty = self
                    .infer_struct_literal_type(name, &field_locals_by_name)
                    .unwrap_or_else(|| MIRType::Struct {
                        name: name.clone(),
                        fields: lowered_fields
                            .iter()
                            .map(|(field_name, local)| {
                                (field_name.clone(), self.get_local_type(*local).clone())
                            })
                            .collect(),
                    });

                let ordered_field_locals: Vec<Local> = match &struct_ty {
                    MIRType::Struct { fields, .. } => fields
                        .iter()
                        .filter_map(|(field_name, _)| field_locals_by_name.get(field_name).copied())
                        .collect(),
                    _ => lowered_fields.iter().map(|(_, local)| *local).collect(),
                };

                let struct_local = self.add_local(None, LocalKind::Temp, struct_ty.clone());
                self.push_inst(Instruction::Aggregate {
                    destination: struct_local,
                    fields: ordered_field_locals,
                    ty: struct_ty.clone(),
                });

                if let MIRType::Struct { name, .. } = &struct_ty {
                    self.type_names.insert(struct_local, name.clone());
                }

                struct_local
            }
            HIRExpr::Field { base, field } => {
                // 字段访问：获取基础表达式local后查字段偏移。
                let base_local = self.lower_expr(base);

                // 计算字段偏移并通过Load指令读取字段值。
                // 处理结构体或Tuple字段访问，计算字段偏移。
                let base_ty = self.get_local_type(base_local).clone();
                let field_index = match &base_ty {
                    MIRType::Struct { fields, .. } => fields
                        .iter()
                        .position(|(name, _)| name == field)
                        .unwrap_or(0),
                    // Tuple fallback for legacy method/struct lowering paths.
                    _ => match field.as_str() {
                        "x" | "left" | "r" => 0,
                        "y" | "right" | "g" => 1,
                        "z" | "b" => 2,
                        "w" | "a" => 3,
                        _ => 0,
                    },
                };
                let elem_ty = match &base_ty {
                    MIRType::Tuple(ref tys) if field_index < tys.len() => tys[field_index].clone(),
                    MIRType::Struct { fields, .. } if field_index < fields.len() => {
                        fields[field_index].1.clone()
                    }
                    _ => MIR_I64,
                };

                // 生成二元运算结果的local并添加指令。
                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Extract {
                    destination: result_local,
                    value: base_local,
                    index: field_index as u32,
                });

                result_local
            }
            HIRExpr::Ref(_is_mut, expr) => {
                // 取引用（Ref）：获取表达式的地址。
                let expr_local = self.lower_expr(expr);
                let expr_ty = self.get_local_type(expr_local).clone();

                // 创建指针类型的局部变量存储地址。
                let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
                let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);

                // 获取表达式地址：创建指针局部变量并赋值。
                let zero_index = self.add_local(None, LocalKind::Temp, MIR_I64);
                self.push_inst(Instruction::Assign {
                    destination: zero_index,
                    value: MirConstant::Int(0),
                });

                self.push_inst(Instruction::IndexAddr {
                    destination: ptr_local,
                    base: expr_local,
                    index: zero_index,
                });

                ptr_local
            }
            HIRExpr::Deref(expr) => {
                // 解引用（Deref）：通过Load读取指针值。
                let ptr_local = self.lower_expr(expr);
                let ptr_ty = self.get_local_type(ptr_local).clone();

                let elem_ty = match ptr_ty {
                    MIRType::Ptr(inner) | MIRType::Ref(inner) => *inner,
                    _ => MIR_UNIT,
                };

                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Load {
                    destination: result_local,
                    source: ptr_local,
                });

                result_local
            }
            HIRExpr::Lambda { params, body } => {
                // Lambda表达式降级，形如|args| body。
                // 收集lambda自由变量（闭包捕获分析）。

                // 获取并递增lambda计数器生成唯一名称。
                let lambda_name = self.lambda_name();

                // 收集 lambda 自由变量，决定是否需要闭包环境。
                let free_vars = self.collect_free_vars(params, body);

                // Lambda 当前统一推断为 `fn(i64, ...) -> i64` 风格的内部表示。
                let mut param_types: Vec<MIRType> = (0..params.len()).map(|_| MIR_I64).collect();
                let ret_type = MIR_I64;

                // 构建lambda的参数类型列表和返回类型。
                let env_param_offset = if free_vars.is_empty() {
                    0
                } else {
                    // 有捕获变量时在参数列表头部插入环境指针（i64指针）。
                    // 若有捕获环境，在lambda参数列表前插入env指针。
                    param_types.insert(0, MIRType::Ptr(Box::new(MIR_I64)));
                    1
                };

                // 构建lambda的MIR函数对象。
                let mut lambda_fn =
                    MirFunction::new(lambda_name.clone(), param_types.clone(), ret_type.clone());
                let lambda_start = lambda_fn.start_block;
                let mut lambda_ctx =
                    LoweringContext::new(
                        &mut lambda_fn,
                        self.lambda_counter,
                        &self.known_functions,
                        &self.function_sigs,
                        self.struct_defs,
                        self.concrete_type_registry.clone(),
                        self.options.clone(),
                        self.inherent_method_templates,
                        self.trait_method_templates,
                    );
                // Set current block for Lambda function entry
                lambda_ctx.current_block = Some(lambda_start);

                // 在子上下文中执行lambda体的降级。
                if !free_vars.is_empty() {
                // 构建lambda子上下文并降级函数体。
                    let env_local = Local::new(1, LocalKind::Param);
                    let env_ptr_name = "__env".to_string();
                    lambda_ctx
                        .local_names
                        .insert(env_ptr_name.clone(), env_local);

                // 将lambda的返回值传出，处理捕获环境。
                    // 将所有捕获变量的值存入环境数组中。
                    for (i, (var_name, _)) in free_vars.iter().enumerate() {
                        // 为每个捕获变量在 lambda 上下文中创建对应的局部变量。
                        let captured_local =
                            lambda_ctx.add_local(Some(var_name.clone()), LocalKind::Temp, MIR_I64);

                        // 使用getelementptr和load读取捕获变量。
                        // 从lambda环境数组按索引加载捕获变量。
                        let index_local = lambda_ctx.add_local(None, LocalKind::Temp, MIR_I64);
                        lambda_ctx.push_inst(Instruction::Assign {
                            destination: index_local,
                            value: MirConstant::Int(i as i64),
                        });

                        let ptr_local = lambda_ctx.add_local(
                            None,
                            LocalKind::Temp,
                            MIRType::Ptr(Box::new(MIR_I64)),
                        );
                        lambda_ctx.push_inst(Instruction::IndexAddr {
                            destination: ptr_local,
                            base: env_local,
                            index: index_local,
                        });

                        // 加载捕获变量的值到lambda上下文。
                        lambda_ctx.push_inst(Instruction::Load {
                            destination: captured_local,
                            source: ptr_local,
                        });

                    // 将捕获变量绑定到lambda上下文的局部变量中。
                        lambda_ctx
                            .local_names
                            .insert(var_name.clone(), captured_local);
                    }

                    // 将函数参数绑定到lambda上下文的局部变量中。
                    for (i, param_name) in params.iter().enumerate() {
                        let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
                        lambda_ctx.local_names.insert(param_name.clone(), local);
                    }
                } else {
                    // 不带捕获变量时，直接绑定函数参数。
                    for (i, param_name) in params.iter().enumerate() {
                        let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
                        lambda_ctx.local_names.insert(param_name.clone(), local);
                    }
                }

                // 降级lambda体，在lambda上下文中执行。
                    // 在lambda上下文中降级函数体（HIRBody）。
                use crate::hir::HIRBody;
                let lambda_body = HIRBody {
                    stmts: vec![],
                    expr: Some(body.clone()),
                };
                lambda_ctx.lower_body_to_block(&lambda_body, lambda_start);

                // 将生成的lambda函数加入函数列表。
                self.lambda_functions.push(lambda_fn);

                // 注册lambda函数签名（带环境参数或不带）。
                if !free_vars.is_empty() {
                    let env_var_types: Vec<(String, MIRType)> = free_vars
                        .iter()
                        .map(|(name, local)| (name.clone(), self.get_local_type(*local).clone()))
                        .collect();
                    self.lambda_environments.insert(
                        lambda_name.clone(),
                        LambdaEnv {
                            vars: free_vars.clone(),
                            env_type: MIRType::Ptr(Box::new(MIR_I64)),
                            env_ptr_local: None, // 由 Let lowering 阶段补回实际 `env_ptr_local`。
                        },
                    );

                    // 带捕获环境的lambda函数签名注册。
                    self.function_sigs.insert(
                        lambda_name.clone(),
                        FunctionSig {
                            ret_type: ret_type.clone(),
                            param_count: param_types.len(),
                            env: env_var_types,
                        },
                    );
                } else {
                    // 不带捕获环境的lambda函数签名注册。
                    self.function_sigs.insert(
                        lambda_name.clone(),
                        FunctionSig {
                            ret_type: ret_type.clone(),
                            param_count: param_types.len(),
                            env: vec![],
                        },
                    );
                }

                // 若无捕获变量，直接返回lambda函数引用。
                // 检查捕获变量列表并获取环境指针。
                let lambda_local = if free_vars.is_empty() {
                    let fn_ty = MIRType::Fn {
                        params: param_types.clone(),
                        ret: Box::new(ret_type.clone()),
                    };
                    let local = self.add_local(None, LocalKind::Temp, fn_ty);
                    self.push_inst(Instruction::Assign {
                        destination: local,
                        value: MirConstant::GlobalRef(lambda_name.clone()),
                    });
                    local
                } else {
                    self.add_local(None, LocalKind::Temp, MIR_I64)
                };

                // 记录 `Local -> Lambda 名称` 的映射，便于后续引用解析。
                self.lambda_names.insert(lambda_local, lambda_name.clone());

                lambda_local
            }
            HIRExpr::Match { scrutinee, arms } => {
                let scrutinee_local = self.lower_expr(scrutinee);
                let scrutinee_ty = self.get_local_type(scrutinee_local).clone();

                match scrutinee_ty {
                    MIRType::Enum { .. } => {
                        let discr_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Discriminant {
                            destination: discr_local,
                            source: scrutinee_local,
                        });

                        let arm_blocks: Vec<usize> =
                            arms.iter().map(|_| self.new_block()).collect();
                        let join_block = self.new_block();

                        let switch_plan = build_match_switch_plan(arms, &arm_blocks, join_block);

                        self.set_terminator(Terminator::Switch {
                            discr: discr_local,
                            targets: switch_plan.targets,
                            otherwise: switch_plan.otherwise_block,
                        });

                        let mut incoming_values: Vec<(Local, usize)> = Vec::new();
                        for (i, arm) in arms.iter().enumerate() {
                            let arm_block = arm_blocks[i];
                            self.set_current_block(arm_block);

                            self.lower_pattern_bindings(&arm.pat, scrutinee_local);
                            let arm_result = self.lower_expr(&arm.body);
                            let arm_end = self.current_block();

                            if let Some(block) = self.mir_fn.block_mut(arm_end) {
                                if block.terminator.is_none() {
                                    block.set_terminator(Terminator::Goto(join_block));
                                    incoming_values.push((arm_result, arm_end));
                                }
                            }
                        }

                        self.set_current_block(join_block);
                        if let Some((first_value, _)) = incoming_values.first().copied() {
                            let result_ty = self.get_local_type(first_value).clone();
                            if is_void_like(&result_ty) {
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            } else {
                                let result = self.add_local(None, LocalKind::Temp, result_ty);
                                self.push_inst(Instruction::Phi {
                                    destination: result,
                                    incoming: incoming_values.clone(),
                                });
                                self.propagate_future_origin_through_phi(result, &incoming_values);
                                result
                            }
                        } else {
                            self.add_local(None, LocalKind::Temp, MIR_UNIT)
                        }
                    }
                    _ => {
                        let join_block = self.new_block();
                        let mut incoming_values: Vec<(Local, usize)> = Vec::new();

                        for (i, arm) in arms.iter().enumerate() {
                            let is_last = i == arms.len() - 1;

                            if is_last {
                                let arm_result = self.lower_expr(&arm.body);
                                let arm_end = self.current_block();
                                if let Some(block) = self.mir_fn.block_mut(arm_end) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(join_block));
                                        incoming_values.push((arm_result, arm_end));
                                    }
                                }
                            } else {
                                let then_block = self.new_block();
                                let next_arm_block = self.new_block();

                                let should_take = self.matches_pattern(&arm.pat, scrutinee_local);
                                self.set_terminator(Terminator::If {
                                    cond: should_take,
                                    then_block,
                                    else_block: next_arm_block,
                                });

                                self.set_current_block(then_block);
                                let arm_result = self.lower_expr(&arm.body);
                                let arm_end = self.current_block();
                                if let Some(block) = self.mir_fn.block_mut(arm_end) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(join_block));
                                        incoming_values.push((arm_result, arm_end));
                                    }
                                }

                                self.set_current_block(next_arm_block);
                            }
                        }

                        self.set_current_block(join_block);
                        if let Some((first_value, _)) = incoming_values.first().copied() {
                            let result_ty = self.get_local_type(first_value).clone();
                            if is_void_like(&result_ty) {
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            } else {
                                let result = self.add_local(None, LocalKind::Temp, result_ty);
                                self.push_inst(Instruction::Phi {
                                    destination: result,
                                    incoming: incoming_values.clone(),
                                });
                                self.propagate_future_origin_through_phi(result, &incoming_values);
                                result
                            }
                        } else {
                            self.add_local(None, LocalKind::Temp, MIR_UNIT)
                        }
                    }
                }
            }
            HIRExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                // 方法调用表达式降级（receiver.method(args)形式）。
                // 进行receiver方法调用的分派解析。

                // 获取方法名，支持路径和标识符形式。
                let receiver_local = self.lower_expr(receiver);
                let receiver_ty = self.get_local_type(receiver_local).clone();

                // 降级所有参数表达式为局部变量列表。
                let arg_locals: Vec<Local> = args.iter().map(|a| self.lower_expr(a)).collect();

                // String built-in method handling: when receiver is a string
                // (Ptr to i8), intercept known methods and generate runtime calls.
                if let MIRType::Ptr(inner) = &receiver_ty {
                    if let MIRType::Int(8) = inner.as_ref() {
                        if method == "len" {
                            // Generate call to sengoo_str_len(receiver) -> i64
                            let result_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Call {
                                destination: result_local,
                                func: "sengoo_str_len".to_string(),
                                args: vec![receiver_local],
                            });
                            return result_local;
                        }
                    }
                }

                // 解析方法调用目标并获取函数签名。
                // 使用Sengoo的方法解析逻辑确定最终调用目标。
                // 解析方法调用的目标函数名称（含trait分派逻辑）。
                let resolved_func_name = match self.resolve_method_call_target(
                    receiver_local,
                    &receiver_ty,
                    method,
                    &arg_locals,
                ) {
                    Ok(name) => name,
                    Err(error) => {
                        self.errors.push(error);
                        return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                    }
                };



                // 获取方法调用的返回类型信息。
                let ret_type = self
                    .function_sigs
                    .get(&resolved_func_name)
                    .map(|sig| sig.ret_type.clone())
                    .unwrap_or(MIR_I64);
                let result_local = self.add_local(None, LocalKind::Temp, ret_type.clone());
                if let MIRType::Struct { name, .. } = &ret_type {
                    self.type_names.insert(result_local, name.clone());
                }

                // 构造调用参数列表，包含receiver和其他参数。
                let mut call_args = vec![receiver_local];
                call_args.extend(arg_locals);

                // 生成Call指令并返回结果local。
                self.push_inst(Instruction::Call {
                    destination: result_local,
                    func: resolved_func_name,
                    args: call_args,
                });

                result_local
            }
                // 处理await表达式（仅用于类型系统，无实际异步支持）。
            HIRExpr::Await(inner) => {
                let future_handle = self.lower_expr(inner);
                self.lower_async_wait(future_handle)
            }
            HIRExpr::AsyncBlock(body) => self.lower_async_block(body),
            _ => self.add_local(None, LocalKind::Temp, MIR_UNIT),
        }
    }

    fn infer_poll_func_from_last_call(&self) -> String {
        let block = &self.mir_fn.basic_blocks[self.current_block()];
        let instructions = block
            .instructions
            .iter()
            .map(|inst_id| self.mir_fn.instruction(*inst_id).clone())
            .collect::<Vec<_>>();
        infer_last_async_start_base(&instructions).unwrap_or_else(|| "unknown".to_string())
    }

    /// Resolve the async function base name for a given future handle local.
    ///
    /// Resolution order:
    ///  1. Direct lookup in `future_origins` — covers `await async_fn(args)`.
    ///  2. If the handle came from a `Load { destination: handle, source: src }`,
    ///     look up `src` in `future_origins` — covers `let f = async_fn(); await f`.
    ///  3. Fall back to backward-scan heuristic via `infer_poll_func_from_last_call`.
    fn resolve_async_base_name(&self, handle: Local) -> String {
        let block = &self.mir_fn.basic_blocks[self.current_block()];
        let instructions = block
            .instructions
            .iter()
            .map(|inst_id| self.mir_fn.instruction(*inst_id).clone())
            .collect::<Vec<_>>();

        infer_async_base_name_from_instructions(handle, &instructions, &self.future_origins)
            .unwrap_or_else(|| self.infer_poll_func_from_last_call())
    }

    /// 从模式中提取可用于匹配的枚举判别值。
    /// 从枚举模式中提取判别值（discriminant）用于匹配检查。

    /// 根据给定值生成与 HIR 模式匹配的判断逻辑。
    /// 判断值是否匹配给定的HIR模式，用于运行时合约检查。
    fn matches_pattern(&mut self, pat: &crate::hir::HIRPattern, value: Local) -> Local {
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
    fn lower_pattern_bindings(&mut self, pat: &crate::hir::HIRPattern, enum_value: Local) {
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

    /// 将HIR字面量降级为MIR常量指令。
    fn lower_literal(&mut self, lit: &HIRLiteral) -> Local {
        let constant = match lit {
            HIRLiteral::Int(n) => MirConstant::Int(*n),
            HIRLiteral::Float(f) => MirConstant::Float(*f),
            HIRLiteral::String(s) => MirConstant::String(s.clone()),
            HIRLiteral::Bool(b) => MirConstant::Bool(*b),
            HIRLiteral::Char(c) => MirConstant::Char(*c),
            HIRLiteral::Null => MirConstant::Unit,
            HIRLiteral::Bytes(b) => MirConstant::Bytes(b.clone()),
            HIRLiteral::Uint(u) => MirConstant::Uint(*u),
        };
        let ty = constant.ty();
        let local = self.add_local(None, LocalKind::Temp, ty);
        self.push_inst(Instruction::Assign {
            destination: local,
            value: constant,
        });
        local
    }

    /// 将HIR一元运算符转换为MIR一元运算符。
    fn lower_un_op(&self, op: &hir::HIRUnaryOp) -> MirUnOp {
        match op {
            hir::HIRUnaryOp::Neg => MirUnOp::Neg,
            hir::HIRUnaryOp::Not => MirUnOp::Not,
            hir::HIRUnaryOp::BitNot => MirUnOp::BitNot,
            hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut | hir::HIRUnaryOp::Deref => MirUnOp::Neg,
        }
    }

    /// 将HIR二元运算符转换为MIR二元运算符。
    fn lower_bin_op(&self, op: &hir::HIRBinaryOp) -> MirBinOp {
        match op {
            hir::HIRBinaryOp::Add => MirBinOp::Add,
            hir::HIRBinaryOp::Sub => MirBinOp::Sub,
            hir::HIRBinaryOp::Mul => MirBinOp::Mul,
            hir::HIRBinaryOp::Div => MirBinOp::Div,
            hir::HIRBinaryOp::Mod => MirBinOp::Rem,
            hir::HIRBinaryOp::BitAnd => MirBinOp::BitAnd,
            hir::HIRBinaryOp::BitOr => MirBinOp::BitOr,
            hir::HIRBinaryOp::BitXor => MirBinOp::BitXor,
            hir::HIRBinaryOp::Shl => MirBinOp::Shl,
            hir::HIRBinaryOp::Shr => MirBinOp::Shr,
            hir::HIRBinaryOp::LogAnd => MirBinOp::LogAnd,
            hir::HIRBinaryOp::LogOr => MirBinOp::LogOr,
            hir::HIRBinaryOp::Eq => MirBinOp::Eq,
            hir::HIRBinaryOp::NotEq => MirBinOp::Ne,
            hir::HIRBinaryOp::Lt => MirBinOp::Lt,
            hir::HIRBinaryOp::Gt => MirBinOp::Gt,
            hir::HIRBinaryOp::Le => MirBinOp::Le,
            hir::HIRBinaryOp::Ge => MirBinOp::Ge,
            hir::HIRBinaryOp::Assign => MirBinOp::Add,
        }
    }
}
