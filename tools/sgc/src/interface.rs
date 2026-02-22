use crate::{
    implementation_fingerprint, source_fingerprint, FunctionFingerprint, FunctionSignatureInfo,
    GenericInstanceFingerprint, GenericItemFingerprint,
};
use sengoo_compiler::{
    ClassMember, Decl, DeclKind, Expr, ExprKind, ExternItem, Function, ImportKind, Param, Parser,
    Path as AstPath, Program, SelfParam, Span, Stmt, StmtKind, TraitBound, TraitItem, Type,
    TypeKind, TypeParam, VariantField, Visibility,
};
use std::collections::{HashMap, HashSet};

fn visibility_label(vis: Visibility) -> &'static str {
    match vis {
        Visibility::Public => "pub",
        Visibility::Private => "priv",
    }
}

fn ast_path_signature(path: &AstPath) -> String {
    if path.segments.is_empty() {
        return "<empty>".to_string();
    }
    path.segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

fn trait_bound_signature(bound: &TraitBound) -> String {
    let mut rendered = ast_path_signature(&bound.path);
    if !bound.params.is_empty() {
        let params = bound
            .params
            .iter()
            .map(type_signature)
            .collect::<Vec<_>>()
            .join(",");
        rendered.push('<');
        rendered.push_str(&params);
        rendered.push('>');
    }
    rendered
}

fn type_signature(ty: &Type) -> String {
    match &ty.kind {
        TypeKind::Path(path) => ast_path_signature(path),
        TypeKind::PathWithArgs { path, args } => {
            let mut rendered = ast_path_signature(path);
            rendered.push('<');
            rendered.push_str(
                &args
                    .iter()
                    .map(type_signature)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            rendered.push('>');
            rendered
        }
        TypeKind::Tuple(types) => {
            let inner = types
                .iter()
                .map(type_signature)
                .collect::<Vec<_>>()
                .join(",");
            format!("({})", inner)
        }
        TypeKind::Array(elem, len) => format!("[{};{}]", type_signature(elem), len),
        TypeKind::Slice(elem) => format!("[{}]", type_signature(elem)),
        TypeKind::Ptr { base, is_mut } => {
            if *is_mut {
                format!("*mut {}", type_signature(base))
            } else {
                format!("*const {}", type_signature(base))
            }
        }
        TypeKind::Ref { base, is_mut } => {
            if *is_mut {
                format!("&mut {}", type_signature(base))
            } else {
                format!("&{}", type_signature(base))
            }
        }
        TypeKind::Fn { params, ret } => {
            let params_repr = params
                .iter()
                .map(type_signature)
                .collect::<Vec<_>>()
                .join(",");
            match ret {
                Some(ret) => format!("fn({})->{}", params_repr, type_signature(ret)),
                None => format!("fn({})", params_repr),
            }
        }
        TypeKind::Never => "!".to_string(),
        TypeKind::Infer => "_".to_string(),
        TypeKind::Dyn(bounds) => {
            let joined = bounds
                .iter()
                .map(trait_bound_signature)
                .collect::<Vec<_>>()
                .join("+");
            format!("dyn {}", joined)
        }
        TypeKind::ImplTrait(bounds) => {
            let joined = bounds
                .iter()
                .map(trait_bound_signature)
                .collect::<Vec<_>>()
                .join("+");
            format!("impl {}", joined)
        }
    }
}

fn param_signature(param: &Param) -> String {
    format!(
        "{}{}:{}",
        if param.is_mut { "mut " } else { "" },
        param.name.name,
        type_signature(&param.ty)
    )
}

fn self_param_signature(self_param: Option<SelfParam>) -> &'static str {
    match self_param {
        Some(SelfParam::Borrowed) => "&self",
        Some(SelfParam::BorrowedMut) => "&mut self",
        Some(SelfParam::Owned) => "self",
        Some(SelfParam::OwnedMut) => "mut self",
        None => "-",
    }
}

fn contract_signature(expr: Option<&Expr>) -> String {
    expr.map(|value| format!("{:?}", value.kind))
        .unwrap_or_else(|| "-".to_string())
}

fn function_signature(function: &Function) -> String {
    let type_params = function
        .type_params
        .iter()
        .map(|tp| {
            let mut repr = tp.name.name.clone();
            if !tp.bounds.is_empty() {
                let bounds = tp
                    .bounds
                    .iter()
                    .map(trait_bound_signature)
                    .collect::<Vec<_>>()
                    .join("+");
                repr.push(':');
                repr.push_str(&bounds);
            }
            if let Some(default) = &tp.default {
                repr.push('=');
                repr.push_str(&type_signature(default));
            }
            repr
        })
        .collect::<Vec<_>>()
        .join(",");
    let params = function
        .params
        .iter()
        .map(param_signature)
        .collect::<Vec<_>>()
        .join(",");
    let ret = function
        .return_type
        .as_ref()
        .map(type_signature)
        .unwrap_or_else(|| "unit".to_string());
    let abi = function.abi.as_deref().unwrap_or("-");
    let requires = contract_signature(function.precondition.as_deref());
    let ensures = contract_signature(function.postcondition.as_deref());
    format!(
        "{}|{}|abi={}|unsafe={}|async={}|no_mangle={}|export_name={}|self={}|tp=[{}]|params=[{}]|ret={}|requires={}|ensures={}",
        visibility_label(function.vis),
        function.name.name,
        abi,
        function.is_unsafe,
        function.is_async,
        function.no_mangle,
        function.export_name.as_deref().unwrap_or("-"),
        self_param_signature(function.self_param),
        type_params,
        params,
        ret,
        requires,
        ensures
    )
}

fn variant_field_signature(field: &VariantField) -> String {
    match field {
        VariantField::Named(name, ty) => format!("{}:{}", name.name, type_signature(ty)),
        VariantField::Unnamed(ty) => type_signature(ty),
    }
}

fn append_decl_interface_signature(out: &mut String, decl: &Decl) {
    match &decl.kind {
        DeclKind::Function(function) => {
            out.push_str("fn|");
            out.push_str(&function_signature(function));
            out.push('\n');
        }
        DeclKind::Struct(struct_decl) => {
            let fields = struct_decl
                .fields
                .iter()
                .map(|field| match &field.name {
                    Some(name) => format!(
                        "{}:{}:{}",
                        visibility_label(field.vis),
                        name.name,
                        type_signature(&field.ty)
                    ),
                    None => format!(
                        "{}:_:{}",
                        visibility_label(field.vis),
                        type_signature(&field.ty)
                    ),
                })
                .collect::<Vec<_>>()
                .join(";");
            out.push_str(&format!(
                "struct|{}|{}|tp={}|fields=[{}]\n",
                visibility_label(struct_decl.vis),
                struct_decl.name.name,
                struct_decl.type_params.len(),
                fields
            ));
        }
        DeclKind::Enum(enum_decl) => {
            let variants = enum_decl
                .variants
                .iter()
                .map(|variant| {
                    let fields = variant
                        .fields
                        .iter()
                        .map(variant_field_signature)
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{}({})", variant.name.name, fields)
                })
                .collect::<Vec<_>>()
                .join("|");
            out.push_str(&format!(
                "enum|{}|{}|tp={}|variants=[{}]\n",
                visibility_label(enum_decl.vis),
                enum_decl.name.name,
                enum_decl.type_params.len(),
                variants
            ));
        }
        DeclKind::Class(class_decl) => {
            let members = class_decl
                .members
                .iter()
                .map(|member| match member {
                    ClassMember::Field(field) => match &field.name {
                        Some(name) => format!("field:{}:{}", name.name, type_signature(&field.ty)),
                        None => format!("field:_:{}", type_signature(&field.ty)),
                    },
                    ClassMember::Method(function) => {
                        format!("method:{}", function_signature(function))
                    }
                })
                .collect::<Vec<_>>()
                .join(";");
            let extends = class_decl
                .extends
                .as_ref()
                .map(ast_path_signature)
                .unwrap_or_else(|| "-".to_string());
            let implements = class_decl
                .implements
                .iter()
                .map(trait_bound_signature)
                .collect::<Vec<_>>()
                .join("+");
            out.push_str(&format!(
                "class|{}|{}|tp={}|extends={}|impl={}|members=[{}]\n",
                visibility_label(class_decl.vis),
                class_decl.name.name,
                class_decl.type_params.len(),
                extends,
                implements,
                members
            ));
        }
        DeclKind::Trait(trait_decl) => {
            let bounds = trait_decl
                .bounds
                .iter()
                .map(trait_bound_signature)
                .collect::<Vec<_>>()
                .join("+");
            let items = trait_decl
                .items
                .iter()
                .map(|item| match item {
                    TraitItem::Function(function) => format!("fn:{}", function_signature(function)),
                    TraitItem::Const(const_decl) => {
                        format!(
                            "const:{}:{}",
                            const_decl.name.name,
                            type_signature(&const_decl.ty)
                        )
                    }
                    TraitItem::Type(alias) => {
                        format!("type:{}={}", alias.name.name, type_signature(&alias.ty))
                    }
                })
                .collect::<Vec<_>>()
                .join(";");
            out.push_str(&format!(
                "trait|{}|{}|tp={}|bounds={}|items=[{}]\n",
                visibility_label(trait_decl.vis),
                trait_decl.name.name,
                trait_decl.type_params.len(),
                bounds,
                items
            ));
        }
        DeclKind::Impl(impl_decl) => {
            let trait_path = impl_decl
                .trait_path
                .as_ref()
                .map(ast_path_signature)
                .unwrap_or_else(|| "-".to_string());
            let methods = impl_decl
                .items
                .iter()
                .map(function_signature)
                .collect::<Vec<_>>()
                .join(";");
            out.push_str(&format!(
                "impl|{}|target={}|trait={}|tp={}|methods=[{}]\n",
                visibility_label(impl_decl.vis),
                type_signature(&impl_decl.target_type),
                trait_path,
                impl_decl.type_params.len(),
                methods
            ));
        }
        DeclKind::TypeAlias(alias) => {
            out.push_str(&format!(
                "type|{}|{}={}\n",
                visibility_label(alias.vis),
                alias.name.name,
                type_signature(&alias.ty)
            ));
        }
        DeclKind::Const(const_decl) => {
            out.push_str(&format!(
                "const|{}|{}:{}\n",
                visibility_label(const_decl.vis),
                const_decl.name.name,
                type_signature(&const_decl.ty)
            ));
        }
        DeclKind::Static(static_decl) => {
            out.push_str(&format!(
                "static|{}|mut={}|{}:{}\n",
                visibility_label(static_decl.vis),
                static_decl.is_mut,
                static_decl.name.name,
                type_signature(&static_decl.ty)
            ));
        }
        DeclKind::Import(import_decl) => {
            let kind = match &import_decl.kind {
                ImportKind::Simple => "simple".to_string(),
                ImportKind::Wildcard => "wildcard".to_string(),
                ImportKind::Selective(names) => format!(
                    "selective:{}",
                    names
                        .iter()
                        .map(|ident| ident.name.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            };
            let alias = import_decl
                .alias
                .as_ref()
                .map(|ident| ident.name.as_str())
                .unwrap_or("-");
            out.push_str(&format!(
                "import|{}|kind={}|alias={}\n",
                ast_path_signature(&import_decl.path),
                kind,
                alias
            ));
        }
        DeclKind::ExternBlock(extern_block) => {
            out.push_str(&format!(
                "extern|abi={}|link={}\n",
                extern_block.abi,
                extern_block.link_name.as_deref().unwrap_or("-")
            ));
            for item in &extern_block.items {
                match item {
                    ExternItem::Function(func) => {
                        let params = func
                            .params
                            .iter()
                            .map(param_signature)
                            .collect::<Vec<_>>()
                            .join(",");
                        let ret = func
                            .return_type
                            .as_ref()
                            .map(type_signature)
                            .unwrap_or_else(|| "unit".to_string());
                        out.push_str(&format!(
                            "extern_fn|{}|unsafe={}|name={}|params=[{}]|ret={}\n",
                            visibility_label(func.vis),
                            func.is_unsafe,
                            func.name.name,
                            params,
                            ret
                        ));
                    }
                    ExternItem::Static(stat) => {
                        out.push_str(&format!(
                            "extern_static|{}|mut={}|{}:{}\n",
                            visibility_label(stat.vis),
                            stat.is_mut,
                            stat.name.name,
                            type_signature(&stat.ty)
                        ));
                    }
                }
            }
        }
        DeclKind::Module(module_decl) => {
            out.push_str(&format!(
                "module|{}|{}|items={}\n",
                visibility_label(module_decl.vis),
                module_decl.name.name,
                module_decl.items.len()
            ));
            for item in &module_decl.items {
                append_decl_interface_signature(out, item);
            }
        }
    }
}

fn interface_signature_from_program(program: &Program) -> String {
    let mut out = String::new();
    for decl in &program.decls {
        append_decl_interface_signature(&mut out, decl);
    }
    out
}

pub(crate) fn interface_fingerprint_from_program(program: &Program) -> u64 {
    source_fingerprint(&interface_signature_from_program(program))
}

pub(crate) fn ast_interface_signature(source: &str) -> Option<String> {
    let program = Parser::parse(source).ok()?;
    Some(interface_signature_from_program(&program))
}

fn source_span_slice<'a>(source: &'a str, span: Span) -> Option<&'a str> {
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

fn infer_expr_type_tag(expr: &Expr) -> String {
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
                format!("array<{}>", infer_expr_type_tag(first))
            } else {
                "array<_>".to_string()
            }
        }
        ExprKind::Tuple(items) => format!("tuple{}", items.len()),
        ExprKind::Struct { path, .. } => format!("struct:{}", ast_path_signature(path)),
        ExprKind::Path(path) => format!("path:{}", ast_path_signature(path)),
        ExprKind::Ident(_) => "_".to_string(),
        _ => "_".to_string(),
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

fn push_instance_if_generic_call(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    call_name: &str,
    args: &[Expr],
    simple_to_symbol: &HashMap<String, Option<String>>,
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
        .map(infer_expr_type_tag)
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

fn collect_generic_instances_in_expr(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    expr: &Expr,
    simple_to_symbol: &HashMap<String, Option<String>>,
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
                simple_to_symbol,
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
                simple_to_symbol,
                callable_meta,
            );
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                right,
                simple_to_symbol,
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
                    simple_to_symbol,
                    callable_meta,
                );
            }
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                func,
                simple_to_symbol,
                callable_meta,
            );
            for arg in args {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    arg,
                    simple_to_symbol,
                    callable_meta,
                );
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                receiver,
                simple_to_symbol,
                callable_meta,
            );
            for arg in args {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    arg,
                    simple_to_symbol,
                    callable_meta,
                );
            }
        }
        ExprKind::Block(block)
        | ExprKind::Loop(block)
        | ExprKind::AsyncBlock(block)
        | ExprKind::ParallelBlock(block) => {
            for stmt in &block.stmts {
                collect_generic_instances_in_stmt(
                    out,
                    seen,
                    module_path,
                    stmt,
                    simple_to_symbol,
                    callable_meta,
                );
            }
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
                simple_to_symbol,
                callable_meta,
            );
            for stmt in &then_branch.stmts {
                collect_generic_instances_in_stmt(
                    out,
                    seen,
                    module_path,
                    stmt,
                    simple_to_symbol,
                    callable_meta,
                );
            }
            if let Some(else_expr) = else_branch.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    else_expr,
                    simple_to_symbol,
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
                simple_to_symbol,
                callable_meta,
            );
            for stmt in &body.stmts {
                collect_generic_instances_in_stmt(
                    out,
                    seen,
                    module_path,
                    stmt,
                    simple_to_symbol,
                    callable_meta,
                );
            }
        }
        ExprKind::For { iter, body, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                iter,
                simple_to_symbol,
                callable_meta,
            );
            for stmt in &body.stmts {
                collect_generic_instances_in_stmt(
                    out,
                    seen,
                    module_path,
                    stmt,
                    simple_to_symbol,
                    callable_meta,
                );
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                scrutinee,
                simple_to_symbol,
                callable_meta,
            );
            for arm in arms {
                if let Some(guard) = arm.guard.as_deref() {
                    collect_generic_instances_in_expr(
                        out,
                        seen,
                        module_path,
                        guard,
                        simple_to_symbol,
                        callable_meta,
                    );
                }
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    &arm.body,
                    simple_to_symbol,
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
                    simple_to_symbol,
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
                simple_to_symbol,
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
                    simple_to_symbol,
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
                    simple_to_symbol,
                    callable_meta,
                );
            }
            if let Some(base) = base.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    base,
                    simple_to_symbol,
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
                    simple_to_symbol,
                    callable_meta,
                );
            }
            if let Some(end) = end.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    end,
                    simple_to_symbol,
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
                simple_to_symbol,
                callable_meta,
            );
        }
        ExprKind::Cast { expr, .. } | ExprKind::Is { expr, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                expr,
                simple_to_symbol,
                callable_meta,
            );
        }
    }
}

fn collect_generic_instances_in_stmt(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    stmt: &Stmt,
    simple_to_symbol: &HashMap<String, Option<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    match &stmt.kind {
        StmtKind::Let {
            value: Some(value), ..
        } => collect_generic_instances_in_expr(
            out,
            seen,
            module_path,
            value,
            simple_to_symbol,
            callable_meta,
        ),
        StmtKind::Const { value, .. } => collect_generic_instances_in_expr(
            out,
            seen,
            module_path,
            value,
            simple_to_symbol,
            callable_meta,
        ),
        StmtKind::Expr(expr) => collect_generic_instances_in_expr(
            out,
            seen,
            module_path,
            expr,
            simple_to_symbol,
            callable_meta,
        ),
        StmtKind::Item(_) | StmtKind::Let { value: None, .. } => {}
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

    let callable_meta = items
        .iter()
        .filter_map(|item| {
            if item.kind != "function" {
                return None;
            }
            Some((
                item.symbol.clone(),
                GenericCallableMeta {
                    stable_item_id: item.stable_item_id.clone(),
                    module_id: item.module_id.clone(),
                    interface_hash: item.interface_hash,
                    body_hash: item.body_hash,
                    type_param_count: item.type_param_count as usize,
                },
            ))
        })
        .collect::<HashMap<_, _>>();

    let mut simple_to_symbol = HashMap::<String, Option<String>>::new();
    for symbol in callable_meta.keys() {
        let simple = symbol.rsplit("::").next().unwrap_or_default().to_string();
        match simple_to_symbol.get_mut(&simple) {
            Some(entry) => *entry = None,
            None => {
                simple_to_symbol.insert(simple, Some(symbol.clone()));
            }
        }
    }

    let mut instances = Vec::<GenericInstanceFingerprint>::new();
    let mut seen_instances = HashSet::<String>::new();
    for decl in &program.decls {
        match &decl.kind {
            DeclKind::Function(function) => {
                for stmt in &function.body.stmts {
                    collect_generic_instances_in_stmt(
                        &mut instances,
                        &mut seen_instances,
                        module_path,
                        stmt,
                        &simple_to_symbol,
                        &callable_meta,
                    );
                }
            }
            DeclKind::Const(const_decl) => {
                collect_generic_instances_in_expr(
                    &mut instances,
                    &mut seen_instances,
                    module_path,
                    &const_decl.value,
                    &simple_to_symbol,
                    &callable_meta,
                );
            }
            DeclKind::Static(static_decl) => {
                collect_generic_instances_in_expr(
                    &mut instances,
                    &mut seen_instances,
                    module_path,
                    &static_decl.value,
                    &simple_to_symbol,
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
