use crate::source_fingerprint;
use sengoo_compiler::{
    ClassMember, Decl, DeclKind, Expr, ExternItem, Function, ImportKind, Param, Parser,
    Path as AstPath, Program, SelfParam, TraitBound, TraitItem, Type, TypeKind, VariantField,
    Visibility,
};

fn visibility_label(vis: Visibility) -> &'static str {
    match vis {
        Visibility::Public => "pub",
        Visibility::Private => "priv",
    }
}

pub(super) fn ast_path_signature(path: &AstPath) -> String {
    if path.segments.is_empty() {
        return "<empty>".to_string();
    }
    path.segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn trait_bound_signature(bound: &TraitBound) -> String {
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

pub(super) fn type_signature(ty: &Type) -> String {
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

pub(super) fn function_signature(function: &Function) -> String {
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
