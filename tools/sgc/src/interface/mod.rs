use crate::{
    implementation_fingerprint, source_fingerprint, FunctionFingerprint, FunctionSignatureInfo,
    GenericInstanceFingerprint, GenericItemFingerprint,
};
use sengoo_compiler::{
    ClassMember, Decl, DeclKind, Expr, ExprKind, Function, Parser, Program, Span, Stmt, StmtKind,
    TraitItem, TypeParam,
};
use std::collections::{HashMap, HashSet};

mod signature;

pub(crate) use self::signature::{ast_interface_signature, interface_fingerprint_from_program};
use self::signature::{
    ast_path_signature, function_signature, trait_bound_signature, type_signature,
};

fn source_span_slice(source: &str, span: Span) -> Option<&str> {
    source.get(span.lo as usize..span.hi as usize)
}

fn call_target_signature(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Ident(ident) => Some(ident.name.clone()),
        ExprKind::Path(path) => Some(ast_path_signature(path)),
        _ => None,
    }
}

fn collect_calls_in_expr(expr: &Expr, calls: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Ident(_) | ExprKind::Path(_) | ExprKind::Continue => {}
        ExprKind::Unary { operand, .. }
        | ExprKind::Await(operand)
        | ExprKind::Try(operand)
        | ExprKind::Paren(operand) => {
            collect_calls_in_expr(operand, calls);
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
        }
        | ExprKind::AssignOp {
            target: left,
            value: right,
            ..
        }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            collect_calls_in_expr(left, calls);
            collect_calls_in_expr(right, calls);
        }
        ExprKind::Call { func, args } => {
            if let Some(target) = call_target_signature(func) {
                calls.push(target);
            }
            collect_calls_in_expr(func, calls);
            for arg in args {
                collect_calls_in_expr(arg, calls);
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            calls.push(format!("method::{}", method.name));
            collect_calls_in_expr(receiver, calls);
            for arg in args {
                collect_calls_in_expr(arg, calls);
            }
        }
        ExprKind::Block(block)
        | ExprKind::Loop(block)
        | ExprKind::AsyncBlock(block)
        | ExprKind::ParallelBlock(block) => {
            for stmt in &block.stmts {
                collect_calls_in_stmt(stmt, calls);
            }
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_calls_in_expr(cond, calls);
            for stmt in &then_branch.stmts {
                collect_calls_in_stmt(stmt, calls);
            }
            if let Some(else_expr) = else_branch.as_deref() {
                collect_calls_in_expr(else_expr, calls);
            }
        }
        ExprKind::While { cond, body } => {
            collect_calls_in_expr(cond, calls);
            for stmt in &body.stmts {
                collect_calls_in_stmt(stmt, calls);
            }
        }
        ExprKind::For { iter, body, .. } => {
            collect_calls_in_expr(iter, calls);
            for stmt in &body.stmts {
                collect_calls_in_stmt(stmt, calls);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_calls_in_expr(scrutinee, calls);
            for arm in arms {
                if let Some(guard) = arm.guard.as_deref() {
                    collect_calls_in_expr(guard, calls);
                }
                collect_calls_in_expr(&arm.body, calls);
            }
        }
        ExprKind::Return(value) | ExprKind::Break(value) | ExprKind::Yield(value) => {
            if let Some(value) = value.as_deref() {
                collect_calls_in_expr(value, calls);
            }
        }
        ExprKind::Field { base, .. } => {
            collect_calls_in_expr(base, calls);
        }
        ExprKind::Array(elements) | ExprKind::Tuple(elements) => {
            for elem in elements {
                collect_calls_in_expr(elem, calls);
            }
        }
        ExprKind::Struct { fields, base, .. } => {
            for field in fields {
                collect_calls_in_expr(&field.value, calls);
            }
            if let Some(base) = base.as_deref() {
                collect_calls_in_expr(base, calls);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(start) = start.as_deref() {
                collect_calls_in_expr(start, calls);
            }
            if let Some(end) = end.as_deref() {
                collect_calls_in_expr(end, calls);
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_calls_in_expr(body, calls);
        }
        ExprKind::Cast { expr, .. } | ExprKind::Is { expr, .. } => {
            collect_calls_in_expr(expr, calls);
        }
    }
}

fn collect_calls_in_stmt(stmt: &Stmt, calls: &mut Vec<String>) {
    match &stmt.kind {
        StmtKind::Let {
            value: Some(value), ..
        } => collect_calls_in_expr(value, calls),
        StmtKind::Const { value, .. } => collect_calls_in_expr(value, calls),
        StmtKind::Expr(expr) => collect_calls_in_expr(expr, calls),
        StmtKind::Item(_) | StmtKind::Let { value: None, .. } => {}
    }
}

fn function_symbol(module_path: &str, scope: &[String], name: &str) -> String {
    let mut parts = Vec::with_capacity(scope.len() + 2);
    parts.push(module_path.to_string());
    parts.extend(scope.iter().cloned());
    parts.push(name.to_string());
    parts.join("::")
}

fn push_function_fingerprint(
    out: &mut Vec<FunctionFingerprint>,
    module_path: &str,
    scope: &[String],
    function: &Function,
    source: &str,
) {
    let abi_hash = source_fingerprint(&function_signature(function));
    let body_hash = source_span_slice(source, function.body.span)
        .map(implementation_fingerprint)
        .unwrap_or_else(|| source_fingerprint(&format!("{:?}", function.body.stmts)));

    let mut calls = Vec::new();
    for stmt in &function.body.stmts {
        collect_calls_in_stmt(stmt, &mut calls);
    }
    calls.sort();
    calls.dedup();

    out.push(FunctionFingerprint {
        symbol: function_symbol(module_path, scope, &function.name.name),
        abi_hash,
        body_hash,
        calls,
        module_imports: Vec::new(),
    });
}

fn collect_function_fingerprints_from_decl(
    out: &mut Vec<FunctionFingerprint>,
    module_path: &str,
    scope: &[String],
    decl: &Decl,
    source: &str,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            push_function_fingerprint(out, module_path, scope, function, source);
        }
        DeclKind::Class(class_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("class".to_string());
            scoped.push(class_decl.name.name.clone());
            for member in &class_decl.members {
                if let ClassMember::Method(function) = member {
                    push_function_fingerprint(out, module_path, &scoped, function, source);
                }
            }
        }
        DeclKind::Trait(trait_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("trait".to_string());
            scoped.push(trait_decl.name.name.clone());
            for item in &trait_decl.items {
                if let TraitItem::Function(function) = item {
                    push_function_fingerprint(out, module_path, &scoped, function, source);
                }
            }
        }
        DeclKind::Impl(impl_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("impl".to_string());
            scoped.push(type_signature(&impl_decl.target_type));
            for function in &impl_decl.items {
                push_function_fingerprint(out, module_path, &scoped, function, source);
            }
        }
        DeclKind::Module(module_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("mod".to_string());
            scoped.push(module_decl.name.name.clone());
            for item in &module_decl.items {
                collect_function_fingerprints_from_decl(out, module_path, &scoped, item, source);
            }
        }
        _ => {}
    }
}

pub(crate) fn function_fingerprints_for_module(
    module_path: &str,
    source: &str,
) -> Vec<FunctionFingerprint> {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return Vec::new(),
    };
    function_fingerprints_for_program(module_path, source, &program)
}

pub(crate) fn function_fingerprints_for_program(
    module_path: &str,
    source: &str,
    program: &Program,
) -> Vec<FunctionFingerprint> {
    let mut functions = Vec::new();
    for decl in &program.decls {
        collect_function_fingerprints_from_decl(&mut functions, module_path, &[], decl, source);
    }

    let mut simple_to_symbol = HashMap::<String, Option<String>>::new();
    for function in &functions {
        let simple = function
            .symbol
            .rsplit("::")
            .next()
            .unwrap_or_default()
            .to_string();
        match simple_to_symbol.get_mut(&simple) {
            Some(entry) => *entry = None,
            None => {
                simple_to_symbol.insert(simple, Some(function.symbol.clone()));
            }
        }
    }

    for function in &mut functions {
        for call in &mut function.calls {
            if call.contains("::") {
                continue;
            }
            if let Some(Some(symbol)) = simple_to_symbol.get(call) {
                *call = symbol.clone();
            }
        }
        function.calls.sort();
        function.calls.dedup();
    }

    functions.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    functions
}

fn push_function_signature_info(
    out: &mut Vec<FunctionSignatureInfo>,
    module_path: &str,
    scope: &[String],
    function: &Function,
) {
    out.push(FunctionSignatureInfo {
        symbol: function_symbol(module_path, scope, &function.name.name),
        signature: function_signature(function),
    });
}

fn collect_function_signatures_from_decl(
    out: &mut Vec<FunctionSignatureInfo>,
    module_path: &str,
    scope: &[String],
    decl: &Decl,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            push_function_signature_info(out, module_path, scope, function);
        }
        DeclKind::Class(class_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("class".to_string());
            scoped.push(class_decl.name.name.clone());
            for member in &class_decl.members {
                if let ClassMember::Method(function) = member {
                    push_function_signature_info(out, module_path, &scoped, function);
                }
            }
        }
        DeclKind::Trait(trait_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("trait".to_string());
            scoped.push(trait_decl.name.name.clone());
            for item in &trait_decl.items {
                if let TraitItem::Function(function) = item {
                    push_function_signature_info(out, module_path, &scoped, function);
                }
            }
        }
        DeclKind::Impl(impl_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("impl".to_string());
            scoped.push(type_signature(&impl_decl.target_type));
            for function in &impl_decl.items {
                push_function_signature_info(out, module_path, &scoped, function);
            }
        }
        DeclKind::Module(module_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("mod".to_string());
            scoped.push(module_decl.name.name.clone());
            for item in &module_decl.items {
                collect_function_signatures_from_decl(out, module_path, &scoped, item);
            }
        }
        _ => {}
    }
}

pub(crate) fn function_signatures_for_module(
    module_path: &str,
    source: &str,
) -> Vec<FunctionSignatureInfo> {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return Vec::new(),
    };

    let mut signatures = Vec::new();
    for decl in &program.decls {
        collect_function_signatures_from_decl(&mut signatures, module_path, &[], decl);
    }
    signatures.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    signatures.dedup_by(|a, b| a.symbol == b.symbol);
    signatures
}

#[derive(Debug, Clone)]
struct GenericCallableMeta {
    stable_item_id: String,
    module_id: String,
    interface_hash: u64,
    body_hash: u64,
    type_param_count: usize,
    type_param_names: Vec<String>,
    receiver_type_template: Option<String>,
    param_type_templates: Vec<String>,
    return_type_template: Option<String>,
}

#[derive(Debug, Clone)]
struct GenericMethodTemplate {
    receiver_type_template: String,
    param_type_templates: Vec<String>,
    return_type_template: Option<String>,
    type_param_names: Vec<String>,
}

fn generic_item_id(module_path: &str, scope: &[String], kind: &str, name: &str) -> String {
    let mut parts = Vec::with_capacity(scope.len() + 3);
    parts.push(module_path.to_string());
    parts.extend(scope.iter().cloned());
    parts.push(kind.to_string());
    parts.push(name.to_string());
    parts.join("::")
}

fn generic_type_param_signature(type_params: &[TypeParam]) -> String {
    type_params
        .iter()
        .map(|tp| {
            let mut repr = tp.name.name.clone();
            if !tp.bounds.is_empty() {
                repr.push(':');
                repr.push_str(
                    &tp.bounds
                        .iter()
                        .map(trait_bound_signature)
                        .collect::<Vec<_>>()
                        .join("+"),
                );
            }
            if let Some(default) = &tp.default {
                repr.push('=');
                repr.push_str(&type_signature(default));
            }
            repr
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn collect_impl_method_templates_from_decl(
    out: &mut HashMap<String, GenericMethodTemplate>,
    module_path: &str,
    scope: &[String],
    decl: &Decl,
) {
    match &decl.kind {
        DeclKind::Impl(impl_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("impl".to_string());
            scoped.push(type_signature(&impl_decl.target_type));

            for function in &impl_decl.items {
                let effective_type_params =
                    impl_decl.type_params.len() + function.type_params.len();
                if effective_type_params == 0 {
                    continue;
                }
                let symbol = function_symbol(module_path, &scoped, &function.name.name);
                let type_param_names = impl_decl
                    .type_params
                    .iter()
                    .chain(function.type_params.iter())
                    .map(|param| param.name.name.clone())
                    .collect::<Vec<_>>();

                out.insert(
                    symbol,
                    GenericMethodTemplate {
                        receiver_type_template: type_signature(&impl_decl.target_type),
                        param_type_templates: function
                            .params
                            .iter()
                            .map(|param| type_signature(&param.ty))
                            .collect(),
                        return_type_template: function.return_type.as_ref().map(type_signature),
                        type_param_names,
                    },
                );
            }
        }
        DeclKind::Module(module_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("mod".to_string());
            scoped.push(module_decl.name.name.clone());
            for item in &module_decl.items {
                collect_impl_method_templates_from_decl(out, module_path, &scoped, item);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn push_generic_item(
    out: &mut Vec<GenericItemFingerprint>,
    kind: &str,
    stable_item_id: String,
    symbol: String,
    module_id: &str,
    interface_hash: u64,
    body_hash: u64,
    type_param_count: usize,
    calls: Vec<String>,
) {
    out.push(GenericItemFingerprint {
        stable_item_id,
        symbol,
        module_id: module_id.to_string(),
        kind: kind.to_string(),
        interface_hash,
        body_hash,
        type_param_count: type_param_count as u32,
        calls,
    });
}

fn collect_generic_item_fingerprints_from_decl(
    out: &mut Vec<GenericItemFingerprint>,
    module_path: &str,
    scope: &[String],
    decl: &Decl,
    source: &str,
    inherited_generic_params: usize,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            let effective_type_params = inherited_generic_params + function.type_params.len();
            if effective_type_params > 0 {
                let mut calls = Vec::new();
                for stmt in &function.body.stmts {
                    collect_calls_in_stmt(stmt, &mut calls);
                }
                calls.sort();
                calls.dedup();
                let symbol = function_symbol(module_path, scope, &function.name.name);
                let stable_item_id = symbol.clone();
                let interface_hash = source_fingerprint(&function_signature(function));
                let body_hash = source_span_slice(source, function.body.span)
                    .map(implementation_fingerprint)
                    .unwrap_or_else(|| source_fingerprint(&format!("{:?}", function.body.stmts)));
                push_generic_item(
                    out,
                    "function",
                    stable_item_id,
                    symbol,
                    module_path,
                    interface_hash,
                    body_hash,
                    effective_type_params,
                    calls,
                );
            }
        }
        DeclKind::Struct(struct_decl) => {
            if !struct_decl.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "struct", &struct_decl.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "struct:{}<{}>",
                    struct_decl.name.name,
                    generic_type_param_signature(&struct_decl.type_params)
                ));
                let body_hash = source_span_slice(source, struct_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "struct",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    struct_decl.type_params.len(),
                    Vec::new(),
                );
            }
        }
        DeclKind::Enum(enum_decl) => {
            if !enum_decl.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "enum", &enum_decl.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "enum:{}<{}>",
                    enum_decl.name.name,
                    generic_type_param_signature(&enum_decl.type_params)
                ));
                let body_hash = source_span_slice(source, enum_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "enum",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    enum_decl.type_params.len(),
                    Vec::new(),
                );
            }
        }
        DeclKind::Class(class_decl) => {
            if !class_decl.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "class", &class_decl.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "class:{}<{}>",
                    class_decl.name.name,
                    generic_type_param_signature(&class_decl.type_params)
                ));
                let body_hash = source_span_slice(source, class_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "class",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    class_decl.type_params.len(),
                    Vec::new(),
                );
            }

            let mut scoped = scope.to_vec();
            scoped.push("class".to_string());
            scoped.push(class_decl.name.name.clone());
            for member in &class_decl.members {
                if let ClassMember::Method(function) = member {
                    let effective_type_params =
                        class_decl.type_params.len() + function.type_params.len();
                    if effective_type_params == 0 {
                        continue;
                    }
                    let mut calls = Vec::new();
                    for stmt in &function.body.stmts {
                        collect_calls_in_stmt(stmt, &mut calls);
                    }
                    calls.sort();
                    calls.dedup();
                    let symbol = function_symbol(module_path, &scoped, &function.name.name);
                    let stable_item_id = symbol.clone();
                    let interface_hash = source_fingerprint(&function_signature(function));
                    let body_hash = source_span_slice(source, function.body.span)
                        .map(implementation_fingerprint)
                        .unwrap_or_else(|| {
                            source_fingerprint(&format!("{:?}", function.body.stmts))
                        });
                    push_generic_item(
                        out,
                        "method",
                        stable_item_id,
                        symbol,
                        module_path,
                        interface_hash,
                        body_hash,
                        effective_type_params,
                        calls,
                    );
                }
            }
        }
        DeclKind::Trait(trait_decl) => {
            if !trait_decl.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "trait", &trait_decl.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "trait:{}<{}>",
                    trait_decl.name.name,
                    generic_type_param_signature(&trait_decl.type_params)
                ));
                let body_hash = source_span_slice(source, trait_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "trait",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    trait_decl.type_params.len(),
                    Vec::new(),
                );
            }
            let mut scoped = scope.to_vec();
            scoped.push("trait".to_string());
            scoped.push(trait_decl.name.name.clone());
            for item in &trait_decl.items {
                if let TraitItem::Function(function) = item {
                    let effective_type_params =
                        trait_decl.type_params.len() + function.type_params.len();
                    if effective_type_params == 0 {
                        continue;
                    }
                    let symbol = function_symbol(module_path, &scoped, &function.name.name);
                    let stable_item_id = symbol.clone();
                    let interface_hash = source_fingerprint(&function_signature(function));
                    let body_hash = source_span_slice(source, function.body.span)
                        .map(implementation_fingerprint)
                        .unwrap_or_else(|| {
                            source_fingerprint(&format!("{:?}", function.body.stmts))
                        });
                    push_generic_item(
                        out,
                        "trait_method",
                        stable_item_id,
                        symbol,
                        module_path,
                        interface_hash,
                        body_hash,
                        effective_type_params,
                        Vec::new(),
                    );
                }
            }
        }
        DeclKind::Impl(impl_decl) => {
            if !impl_decl.type_params.is_empty() {
                let stable_item_id = generic_item_id(
                    module_path,
                    scope,
                    "impl",
                    &type_signature(&impl_decl.target_type),
                );
                let interface_hash = source_fingerprint(&format!(
                    "impl:{}<{}>",
                    type_signature(&impl_decl.target_type),
                    generic_type_param_signature(&impl_decl.type_params)
                ));
                let body_hash = source_span_slice(source, impl_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "impl",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    impl_decl.type_params.len(),
                    Vec::new(),
                );
            }
            let mut scoped = scope.to_vec();
            scoped.push("impl".to_string());
            scoped.push(type_signature(&impl_decl.target_type));
            for function in &impl_decl.items {
                let effective_type_params =
                    impl_decl.type_params.len() + function.type_params.len();
                if effective_type_params == 0 {
                    continue;
                }
                let mut calls = Vec::new();
                for stmt in &function.body.stmts {
                    collect_calls_in_stmt(stmt, &mut calls);
                }
                calls.sort();
                calls.dedup();
                let symbol = function_symbol(module_path, &scoped, &function.name.name);
                let stable_item_id = symbol.clone();
                let interface_hash = source_fingerprint(&function_signature(function));
                let body_hash = source_span_slice(source, function.body.span)
                    .map(implementation_fingerprint)
                    .unwrap_or_else(|| source_fingerprint(&format!("{:?}", function.body.stmts)));
                push_generic_item(
                    out,
                    "impl_method",
                    stable_item_id,
                    symbol,
                    module_path,
                    interface_hash,
                    body_hash,
                    effective_type_params,
                    calls,
                );
            }
        }
        DeclKind::TypeAlias(alias) => {
            if !alias.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "type_alias", &alias.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "type_alias:{}<{}>",
                    alias.name.name,
                    generic_type_param_signature(&alias.type_params)
                ));
                let body_hash = source_span_slice(source, alias.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "type_alias",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    alias.type_params.len(),
                    Vec::new(),
                );
            }
        }
        DeclKind::Module(module_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("mod".to_string());
            scoped.push(module_decl.name.name.clone());
            for item in &module_decl.items {
                collect_generic_item_fingerprints_from_decl(
                    out,
                    module_path,
                    &scoped,
                    item,
                    source,
                    inherited_generic_params,
                );
            }
        }
        _ => {}
    }
}

fn split_top_level_type_args(args: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth_angle = 0usize;
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut start = 0usize;

    for (idx, ch) in args.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle = depth_angle.saturating_sub(1),
            '(' => depth_paren += 1,
            ')' => depth_paren = depth_paren.saturating_sub(1),
            '[' => depth_bracket += 1,
            ']' => depth_bracket = depth_bracket.saturating_sub(1),
            ',' if depth_angle == 0 && depth_paren == 0 && depth_bracket == 0 => {
                parts.push(args[start..idx].trim().to_string());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    if start < args.len() {
        parts.push(args[start..].trim().to_string());
    }

    parts.into_iter().filter(|part| !part.is_empty()).collect()
}

fn parse_named_type_signature(sig: &str) -> Option<(String, Vec<String>)> {
    let trimmed = sig.trim();
    let start = trimmed.find('<')?;
    if !trimmed.ends_with('>') {
        return None;
    }
    let head = trimmed[..start].trim().to_string();
    let inner = &trimmed[start + 1..trimmed.len() - 1];
    Some((head, split_top_level_type_args(inner)))
}

fn extract_impl_receiver_template(symbol: &str) -> Option<String> {
    let marker = "::impl::";
    let start = symbol.find(marker)? + marker.len();
    let tail = &symbol[start..];
    let method_sep = tail.rfind("::")?;
    Some(tail[..method_sep].to_string())
}

fn infer_expr_type_signature(expr: &Expr, local_types: &HashMap<String, String>) -> String {
    match &expr.kind {
        ExprKind::Literal(lit) => match lit {
            sengoo_compiler::Literal::Int(_) => "i64".to_string(),
            sengoo_compiler::Literal::Float(_) => "f64".to_string(),
            sengoo_compiler::Literal::Bool(_) => "bool".to_string(),
            sengoo_compiler::Literal::String(_) => "str".to_string(),
            sengoo_compiler::Literal::Char(_) => "char".to_string(),
            sengoo_compiler::Literal::Bytes(_) => "bytes".to_string(),
            sengoo_compiler::Literal::Null => "null".to_string(),
            sengoo_compiler::Literal::Unit => "unit".to_string(),
        },
        ExprKind::Array(items) => {
            if let Some(first) = items.first() {
                format!("array<{}>", infer_expr_type_signature(first, local_types))
            } else {
                "array<_>".to_string()
            }
        }
        ExprKind::Tuple(items) => format!("tuple{}", items.len()),
        ExprKind::Struct { path, .. } => format!("struct:{}", ast_path_signature(path)),
        ExprKind::Path(path) => path
            .as_simple()
            .and_then(|ident| local_types.get(&ident.name))
            .cloned()
            .unwrap_or_else(|| format!("path:{}", ast_path_signature(path))),
        ExprKind::Ident(ident) => local_types
            .get(&ident.name)
            .cloned()
            .unwrap_or_else(|| "_".to_string()),
        ExprKind::Paren(inner) => infer_expr_type_signature(inner, local_types),
        _ => "_".to_string(),
    }
}

fn substitute_type_signature(template: &str, subst: &HashMap<String, String>) -> String {
    if let Some(replacement) = subst.get(template.trim()) {
        return replacement.clone();
    }

    let Some((head, args)) = parse_named_type_signature(template) else {
        return template.to_string();
    };

    let resolved_args = args
        .iter()
        .map(|arg| substitute_type_signature(arg, subst))
        .collect::<Vec<_>>()
        .join(",");
    format!("{head}<{resolved_args}>")
}

fn unify_type_signature_template(
    template: &str,
    actual: &str,
    type_param_names: &[String],
    subst: &mut HashMap<String, String>,
) -> bool {
    let template = template.trim();
    let actual = actual.trim();

    if type_param_names.iter().any(|name| name == template) {
        match subst.get(template) {
            Some(existing) => existing == actual,
            None => {
                subst.insert(template.to_string(), actual.to_string());
                true
            }
        }
    } else if let (Some((template_head, template_args)), Some((actual_head, actual_args))) = (
        parse_named_type_signature(template),
        parse_named_type_signature(actual),
    ) {
        template_head == actual_head
            && template_args.len() == actual_args.len()
            && template_args
                .iter()
                .zip(actual_args.iter())
                .all(|(template_arg, actual_arg)| {
                    unify_type_signature_template(template_arg, actual_arg, type_param_names, subst)
                })
    } else {
        template == actual
    }
}

fn type_param_substitution(
    meta: &GenericCallableMeta,
    canonical_type_args: &[String],
) -> HashMap<String, String> {
    meta.type_param_names
        .iter()
        .cloned()
        .zip(canonical_type_args.iter().cloned())
        .collect()
}

fn infer_expr_type_signature_with_methods(
    expr: &Expr,
    local_types: &HashMap<String, String>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) -> String {
    match &expr.kind {
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let Some((symbol, canonical_type_args)) = generic_method_call_instance_parts(
                receiver,
                &method.name,
                args,
                local_types,
                method_to_symbols,
                callable_meta,
            ) else {
                return "_".to_string();
            };
            let Some(meta) = callable_meta.get(&symbol) else {
                return "_".to_string();
            };
            let Some(return_type_template) = meta.return_type_template.as_deref() else {
                return "_".to_string();
            };
            let subst = type_param_substitution(meta, &canonical_type_args);
            substitute_type_signature(return_type_template, &subst)
        }
        _ => infer_expr_type_signature(expr, local_types),
    }
}

fn generic_instance_base_key(item_stable_id: &str, canonical_type_args: &[String]) -> String {
    if canonical_type_args.is_empty() {
        return format!("{}<>", item_stable_id);
    }
    format!("{}<{}>", item_stable_id, canonical_type_args.join(","))
}

fn resolve_generic_call_symbol(
    call_name: &str,
    simple_to_symbol: &HashMap<String, Option<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) -> Option<String> {
    if callable_meta.contains_key(call_name) {
        return Some(call_name.to_string());
    }
    match simple_to_symbol.get(call_name) {
        Some(Some(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_instance_if_generic_call(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    call_name: &str,
    args: &[Expr],
    local_types: &HashMap<String, String>,
    simple_to_symbol: &HashMap<String, Option<String>>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    let Some(symbol) = resolve_generic_call_symbol(call_name, simple_to_symbol, callable_meta)
    else {
        return;
    };
    let Some(meta) = callable_meta.get(&symbol) else {
        return;
    };
    let _ = &meta.module_id;

    let mut canonical_type_args = args
        .iter()
        .map(|arg| {
            infer_expr_type_signature_with_methods(
                arg,
                local_types,
                method_to_symbols,
                callable_meta,
            )
        })
        .take(meta.type_param_count)
        .collect::<Vec<_>>();
    while canonical_type_args.len() < meta.type_param_count {
        canonical_type_args.push("_".to_string());
    }
    let instance_key = generic_instance_base_key(&meta.stable_item_id, &canonical_type_args);
    if !seen.insert(instance_key.clone()) {
        return;
    }

    out.push(GenericInstanceFingerprint {
        item_stable_id: meta.stable_item_id.clone(),
        module_id: module_path.to_string(),
        canonical_type_args,
        instance_key,
        interface_hash: meta.interface_hash,
        body_hash: meta.body_hash,
    });
}

fn generic_method_call_instance_parts(
    receiver: &Expr,
    method_name: &str,
    args: &[Expr],
    local_types: &HashMap<String, String>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) -> Option<(String, Vec<String>)> {
    let receiver_sig = infer_expr_type_signature_with_methods(
        receiver,
        local_types,
        method_to_symbols,
        callable_meta,
    );
    let candidate_symbols = method_to_symbols.get(method_name)?;

    for symbol in candidate_symbols {
        let Some(meta) = callable_meta.get(symbol) else {
            continue;
        };
        let Some(template) = meta.receiver_type_template.as_deref() else {
            continue;
        };
        let mut subst = HashMap::new();
        if !unify_type_signature_template(
            template,
            &receiver_sig,
            &meta.type_param_names,
            &mut subst,
        ) {
            continue;
        }

        if meta.param_type_templates.len() != args.len() {
            continue;
        }

        let mut param_mismatch = false;
        for (template_arg, actual_arg) in meta.param_type_templates.iter().zip(args.iter()) {
            let actual_sig = infer_expr_type_signature_with_methods(
                actual_arg,
                local_types,
                method_to_symbols,
                callable_meta,
            );
            if !unify_type_signature_template(
                template_arg,
                &actual_sig,
                &meta.type_param_names,
                &mut subst,
            ) {
                param_mismatch = true;
                break;
            }
        }
        if param_mismatch {
            continue;
        }

        let canonical_type_args = meta
            .type_param_names
            .iter()
            .map(|param| subst.get(param).cloned().unwrap_or_else(|| "_".to_string()))
            .take(meta.type_param_count)
            .collect::<Vec<_>>();

        return Some((symbol.clone(), canonical_type_args));
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn push_instance_if_generic_method_call(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    receiver: &Expr,
    method_name: &str,
    args: &[Expr],
    local_types: &HashMap<String, String>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    let Some((symbol, canonical_type_args)) = generic_method_call_instance_parts(
        receiver,
        method_name,
        args,
        local_types,
        method_to_symbols,
        callable_meta,
    ) else {
        return;
    };
    let Some(meta) = callable_meta.get(&symbol) else {
        return;
    };
    let instance_key = generic_instance_base_key(&meta.stable_item_id, &canonical_type_args);
    if !seen.insert(instance_key.clone()) {
        return;
    }

    out.push(GenericInstanceFingerprint {
        item_stable_id: meta.stable_item_id.clone(),
        module_id: module_path.to_string(),
        canonical_type_args,
        instance_key,
        interface_hash: meta.interface_hash,
        body_hash: meta.body_hash,
    });
}

#[allow(clippy::too_many_arguments)]
fn collect_generic_instances_in_block(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    stmts: &[Stmt],
    local_types: &HashMap<String, String>,
    simple_to_symbol: &HashMap<String, Option<String>>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    let mut scoped_locals = local_types.clone();
    for stmt in stmts {
        collect_generic_instances_in_stmt(
            out,
            seen,
            module_path,
            stmt,
            &mut scoped_locals,
            simple_to_symbol,
            method_to_symbols,
            callable_meta,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_generic_instances_in_expr(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    expr: &Expr,
    local_types: &HashMap<String, String>,
    simple_to_symbol: &HashMap<String, Option<String>>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Ident(_) | ExprKind::Path(_) | ExprKind::Continue => {}
        ExprKind::Unary { operand, .. }
        | ExprKind::Await(operand)
        | ExprKind::Try(operand)
        | ExprKind::Paren(operand) => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                operand,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
        }
        | ExprKind::AssignOp {
            target: left,
            value: right,
            ..
        }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                left,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                right,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Call { func, args } => {
            if let Some(target) = call_target_signature(func) {
                push_instance_if_generic_call(
                    out,
                    seen,
                    module_path,
                    &target,
                    args,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                func,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            for arg in args {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    arg,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            push_instance_if_generic_method_call(
                out,
                seen,
                module_path,
                receiver,
                &method.name,
                args,
                local_types,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                receiver,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            for arg in args {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    arg,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Block(block)
        | ExprKind::Loop(block)
        | ExprKind::AsyncBlock(block)
        | ExprKind::ParallelBlock(block) => {
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &block.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                cond,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &then_branch.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            if let Some(else_expr) = else_branch.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    else_expr,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::While { cond, body } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                cond,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &body.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::For { iter, body, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                iter,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &body.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                scrutinee,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            for arm in arms {
                if let Some(guard) = arm.guard.as_deref() {
                    collect_generic_instances_in_expr(
                        out,
                        seen,
                        module_path,
                        guard,
                        local_types,
                        simple_to_symbol,
                        method_to_symbols,
                        callable_meta,
                    );
                }
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    &arm.body,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Return(value) | ExprKind::Break(value) | ExprKind::Yield(value) => {
            if let Some(value) = value.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    value,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Field { base, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                base,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Array(elements) | ExprKind::Tuple(elements) => {
            for elem in elements {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    elem,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Struct { fields, base, .. } => {
            for field in fields {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    &field.value,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            if let Some(base) = base.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    base,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(start) = start.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    start,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            if let Some(end) = end.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    end,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                body,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Cast { expr, .. } | ExprKind::Is { expr, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                expr,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_generic_instances_in_stmt(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    stmt: &Stmt,
    local_types: &mut HashMap<String, String>,
    simple_to_symbol: &HashMap<String, Option<String>>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    match &stmt.kind {
        StmtKind::Let { name, ty, value } => {
            if let Some(value) = value.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    value,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            let inferred = ty.as_ref().map(type_signature).or_else(|| {
                value.as_deref().map(|expr| {
                    infer_expr_type_signature_with_methods(
                        expr,
                        local_types,
                        method_to_symbols,
                        callable_meta,
                    )
                })
            });
            if let Some(inferred) = inferred.filter(|ty| ty != "_") {
                local_types.insert(name.name.clone(), inferred);
            }
        }
        StmtKind::Const { name, ty, value } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                value,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            local_types.insert(name.name.clone(), type_signature(ty));
        }
        StmtKind::Expr(expr) => collect_generic_instances_in_expr(
            out,
            seen,
            module_path,
            expr,
            local_types,
            simple_to_symbol,
            method_to_symbols,
            callable_meta,
        ),
        StmtKind::Item(_) => {}
    }
}

pub(crate) fn generic_fingerprints_for_module(
    module_path: &str,
    source: &str,
) -> (Vec<GenericItemFingerprint>, Vec<GenericInstanceFingerprint>) {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return (Vec::new(), Vec::new()),
    };
    generic_fingerprints_for_program(module_path, source, &program)
}

pub(crate) fn generic_fingerprints_for_program(
    module_path: &str,
    source: &str,
    program: &Program,
) -> (Vec<GenericItemFingerprint>, Vec<GenericInstanceFingerprint>) {
    let mut items = Vec::new();
    for decl in &program.decls {
        collect_generic_item_fingerprints_from_decl(&mut items, module_path, &[], decl, source, 0);
    }
    items.sort_by(|a, b| a.stable_item_id.cmp(&b.stable_item_id));
    items.dedup_by(|a, b| a.stable_item_id == b.stable_item_id);

    let mut method_templates = HashMap::<String, GenericMethodTemplate>::new();
    for decl in &program.decls {
        collect_impl_method_templates_from_decl(&mut method_templates, module_path, &[], decl);
    }

    let callable_meta = items
        .iter()
        .filter_map(|item| {
            if item.kind != "function" && item.kind != "impl_method" {
                return None;
            }
            let method_template = method_templates.get(&item.symbol);
            Some((
                item.symbol.clone(),
                GenericCallableMeta {
                    stable_item_id: item.stable_item_id.clone(),
                    module_id: item.module_id.clone(),
                    interface_hash: item.interface_hash,
                    body_hash: item.body_hash,
                    type_param_count: item.type_param_count as usize,
                    type_param_names: method_template
                        .map(|template| template.type_param_names.clone())
                        .unwrap_or_default(),
                    receiver_type_template: if item.kind == "impl_method" {
                        method_template
                            .map(|template| template.receiver_type_template.clone())
                            .or_else(|| extract_impl_receiver_template(&item.symbol))
                    } else {
                        None
                    },
                    param_type_templates: method_template
                        .map(|template| template.param_type_templates.clone())
                        .unwrap_or_default(),
                    return_type_template: method_template
                        .and_then(|template| template.return_type_template.clone()),
                },
            ))
        })
        .collect::<HashMap<_, _>>();

    let mut simple_to_symbol = HashMap::<String, Option<String>>::new();
    let mut method_to_symbols = HashMap::<String, Vec<String>>::new();
    for (symbol, meta) in &callable_meta {
        let simple = symbol.rsplit("::").next().unwrap_or_default().to_string();
        if meta.receiver_type_template.is_some() {
            method_to_symbols
                .entry(simple)
                .or_default()
                .push(symbol.clone());
        } else {
            match simple_to_symbol.get_mut(&simple) {
                Some(entry) => *entry = None,
                None => {
                    simple_to_symbol.insert(simple, Some(symbol.clone()));
                }
            }
        }
    }

    let mut instances = Vec::<GenericInstanceFingerprint>::new();
    let mut seen_instances = HashSet::<String>::new();
    for decl in &program.decls {
        match &decl.kind {
            DeclKind::Function(function) => {
                let mut local_types = function
                    .params
                    .iter()
                    .map(|param| (param.name.name.clone(), type_signature(&param.ty)))
                    .collect::<HashMap<_, _>>();
                for stmt in &function.body.stmts {
                    collect_generic_instances_in_stmt(
                        &mut instances,
                        &mut seen_instances,
                        module_path,
                        stmt,
                        &mut local_types,
                        &simple_to_symbol,
                        &method_to_symbols,
                        &callable_meta,
                    );
                }
            }
            DeclKind::Const(const_decl) => {
                let local_types = HashMap::new();
                collect_generic_instances_in_expr(
                    &mut instances,
                    &mut seen_instances,
                    module_path,
                    &const_decl.value,
                    &local_types,
                    &simple_to_symbol,
                    &method_to_symbols,
                    &callable_meta,
                );
            }
            DeclKind::Static(static_decl) => {
                let local_types = HashMap::new();
                collect_generic_instances_in_expr(
                    &mut instances,
                    &mut seen_instances,
                    module_path,
                    &static_decl.value,
                    &local_types,
                    &simple_to_symbol,
                    &method_to_symbols,
                    &callable_meta,
                );
            }
            _ => {}
        }
    }
    instances.sort_by(|a, b| a.instance_key.cmp(&b.instance_key));
    instances.dedup_by(|a, b| a.instance_key == b.instance_key);
    (items, instances)
}
