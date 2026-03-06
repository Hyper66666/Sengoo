//! HIR 閿?MIR 閻ㄥ嫯娴嗛敓?

use crate::hir::{
    self, HIRBody, HIRExpr, HIRItem, HIRLiteral, HIRParam, HIRStmt, HIRType, HIRTypeKind,
};
use crate::hir::{HIRTrait, HIRTraitItem};
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirBinOp, MirConstant, MirFunction, MirUnOp,
    Terminator, MIR_BOOL, MIR_I64, MIR_UNIT,
};
use crate::symbol::SymbolId;
use std::collections::{HashMap, HashSet};

/// 閿?HIRType 鏉烆剚宕叉稉铏硅閸ㄥ澧犵紓鈧€涙顑佹稉璇х礄閻劋绨弬瑙勭《閸氬秳鎱ㄦ甯礆
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirLowerOptions {
    pub runtime_contract_checks: bool,
    pub lazy_generic_mono: bool,
}

impl Default for MirLowerOptions {
    fn default() -> Self {
        Self {
            runtime_contract_checks: false,
            lazy_generic_mono: true,
        }
    }
}

fn hir_type_to_prefix(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Int(ik) => format!("i{}", ik.bits()),
        HIRTypeKind::Float(fk) => format!("f{}", fk.bits()),
        HIRTypeKind::Bool => "bool".to_string(),
        HIRTypeKind::Unit => "unit".to_string(),
        HIRTypeKind::Named { name, .. } => name.clone(),
        _ => "unknown".to_string(),
    }
}

/// 閿?HIR 濡€虫健鏉烆剚宕查敓?MIR 閸戣姤鏆熼梿鍡楁値
pub fn lower_hir(items: &[HIRItem]) -> Result<Vec<MirFunction>, String> {
    lower_hir_with_options(items, MirLowerOptions::default())
}

fn collect_direct_calls_in_expr(expr: &HIRExpr, out: &mut HashSet<String>) {
    match expr {
        HIRExpr::Lit(_) | HIRExpr::Var { .. } | HIRExpr::Continue => {}
        HIRExpr::Unary(_, inner)
        | HIRExpr::Deref(inner)
        | HIRExpr::Ref(_, inner)
        | HIRExpr::Cast(inner, _)
        | HIRExpr::Ascribe(inner, _)
        | HIRExpr::Await(inner) => {
            collect_direct_calls_in_expr(inner, out);
        }
        HIRExpr::Binary(_, lhs, rhs)
        | HIRExpr::And(lhs, rhs)
        | HIRExpr::Or(lhs, rhs)
        | HIRExpr::Assign {
            target: lhs,
            value: rhs,
        }
        | HIRExpr::AssignOp {
            target: lhs,
            value: rhs,
            ..
        } => {
            collect_direct_calls_in_expr(lhs, out);
            collect_direct_calls_in_expr(rhs, out);
        }
        HIRExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_direct_calls_in_expr(cond, out);
            collect_direct_calls_in_body(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_direct_calls_in_body(else_branch, out);
            }
        }
        HIRExpr::Match { scrutinee, arms } => {
            collect_direct_calls_in_expr(scrutinee, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_deref() {
                    collect_direct_calls_in_expr(guard, out);
                }
                collect_direct_calls_in_expr(&arm.body, out);
            }
        }
        HIRExpr::Loop(body) | HIRExpr::Block(body) => {
            collect_direct_calls_in_body(body, out);
        }
        HIRExpr::While { cond, body } => {
            collect_direct_calls_in_expr(cond, out);
            collect_direct_calls_in_body(body, out);
        }
        HIRExpr::For { iter, body, .. } => {
            collect_direct_calls_in_expr(iter, out);
            collect_direct_calls_in_body(body, out);
        }
        HIRExpr::Call { func, args } => {
            if let HIRExpr::Var { name, .. } = func.as_ref() {
                out.insert(name.clone());
            }
            collect_direct_calls_in_expr(func, out);
            for arg in args {
                collect_direct_calls_in_expr(arg, out);
            }
        }
        HIRExpr::MethodCall { receiver, args, .. } => {
            collect_direct_calls_in_expr(receiver, out);
            for arg in args {
                collect_direct_calls_in_expr(arg, out);
            }
        }
        HIRExpr::Struct { fields, .. } => {
            for (_, value) in fields {
                collect_direct_calls_in_expr(value, out);
            }
        }
        HIRExpr::Array(values) | HIRExpr::Tuple(values) => {
            for value in values {
                collect_direct_calls_in_expr(value, out);
            }
        }
        HIRExpr::Index { base, index } => {
            collect_direct_calls_in_expr(base, out);
            collect_direct_calls_in_expr(index, out);
        }
        HIRExpr::Field { base, .. } => {
            collect_direct_calls_in_expr(base, out);
        }
        HIRExpr::Return(value) | HIRExpr::Break(value) => {
            if let Some(value) = value.as_deref() {
                collect_direct_calls_in_expr(value, out);
            }
        }
        HIRExpr::Range { start, end, .. } => {
            if let Some(start) = start.as_deref() {
                collect_direct_calls_in_expr(start, out);
            }
            if let Some(end) = end.as_deref() {
                collect_direct_calls_in_expr(end, out);
            }
        }
        HIRExpr::Lambda { body, .. } => {
            collect_direct_calls_in_expr(body, out);
        }
    }
}

fn collect_direct_calls_in_stmt(stmt: &HIRStmt, out: &mut HashSet<String>) {
    match stmt {
        HIRStmt::Let { value, .. } => {
            if let Some(value) = value {
                collect_direct_calls_in_expr(value, out);
            }
        }
        HIRStmt::Expr(expr) => collect_direct_calls_in_expr(expr, out),
        HIRStmt::Item => {}
    }
}

fn collect_direct_calls_in_body(body: &HIRBody, out: &mut HashSet<String>) {
    for stmt in &body.stmts {
        collect_direct_calls_in_stmt(stmt, out);
    }
    if let Some(expr) = body.expr.as_deref() {
        collect_direct_calls_in_expr(expr, out);
    }
}

fn collect_direct_call_names(items: &[HIRItem]) -> HashSet<String> {
    let mut out = HashSet::new();
    for item in items {
        match item {
            HIRItem::Function(function) => collect_direct_calls_in_body(&function.body, &mut out),
            HIRItem::Impl(impl_item) => {
                for method in &impl_item.items {
                    collect_direct_calls_in_body(&method.body, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

#[derive(Debug, Clone)]
struct AsyncFunctionInfo {
    params: Vec<MIRType>,
    ret_type: MIRType,
    body_symbol: String,
}

fn async_start_symbol(name: &str) -> String {
    format!("{name}__start")
}

fn async_poll_symbol(name: &str) -> String {
    format!("{name}__poll")
}

fn async_result_symbol(name: &str) -> String {
    format!("{name}__result")
}

fn async_body_symbol(name: &str) -> String {
    format!("{name}__async_body")
}

fn mir_type_size_bytes(ty: &MIRType) -> u64 {
    match ty {
        MIRType::Unit | MIRType::Never => 1,
        MIRType::Bool => 1,
        MIRType::Int(bits) => ((*bits as u64) / 8).max(1),
        MIRType::Float(bits) => ((*bits as u64) / 8).max(1),
        MIRType::Ref(_) | MIRType::Ptr(_) | MIRType::Fn { .. } => 8,
        MIRType::Array(elem, len) => mir_type_size_bytes(elem).saturating_mul(*len),
        MIRType::Tuple(fields) => fields.iter().map(mir_type_size_bytes).sum::<u64>().max(1),
        MIRType::Struct { fields, .. } => fields
            .iter()
            .map(|(_, ty)| mir_type_size_bytes(ty))
            .sum::<u64>()
            .max(1),
        MIRType::Enum { .. } => 16,
    }
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

    // Collect trait definitions for default method resolution
    let mut trait_defs: HashMap<String, &HIRTrait> = HashMap::new();
    for item in items {
        if let HIRItem::Trait(trait_item) = item {
            trait_defs.insert(trait_item.name.clone(), trait_item);
        }
    }

    // First pass: collect all known function names (top-level functions and impl methods)
    let mut known_functions: HashSet<String> = HashSet::new();
    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                known_functions.insert(fn_item.name.clone());
            }
            HIRItem::Impl(impl_item) => {
                let type_prefix = hir_type_to_prefix(&impl_item.target_type);
                if let Some(trait_name) = &impl_item.trait_name {
                    // Collect method names that are explicitly implemented
                    let mut impl_method_names: HashSet<String> = HashSet::new();
                    for method in &impl_item.items {
                        let original_method_name = method
                            .name
                            .strip_prefix(&format!("{}_", type_prefix))
                            .unwrap_or(&method.name);
                        impl_method_names.insert(original_method_name.to_string());
                        let three_part_name =
                            format!("{}_{}_{}", type_prefix, trait_name, original_method_name);
                        known_functions.insert(three_part_name);
                    }

                    // Also register default methods from the trait definition
                    // that are not overridden by the impl
                    if let Some(trait_def) = trait_defs.get(trait_name.as_str()) {
                        for trait_item in &trait_def.items {
                            if let HIRTraitItem::Function(trait_fn) = trait_item {
                                if !impl_method_names.contains(&trait_fn.name) {
                                    // This trait method has a default implementation
                                    // and is not overridden 閿?register it
                                    let three_part_name =
                                        format!("{}_{}_{}", type_prefix, trait_name, trait_fn.name);
                                    known_functions.insert(three_part_name);
                                }
                            }
                        }
                    }
                } else {
                    for method in &impl_item.items {
                        // Inherent impl: method names are already mangled as TypePrefix_MethodName by HIR lowering
                        known_functions.insert(method.name.clone());
                    }
                }
            }
            _ => {}
        }
    }

    let mut async_functions: HashMap<String, AsyncFunctionInfo> = HashMap::new();
    for item in items {
        if let HIRItem::Function(fn_item) = item {
            if fn_item.is_async {
                async_functions.insert(
                    fn_item.name.clone(),
                    AsyncFunctionInfo {
                        params: fn_item.params.iter().map(|p| p.ty.clone().into()).collect(),
                        ret_type: fn_item.return_type.clone().into(),
                        body_symbol: if fn_item.name == "main" {
                            async_body_symbol(&fn_item.name)
                        } else {
                            fn_item.name.clone()
                        },
                    },
                );
            }
        }
    }

    // Second pass: lower all items
    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                if options.lazy_generic_mono
                    && !fn_item.type_params.is_empty()
                    && !direct_calls.contains(&fn_item.name)
                {
                    continue;
                }

                let mut lowered_fn = fn_item.clone();
                if fn_item.is_async && fn_item.name == "main" {
                    lowered_fn.name = async_body_symbol(&fn_item.name);
                }

                match lower_function(
                    &lowered_fn,
                    &mut lambda_counter,
                    &known_functions,
                    &async_functions,
                    options,
                ) {
                    Ok((mir_fn, lambdas)) => {
                        results.push(mir_fn);
                        results.extend(lambdas);

                        if fn_item.is_async {
                            let info = async_functions
                                .get(&fn_item.name)
                                .expect("async function metadata should exist");
                            results.push(synthesize_async_start_function(&fn_item.name, info));
                            results.push(synthesize_async_poll_function(&fn_item.name));
                            results.push(synthesize_async_result_function(&fn_item.name, info));
                            if fn_item.name == "main" {
                                if info.ret_type == MIR_I64 {
                                    results.push(synthesize_async_main_wrapper(info));
                                } else {
                                    errors.push(
                                        "phase-1 async main currently requires an i64 return type"
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => errors.push(e),
                }
            }
            HIRItem::Impl(impl_item) => {
                let type_prefix = hir_type_to_prefix(&impl_item.target_type);
                // 婢跺嫮鎮?impl 閸фぞ鑵戦惃鍕煙閿?
                let mut impl_method_names: HashSet<String> = HashSet::new();
                for method in &impl_item.items {
                    if let Some(trait_name) = &impl_item.trait_name {
                        // Trait impl: rename method to three-part mangled name
                        // {TypePrefix}_{TraitName}_{MethodName}
                        let original_method_name = method
                            .name
                            .strip_prefix(&format!("{}_", type_prefix))
                            .unwrap_or(&method.name);
                        impl_method_names.insert(original_method_name.to_string());
                        let three_part_name =
                            format!("{}_{}_{}", type_prefix, trait_name, original_method_name);
                        let mut renamed_method = method.clone();
                        renamed_method.name = three_part_name;
                        match lower_function(
                            &renamed_method,
                            &mut lambda_counter,
                            &known_functions,
                            &async_functions,
                            options,
                        ) {
                            Ok((mir_fn, lambdas)) => {
                                results.push(mir_fn);
                                results.extend(lambdas);
                            }
                            Err(e) => errors.push(e),
                        }
                    } else {
                        // Inherent impl: use existing two-part mangled name
                        match lower_function(method, &mut lambda_counter, &known_functions, &async_functions, options)
                        {
                            Ok((mir_fn, lambdas)) => {
                                results.push(mir_fn);
                                results.extend(lambdas);
                            }
                            Err(e) => errors.push(e),
                        }
                    }
                }

                // Handle default trait method implementations:
                // For trait impls, check if any trait methods are missing from the impl
                // and generate wrapper functions for default implementations.
                if let Some(trait_name) = &impl_item.trait_name {
                    if let Some(trait_def) = trait_defs.get(trait_name.as_str()) {
                        for trait_item in &trait_def.items {
                            if let HIRTraitItem::Function(trait_fn) = trait_item {
                                if !impl_method_names.contains(&trait_fn.name) {
                                    // This method was not overridden 閿?use the default implementation.
                                    // Create a new HIRFunction with:
                                    // - Three-part mangled name: {TypePrefix}_{TraitName}_{MethodName}
                                    // - self parameter added with the impl target type
                                    // - The default body from the trait definition
                                    let three_part_name =
                                        format!("{}_{}_{}", type_prefix, trait_name, trait_fn.name);

                                    // Build the parameter list: add self as first param if the
                                    // trait method has a self parameter (check if any param is named "self"
                                    // or if the original AST had a self_param).
                                    // Since trait methods lowered via lower_function (without self_type)
                                    // don't get a self param, we need to add it ourselves.
                                    let mut params = Vec::new();
                                    let has_self = trait_fn.params.iter().any(|p| p.name == "self");
                                    if !has_self {
                                        // The trait method likely takes self but it wasn't added
                                        // during HIR lowering (since lower_function was used without
                                        // self_type). Add self with the impl target type.
                                        params.push(HIRParam::new(
                                            "self".to_string(),
                                            SymbolId::INVALID,
                                            impl_item.target_type.clone(),
                                        ));
                                    }
                                    params.extend(trait_fn.params.iter().cloned());

                                    let default_fn = hir::HIRFunction {
                                        name: three_part_name,
                                        type_params: trait_fn.type_params.clone(),
                                        params,
                                        return_type: trait_fn.return_type.clone(),
                                        precondition: trait_fn.precondition.clone(),
                                        postcondition: trait_fn.postcondition.clone(),
                                        body: trait_fn.body.clone(),
                                        is_async: trait_fn.is_async,
                                        abi: trait_fn.abi.clone(),
                                        is_unsafe: trait_fn.is_unsafe,
                                        no_mangle: trait_fn.no_mangle,
                                        export_name: trait_fn.export_name.clone(),
                                        is_pub: trait_fn.is_pub,
                                    };

                                    match lower_function(
                                        &default_fn,
                                        &mut lambda_counter,
                                        &known_functions,
                                        &async_functions,
                                        options,
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
                    }
                }
            }
            // 閸忔湹绮?HIR 妞ょ櫢绱橲truct, Enum, Trait 缁涘绱氶弳鍌涙鐠哄疇绻?
            _ => {}
        }
    }

    if !errors.is_empty() {
        return Err(format!("MIR lowering failed:\n{}", errors.join("\n")));
    }

    Ok(results)
}

/// 閿?HIR 閸戣姤鏆熸潪顒佸床閿?MIR 閸戣姤鏆?
/// 鏉╂柨娲?(娑撹鍤遍敓? Lambda 鏉堝懎濮崙鑺ユ殶閸掓銆?
fn synthesize_async_start_function(name: &str, info: &AsyncFunctionInfo) -> MirFunction {
    let mut mir_fn = MirFunction::new(async_start_symbol(name), info.params.clone(), MIR_I64);
    let block = mir_fn.start_block;
    let args: Vec<Local> = (1..=info.params.len())
        .map(|idx| Local::new(idx, LocalKind::Param))
        .collect();

    if matches!(info.ret_type, MIRType::Unit | MIRType::Never) {
        let call_dest = mir_fn.add_local(LocalKind::Temp, MIR_UNIT);
        mir_fn.push_inst_to_block(
            block,
            Instruction::Call {
                destination: call_dest,
                func: info.body_symbol.clone(),
                args,
            },
        );
        let handle = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        mir_fn.push_inst_to_block(
            block,
            Instruction::Assign {
                destination: handle,
                value: MirConstant::Int(0),
            },
        );
        mir_fn.basic_blocks[block].set_terminator(Terminator::Return(Some(handle)));
        return mir_fn;
    }

    let body_result = mir_fn.add_local(LocalKind::Temp, info.ret_type.clone());
    mir_fn.push_inst_to_block(
        block,
        Instruction::Call {
            destination: body_result,
            func: info.body_symbol.clone(),
            args,
        },
    );

    let size_local = mir_fn.add_local(LocalKind::Temp, MIR_I64);
    mir_fn.push_inst_to_block(
        block,
        Instruction::Assign {
            destination: size_local,
            value: MirConstant::Int(mir_type_size_bytes(&info.ret_type) as i64),
        },
    );

    let raw_ptr_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let raw_ptr = mir_fn.add_local(LocalKind::Temp, raw_ptr_ty.clone());
    mir_fn.push_inst_to_block(
        block,
        Instruction::Call {
            destination: raw_ptr,
            func: "malloc".to_string(),
            args: vec![size_local],
        },
    );

    let typed_ptr_ty = MIRType::Ptr(Box::new(info.ret_type.clone()));
    let typed_ptr = mir_fn.add_local(LocalKind::Temp, typed_ptr_ty.clone());
    mir_fn.push_inst_to_block(
        block,
        Instruction::Cast {
            destination: typed_ptr,
            value: raw_ptr,
            to: typed_ptr_ty,
        },
    );
    mir_fn.push_inst_to_block(
        block,
        Instruction::Store {
            destination: typed_ptr,
            value: body_result,
        },
    );

    let handle = mir_fn.add_local(LocalKind::Temp, MIR_I64);
    mir_fn.push_inst_to_block(
        block,
        Instruction::Cast {
            destination: handle,
            value: typed_ptr,
            to: MIR_I64,
        },
    );
    mir_fn.basic_blocks[block].set_terminator(Terminator::Return(Some(handle)));
    mir_fn
}

fn synthesize_async_poll_function(name: &str) -> MirFunction {
    let mut mir_fn = MirFunction::new(async_poll_symbol(name), vec![MIR_I64], MIR_I64);
    let block = mir_fn.start_block;
    let ready = mir_fn.add_local(LocalKind::Temp, MIR_I64);
    mir_fn.push_inst_to_block(
        block,
        Instruction::Assign {
            destination: ready,
            value: MirConstant::Int(1),
        },
    );
    mir_fn.basic_blocks[block].set_terminator(Terminator::Return(Some(ready)));
    mir_fn
}

fn synthesize_async_result_function(name: &str, info: &AsyncFunctionInfo) -> MirFunction {
    let mut mir_fn = MirFunction::new(async_result_symbol(name), vec![MIR_I64], info.ret_type.clone());
    let block = mir_fn.start_block;
    let handle = Local::new(1, LocalKind::Param);

    if matches!(info.ret_type, MIRType::Unit | MIRType::Never) {
        mir_fn.basic_blocks[block].set_terminator(Terminator::Return(None));
        return mir_fn;
    }

    let typed_ptr_ty = MIRType::Ptr(Box::new(info.ret_type.clone()));
    let typed_ptr = mir_fn.add_local(LocalKind::Temp, typed_ptr_ty.clone());
    mir_fn.push_inst_to_block(
        block,
        Instruction::Cast {
            destination: typed_ptr,
            value: handle,
            to: typed_ptr_ty,
        },
    );

    let result = mir_fn.add_local(LocalKind::Temp, info.ret_type.clone());
    mir_fn.push_inst_to_block(
        block,
        Instruction::Load {
            destination: result,
            source: typed_ptr,
        },
    );

    let raw_ptr_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let raw_ptr = mir_fn.add_local(LocalKind::Temp, raw_ptr_ty.clone());
    mir_fn.push_inst_to_block(
        block,
        Instruction::Cast {
            destination: raw_ptr,
            value: typed_ptr,
            to: raw_ptr_ty,
        },
    );

    let free_dest = mir_fn.add_local(LocalKind::Temp, MIR_UNIT);
    mir_fn.push_inst_to_block(
        block,
        Instruction::Call {
            destination: free_dest,
            func: "free".to_string(),
            args: vec![raw_ptr],
        },
    );

    mir_fn.basic_blocks[block].set_terminator(Terminator::Return(Some(result)));
    mir_fn
}

fn synthesize_async_main_wrapper(info: &AsyncFunctionInfo) -> MirFunction {
    let mut mir_fn = MirFunction::new("main".to_string(), vec![], info.ret_type.clone());
    let block = mir_fn.start_block;

    let result = mir_fn.add_local(LocalKind::Temp, MIR_I64);
    mir_fn.push_inst_to_block(
        block,
        Instruction::Call {
            destination: result,
            func: "sengoo_async_run_main_i64".to_string(),
            args: vec![],
        },
    );

    mir_fn.basic_blocks[block].set_terminator(Terminator::Return(Some(result)));
    mir_fn
}

fn lower_function(
    fn_item: &hir::HIRFunction,
    lambda_counter: &mut usize,
    known_functions: &HashSet<String>,
    async_functions: &HashMap<String, AsyncFunctionInfo>,
    options: MirLowerOptions,
) -> Result<(MirFunction, Vec<MirFunction>), String> {
    let params: Vec<MIRType> = fn_item.params.iter().map(|p| p.ty.clone().into()).collect();
    let return_type: MIRType = fn_item.return_type.clone().into();

    let mut mir_fn = MirFunction::new(fn_item.name.clone(), params, return_type);
    let start_block = mir_fn.start_block; // 娣囨繂鐡?start_block
    let mut ctx = LoweringContext::new(&mut mir_fn, lambda_counter, known_functions, async_functions);

    // 閸欏倹鏆熷鑼病鐞氼偅鍧婇崝鐘插煂 locals 娑擃叏绱濋棁鈧憰浣筋唶瑜版洖鐣犳禒顒傛畱閸氬秶袨
    for (i, param) in fn_item.params.iter().enumerate() {
        let local = Local::new(i + 1, LocalKind::Param);
        ctx.local_names.insert(param.name.clone(), local);
        ctx.bind_local_symbol(param.symbol, local);
        ctx.contract_param_bindings
            .push((param.name.clone(), param.symbol, local));
    }

    // 闂勫秳缍嗛崙鑺ユ殶娴ｆ挸鍩屽鍙夋箒閻ㄥ嫬鍙嗛崣锝呮健
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

    // 濡偓閺屻儲妲搁崥锔芥箒闁挎瑨顕ら崣鎴犳晸
    if !ctx.errors.is_empty() {
        return Err(format!(
            "MIR lowering errors in function '{}':\n  {}",
            fn_item.name,
            ctx.errors.join("\n  ")
        ));
    }

    // 閹绘劕褰?lambda_functions閿涘矂鍣撮弨鎯ь嚠 mir_fn 閻ㄥ嫬鈧喓鏁?
    let lambda_functions = ctx.lambda_functions;
    Ok((mir_fn, lambda_functions))
}

/// 瀵邦亞骞嗘稉濠佺瑓閺傚浄绱濋悽銊ょ艾 break/continue
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    /// break 鐠哄疇娴嗛崚鎵畱閻╊喗鐖ｉ敓?
    break_block: usize,
    /// continue 鐠哄疇娴嗛崚鎵畱閻╊喗鐖ｉ敓?
    continue_block: usize,
}

/// 閸戣姤鏆熺粵鎯ф倳娣団剝浼呴敍鍫ｇ箲閸ョ偟琚崹瀣剁礆
#[derive(Clone)]
struct FunctionSig {
    ret_type: MIRType,
    /// 閹规洝骞忛惃鍕殰閻㈠崬褰夐柌蹇ョ礄閸氬秶袨, 缁鐎烽敓?
    #[allow(dead_code)]
    env: Vec<(String, MIRType)>,
}

/// Lambda 閻滎垰顣ㄦ穱鈩冧紖
struct LambdaEnv {
    /// 閻滎垰顣ㄩ崣姗€鍣洪崥宥囆為崪灞筋嚠鎼存梻娈?Local
    vars: Vec<(String, Local)>,
    /// 閻滎垰顣ㄧ紒鎾寸€担鎾惰閸ㄥ绱欓悽銊ょ艾娴狅絿鐖滈悽鐔稿灇閿?
    #[allow(dead_code)]
    env_type: MIRType,
    /// 閻滎垰顣ㄩ幐鍥嫛閿?Local閿涘牆婀拫鍐暏閺冩湹濞囬悽顭掔礆
    env_ptr_local: Option<Local>,
}

/// 鏉烆剚宕叉稉濠佺瑓閿?
struct LoweringContext<'a> {
    mir_fn: &'a mut MirFunction,
    /// 閸氬秶袨閸掓澘鐪柈銊ュ綁闁插繒娈戦弰鐘茬殸
    local_names: HashMap<String, Local>,
    local_symbols: HashMap<SymbolId, Local>,
    contract_param_bindings: Vec<(String, SymbolId, Local)>,
    /// 瑜版挸澧犻崺鐑樻拱閿?
    current_block: Option<usize>,
    /// 閺€鍫曟肠閻ㄥ嫰鏁婄拠顖欎繆閿?
    errors: Vec<String>,
    /// 瀵邦亞骞嗛弽鍫礉閻劋绨径鍕倞 break/continue
    loop_stack: Vec<LoopContext>,
    /// Lambda 鐠佲剝鏆熼崳顭掔礄閻劋绨悽鐔稿灇閸烆垯绔撮崥宥囆為敓?
    lambda_counter: &'a mut usize,
    /// 閻㈢喐鍨氶敓?Lambda 鏉堝懎濮崙鑺ユ殶
    lambda_functions: Vec<MirFunction>,
    /// Local 閿?Lambda 閸戣姤鏆熼崥宥囨畱閺勭姴鐨?
    lambda_names: HashMap<Local, String>,
    /// 閸戣姤鏆熼崥宥呭煂缁涙儳鎮曢惃鍕Ё閿?
    function_sigs: HashMap<String, FunctionSig>,
    /// Lambda 閸戣姤鏆熼崥宥呭煂閻滎垰顣ㄦ穱鈩冧紖閻ㄥ嫭妲ч敓?
    lambda_environments: HashMap<String, LambdaEnv>,
    /// 閺勭姴鐨?Local 閿?閸樼喎顫愮猾璇茬€烽崥宥囆為敍鍫㈡暏娴滃海绮ㄩ弸鍕秼閺傝纭剁拫鍐暏鐟欙絾鐎介敓?
    type_names: HashMap<Local, String>,
    /// 瀹歌尙鐓￠惃鍕毐閺佹澘鎮曢梿鍡楁値閿涘牏鏁ゆ禍搴㈡煙濞夋洝鐨熼悽銊╃崣鐠囦緤绱?
    known_functions: &'a HashSet<String>,
    async_functions: &'a HashMap<String, AsyncFunctionInfo>,
}

impl<'a> LoweringContext<'a> {
    fn new(
        mir_fn: &'a mut MirFunction,
        lambda_counter: &'a mut usize,
        known_functions: &'a HashSet<String>,
        async_functions: &'a HashMap<String, AsyncFunctionInfo>,
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
            function_sigs: HashMap::new(),
            lambda_environments: HashMap::new(),
            type_names: HashMap::new(),
            known_functions,
            async_functions,
        }
    }

    /// 閻㈢喐鍨氶崬顖欑閿?Lambda 閸戣姤鏆熼敓?
    fn lambda_name(&mut self) -> String {
        let name = format!("$__lambda{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }

    /// 鏉╂稑鍙嗗顏嗗箚閿涘苯鐨?break/continue 閻╊喗鐖ｉ幒銊ュ弳閿?
    fn push_loop(&mut self, break_block: usize, continue_block: usize) {
        self.loop_stack.push(LoopContext {
            break_block,
            continue_block,
        });
    }

    /// 閺€鍫曟肠 Lambda body 娑擃厺濞囬悽銊ф畱閼奉亞鏁遍崣姗€鍣洪敍鍫ユ姜閸欏倹鏆熼惃鍕樆闁劌褰夐柌蹇ョ礆
    /// 鏉╂柨娲栭懛顏嗘暠閸欐﹢鍣洪崥宥囆為崚妤勩€冮崪灞筋嚠鎼存梻娈?Local
    fn collect_free_vars(
        &self,
        params: &[String],
        body: &crate::hir::HIRExpr,
    ) -> Vec<(String, Local)> {
        let param_names: std::collections::HashSet<String> = params.iter().cloned().collect();

        let mut free_vars = Vec::new();
        self.collect_vars_from_expr(body, &param_names, &mut free_vars);
        free_vars
    }

    /// 闁帒缍婇弨鍫曟肠鐞涖劏鎻蹇庤厬娴ｈ法鏁ら惃鍕殰閻㈠崬褰夐敓?
    fn collect_vars_from_expr(
        &self,
        expr: &crate::hir::HIRExpr,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        use crate::hir::HIRExpr;

        match expr {
            HIRExpr::Var { name, .. } => {
                // 婵″倹鐏夐弰顖氬綁闁插繋绗栨稉宥嗘Ц閸欏倹鏆熼敍灞藉灟閺勵垵鍤滈悽鍗炲綁閿?
                if !param_names.contains(name) {
                    if let Some(&local) = self.local_names.get(name) {
                        if !free_vars.iter().any(|(n, _)| n == name) {
                            free_vars.push((name.clone(), local));
                        }
                    }
                }
            }
            HIRExpr::Lit(_) => {}
            HIRExpr::Unary(_, operand) => {
                self.collect_vars_from_expr(operand, param_names, free_vars);
            }
            HIRExpr::Await(operand) => {
                self.collect_vars_from_expr(operand, param_names, free_vars);
            }
            HIRExpr::Binary(_op, left, right) => {
                self.collect_vars_from_expr(left, param_names, free_vars);
                self.collect_vars_from_expr(right, param_names, free_vars);
            }
            HIRExpr::Call { func, args } => {
                self.collect_vars_from_expr(func, param_names, free_vars);
                for arg in args {
                    self.collect_vars_from_expr(arg, param_names, free_vars);
                }
            }
            HIRExpr::Lambda {
                params: inner_params,
                body: inner_body,
            } => {
                // 閸愬懘鍎?Lambda 閺堝鍤滃杈╂畱閸欏倹鏆熼梿鍡楁値
                let inner_param_names: std::collections::HashSet<String> =
                    inner_params.iter().cloned().collect();
                self.collect_vars_from_expr(inner_body, &inner_param_names, free_vars);
            }
            HIRExpr::Block(body) => {
                for stmt in &body.stmts {
                    self.collect_vars_from_stmt(stmt, param_names, free_vars);
                }
                if let Some(expr) = &body.expr {
                    self.collect_vars_from_expr(expr, param_names, free_vars);
                }
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_vars_from_expr(cond, param_names, free_vars);
                // then_branch 閿?else_branch 閿?HIRBody閿涘矂娓剁憰浣哄濞堝﹤顦╅敓?
                self.collect_vars_from_body(then_branch, param_names, free_vars);
                if let Some(else_b) = else_branch {
                    self.collect_vars_from_body(else_b, param_names, free_vars);
                }
            }
            HIRExpr::Loop(body) => {
                self.collect_vars_from_body(body, param_names, free_vars);
            }
            HIRExpr::While { cond, body } => {
                self.collect_vars_from_expr(cond, param_names, free_vars);
                self.collect_vars_from_body(body, param_names, free_vars);
            }
            HIRExpr::Break(_) | HIRExpr::Continue => {}
            HIRExpr::Array(elems) => {
                for elem in elems {
                    self.collect_vars_from_expr(elem, param_names, free_vars);
                }
            }
            HIRExpr::Index { base, index } => {
                self.collect_vars_from_expr(base, param_names, free_vars);
                self.collect_vars_from_expr(index, param_names, free_vars);
            }
            HIRExpr::Struct { fields, .. } => {
                for (_, field_val) in fields {
                    self.collect_vars_from_expr(field_val, param_names, free_vars);
                }
            }
            HIRExpr::Field { base, .. } => {
                self.collect_vars_from_expr(base, param_names, free_vars);
            }
            HIRExpr::For {
                var_name,
                iter,
                body,
                ..
            } => {
                self.collect_vars_from_expr(iter, param_names, free_vars);
                // for 閸欐﹢鍣洪崷銊ユ儕閻滎垯缍嬮崘鍛Ц缂佹垵鐣鹃惃鍕剁礉娑撳秶鐣婚懛顏嗘暠閸欐﹢鍣?
                let mut extended_params = param_names.clone();
                extended_params.insert(var_name.clone());
                self.collect_vars_from_body(body, &extended_params, free_vars);
            }
            HIRExpr::Assign { target, value } => {
                self.collect_vars_from_expr(target, param_names, free_vars);
                self.collect_vars_from_expr(value, param_names, free_vars);
            }
            HIRExpr::AssignOp {
                target,
                op: _,
                value,
            } => {
                self.collect_vars_from_expr(target, param_names, free_vars);
                self.collect_vars_from_expr(value, param_names, free_vars);
            }
            HIRExpr::And(left, right) | HIRExpr::Or(left, right) => {
                self.collect_vars_from_expr(left, param_names, free_vars);
                self.collect_vars_from_expr(right, param_names, free_vars);
            }
            HIRExpr::MethodCall { receiver, args, .. } => {
                self.collect_vars_from_expr(receiver, param_names, free_vars);
                for arg in args {
                    self.collect_vars_from_expr(arg, param_names, free_vars);
                }
            }
            _ => {
                // 閸忔湹绮悰銊ㄦ彧瀵繒琚崹瀣畯娑撳秴顦╅敓?
            }
        }
    }

    /// 閿?HIRBody 娑擃厽鏁归梿鍡楀綁閿?
    fn collect_vars_from_body(
        &self,
        body: &crate::hir::HIRBody,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        for stmt in &body.stmts {
            self.collect_vars_from_stmt(stmt, param_names, free_vars);
        }
        if let Some(expr) = &body.expr {
            self.collect_vars_from_expr(expr, param_names, free_vars);
        }
    }

    /// 娴犲氦顕㈤崣銉よ厬閺€鍫曟肠閸欐﹢鍣?
    fn collect_vars_from_stmt(
        &self,
        stmt: &crate::hir::HIRStmt,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        use crate::hir::HIRStmt;

        match stmt {
            HIRStmt::Let { name: _, value, .. } => {
                if let Some(v) = value {
                    self.collect_vars_from_expr(v, param_names, free_vars);
                }
                // let 缂佹垵鐣鹃惃鍕綁闁插繋绗夐弰顖濆殰閻㈠崬褰夐敓?
            }
            HIRStmt::Expr(expr) => {
                self.collect_vars_from_expr(expr, param_names, free_vars);
            }
            HIRStmt::Item => {}
        }
    }

    /// 闁偓閸戝搫鎯婇敓?
    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    /// 閼惧嘲褰囪ぐ鎾冲瀵邦亞骞嗛敓?break 閻╊喗鐖ｉ敓?
    fn get_break_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.break_block)
    }

    /// 閼惧嘲褰囪ぐ鎾冲瀵邦亞骞嗛敓?continue 閻╊喗鐖ｉ敓?
    fn get_continue_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.continue_block)
    }

    /// 濞ｈ濮為弬鎵畱鐏炩偓闁劌褰夐敓?
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

    /// 閼惧嘲褰囩仦鈧柈銊ュ綁闁插繒娈戠猾璇茬€烽敍鍫ｇ箲閸ョ偛绱╅悽顭掔礉闁灝鍘ゆ稉宥呯箑鐟曚胶娈?clone閿?
    fn get_local_type(&self, local: Local) -> &MIRType {
        if let Some((_, ty)) = self.mir_fn.locals.get(local.index()) {
            ty
        } else {
            &MIR_UNIT
        }
    }

    /// 鐟欙絾鐎界仦鈧柈銊ュ綁閿?
    /// 婵″倹鐏夐崣姗€鍣洪張顏勭暰娑斿绱濈拋鏉跨秿闁挎瑨顕ら獮鎯扮箲閸ョ偘绔存稉顏勫窗娴ｅ秶顑?local
    fn resolve_local(&mut self, name: &str, symbol: SymbolId) -> Local {
        if symbol.is_valid() {
            if let Some(&local) = self.local_symbols.get(&symbol) {
                return local;
            }
        }
        match self.local_names.get(name) {
            Some(&local) => local,
            None => {
                // 鐠佹澘缍嶉柨娆掝嚖
                self.errors.push(format!("undefined variable: '{}'", name));
                // 鏉╂柨娲栨稉鈧稉顏勫窗娴ｅ秶顑?local閿涘矁顔€缂傛牞鐦х紒褏鐢?
                self.mir_fn.add_local(LocalKind::Temp, MIR_UNIT)
            }
        }
    }

    /// 閸掓稑缂撻弬鎵畱閸╃儤婀伴敓?
    fn new_block(&mut self) -> usize {
        self.mir_fn.add_block()
    }

    /// 鐠佸墽鐤嗚ぐ鎾冲閸╃儤婀伴敓?
    fn set_current_block(&mut self, block: usize) {
        self.current_block = Some(block);
    }

    /// 閼惧嘲褰囪ぐ鎾冲閸╃儤婀伴敓?
    fn current_block(&self) -> usize {
        self.current_block.expect("no current block set")
    }

    /// Check if two types are compatible for binary operations and, if not,
    /// try to insert Cast instructions to reconcile them.  Returns the
    /// (possibly cast) left and right locals whose types now match, or pushes
    /// an error and returns the originals unchanged.
    fn reconcile_binary_operand_types(&mut self, left: Local, right: Local) -> (Local, Local) {
        let left_ty = self.get_local_type(left).clone();
        let right_ty = self.get_local_type(right).clone();

        // Types already match 閿?nothing to do.
        if left_ty == right_ty {
            return (left, right);
        }

        // Determine if a cast between two types is valid and, if so,
        // which direction to cast (returns the common target type).
        match (&left_ty, &right_ty) {
            // Int widening: smaller int 閿?larger int
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

            // Float widening: smaller float 閿?larger float
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

            // Int 閿?Float promotion (either direction)
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

            // Bool 閿?Int promotion (either direction)
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

            // Incompatible types 閿?report an error and return originals.
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

    /// 濞ｈ濮為幐鍥︽姢閸掓澘缍嬮崜宥呯唨閺堫剙娼?
    fn push_inst(&mut self, inst: Instruction) {
        let block_id = self.current_block();
        self.mir_fn.push_inst_to_block(block_id, inst);
    }

    /// 鐠佸墽鐤嗚ぐ鎾冲閸╃儤婀伴崸妤冩畱缂佸牊顒涢敓?
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
            Self::collect_named_symbols(condition, "result", &mut result_symbols);
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

    fn collect_named_symbols(expr: &HIRExpr, target_name: &str, out: &mut Vec<SymbolId>) {
        match expr {
            HIRExpr::Var { name, symbol } => {
                if name == target_name {
                    out.push(*symbol);
                }
            }
            HIRExpr::Unary(_, operand) | HIRExpr::Await(operand) => {
                Self::collect_named_symbols(operand, target_name, out)
            }
            HIRExpr::Binary(_, left, right)
            | HIRExpr::And(left, right)
            | HIRExpr::Or(left, right) => {
                Self::collect_named_symbols(left, target_name, out);
                Self::collect_named_symbols(right, target_name, out);
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                Self::collect_named_symbols(cond, target_name, out);
                Self::collect_named_symbols_in_body(then_branch, target_name, out);
                if let Some(else_body) = else_branch {
                    Self::collect_named_symbols_in_body(else_body, target_name, out);
                }
            }
            HIRExpr::Match { scrutinee, arms } => {
                Self::collect_named_symbols(scrutinee, target_name, out);
                for arm in arms {
                    Self::collect_named_symbols(&arm.body, target_name, out);
                }
            }
            HIRExpr::Loop(body) | HIRExpr::Block(body) => {
                Self::collect_named_symbols_in_body(body, target_name, out);
            }
            HIRExpr::While { cond, body } => {
                Self::collect_named_symbols(cond, target_name, out);
                Self::collect_named_symbols_in_body(body, target_name, out);
            }
            HIRExpr::For { iter, body, .. } => {
                Self::collect_named_symbols(iter, target_name, out);
                Self::collect_named_symbols_in_body(body, target_name, out);
            }
            HIRExpr::Call { func, args } => {
                Self::collect_named_symbols(func, target_name, out);
                for arg in args {
                    Self::collect_named_symbols(arg, target_name, out);
                }
            }
            HIRExpr::MethodCall { receiver, args, .. } => {
                Self::collect_named_symbols(receiver, target_name, out);
                for arg in args {
                    Self::collect_named_symbols(arg, target_name, out);
                }
            }
            HIRExpr::Struct { fields, .. } => {
                for (_, expr) in fields {
                    Self::collect_named_symbols(expr, target_name, out);
                }
            }
            HIRExpr::Array(items) | HIRExpr::Tuple(items) => {
                for item in items {
                    Self::collect_named_symbols(item, target_name, out);
                }
            }
            HIRExpr::Index { base, index } => {
                Self::collect_named_symbols(base, target_name, out);
                Self::collect_named_symbols(index, target_name, out);
            }
            HIRExpr::Field { base, .. }
            | HIRExpr::Return(Some(base))
            | HIRExpr::Break(Some(base))
            | HIRExpr::Cast(base, _)
            | HIRExpr::Ascribe(base, _)
            | HIRExpr::Ref(_, base)
            | HIRExpr::Deref(base) => Self::collect_named_symbols(base, target_name, out),
            HIRExpr::Assign { target, value } | HIRExpr::AssignOp { target, value, .. } => {
                Self::collect_named_symbols(target, target_name, out);
                Self::collect_named_symbols(value, target_name, out);
            }
            HIRExpr::Range { start, end, .. } => {
                if let Some(start) = start {
                    Self::collect_named_symbols(start, target_name, out);
                }
                if let Some(end) = end {
                    Self::collect_named_symbols(end, target_name, out);
                }
            }
            HIRExpr::Lambda { body, .. } => {
                Self::collect_named_symbols(body, target_name, out);
            }
            HIRExpr::Lit(_) | HIRExpr::Return(None) | HIRExpr::Break(None) | HIRExpr::Continue => {}
        }
    }

    fn collect_named_symbols_in_body(body: &HIRBody, target_name: &str, out: &mut Vec<SymbolId>) {
        for stmt in &body.stmts {
            match stmt {
                HIRStmt::Expr(expr) => {
                    Self::collect_named_symbols(expr, target_name, out);
                }
                HIRStmt::Let { value, .. } => {
                    if let Some(value) = value {
                        Self::collect_named_symbols(value, target_name, out);
                    }
                }
                HIRStmt::Item => {}
            }
        }
        if let Some(expr) = &body.expr {
            Self::collect_named_symbols(expr, target_name, out);
        }
    }

    /// 闂勫秳缍?HIR 閸ф鍩岄幐鍥х暰閿?
    fn lower_body_to_block(&mut self, body: &HIRBody, target_block: usize) {
        self.lower_body_to_block_with_return(body, target_block, true);
    }

    /// 闂勫秳缍?HIR 閸ф鍩岄幐鍥х暰閸ф绱濇稉宥嗗潑閿?return閿涘矁绻戦崶鐐存付缂佸牐銆冩潏鎯х础閿?Local閿涘牆顩ч弸婊勬箒閿?
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

    /// 闂勫秳缍?HIR 閸ф鍩岄幐鍥х暰閸ф绱欓幒褍鍩楅弰顖氭儊濞ｈ濮?return閿?
    fn lower_body_to_block_with_return(
        &mut self,
        body: &HIRBody,
        target_block: usize,
        add_return: bool,
    ) {
        self.set_current_block(target_block);

        // 闂勫秳缍嗛幍鈧張澶庮嚔閿?
        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }

        // 婢跺嫮鎮婇張鈧紒鍫ｃ€冩潏鎯х础
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
                    // 濡偓閺屻儲妲搁崥锔芥Ц main 閸戣姤鏆熸稉鏃囩箲閸ョ偟琚崹瀣Ц閺佸瓨鏆?
                    // 婵″倹鐏夐弰顖欑瑬鐞涖劏鎻蹇曠波閺嬫粍妲?unit 缁鐎烽敍灞藉灟娑撳秷绻戦敓?unit 閸婄》绱濋懓灞炬Ц鏉╂柨娲?None閿涘牅鍞惍浣烘晸閹存劕娅掓导姘崇箲閿?0閿?
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
            // 婵″倹鐏?add_return = false閿涘奔绗夊ǎ璇插 terminator閿涘牏鏁辩悰銊ㄦ彧瀵繗鍤滃杈啎缂冾噯绱濋敓?break閿?
        } else if add_return {
            // 濞屸剝婀佺悰銊ㄦ彧瀵繋绲鹃棁鈧敓?return閿涘本鍧婇崝鐘碘敄 return
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

    /// 闂勫秳缍?HIR 閿?
    fn lower_body(&mut self, body: &HIRBody) -> usize {
        let entry_block = self.new_block();
        self.lower_body_to_block(body, entry_block);
        entry_block
    }

    /// 闂勫秳缍?HIR 鐠囶厼褰?
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
                    // 閸忓牓妾锋担搴ゃ€冩潏鎯х础瀵版鍩岄敓?
                    let value_local = self.lower_expr(value_expr);

                    // 濡偓閺屻儲妲搁崥锔芥Ц Lambda閿涘牆鍘犻梾鍡曚簰闁灝鍘ら崐鐔烘暏閸愯尙鐛婇敓?
                    let lambda_name = self.lambda_names.get(&value_local).cloned();

                    if let Some(ln) = lambda_name {
                        // 鏉╂瑦妲告稉鈧敓?Lambda閿涘矂娓剁憰浣稿灡瀵よ櫣骞嗘晶鍐ㄨ嫙鐎涙ê鍋嶉幑鏇″箯閻ㄥ嫬褰夐敓?
                        let local = self.add_local(Some(name.clone()), kind, mir_ty);
                        self.bind_local_symbol(*symbol, local);

                        // 閿?Lambda 閸氬秶袨閺勭姴鐨犻崚鐗堟煀閿?local閿涘牏鏁ゆ禍搴ょ殶閻劍妞傞弻銉﹀閿?
                        self.lambda_names.insert(local, ln.clone());

                        // 濡偓閿?Lambda 閺勵垰鎯侀張澶屽箚婢у啫褰夐柌蹇涙付鐟曚焦宕熼敓?
                        // 闂団偓鐟曚礁鍘犻敓?vars 娴犮儵浼╅崗宥呪偓鐔烘暏濡偓閺屻儵妫堕敓?
                        let env_vars = self
                            .lambda_environments
                            .get(&ln)
                            .map(|env| env.vars.clone())
                            .unwrap_or_default();

                        if !env_vars.is_empty() {
                            // 閸掓稑缂撻悳顖氼暔缂佹挻鐎敓?
                            // 閻滎垰顣ㄩ弰顖欑娑擃亝鏆熺紒鍕剁礉濮ｅ繋閲滈幑鏇″箯閻ㄥ嫬褰夐柌蹇斿瘻妞ゅ搫绨€涙ê鍋?
                            let env_elem_ty = MIR_I64;
                            let env_ty = MIRType::Array(
                                Box::new(env_elem_ty.clone()),
                                env_vars.len() as u64,
                            );

                            // 閸掑棝鍘ら悳顖氼暔缁屾椽妫?- 娴ｈ法鏁?User 缁鐎锋禒銉ょ┒濮濓絿鈥?alloca
                            let env_local = self.mir_fn.add_local(LocalKind::User, env_ty);

                            // 鐎涙ê鍋嶅В蹇庨嚋閹规洝骞忛惃鍕綁闁插繐鍩岄悳顖氼暔閿?
                            for (i, (var_name, _var_local)) in env_vars.iter().enumerate() {
                                // 娴犲骸缍嬮崜宥勭瑐娑撳鏋冮懢宄板絿閹规洝骞忛崣姗€鍣洪敓?local
                                if let Some(&captured_local) = self.local_names.get(var_name) {
                                    // 閼惧嘲褰囬悳顖氼暔閸欐﹢鍣洪惃鍕勾閸р偓
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

                                    // 閸旂姾娴囬幑鏇″箯閻ㄥ嫬褰夐柌蹇ユ嫹?
                                    let captured_value_local =
                                        self.add_local(None, LocalKind::Temp, env_elem_ty.clone());
                                    self.push_inst(Instruction::Load {
                                        destination: captured_value_local,
                                        source: captured_local,
                                    });

                                    // 鐎涙ê鍋嶉崚鎵箚閿?
                                    self.push_inst(Instruction::Store {
                                        destination: elem_addr_local,
                                        value: captured_value_local,
                                    });
                                }
                            }

                            // 閼惧嘲褰囬悳顖氼暔閻ㄥ嫬婀撮崸鈧敍鍫滅稊娑撶儤瀵氶柦鍫滅炊闁帞绮?Lambda閿?
                            // 閻╁瓨甯存担璺ㄦ暏 mir_fn.add_local 閼板奔绗夐敓?add_local閿涘矂浼╅崗宥呯殺閻滎垰顣ㄩ崣姗€鍣哄ǎ璇插閿?local_names
                            let env_ptr_local = self
                                .mir_fn
                                .add_local(LocalKind::Temp, MIRType::Ptr(Box::new(env_elem_ty)));
                            self.push_inst(Instruction::AddrOf {
                                destination: env_ptr_local,
                                source: env_local,
                            });

                            // 鐏忓棛骞嗘晶鍐╁瘹闁藉牆鐡ㄩ崒銊ュ煂 lambda_environments 娑擃叏绱濇禒銉ょ┒閸︺劏鐨熼悽銊︽娴ｈ法鏁?
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
                        // 閺咁噣鈧艾鈧》绱濋崚娑樼紦 local 楠炶泛鐡ㄩ敓?
                        // 閻楄鐣╂径鍕倞閿涙艾顩ч弸婊冨礁閸婂吋妲搁弫鎵矋缁鐎烽惃鍕暏閹村嘲褰夐柌蹇ョ礉閻╁瓨甯撮柌宥呮嚒閸氬秴鐣犻懓灞肩瑝閺勵垰鍨卞鐑樻煀閸欐﹢鍣?
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
                                return;
                            }
                        };

                        if matches!(value_ty, MIRType::Array(_, _))
                            && value_info.kind == LocalKind::User
                        {
                            // 閸欏啿鈧吋妲搁弫鎵矋缁鐎烽惃鍕暏閹村嘲褰夐柌蹇ョ礉閻╁瓨甯寸亸鍡楀従闁插秴鎳￠崥宥勮礋閻╊喗鐖ｉ崣姗€鍣?
                            // 閿?local_names 娑擃厼鍨归梽銈嗘＋閻ㄥ嫭妲х亸鍕剁礉濞ｈ濮為弬鎵畱閺勭姴鐨?
                            self.local_names.insert(name.clone(), value_local);
                            self.bind_local_symbol(*symbol, value_local);
                            // 娑撳秶鏁撻敓?Store 閹稿洣鎶?
                        } else {
                            // 閺咁噣鈧艾鈧》绱濋崚娑樼紦 local 楠炶泛鐡ㄩ敓?
                            // 娴ｈ法鏁ら崐鑲╂畱鐎圭偤妾猾璇茬€烽敍鍫濐洤閿?HIR 缁鐎锋稉宥咁檮缁墽鈥橀敍灞肩伐婵″倻绮ㄩ弸鍕秼缁鐎烽敓?
                            let actual_ty = if matches!(value_ty, MIRType::Struct { .. }) {
                                value_ty.clone()
                            } else {
                                mir_ty
                            };
                            let local = self.add_local(Some(name.clone()), kind, actual_ty);
                            self.bind_local_symbol(*symbol, local);
                            // 娴肩姵鎸辩猾璇茬€烽崥宥囆為敍姘洤閺嬫粌褰搁崐鍏兼箒缁鐎烽崥宥囆為敍灞界殺閸忔湹绱堕幘顓炲煂閺傛壆娈?local
                            if let Some(type_name) = self.type_names.get(&value_local).cloned() {
                                self.type_names.insert(local, type_name);
                            }
                            self.push_inst(Instruction::Store {
                                destination: local,
                                value: value_local,
                            });
                        }
                    }
                } else {
                    // 濞屸剝婀侀崚婵嗩潗閸婅偐娈?let 缂佹垵鐣?
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

    /// 闂勫秳缍?HIR 鐞涖劏鎻敓?
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

    fn resolve_async_callee_name(&self, func: &HIRExpr) -> Option<String> {
        match func {
            HIRExpr::Var { name, .. } if self.async_functions.contains_key(name) => {
                Some(name.clone())
            }
            _ => None,
        }
    }

    fn lower_async_await(&mut self, expr: &HIRExpr) -> Local {
        let HIRExpr::Call { func, args } = expr else {
            self.errors
                .push("phase-1 await lowering expects an async call expression".to_string());
            return self.lower_expr(expr);
        };

        let Some(async_name) = self.resolve_async_callee_name(func) else {
            self.errors
                .push("phase-1 await lowering requires a known async callee".to_string());
            return self.lower_expr(expr);
        };

        let info = self
            .async_functions
            .get(&async_name)
            .expect("async callee metadata should exist")
            .clone();
        let arg_locals: Vec<Local> = args.iter().map(|arg| self.lower_expr(arg)).collect();

        let handle = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: handle,
            func: async_start_symbol(&async_name),
            args: arg_locals,
        });

        let poll = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: poll,
            func: async_poll_symbol(&async_name),
            args: vec![handle],
        });

        let result_ty = if matches!(info.ret_type, MIRType::Unit | MIRType::Never) {
            MIR_UNIT
        } else {
            info.ret_type.clone()
        };
        let result = self.add_local(None, LocalKind::Temp, result_ty);
        self.push_inst(Instruction::Call {
            destination: result,
            func: async_result_symbol(&async_name),
            args: vec![handle],
        });
        result
    }

    fn lower_expr(&mut self, expr: &HIRExpr) -> Local {
        match expr {
            HIRExpr::Lit(lit) => self.lower_literal(lit),
            HIRExpr::Var { name, symbol } => self.resolve_local(name, *symbol),
            HIRExpr::Await(expr) => self.lower_async_await(expr),
            HIRExpr::Unary(op, operand) => {
                // 閻楄鐣╂径鍕倞瀵洜鏁ら崪宀冃掑鏇犳暏鏉╂劗鐣婚敓?
                match op {
                    hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut => {
                        // &expr - 閼惧嘲褰囩悰銊ㄦ彧瀵繒娈戦崷鏉挎絻
                        let expr_local = self.lower_expr(operand);
                        let expr_ty = self.get_local_type(expr_local).clone();

                        // 閸掓稑缂撻幐鍥嫛缁鐎?
                        let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
                        let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);

                        // 娴ｈ法鏁?AddrOf 閹稿洣鎶ら懢宄板絿閸︽澘娼?
                        self.push_inst(Instruction::AddrOf {
                            destination: ptr_local,
                            source: expr_local,
                        });

                        ptr_local
                    }
                    hir::HIRUnaryOp::Deref => {
                        // *ptr - 鐟欙絽绱╅敓?
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
                        // 閸忔湹绮稉鈧崗鍐箥缁犳顑?
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
                        // For Eq: result != 0 means strings are equal 閿?true
                        // For Ne: result == 0 means strings are not equal 閿?true
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

                // 濮ｆ棁绶濋崪宀勨偓鏄忕帆閹垮秳缍旀潻鏂挎礀 bool閿涘苯鍙炬禒鏍ㄦ惙娴ｆ粏绻戦敓?int(64)
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

                // 闂勫秳缍?then 閸掑棙鏁?
                let then_val = self.lower_body_to_block_val(then_branch, then_block);
                let then_end = self.current_block();
                if let Some(block) = self.mir_fn.block_mut(then_end) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(join_block));
                    }
                }

                // 闂勫秳缍?else 閸掑棙鏁?
                if let Some(e) = else_branch {
                    let else_val = self.lower_body_to_block_val(e, else_block);
                    let else_end = self.current_block();
                    if let Some(block) = self.mir_fn.block_mut(else_end) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(join_block));
                        }
                    }

                    // 閸?join_block 閸氬牆鑻熸稉銈勯嚋閸掑棙鏁紒鎾寸亯閵?
                    // 濞夈劍鍓伴敍姝€LVM 娑撳秴鍘戠拋?`phi void`閿涘苯娲滃?Unit 缁鐎锋稉宥囨晸閹?Phi閵?
                    self.set_current_block(join_block);
                    let then_ty = self.get_local_type(then_val).clone();
                    let is_void_like = match &then_ty {
                        MIRType::Unit | MIRType::Never => true,
                        MIRType::Tuple(fields) if fields.is_empty() => true,
                        _ => false,
                    };
                    if is_void_like {
                        self.add_local(None, LocalKind::Temp, MIR_UNIT)
                    } else {
                        let result = self.add_local(None, LocalKind::Temp, then_ty);
                        self.push_inst(Instruction::Phi {
                            destination: result,
                            incoming: vec![(then_val, then_end), (else_val, else_end)],
                        });
                        result
                    }
                } else {
                    // 濞屸剝婀?else 閸掑棙鏁敍瀹攍se_block 閻╁瓨甯寸捄瀹犳祮閿?join_block
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

                // 鏉╂稑鍙嗗顏嗗箚娑撳﹣绗呴弬鍥风窗break -> exit_block, continue -> loop_block
                self.push_loop(exit_block, loop_block);

                // 闂勫秳缍?body 閿?loop_block閿涘牅绗夊ǎ璇插 return閿?
                self.lower_body_to_block_with_return(body, loop_block, false);

                // 闁偓閸戝搫鎯婇悳顖欑瑐娑撳鏋?
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

                // 闂勫秳缍嗛弶鈥叉鐞涖劏鎻蹇撳煂 cond_block
                self.set_current_block(cond_block);
                let cond_local = self.lower_expr(cond);
                self.set_terminator(Terminator::If {
                    cond: cond_local,
                    then_block: body_block,
                    else_block: exit_block,
                });

                // 鏉╂稑鍙嗗顏嗗箚娑撳﹣绗呴弬鍥风窗break -> exit_block, continue -> cond_block
                self.push_loop(exit_block, cond_block);

                // 闂勫秳缍?body 閿?body_block閿涘牅绗夊ǎ璇插 return閿?
                self.lower_body_to_block_with_return(body, body_block, false);

                // 闁偓閸戝搫鎯婇悳顖欑瑐娑撳鏋?
                self.pop_loop();

                // body 缂佹挻娼崥搴ょ儲鏉烆剙娲?cond_block
                // 濞夈劍鍓伴敍姝渙dy 閸欘垵鍏橀崠鍛儓閹貉冨煑濞翠緤绱欓敓?if/else閿涘绱濈€佃壈鍤?current_block 娑撳秴鍟€閿?body_block
                // 闂団偓鐟曚礁婀?body 閻ㄥ嫭娓堕崥搴濈娑擃亝妞跨捄鍐ㄦ健娑撳﹨顔曢敓?Goto(cond_block)
                let body_end_block = self.current_block();
                if body_end_block != body_block {
                    // body 閸栧懎鎯堥幒褍鍩楀ù渚婄礉閺堚偓閸氬簼绔存稉顏勬健娑撳秵妲?body_block
                    if let Some(block) = self.mir_fn.block_mut(body_end_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(cond_block));
                        }
                    }
                }
                // 娑旂喐顥呴敓?body_block 閺堫剝闊╅敍鍫㈢暆閿?body 閻ㄥ嫭鍎忛崘纰夌礆
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
                // 濡偓閺屻儲妲搁崥锔胯礋閼煎啫娲挎潻顓濆敩
                match iter.as_ref() {
                    HIRExpr::Range {
                        start,
                        end,
                        inclusive,
                    } => {
                        // for x in start..end { body }  闂勫秳缍嗛敓?while 瀵邦亞骞?
                        let cond_block = self.new_block();
                        let body_block = self.new_block();
                        let inc_block = self.new_block(); // 婢х偛濮炲顏嗗箚閸欐﹢鍣洪惃鍕健
                        let exit_block = self.new_block();

                        // 闂勫秳缍?start 閿?end
                        let start_local = if let Some(s) = start {
                            self.lower_expr(s)
                        } else {
                            // 姒涙顓婚敓?0 瀵偓閿?
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
                            // 濞屸剝婀佺紒鎾存将閸婄》绱濋崚娑樼紦娑撯偓娑擃亜宕版担宥囶儊閿涘牊妫ら梽鎰儕閻滎垽绱?
                            let max = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Assign {
                                destination: max,
                                value: MirConstant::Int(i64::MAX),
                            });
                            max
                        };

                        // 閸掓稑缂撳顏嗗箚閸欐﹢鍣洪獮璺哄灥婵瀵查敓?start
                        let loop_var =
                            self.add_local(Some(var_name.clone()), LocalKind::User, MIR_I64);
                        self.push_inst(Instruction::Store {
                            destination: loop_var,
                            value: start_local,
                        });

                        // 鐠哄疇娴嗛崚鐗堟蒋娴犺泛娼?
                        self.set_terminator(Terminator::Goto(cond_block));

                        // 閺夆€叉閸ф绱板Λ鈧弻銉ユ儕閻滎垰褰夐敓?< end
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

                        // 濮ｆ棁绶濋幙宥勭稊
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

                        // 鏉╂稑鍙嗗顏嗗箚娑撳﹣绗呴弬鍥风窗break -> exit_block, continue -> inc_block
                        self.push_loop(exit_block, inc_block);

                        // 瀵邦亞骞嗘担鎿勭礄娑撳秵鍧婇敓?return閿?
                        self.lower_body_to_block_with_return(body, body_block, false);

                        // 闁偓閸戝搫鎯婇悳顖欑瑐娑撳鏋?
                        self.pop_loop();

                        // body_block 缂佹挻娼崥搴ょ儲鏉烆剙鍩?inc_block
                        if let Some(block) = self.mir_fn.block_mut(body_block) {
                            if block.terminator.is_none() {
                                block.set_terminator(Terminator::Goto(inc_block));
                            }
                        }

                        // 婢х偛濮為崸妤嬬窗婢х偛濮炲顏嗗箚閸欐﹢鍣?
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

                        // 鐠哄疇娴嗛崶鐐存蒋娴犺泛娼?
                        self.set_terminator(Terminator::Goto(cond_block));

                        self.set_current_block(exit_block);
                        self.add_local(None, LocalKind::Temp, MIR_UNIT)
                    }
                    _ => {
                        // 鐏忔繆鐦弫鎵矋鏉╊厺鍞? for x in [1, 2, 3] 閿?for x in arr
                        let iter_local = self.lower_expr(iter);
                        let iter_ty = self.get_local_type(iter_local).clone();

                        match iter_ty {
                            MIRType::Array(elem_ty, len) => {
                                // 閺佹壆绮嶆潻顓濆敩: for x in arr { body }
                                let cond_block = self.new_block();
                                let body_block = self.new_block();
                                let inc_block = self.new_block();
                                let exit_block = self.new_block();

                                // 閸掓稑缂撶槐銏犵穿閸欐﹢鍣洪獮璺哄灥婵瀵查敓?0
                                // 缁便垹绱╅崣姗€鍣洪棁鈧憰浣告躬瀵邦亞骞嗘稉顓熸纯閺傚府绱濇担璺ㄦ暏 User 缁鐎?
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

                                // 閸掓稑缂撳顏嗗箚閸欐﹢鍣洪敍鍫滅瑢閺佹壆绮嶉崗鍐缁鐎烽惄绋挎倱閿?
                                let loop_var = self.add_local(
                                    Some(var_name.clone()),
                                    LocalKind::User,
                                    (*elem_ty).clone(),
                                );

                                // 閸掓稑缂撻弫鎵矋闂€鍨鐢悂鍣?
                                let len_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: len_local,
                                    value: MirConstant::Int(len as i64),
                                });

                                // 鐠哄疇娴嗛崚鐗堟蒋娴犺泛娼?
                                self.set_terminator(Terminator::Goto(cond_block));

                                // 閺夆€叉閸ф绱板Λ鈧敓?index < len
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

                                // 濮ｆ棁绶?index < len
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

                                // 鏉╂稑鍙嗗顏嗗箚娑撳﹣绗呴敓?
                                self.push_loop(exit_block, inc_block);

                                // 瀵邦亞骞嗘担鎿勭窗妫ｆ牕鍘涢崝鐘烘祰 arr[index] 閸掓澘鎯婇悳顖氬綁閿?
                                self.set_current_block(body_block);

                                // 鐠侊紕鐣婚崗鍐閸︽澘娼? &arr[index]
                                // 閿?load index_var閿涘湶ser local閿涘鍩?Temp閿涘苯鍟€娴肩姷绮?IndexAddr
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

                                // 閸旂姾娴囬崗鍐閸婄厧鍩屽顏嗗箚閸欐﹢鍣?
                                let elem_loaded =
                                    self.add_local(None, LocalKind::Temp, (*elem_ty).clone());
                                self.push_inst(Instruction::Load {
                                    destination: elem_loaded,
                                    source: elem_addr_local,
                                });

                                // 鐎涙ê鍋嶉崚鏉挎儕閻滎垰褰夐敓?
                                self.push_inst(Instruction::Store {
                                    destination: loop_var,
                                    value: elem_loaded,
                                });

                                // 闂勫秳缍嗗顏嗗箚閿?
                                self.lower_body_to_block_with_return(body, body_block, false);

                                // 闁偓閸戝搫鎯婇悳顖欑瑐娑撳鏋?
                                self.pop_loop();

                                // body_block 缂佹挻娼崥搴ょ儲鏉烆剙鍩?inc_block
                                if let Some(block) = self.mir_fn.block_mut(body_block) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(inc_block));
                                    }
                                }

                                // 婢х偛濮為崸妤嬬窗index++
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

                                // 鐠哄疇娴嗛崶鐐存蒋娴犺泛娼?
                                self.set_terminator(Terminator::Goto(cond_block));

                                self.set_current_block(exit_block);
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            }
                            _ => {
                                // 娑撳秵鏁幐浣烘畱鏉╊厺鍞崳銊ц閿?
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

                // 閼惧嘲褰囬崙鑺ユ殶閸氬秴鎷版潻鏂挎礀缁鐎烽敍灞炬暜閿?Lambda 鐠嬪啰鏁?
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
                                (name.clone(), MIR_I64, None)
                            }
                        } else if name == "print" {
                            return self.lower_builtin_print(&arg_locals);
                        } else {
                            (name.clone(), MIR_I64, None)
                        }
                    }
                    _ => (String::new(), MIR_UNIT, None),
                };

                let local = self.add_local(None, LocalKind::Temp, ret_type);

                // 婵″倹鐏夐張澶屽箚婢у啯瀵氶柦鍫礉鐏忓棗鍙炬担婊€璐熺粭顑跨娑擃亜寮弫棰佺炊閿?
                let mut final_args = Vec::new();
                if let Some(env_ptr) = env_ptr_local {
                    final_args.push(env_ptr);
                }
                final_args.extend(arg_locals);

                self.push_inst(Instruction::Call {
                    destination: local,
                    func: func_name,
                    args: final_args,
                });
                local
            }
            HIRExpr::And(left, right) => {
                // 閻叀鐭鹃柅鏄忕帆閿?- 缁犫偓閸栨牔璐熸禍灞藉帗鏉╂劗鐣?
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
                // 閻叀鐭鹃柅鏄忕帆閿?- 缁犫偓閸栨牔璐熸禍灞藉帗鏉╂劗鐣?
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
                // 婢跺嫮鎮?break
                if let Some(target) = self.get_break_target() {
                    // 闂勫秳缍嗛崣顖炩偓澶屾畱鏉╂柨娲栭敓?
                    if let Some(v) = value {
                        self.lower_expr(v);
                    }
                    self.set_terminator(Terminator::Break { target });
                    // break 閸氬簼绗夐崣顖濇彧閿涘矁绻戦崶鐐扮娑擃亜宕版担宥囶儊 Local
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                } else {
                    self.errors.push("break outside of loop".to_string());
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Continue => {
                // 婢跺嫮鎮?continue
                if let Some(target) = self.get_continue_target() {
                    self.set_terminator(Terminator::Continue { target });
                    // continue 閸氬簼绗夐崣顖濇彧閿涘矁绻戦崶鐐扮娑擃亜宕版担宥囶儊 Local
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                } else {
                    self.errors.push("continue outside of loop".to_string());
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Assign { target, value } => {
                // 鐠у鈧壈銆冩潏鎯х础: target = value
                // 闂勫秳缍嗛崣绛规嫹?
                let value_local = self.lower_expr(value);

                // 闂勫秳缍嗗锔兼嫹?閿?閼惧嘲褰囬惄顔界垼閸欐﹢鍣?
                match target.as_ref() {
                    HIRExpr::Var { name, symbol } => {
                        let target_local = self.resolve_local(name, *symbol);
                        if value_local == target_local {
                            // Skip no-op self-assignment (`x = x`) to reduce temp churn.
                            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                        }
                        // 娴肩姵鎸辩猾璇茬€烽崥宥囆為敍姘洤閺嬫粌褰搁崐鍏兼箒缁鐎烽崥宥囆為敍灞界殺閸忔湹绱堕幘顓炲煂閻╊喗鐖?local
                        if let Some(type_name) = self.type_names.get(&value_local).cloned() {
                            self.type_names.insert(target_local, type_name);
                        }
                        self.push_inst(Instruction::Store {
                            destination: target_local,
                            value: value_local,
                        });
                    }
                    HIRExpr::Index { base, index } => {
                        // 閺佹壆绮嶉崗鍐鐠у鎷? arr[i] = value
                        let base_local = self.lower_expr(base);
                        let index_local = self.lower_expr(index);

                        // 鐠侊紕鐣婚崗鍐閸︽澘娼?
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

                        // 鐎涙ê鍋嶉崐鐓庡煂鐠侊紕鐣婚崙铏规畱閸︽澘娼?
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
                // 婢跺秴鎮庣挧瀣偓鑹般€冩潏鎯х础: target op= value (e.g., x += 1)
                // 闂勫秳缍嗛崣绛规嫹?
                let value_local = self.lower_expr(value);

                match target.as_ref() {
                    HIRExpr::Var { name, symbol } => {
                        let target_local = self.resolve_local(name, *symbol);
                        // 閸旂姾娴囪ぐ鎾冲閿?
                        let target_ty = self.get_local_type(target_local).clone();
                        let current_val = self.add_local(None, LocalKind::Temp, target_ty.clone());
                        self.push_inst(Instruction::Load {
                            destination: current_val,
                            source: target_local,
                        });
                        // 閹笛嗩攽鏉╂劗鐣?
                        let mir_op = self.lower_bin_op(op);
                        let result = self.add_local(None, LocalKind::Temp, target_ty);
                        self.push_inst(Instruction::Binary {
                            destination: result,
                            op: mir_op,
                            left: current_val,
                            right: value_local,
                        });
                        // 鐎涙ê鍋嶇紒鎾寸亯
                        self.push_inst(Instruction::Store {
                            destination: target_local,
                            value: result,
                        });
                    }
                    HIRExpr::Index { base, index } => {
                        // 閺佹壆绮嶉崗鍐婢跺秴鎮庣挧瀣舵嫹? arr[i] += value
                        let base_local = self.lower_expr(base);
                        let index_local = self.lower_expr(index);

                        // 鐠侊紕鐣婚崗鍐閸︽澘娼?
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

                        // 閸旂姾娴囪ぐ鎾冲閸忓啰绀岄敓?
                        let current_val = self.add_local(None, LocalKind::Temp, elem_ty.clone());
                        self.push_inst(Instruction::Load {
                            destination: current_val,
                            source: addr_local,
                        });

                        // 閹笛嗩攽鏉╂劗鐣?
                        let mir_op = self.lower_bin_op(op);
                        let result = self.add_local(None, LocalKind::Temp, elem_ty);
                        self.push_inst(Instruction::Binary {
                            destination: result,
                            op: mir_op,
                            left: current_val,
                            right: value_local,
                        });

                        // 鐎涙ê鍋嶇紒鎾寸亯閸ョ偛鍘撶槐鐘叉勾閸р偓
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
                // 閺佹壆绮嶇€涙娼伴敓?[a, b, c]
                // 闂勫秳缍嗗В蹇庨嚋閸忓啰绀岄獮鑸垫暪闂嗗棗鐣犳禒顒傛畱 locals
                let elem_locals: Vec<Local> = elems.iter().map(|e| self.lower_expr(e)).collect();

                // 绾喖鐣鹃崗鍐缁鐎烽崪灞炬殶缂佸嫮琚敓?
                let elem_ty = if let Some(first_local) = elem_locals.first() {
                    self.get_local_type(*first_local).clone()
                } else {
                    MIR_UNIT
                };
                let array_ty = MIRType::Array(Box::new(elem_ty), elems.len() as u64);

                // 閺佹壆绮嶉棁鈧憰浣告躬閸愬懎鐡ㄦ稉顓炲瀻闁板秶鈹栭梻杈剧礉娴ｈ法鏁?User 缁鐎?
                let array_local = self.add_local(None, LocalKind::User, array_ty.clone());
                self.push_inst(Instruction::Aggregate {
                    destination: array_local,
                    fields: elem_locals,
                    ty: array_ty,
                });

                array_local
            }
            HIRExpr::Index { base, index } => {
                // 閺佹壆绮嶇槐銏犵穿 arr[i]
                let base_local = self.lower_expr(base);
                let index_local = self.lower_expr(index);

                // 閼惧嘲褰囬弫鎵矋缁鐎锋禒銉р€樼€规艾鍘撶槐鐘佃閿?
                let base_ty = self.get_local_type(base_local).clone();
                let elem_ty = match base_ty {
                    MIRType::Array(elem, _) => *elem,
                    _ => MIR_UNIT,
                };

                // 閸掓稑缂?IndexAddr 閹稿洣鎶ら弶銉吀缁犳鍘撶槐鐘叉勾閸р偓
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

                // 娴犲骸婀撮崸鈧崝鐘烘祰閿?
                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Load {
                    destination: result_local,
                    source: addr_local,
                });

                result_local
            }
            HIRExpr::Struct { name, fields } => {
                // 缂佹挻鐎担鎾崇杽娓氬瀵?Struct { field1: val1, field2: val2 }
                let field_locals: Vec<Local> = fields
                    .iter()
                    .map(|(_, expr)| self.lower_expr(expr))
                    .collect();

                // 娴ｈ法鏁?MIRType::Struct 鐞涖劎銇氱紒鎾寸€担鎿勭礉閸栧懎鎯堢€涙顔岄崥宥呮嫲缁鐎?
                let struct_fields: Vec<(String, MIRType)> = fields
                    .iter()
                    .zip(field_locals.iter())
                    .map(|((field_name, _), &local)| {
                        (field_name.clone(), self.get_local_type(local).clone())
                    })
                    .collect();
                let struct_ty = MIRType::Struct {
                    name: name.clone(),
                    fields: struct_fields,
                };

                let struct_local = self.add_local(None, LocalKind::Temp, struct_ty.clone());
                self.push_inst(Instruction::Aggregate {
                    destination: struct_local,
                    fields: field_locals,
                    ty: struct_ty,
                });

                // 鐠佹澘缍嶇紒鎾寸€担鎾惰閸ㄥ鎮曠粔甯礉閻劋绨崥搴ｇ敾閺傝纭剁拫鍐暏鐟欙絾鐎?
                if !name.is_empty() {
                    self.type_names.insert(struct_local, name.clone());
                }

                struct_local
            }
            HIRExpr::Field { base, field } => {
                // 鐎涙顔岀拋鍧楁６ obj.field
                let base_local = self.lower_expr(base);

                // 鐎甸€涚艾娴ｈ法鏁?Tuple 鐞涖劎銇氶惃鍕波閺嬪嫪缍嬮敍灞煎▏閻劎鍌ㄥ鏇☆問閿?
                // 娑撳瓨妞傞弬瑙勵攳閿涙氨鈥栫紓鏍垳鐢瓕顫嗙€涙顔岄崥宥呭煂缁便垹绱╅惃鍕Ё閿?
                let field_index = match field.as_str() {
                    "x" | "left" | "r" => 0,
                    "y" | "right" | "g" => 1,
                    "z" | "b" => 2,
                    "w" | "a" => 3,
                    _ => 0,
                };

                let base_ty = self.get_local_type(base_local).clone();
                let elem_ty = match &base_ty {
                    MIRType::Tuple(ref tys) if field_index < tys.len() => tys[field_index].clone(),
                    MIRType::Struct { fields, .. } if field_index < fields.len() => {
                        fields[field_index].1.clone()
                    }
                    _ => MIR_I64,
                };

                // 缂佹挻鐎敓?閸忓啰绮嶉弰顖氣偓鑲╄閸ㄥ绱濇担璺ㄦ暏 Extract (extractvalue) 閼板矂娼?FieldAddr+Load
                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Extract {
                    destination: result_local,
                    value: base_local,
                    index: field_index as u32,
                });

                result_local
            }
            HIRExpr::Ref(_is_mut, expr) => {
                // 瀵洜鏁?&expr - 閺嗗倹妞傛潻鏂挎礀鐞涖劏鎻蹇曟畱閸︽澘娼?
                let expr_local = self.lower_expr(expr);
                let expr_ty = self.get_local_type(expr_local).clone();

                // 閸掓稑缂撻幐鍥嫛缁鐎?
                let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
                let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);

                // 鐎甸€涚艾鐏炩偓闁劌褰夐柌蹇ョ礉閼惧嘲褰囬崗璺烘勾閸р偓閿涘牅濞囬敓?IndexAddr with index 0閿?
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
                // 鐟欙絽绱╅敓?*ptr
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
                // Lambda 闂傤厼瀵?|args| body
                // 閸掓稑缂撴稉鈧稉顏囩窡閸斺晛鍤遍弫鏉胯嫙鏉╂柨娲栭崙鑺ユ殶瀵洜鏁?

                // 閻㈢喐鍨氶崬顖欑閿?Lambda 閸戣姤鏆熼敓?
                let lambda_name = self.lambda_name();

                // 閺€鍫曟肠閼奉亞鏁遍崣姗€鍣洪敍鍫㈠箚婢у啯宕熼懢鍑ょ礆
                let free_vars = self.collect_free_vars(params, body);

                // Lambda 缁鐎烽敍姘剁帛鐠併倕寮弫鏉挎嫲鏉╂柨娲栫猾璇茬€烽柈鑺ユЦ i64
                let mut param_types: Vec<MIRType> = (0..params.len()).map(|_| MIR_I64).collect();
                let ret_type = MIR_I64;

                // 婵″倹鐏夐張澶庡殰閻㈠崬褰夐柌蹇ョ礉濞ｈ濮為悳顖氼暔閸欏倹鏆熸担婊€璐熺粭顑跨娑擃亜寮敓?
                let env_param_offset = if free_vars.is_empty() {
                    0
                } else {
                    // 閻滎垰顣ㄩ崣鍌涙殶閿涙矮濞囬悽銊х波閺嬪嫪缍嬬猾璇茬€风悰銊с仛閹规洝骞忛惃鍕箚閿?
                    // 缁犫偓閸栨牭绱版担璺ㄦ暏 i64* 閹稿洭鎷￠幐鍥ф倻閻滎垰顣?
                    param_types.insert(0, MIRType::Ptr(Box::new(MIR_I64)));
                    1
                };

                // 閸掓稑缂?Lambda 鏉堝懎濮崙鑺ユ殶
                let mut lambda_fn =
                    MirFunction::new(lambda_name.clone(), param_types.clone(), ret_type.clone());
                let lambda_start = lambda_fn.start_block;
                let mut lambda_ctx =
                    LoweringContext::new(&mut lambda_fn, self.lambda_counter, self.known_functions, self.async_functions);
                // Set current block for Lambda function entry
                lambda_ctx.current_block = Some(lambda_start);

                // 缂佹垵鐣鹃悳顖氼暔閸欏倹鏆熼敓?Lambda 閸欏倹鏆熼敓?Lambda 閸戣姤鏆?
                if !free_vars.is_empty() {
                    // 缁楊兛绔存稉顏勫棘閺佺増妲搁悳顖氼暔閿涘牊瀵氶柦鍫礆
                    let env_local = Local::new(1, LocalKind::Param);
                    let env_ptr_name = "__env".to_string();
                    lambda_ctx
                        .local_names
                        .insert(env_ptr_name.clone(), env_local);

                    // 娴犲海骞嗘晶鍐ㄥ鏉炶姤宕熼懢椋庢畱閸欐﹢鍣?
                    // 閻滎垰顣ㄩ弰顖欑娑擃亞绮ㄩ弸鍕秼閿涘本鐦℃稉顏呭礋閼鹃娈戦崣姗€鍣洪幐澶愩€庢惔蹇撶摠閿?
                    for (i, (var_name, _)) in free_vars.iter().enumerate() {
                        // 娑撶儤宕熼懢椋庢畱閸欐﹢鍣洪崚娑樼紦娑撯偓閿?local
                        let captured_local =
                            lambda_ctx.add_local(Some(var_name.clone()), LocalKind::Temp, MIR_I64);

                        // 娴犲海骞嗘晶鍐╁瘹闁藉牆濮炴潪钘夊綁閿?
                        // 娴ｈ法鏁?getelementptr 閿?load
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

                        // 閸旂姾娴囬敓?
                        lambda_ctx.push_inst(Instruction::Load {
                            destination: captured_local,
                            source: ptr_local,
                        });

                        // 鐏忓棙宕熼懢椋庢畱閸欐﹢鍣虹紒鎴濈暰閸掓澘鎮曠粔甯礄鏉╂瑦鐗?body 娑擃厼姘ㄩ崣顖欎簰閻╁瓨甯存担璺ㄦ暏娴滃棴绱?
                        lambda_ctx
                            .local_names
                            .insert(var_name.clone(), captured_local);
                    }

                    // 缂佹垵鐣?Lambda 閸欏倹鏆熼敍鍫濅焊閿?1閿涘苯娲滄稉铏瑰箚婢у啫寮弫鏉垮窗閻劋绨℃担宥囩枂 1閿?
                    for (i, param_name) in params.iter().enumerate() {
                        let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
                        lambda_ctx.local_names.insert(param_name.clone(), local);
                    }
                } else {
                    // 濞屸剝婀侀悳顖氼暔閿涘本顒滅敮鍝ョ拨鐎规艾寮敓?
                    for (i, param_name) in params.iter().enumerate() {
                        let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
                        lambda_ctx.local_names.insert(param_name.clone(), local);
                    }
                }

                // 闂勫秳缍?body 閿?Lambda 閸戣姤鏆?
                // Lambda body 閿?HIRExpr閿涘矂娓剁憰浣稿瘶鐟佸懏鍨?HIRBody
                use crate::hir::HIRBody;
                let lambda_body = HIRBody {
                    stmts: vec![],
                    expr: Some(body.clone()),
                };
                lambda_ctx.lower_body_to_block(&lambda_body, lambda_start);

                // 鐏忓棛鏁撻幋鎰畱 Lambda 閸戣姤鏆熷ǎ璇插閸掓澘鍨悰銊よ厬
                self.lambda_functions.push(lambda_fn);

                // 鐠佹澘缍嶉悳顖氼暔娣団剝浼?
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
                            env_ptr_local: None, // 缁嬪秴鎮楅敓?Let lowering 娑擃叀顔曢敓?
                        },
                    );

                    // 鐠佹澘缍嶉崙鑺ユ殶缁涙儳鎮曢敍鍫濆瘶閸氼偆骞嗘晶鍐т繆閹垽绱?
                    self.function_sigs.insert(
                        lambda_name.clone(),
                        FunctionSig {
                            ret_type: ret_type.clone(),
                            env: env_var_types,
                        },
                    );
                } else {
                    // 鐠佹澘缍嶉崙鑺ユ殶缁涙儳鎮曢敍鍫熸￥閻滎垰顣ㄩ敓?
                    self.function_sigs.insert(
                        lambda_name.clone(),
                        FunctionSig {
                            ret_type: ret_type.clone(),
                            env: vec![],
                        },
                    );
                }

                // 閸掓稑缂撴稉鈧稉顏冨閿?local 閺夈儱鐡ㄩ敓?Lambda 閸戣姤鏆熼敓?
                // 娴ｈ法鏁ら弫瀛樻殶缁鐎锋担婊€璐?Lambda 閻ㄥ嫯銆冪粈鐚寸礄閸戣姤鏆熼幐鍥嫛閿?
                let lambda_local = self.add_local(None, LocalKind::Temp, MIR_I64);

                // 鐎涙ê鍋?Local -> Lambda 閸戣姤鏆熼崥宥囨畱閺勭姴鐨?
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

                        let mut targets = Vec::new();
                        let mut otherwise_block = join_block;
                        for (i, arm) in arms.iter().enumerate() {
                            if let Some(value) = self.extract_discriminant_from_pattern(&arm.pat) {
                                targets.push((value, arm_blocks[i]));
                            } else {
                                otherwise_block = arm_blocks[i];
                            }
                        }

                        self.set_terminator(Terminator::Switch {
                            discr: discr_local,
                            targets,
                            otherwise: otherwise_block,
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
                            let is_void_like = match &result_ty {
                                MIRType::Unit | MIRType::Never => true,
                                MIRType::Tuple(fields) if fields.is_empty() => true,
                                _ => false,
                            };
                            if is_void_like {
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            } else {
                                let result = self.add_local(None, LocalKind::Temp, result_ty);
                                self.push_inst(Instruction::Phi {
                                    destination: result,
                                    incoming: incoming_values,
                                });
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
                            let is_void_like = match &result_ty {
                                MIRType::Unit | MIRType::Never => true,
                                MIRType::Tuple(fields) if fields.is_empty() => true,
                                _ => false,
                            };
                            if is_void_like {
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            } else {
                                let result = self.add_local(None, LocalKind::Temp, result_ty);
                                self.push_inst(Instruction::Phi {
                                    destination: result,
                                    incoming: incoming_values,
                                });
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
                // 閺傝纭剁拫鍐暏 receiver.method(args)
                // 闂勫秳缍嗘稉鐑樻珮闁艾鍤遍弫鎷岀殶閿? TypeName_method(receiver, args)

                // 闂勫秳缍嗛幒銉︽暪閿?
                let receiver_local = self.lower_expr(receiver);
                let receiver_ty = self.get_local_type(receiver_local).clone();

                // 闂勫秳缍嗛崣鍌涙殶
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

                // 閻㈢喐鍨氶弬瑙勭《閸戣姤鏆熼崥宥忕窗TypeName_method
                // 闁潧鎯?Sengoo 閸涜棄鎮曠痪锕€鐣?
                // 妫ｆ牕鍘涘Λ鈧敓?type_names 娴犮儴骞忛崣鏍х杽闂勫懐娈戠紒鎾寸€担鎾惰閸ㄥ鎮曢敓?
                let method_func_name = if let Some(type_name) = self.type_names.get(&receiver_local)
                {
                    format!("{}_{}", type_name, method)
                } else {
                    // 閸ョ偤鈧偓閸掓澘鐔€閿?MIRType 閻ㄥ嫯袙閺嬫劧绱欐径鍕倞閸愬懐鐤嗙猾璇茬€烽敓?
                    match &receiver_ty {
                        MIRType::Int(bits) => {
                            format!("i{}_{}", bits, method)
                        }
                        MIRType::Float(bits) => {
                            format!("f{}_{}", bits, method)
                        }
                        MIRType::Bool => {
                            format!("bool_{}", method)
                        }
                        MIRType::Array(_, _) => {
                            format!("array_{}", method)
                        }
                        MIRType::Tuple(_) => {
                            format!("tuple_{}", method)
                        }
                        MIRType::Ptr(inner) | MIRType::Ref(inner) => {
                            // 閹稿洭鎷?瀵洜鏁ょ猾璇茬€烽惃鍕煙閿?
                            match inner.as_ref() {
                                MIRType::Int(bits) => format!("i{}_ptr_{}", bits, method),
                                MIRType::Float(bits) => format!("f{}_ptr_{}", bits, method),
                                MIRType::Bool => format!("bool_ptr_{}", method),
                                _ => format!("ptr_{}", method),
                            }
                        }
                        _ => {
                            // 鐎甸€涚艾閺堫亞鐓＄猾璇茬€烽敍宀勭帛鐠併倓濞囬敓?i64 閺傝纭堕敓?
                            format!("i64_{}", method)
                        }
                    }
                };

                // Determine the type name for the error message
                let type_display = if let Some(type_name) = self.type_names.get(&receiver_local) {
                    type_name.clone()
                } else {
                    match &receiver_ty {
                        MIRType::Int(bits) => format!("i{}", bits),
                        MIRType::Float(bits) => format!("f{}", bits),
                        MIRType::Bool => "bool".to_string(),
                        MIRType::Array(_, _) => "array".to_string(),
                        MIRType::Tuple(_) => "tuple".to_string(),
                        MIRType::Ptr(_) | MIRType::Ref(_) => "ptr".to_string(),
                        _ => format!("{:?}", receiver_ty),
                    }
                };

                // Check if the method exists in any known function.
                // First try the two-part inherent impl name (e.g. "i64_show").
                // If not found, search for a three-part trait impl name matching
                // "{type_prefix}_{TraitName}_{method}" in known_functions.
                let resolved_func_name = if self.known_functions.contains(&method_func_name) {
                    method_func_name.clone()
                } else {
                    // Build the type prefix used for matching three-part names
                    let type_prefix = if let Some(type_name) = self.type_names.get(&receiver_local)
                    {
                        type_name.clone()
                    } else {
                        match &receiver_ty {
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
                            _ => "i64".to_string(),
                        }
                    };

                    // Search for three-part mangled names: {type_prefix}_{TraitName}_{method}
                    let suffix = format!("_{}", method);
                    let prefix = format!("{}_", type_prefix);
                    let found = self.known_functions.iter().find(|name| {
                        name.starts_with(&prefix)
                            && name.ends_with(&suffix)
                            && *name != &method_func_name
                            && {
                                // Ensure there is a middle part (trait name) between
                                // the type prefix and the method name suffix.
                                let middle = &name[prefix.len()..name.len() - suffix.len()];
                                !middle.is_empty()
                            }
                    });

                    match found {
                        Some(trait_func_name) => trait_func_name.clone(),
                        None => {
                            self.errors.push(format!(
                                "method '{}' not found for type '{}'",
                                method, type_display
                            ));
                            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                        }
                    }
                };

                // 绾喖鐣炬潻鏂挎礀缁鐎烽敍鍫ョ帛閿?i64閿?
                let ret_type = MIR_I64;
                let result_local = self.add_local(None, LocalKind::Temp, ret_type);

                // 閺嬪嫬缂撻崣鍌涙殶閸掓銆冮敍姝砮ceiver + args
                let mut call_args = vec![receiver_local];
                call_args.extend(arg_locals);

                // 閻㈢喐鍨?Call 閹稿洣鎶?
                self.push_inst(Instruction::Call {
                    destination: result_local,
                    func: resolved_func_name,
                    args: call_args,
                });

                result_local
            }
            // 閸忔湹绮張顏勭杽閻滄壆娈?HIR 鐞涖劏鎻蹇曡閸ㄥ绱濇潻鏂挎礀閸楃姳缍呴敓?
            _ => self.add_local(None, LocalKind::Temp, MIR_UNIT),
        }
    }

    /// 娴犲孩膩瀵繋鑵戦幓鎰絿閸掋倕鍩嗛敓?
    /// 鐎甸€涚艾鐎涙娼伴柌蹇斈佸蹇氱箲閿?Some(value)閿涘苯鍙炬禒鏍箲閿?None
    fn extract_discriminant_from_pattern(&self, pat: &crate::hir::HIRPattern) -> Option<u32> {
        use crate::hir::HIRPattern;
        match pat {
            HIRPattern::Lit(lit) => match lit {
                HIRLiteral::Int(n) if *n >= 0 && *n < u32::MAX as i64 => Some(*n as u32),
                _ => None,
            },
            HIRPattern::Wild => None,
            HIRPattern::Var { .. } => None,
            _ => None,
        }
    }

    /// 濡偓閺屻儱鈧吋妲搁崥锕€灏柊宥喣侀敓?
    /// 鏉╂柨娲栨稉鈧稉顏勫瘶閸氼偄绔风亸鏃傜波閺嬫粎娈?Local
    fn matches_pattern(&mut self, pat: &crate::hir::HIRPattern, value: Local) -> Local {
        use crate::hir::HIRPattern;
        let result = self.add_local(None, LocalKind::Temp, MIR_BOOL);

        match pat {
            HIRPattern::Wild => {
                // 闁岸鍘ょ粭锔解偓缁樻Ц閸栧綊鍘?
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
            HIRPattern::Lit(lit) => {
                // 鐎涙娼伴柌蹇斈佸蹇ョ窗濮ｆ棁绶濋敓?
                let lit_local = self.lower_literal(lit);
                self.push_inst(Instruction::Binary {
                    destination: result,
                    op: MirBinOp::Eq,
                    left: value,
                    right: lit_local,
                });
                result
            }
            HIRPattern::Var { .. } => {
                // 閸欐﹢鍣哄Ο鈥崇础閹粯妲搁崠褰掑帳
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
            _ => {
                // 閸忔湹绮Ο鈥崇础閺嗗倷绗夌€圭偟骞?
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
        }
    }

    /// 闂勫秳缍嗗Ο鈥崇础缂佹垵鐣?
    /// 婵″倹鐏夊Ο鈥崇础閸栧懎鎯堥崣姗€鍣虹紒鎴濈暰閿涘奔绮犻弸姘娑擃厽褰侀崣鏍祰閼藉嘲鑻熺紒鎴濈暰
    fn lower_pattern_bindings(&mut self, pat: &crate::hir::HIRPattern, enum_value: Local) {
        use crate::hir::HIRPattern;
        match pat {
            HIRPattern::Var { name, .. } => {
                // 缁犫偓閸楁洖褰夐柌蹇曠拨鐎规熬绱伴弫缈犻嚋閺嬫矮濡囬崐鑲╃拨鐎规艾鍩岄崣姗€鍣?
                let _ = self.add_local(Some(name.clone()), LocalKind::User, MIR_I64);
            }
            HIRPattern::Tuple(patterns) => {
                if !patterns.is_empty() {
                    let payload_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                    self.push_inst(Instruction::ExtractPayload {
                        destination: payload_local,
                        source: enum_value,
                    });
                    for (index, sub_pat) in patterns.iter().enumerate() {
                        if let HIRPattern::Var { name, .. } = sub_pat {
                            let field_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Extract {
                                destination: field_local,
                                value: payload_local,
                                index: index as u32,
                            });
                            let bound_local =
                                self.add_local(Some(name.clone()), LocalKind::User, MIR_I64);
                            self.push_inst(Instruction::Store {
                                destination: bound_local,
                                value: field_local,
                            });
                        }
                    }
                }
            }
            _ => {
                // 閸忔湹绮Ο鈥崇础閺嗗倷绗夋径鍕倞
            }
        }
    }

    /// 闂勫秳缍嗙€涙娼伴敓?
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

    /// 闂勫秳缍嗘稉鈧崗鍐╂惙娴ｆ粎顑?
    fn lower_un_op(&self, op: &hir::HIRUnaryOp) -> MirUnOp {
        match op {
            hir::HIRUnaryOp::Neg => MirUnOp::Neg,
            hir::HIRUnaryOp::Not => MirUnOp::Not,
            hir::HIRUnaryOp::BitNot => MirUnOp::BitNot,
            hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut | hir::HIRUnaryOp::Deref => MirUnOp::Neg,
        }
    }

    /// 闂勫秳缍嗘禍灞藉帗閹垮秳缍旈敓?
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








