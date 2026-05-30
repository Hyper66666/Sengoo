use sengoo_compiler::{Expr, ExprKind};
use std::collections::HashMap;

use super::super::generic_items::GenericCallableMeta;
use super::super::signature::ast_path_signature;

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

pub(super) fn extract_impl_receiver_template(symbol: &str) -> Option<String> {
    let marker = "::impl::";
    let start = symbol.find(marker)? + marker.len();
    let tail = &symbol[start..];
    let method_sep = tail.rfind("::")?;
    Some(tail[..method_sep].to_string())
}

pub(super) fn infer_expr_type_signature(
    expr: &Expr,
    local_types: &HashMap<String, String>,
) -> String {
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

pub(super) fn substitute_type_signature(template: &str, subst: &HashMap<String, String>) -> String {
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

pub(super) fn unify_type_signature_template(
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

pub(super) fn type_param_substitution(
    meta: &GenericCallableMeta,
    canonical_type_args: &[String],
) -> HashMap<String, String> {
    meta.type_param_names
        .iter()
        .cloned()
        .zip(canonical_type_args.iter().cloned())
        .collect()
}
