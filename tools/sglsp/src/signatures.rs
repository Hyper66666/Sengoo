use sengoo_compiler::ast::{Decl, DeclKind, Function, SelfParam, Type};
use sengoo_compiler::Parser as SgParser;
use tower_lsp::lsp_types::Range;

use super::semantic::is_identifier_byte;
use super::text_editing::{clamp_to_char_boundary, span_to_range};

#[derive(Debug, Clone)]
pub(super) struct FunctionSignatureInfo {
    pub(super) name: String,
    pub(super) label: String,
    pub(super) params: Vec<String>,
    pub(super) range: Range,
}

fn span_text(content: &str, lo: u32, hi: u32) -> String {
    let len = content.len();
    let mut start = (lo as usize).min(len);
    let mut end = (hi as usize).min(len);

    start = clamp_to_char_boundary(content, start);
    end = clamp_to_char_boundary(content, end);
    if end < start {
        std::mem::swap(&mut start, &mut end);
    }

    content[start..end].trim().to_string()
}

fn type_snippet(content: &str, ty: &Type) -> String {
    let text = span_text(content, ty.span.lo, ty.span.hi);
    if text.is_empty() {
        "_".to_string()
    } else {
        text
    }
}

fn self_param_snippet(self_param: SelfParam) -> &'static str {
    match self_param {
        SelfParam::Borrowed => "self",
        SelfParam::BorrowedMut => "mut self",
        SelfParam::Owned => "self",
        SelfParam::OwnedMut => "mut self",
    }
}

fn function_signature_label(content: &str, function: &Function) -> (String, Vec<String>) {
    let mut params = Vec::new();
    if let Some(self_param) = function.self_param {
        params.push(self_param_snippet(self_param).to_string());
    }

    for param in &function.params {
        params.push(format!(
            "{}: {}",
            param.name.name,
            type_snippet(content, &param.ty)
        ));
    }

    let generic_suffix = if function.type_params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            function
                .type_params
                .iter()
                .map(|tp| tp.name.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let ret = function
        .return_type
        .as_ref()
        .map(|ty| type_snippet(content, ty))
        .unwrap_or_else(|| "unit".to_string());

    let async_prefix = if function.is_async { "async " } else { "" };
    (
        format!(
            "{}def {}{}({}) -> {}",
            async_prefix,
            function.name.name,
            generic_suffix,
            params.join(", "),
            ret
        ),
        params,
    )
}

fn collect_function_signatures_from_decl(
    content: &str,
    decl: &Decl,
    out: &mut Vec<FunctionSignatureInfo>,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            let (label, params) = function_signature_label(content, function);
            out.push(FunctionSignatureInfo {
                name: function.name.name.clone(),
                label,
                params,
                range: span_to_range(content, function.name.span.lo, function.name.span.hi),
            });
        }
        DeclKind::Module(module_decl) => {
            for nested in &module_decl.items {
                collect_function_signatures_from_decl(content, nested, out);
            }
        }
        DeclKind::Impl(impl_decl) => {
            let target = span_text(
                content,
                impl_decl.target_type.span.lo,
                impl_decl.target_type.span.hi,
            );
            let target = if target.is_empty() {
                "_".to_string()
            } else {
                target
            };
            for method in &impl_decl.items {
                let (base_label, params) = function_signature_label(content, method);
                out.push(FunctionSignatureInfo {
                    name: method.name.name.clone(),
                    label: format!("{} [impl {}]", base_label, target),
                    params,
                    range: span_to_range(content, method.name.span.lo, method.name.span.hi),
                });
            }
        }
        _ => {}
    }
}

pub(super) fn collect_function_signatures(content: &str) -> Vec<FunctionSignatureInfo> {
    let Ok(program) = SgParser::parse(content) else {
        return Vec::new();
    };

    let mut signatures = Vec::new();
    for decl in &program.decls {
        collect_function_signatures_from_decl(content, decl, &mut signatures);
    }
    signatures
}

pub(super) fn active_call_site(content: &str, offset: usize) -> Option<(String, u32)> {
    let bytes = content.as_bytes();
    let limit = offset.min(bytes.len());
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0usize;

    while i < limit {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < limit {
                    if bytes[i] == b'\\' && i + 1 < limit {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < limit && bytes[i + 1] == b'/' => {
                while i < limit && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'(' => stack.push(i),
            b')' => {
                stack.pop();
            }
            _ => {}
        }
        i += 1;
    }

    let open = *stack.last()?;

    let mut end = open;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }

    let mut start = end;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    if start == end {
        return None;
    }

    let name = content[start..end].to_string();
    let mut nested_depth = 0u32;
    let mut active_param = 0u32;
    let mut j = open + 1;

    while j < limit {
        match bytes[j] {
            b'"' => {
                j += 1;
                while j < limit {
                    if bytes[j] == b'\\' && j + 1 < limit {
                        j += 2;
                        continue;
                    }
                    if bytes[j] == b'"' {
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                continue;
            }
            b'/' if j + 1 < limit && bytes[j + 1] == b'/' => {
                while j < limit && bytes[j] != b'\n' {
                    j += 1;
                }
                continue;
            }
            b'(' => nested_depth += 1,
            b')' => {
                if nested_depth == 0 {
                    break;
                }
                nested_depth -= 1;
            }
            b',' if nested_depth == 0 => active_param += 1,
            _ => {}
        }

        j += 1;
    }

    Some((name, active_param))
}
