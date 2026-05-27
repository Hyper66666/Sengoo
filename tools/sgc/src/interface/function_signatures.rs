use crate::FunctionSignatureInfo;
use sengoo_compiler::{ClassMember, Decl, DeclKind, Function, Parser, TraitItem};

use super::function_symbol;
use super::signature::{function_signature, type_signature};

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
