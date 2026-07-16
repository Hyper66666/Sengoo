use sengoo_compiler::ast::{
    ClassMember, Decl, DeclKind, Function, Path, SelfParam, TraitBound, TraitItem, Type, TypeKind,
};
use sengoo_compiler::Parser as SgParser;
use tower_lsp::lsp_types::{
    Documentation, ParameterInformation, ParameterLabel, Position, Range, SignatureHelp,
    SignatureInformation, Url,
};

use super::completion::signature_receiver_type;
use super::semantic::is_identifier_byte;
use super::text_editing::{clamp_to_char_boundary, position_to_byte_index, span_to_range};
use super::workspace_index::WorkspaceIndex;

#[derive(Debug, Clone)]
pub(super) struct FunctionSignatureInfo {
    pub(super) name: String,
    pub(super) label: String,
    pub(super) params: Vec<String>,
    pub(super) param_types: Vec<String>,
    pub(super) module_path: Option<String>,
    pub(super) qualified_owner: Option<String>,
    pub(super) declared_module_path: Option<String>,
    pub(super) declared_owner: Option<String>,
    pub(super) has_receiver: bool,
    pub(super) documentation: Option<String>,
    pub(super) parameter_documentation: Vec<Option<String>>,
    pub(super) range: Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallSeparator {
    Member,
    Namespace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActiveCallSite {
    pub(super) callee: String,
    pub(super) qualifier: Option<String>,
    pub(super) separator: Option<CallSeparator>,
    pub(super) argument_index: u32,
    pub(super) arguments: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct SignatureSelection {
    pub(super) signatures: Vec<FunctionSignatureInfo>,
    pub(super) active_signature: usize,
    pub(super) active_parameter: u32,
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

fn path_label(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

fn trait_bound_label(bound: &TraitBound) -> String {
    if bound.params.is_empty() {
        return path_label(&bound.path);
    }

    format!(
        "{}<{}>",
        path_label(&bound.path),
        bound
            .params
            .iter()
            .map(type_snippet)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn type_snippet(ty: &Type) -> String {
    match &ty.kind {
        TypeKind::SelfType => "Self".to_string(),
        TypeKind::Path(path) => path_label(path),
        TypeKind::PathWithArgs { path, args } => format!(
            "{}<{}>",
            path_label(path),
            args.iter().map(type_snippet).collect::<Vec<_>>().join(", ")
        ),
        TypeKind::Tuple(types) if types.is_empty() => "unit".to_string(),
        TypeKind::Tuple(types) => format!(
            "({})",
            types
                .iter()
                .map(type_snippet)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeKind::Array(elem, len) => format!("[{}; {}]", type_snippet(elem), len),
        TypeKind::Slice(elem) => format!("[{}]", type_snippet(elem)),
        TypeKind::Ptr { base, is_mut } => {
            if *is_mut {
                format!("*mut {}", type_snippet(base))
            } else {
                format!("*const {}", type_snippet(base))
            }
        }
        TypeKind::Ref { base, is_mut } => {
            if *is_mut {
                format!("&mut {}", type_snippet(base))
            } else {
                format!("&{}", type_snippet(base))
            }
        }
        TypeKind::Fn { params, ret } => {
            let params = params
                .iter()
                .map(type_snippet)
                .collect::<Vec<_>>()
                .join(", ");
            let ret = ret
                .as_ref()
                .map(|ty| type_snippet(ty))
                .unwrap_or_else(|| "unit".to_string());
            format!("fn({params}) -> {ret}")
        }
        TypeKind::Never => "!".to_string(),
        TypeKind::Infer => "_".to_string(),
        TypeKind::Dyn(bounds) => format!(
            "dyn {}",
            bounds
                .iter()
                .map(trait_bound_label)
                .collect::<Vec<_>>()
                .join(" + ")
        ),
        TypeKind::ImplTrait(bounds) => format!(
            "impl {}",
            bounds
                .iter()
                .map(trait_bound_label)
                .collect::<Vec<_>>()
                .join(" + ")
        ),
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

fn function_signature_label(function: &Function) -> (String, Vec<String>, Vec<String>, bool) {
    let mut display_params = Vec::new();
    if let Some(self_param) = function.self_param {
        display_params.push(self_param_snippet(self_param).to_string());
    }

    let mut params = Vec::new();
    let mut param_types = Vec::new();
    for param in &function.params {
        let ty = type_snippet(&param.ty);
        params.push(format!("{}: {}", param.name.name, ty));
        param_types.push(ty);
    }
    display_params.extend(params.iter().cloned());

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
        .map(type_snippet)
        .unwrap_or_else(|| "unit".to_string());

    let async_prefix = if function.is_async { "async " } else { "" };
    (
        format!(
            "{}def {}{}({}) -> {}",
            async_prefix,
            function.name.name,
            generic_suffix,
            display_params.join(", "),
            ret
        ),
        params,
        param_types,
        function.self_param.is_some(),
    )
}

fn function_documentation(
    content: &str,
    function: &Function,
) -> (Option<String>, Vec<Option<String>>) {
    let line = span_to_range(content, function.name.span.lo, function.name.span.hi)
        .start
        .line as usize;
    let lines = content.lines().collect::<Vec<_>>();
    let mut cursor = line;
    let mut parts = Vec::new();
    while cursor > 0 {
        let previous = lines[cursor - 1].trim_start();
        if parts.is_empty() && previous.starts_with("#[") {
            cursor -= 1;
            continue;
        }
        let Some(doc) = previous.strip_prefix("///") else {
            break;
        };
        parts.push(doc.trim().to_string());
        cursor -= 1;
    }
    parts.reverse();

    let mut param_docs = std::collections::HashMap::<String, String>::new();
    let mut callable = Vec::new();
    for part in parts {
        if let Some(rest) = part.strip_prefix("@param ") {
            let (name, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            param_docs.insert(name.to_string(), text.trim().to_string());
        } else if !part.is_empty() {
            callable.push(part);
        }
    }
    let parameter_documentation = function
        .params
        .iter()
        .map(|param| param_docs.get(&param.name.name).cloned())
        .collect();
    (
        (!callable.is_empty()).then(|| callable.join("\n")),
        parameter_documentation,
    )
}

fn collect_function_signatures_from_decl(
    content: &str,
    decl: &Decl,
    module_path: Option<String>,
    out: &mut Vec<FunctionSignatureInfo>,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            let (label, params, param_types, has_receiver) = function_signature_label(function);
            let (documentation, parameter_documentation) =
                function_documentation(content, function);
            out.push(FunctionSignatureInfo {
                name: function.name.name.clone(),
                label,
                params,
                param_types,
                module_path: None,
                qualified_owner: None,
                declared_module_path: module_path,
                declared_owner: None,
                has_receiver,
                documentation,
                parameter_documentation,
                range: span_to_range(content, function.name.span.lo, function.name.span.hi),
            });
        }
        DeclKind::Module(module_decl) => {
            let nested_module = Some(match module_path {
                Some(module_path) => format!("{module_path}::{}", module_decl.name.name),
                None => module_decl.name.name.clone(),
            });
            for nested in &module_decl.items {
                collect_function_signatures_from_decl(content, nested, nested_module.clone(), out);
            }
        }
        DeclKind::Impl(impl_decl) => {
            let target = span_text(
                content,
                impl_decl.target_type.span.lo,
                impl_decl.target_type.span.hi,
            );
            let target = target.trim_end_matches('{').trim().to_string();
            let target = if target.is_empty() {
                "_".to_string()
            } else {
                target
            };
            for method in &impl_decl.items {
                let (base_label, params, param_types, has_receiver) =
                    function_signature_label(method);
                let (documentation, parameter_documentation) =
                    function_documentation(content, method);
                out.push(FunctionSignatureInfo {
                    name: method.name.name.clone(),
                    label: format!("{} [impl {}]", base_label, target),
                    params,
                    param_types,
                    module_path: None,
                    qualified_owner: None,
                    declared_module_path: module_path.clone(),
                    declared_owner: Some(target.clone()),
                    has_receiver,
                    documentation,
                    parameter_documentation,
                    range: span_to_range(content, method.name.span.lo, method.name.span.hi),
                });
            }
        }
        DeclKind::Class(class_decl) => {
            for member in &class_decl.members {
                if let ClassMember::Method(method) = member {
                    push_method_signature(
                        content,
                        method,
                        module_path.clone(),
                        &class_decl.name.name,
                        out,
                    );
                }
            }
        }
        DeclKind::Trait(trait_decl) => {
            for item in &trait_decl.items {
                if let TraitItem::Function(method) = item {
                    push_method_signature(
                        content,
                        method,
                        module_path.clone(),
                        &trait_decl.name.name,
                        out,
                    );
                }
            }
        }
        _ => {}
    }
}

fn push_method_signature(
    content: &str,
    method: &Function,
    module_path: Option<String>,
    owner: &str,
    out: &mut Vec<FunctionSignatureInfo>,
) {
    let (label, params, param_types, has_receiver) = function_signature_label(method);
    let (documentation, parameter_documentation) = function_documentation(content, method);
    out.push(FunctionSignatureInfo {
        name: method.name.name.clone(),
        label,
        params,
        param_types,
        module_path: None,
        qualified_owner: None,
        declared_module_path: module_path,
        declared_owner: Some(owner.to_string()),
        has_receiver,
        documentation,
        parameter_documentation,
        range: span_to_range(content, method.name.span.lo, method.name.span.hi),
    });
}

pub(super) fn qualify_function_signatures(
    signatures: &mut [FunctionSignatureInfo],
    canonical_module_path: &str,
) {
    for signature in signatures {
        let module_path = match &signature.declared_module_path {
            Some(declared) if !canonical_module_path.is_empty() => {
                format!("{canonical_module_path}::{declared}")
            }
            Some(declared) => declared.clone(),
            None => canonical_module_path.to_string(),
        };
        signature.module_path = (!module_path.is_empty()).then_some(module_path.clone());
        signature.qualified_owner = signature.declared_owner.as_ref().map(|owner| {
            let owner = owner
                .trim_start_matches('&')
                .trim_start_matches("mut ")
                .split('<')
                .next()
                .unwrap_or(owner)
                .trim();
            if module_path.is_empty() {
                owner.to_string()
            } else {
                format!("{module_path}::{owner}")
            }
        });
    }
}

pub(super) fn collect_function_signatures(content: &str) -> Vec<FunctionSignatureInfo> {
    let parse_source = content
        .split_inclusive('\n')
        .map(|line| {
            if line.trim_start().starts_with("#[") {
                line.chars()
                    .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
                    .collect::<String>()
            } else {
                line.to_string()
            }
        })
        .collect::<String>();
    let Ok(program) = SgParser::parse(&parse_source) else {
        return Vec::new();
    };

    let mut signatures = Vec::new();
    for decl in &program.decls {
        collect_function_signatures_from_decl(content, decl, None, &mut signatures);
    }
    signatures
}

fn identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn previous_char(content: &str, offset: usize) -> Option<(usize, char)> {
    content.get(..offset)?.char_indices().next_back()
}

fn callable_before(
    content: &str,
    open: usize,
) -> Option<(String, Option<String>, Option<CallSeparator>)> {
    let mut end = open;
    while let Some((index, ch)) = previous_char(content, end) {
        if !ch.is_whitespace() {
            break;
        }
        end = index;
    }
    if content.get(..end)?.ends_with('>') {
        let mut depth = 0u32;
        let mut generic_start = None;
        for (index, ch) in content[..end].char_indices().rev() {
            match ch {
                '>' => depth += 1,
                '<' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        generic_start = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(start) = generic_start {
            end = start;
            if content.get(..end)?.ends_with("::") {
                end -= 2;
            }
        }
    }
    let mut start = end;
    while let Some((index, ch)) = previous_char(content, start) {
        if identifier_char(ch) || matches!(ch, '.' | ':' | '(' | ')') {
            start = index;
        } else {
            break;
        }
    }
    let expression = content.get(start..end)?.trim_matches(':').trim_matches('.');
    if expression.is_empty() {
        return None;
    }
    if let Some((qualifier, callee)) = expression.rsplit_once("::") {
        return identifier_char(callee.chars().next()?).then(|| {
            (
                callee.to_string(),
                Some(qualifier.to_string()),
                Some(CallSeparator::Namespace),
            )
        });
    }
    if let Some((qualifier, callee)) = expression.rsplit_once('.') {
        return identifier_char(callee.chars().next()?).then(|| {
            (
                callee.to_string(),
                Some(qualifier.to_string()),
                Some(CallSeparator::Member),
            )
        });
    }
    expression
        .chars()
        .all(identifier_char)
        .then(|| (expression.to_string(), None, None))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanDelimiter {
    Paren,
    Bracket,
    Brace,
    Generic,
}

fn skip_non_code(bytes: &[u8], mut i: usize, limit: usize) -> Option<usize> {
    if bytes.get(i..i + 3) == Some(b"\"\"\"") {
        i += 3;
        while i + 2 < limit {
            if bytes.get(i..i + 3) == Some(b"\"\"\"") {
                return Some(i + 3);
            }
            i += 1;
        }
        return Some(limit);
    }
    if bytes[i] == b'"' || bytes[i] == b'\'' {
        let quote = bytes[i];
        i += 1;
        while i < limit {
            if bytes[i] == b'\\' && i + 1 < limit {
                i += 2;
            } else if bytes[i] == quote {
                return Some(i + 1);
            } else {
                i += 1;
            }
        }
        return Some(limit);
    }
    if bytes[i] == b'/' && i + 1 < limit && bytes[i + 1] == b'/' {
        return Some(
            bytes[i..limit]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(limit, |n| i + n),
        );
    }
    if bytes[i] == b'/' && i + 1 < limit && bytes[i + 1] == b'*' {
        let mut depth = 1u32;
        i += 2;
        while i + 1 < limit {
            if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                depth += 1;
                i += 2;
            } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return Some(i);
                }
            } else {
                i += 1;
            }
        }
        return Some(limit);
    }
    None
}

fn looks_like_generic_open(bytes: &[u8], index: usize, limit: usize) -> bool {
    if index == 0 || index + 1 >= limit || bytes[index + 1] == b'=' {
        return false;
    }
    if index >= 2 && bytes[index - 2..index] == *b"::" {
        return true;
    }
    let mut depth = 1u32;
    let mut cursor = index + 1;
    let mut close = None;
    while cursor < limit {
        match bytes[cursor] {
            b'<' if bytes.get(cursor + 1) != Some(&b'=') => depth += 1,
            b'>' if bytes.get(cursor + 1) != Some(&b'=') => {
                depth -= 1;
                if depth == 0 {
                    close = Some(cursor);
                    break;
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    let Some(close) = close else {
        return false;
    };
    let mut after = close + 1;
    while after < limit && bytes[after].is_ascii_whitespace() {
        after += 1;
    }
    if after < limit && matches!(bytes[after], b'{' | b':' | b'(' | b'[' | b'.' | b'>') {
        return true;
    }

    let mut name_start = index;
    while name_start > 0 && is_identifier_byte(bytes[name_start - 1]) {
        name_start -= 1;
    }
    let mut before = name_start;
    while before > 0 && bytes[before - 1].is_ascii_whitespace() {
        before -= 1;
    }
    if before > 0 && bytes[before - 1] == b':' {
        return before < 2 || bytes[before - 2] != b':';
    }
    let prefix = &bytes[..before];
    prefix.ends_with(b"->")
        || prefix
            .strip_suffix(b"as")
            .is_some_and(|head| head.last().is_none_or(|byte| !is_identifier_byte(*byte)))
}

pub(super) fn active_call_site(content: &str, offset: usize) -> Option<ActiveCallSite> {
    let bytes = content.as_bytes();
    let limit = offset.min(bytes.len());
    let mut stack: Vec<(ScanDelimiter, usize)> = Vec::new();
    let mut i = 0usize;

    while i < limit {
        if let Some(next) = skip_non_code(bytes, i, limit) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'(' => stack.push((ScanDelimiter::Paren, i)),
            b'[' => stack.push((ScanDelimiter::Bracket, i)),
            b'{' => stack.push((ScanDelimiter::Brace, i)),
            b'<' if looks_like_generic_open(bytes, i, limit) => {
                stack.push((ScanDelimiter::Generic, i));
            }
            b')' => pop_matching(&mut stack, ScanDelimiter::Paren),
            b']' => pop_matching(&mut stack, ScanDelimiter::Bracket),
            b'}' => pop_matching(&mut stack, ScanDelimiter::Brace),
            b'>' => pop_matching(&mut stack, ScanDelimiter::Generic),
            _ => {}
        }
        i += 1;
    }

    let open = stack.iter().rev().find_map(|(kind, open)| {
        (*kind == ScanDelimiter::Paren && callable_before(content, *open).is_some())
            .then_some(*open)
    })?;
    let (callee, qualifier, separator) = callable_before(content, open)?;
    let mut nested = Vec::<ScanDelimiter>::new();
    let mut active_param = 0u32;
    let mut argument_start = open + 1;
    let mut arguments = Vec::new();
    let mut j = open + 1;

    while j < limit {
        if let Some(next) = skip_non_code(bytes, j, limit) {
            j = next;
            continue;
        }
        match bytes[j] {
            b'(' => nested.push(ScanDelimiter::Paren),
            b'[' => nested.push(ScanDelimiter::Bracket),
            b'{' => nested.push(ScanDelimiter::Brace),
            b'<' if looks_like_generic_open(bytes, j, limit) => nested.push(ScanDelimiter::Generic),
            b')' => {
                if nested.is_empty() {
                    break;
                }
                if nested.last() == Some(&ScanDelimiter::Paren) {
                    nested.pop();
                }
            }
            b']' if nested.last() == Some(&ScanDelimiter::Bracket) => {
                nested.pop();
            }
            b'}' if nested.last() == Some(&ScanDelimiter::Brace) => {
                nested.pop();
            }
            b'>' if nested.last() == Some(&ScanDelimiter::Generic) => {
                nested.pop();
            }
            b',' if nested.is_empty() => {
                arguments.push(content[argument_start..j].trim().to_string());
                argument_start = j + 1;
                active_param += 1;
            }
            _ => {}
        }

        j += 1;
    }
    arguments.push(content[argument_start..limit].trim().to_string());
    Some(ActiveCallSite {
        callee,
        qualifier,
        separator,
        argument_index: active_param,
        arguments,
    })
}

fn pop_matching(stack: &mut Vec<(ScanDelimiter, usize)>, expected: ScanDelimiter) {
    if stack.last().map(|(kind, _)| *kind) == Some(expected) {
        stack.pop();
    }
}

pub(super) fn select_signature_help(
    call: &ActiveCallSite,
    candidates: &[FunctionSignatureInfo],
    qualified_target: Option<&str>,
) -> Option<SignatureSelection> {
    let required_owner = match call.separator {
        Some(CallSeparator::Member | CallSeparator::Namespace) => Some(qualified_target?),
        None => None,
    };
    let mut signatures = candidates
        .iter()
        .filter(|signature| {
            if signature.name != call.callee {
                return false;
            }
            match call.separator {
                Some(CallSeparator::Member) => {
                    signature.has_receiver && signature.qualified_owner.as_deref() == required_owner
                }
                Some(CallSeparator::Namespace) => {
                    signature.module_path.as_deref() == required_owner
                        || signature.qualified_owner.as_deref() == required_owner
                }
                None => signature.declared_owner.is_none() && !signature.has_receiver,
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    signatures.sort_by(|left, right| {
        left.params
            .len()
            .cmp(&right.params.len())
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.range.start.line.cmp(&right.range.start.line))
    });
    if signatures.is_empty() {
        return None;
    }
    let supplied_arguments = call
        .arguments
        .iter()
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>();
    let supplied = supplied_arguments.len();
    let active_signature = signatures
        .iter()
        .enumerate()
        .min_by_key(|(index, signature)| {
            let type_mismatches = supplied_arguments
                .iter()
                .zip(&signature.param_types)
                .filter(|(argument, expected)| {
                    infer_argument_type(argument)
                        .is_some_and(|actual| !type_matches(actual, expected))
                })
                .count();
            (
                type_mismatches,
                signature.params.len().abs_diff(supplied),
                signature.params.len() < supplied,
                *index,
            )
        })
        .map(|(index, _)| index)
        .unwrap_or(0);
    let count = signatures[active_signature].params.len();
    let active_parameter = if count == 0 {
        0
    } else {
        call.argument_index.min((count - 1) as u32)
    };
    Some(SignatureSelection {
        signatures,
        active_signature,
        active_parameter,
    })
}

fn infer_argument_type(argument: &str) -> Option<&'static str> {
    let argument = argument.trim();
    if argument.starts_with('"') {
        Some("&str")
    } else if argument.starts_with('\'') {
        Some("char")
    } else if matches!(argument, "true" | "false") {
        Some("bool")
    } else if argument.parse::<i128>().is_ok() {
        Some("integer")
    } else if argument.parse::<f64>().is_ok() {
        Some("float")
    } else {
        None
    }
}

fn type_matches(actual: &str, expected: &str) -> bool {
    match actual {
        "integer" => matches!(
            expected,
            "i8" | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
        ),
        "float" => matches!(expected, "f32" | "f64"),
        other => other == expected,
    }
}

pub(crate) fn signature_help_for_request(
    index: &WorkspaceIndex,
    uri: &Url,
    content: &str,
    position: Position,
) -> Option<SignatureHelp> {
    let call = active_call_site(content, position_to_byte_index(content, position)?)?;
    let mut candidates = if call.separator.is_none() {
        index.visible_unqualified_signature_candidates(uri)
    } else {
        index.signature_candidates(uri)
    };
    if call.callee == "print" && !candidates.iter().any(|signature| signature.name == "print") {
        candidates.push(FunctionSignatureInfo {
            name: "print".into(),
            label: "def print(value: Any) -> unit".into(),
            params: vec!["value: Any".into()],
            param_types: vec!["Any".into()],
            module_path: None,
            qualified_owner: None,
            declared_module_path: None,
            declared_owner: None,
            has_receiver: false,
            documentation: Some("Writes a value to standard output.".into()),
            parameter_documentation: vec![Some("Value to write.".into())],
            range: Range::default(),
        });
    }
    let target = match call.separator {
        Some(CallSeparator::Member) => call.qualifier.as_deref().and_then(|expression| {
            signature_receiver_type(index, uri, content, position, expression, &candidates)
        }),
        Some(CallSeparator::Namespace) => call.qualifier.as_deref().and_then(|qualifier| {
            index.canonical_signature_qualifier_from(uri, qualifier, &candidates)
        }),
        None => None,
    };
    let selection = select_signature_help(&call, &candidates, target.as_deref())?;
    Some(SignatureHelp {
        signatures: selection
            .signatures
            .iter()
            .map(|signature| SignatureInformation {
                label: signature.label.clone(),
                documentation: signature.documentation.clone().map(Documentation::String),
                parameters: Some(
                    signature
                        .params
                        .iter()
                        .zip(&signature.parameter_documentation)
                        .map(|(parameter, documentation)| ParameterInformation {
                            label: ParameterLabel::Simple(parameter.clone()),
                            documentation: documentation.clone().map(Documentation::String),
                        })
                        .collect(),
                ),
                active_parameter: None,
            })
            .collect(),
        active_signature: Some(selection.active_signature as u32),
        active_parameter: Some(selection.active_parameter),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sengoo_compiler::ast::TraitItem;

    #[test]
    fn signature_type_snippet_preserves_self_type() {
        let program = SgParser::parse("trait CloneLike { def clone_like(self) -> Self { self } }")
            .expect("trait source should parse");
        let return_type = program
            .decls
            .iter()
            .find_map(|decl| match &decl.kind {
                DeclKind::Trait(trait_decl) => {
                    trait_decl.items.iter().find_map(|item| match item {
                        TraitItem::Function(function) => function.return_type.as_ref(),
                        _ => None,
                    })
                }
                _ => None,
            })
            .expect("trait method should have a return type");

        assert_eq!(type_snippet(return_type), "Self");
    }

    #[test]
    fn active_call_site_tracks_receiver_and_nested_delimiters() {
        let src = r#"def main() {
    service.send(build(1, 2), [3, 4], { "comma, text": Vec::<Pair<i64, i64>>() },
}"#;
        let cursor = src.rfind('\n').unwrap();
        let call = active_call_site(src, cursor).expect("outer method call should be active");
        assert_eq!(call.callee, "send");
        assert_eq!(call.qualifier.as_deref(), Some("service"));
        assert_eq!(call.separator, Some(CallSeparator::Member));
        assert_eq!(call.argument_index, 3);
    }

    #[test]
    fn active_call_site_selects_innermost_incomplete_call() {
        let src = "outer(1, module::inner(\"ignored, )\", value";
        let call = active_call_site(src, src.len()).expect("inner call should be active");
        assert_eq!(call.callee, "inner");
        assert_eq!(call.qualifier.as_deref(), Some("module"));
        assert_eq!(call.separator, Some(CallSeparator::Namespace));
        assert_eq!(call.argument_index, 1);
    }

    #[test]
    fn active_call_site_ignores_comment_delimiters_and_unicode_prefix() {
        let src = "函数(); target(1 /* ), bogus(, */, \"x,y\", 值";
        let call = active_call_site(src, src.len()).expect("target call should remain active");
        assert_eq!(call.callee, "target");
        assert_eq!(call.argument_index, 2);
    }

    #[test]
    fn signature_selection_resolves_receiver_and_overloads_deterministically() {
        let src = r#"
struct Client {}
impl Client {
    /// Sends one value.
    /// @param value value to send
    def send(self, value: i64) -> unit {}
    /// Sends a value with a label.
    /// @param value value to send
    /// @param label label to attach
    def send(self, value: i64, label: &str) -> unit {}
}
def main(client: Client) { client.send(1, "fast", ) }
"#;
        let mut signatures = collect_function_signatures(src);
        qualify_function_signatures(&mut signatures, "");
        let call = active_call_site(src, src.find(") }\n").unwrap()).unwrap();
        let selection = select_signature_help(&call, &signatures, Some("Client"))
            .expect("receiver methods should resolve");
        assert_eq!(selection.signatures.len(), 2);
        assert_eq!(selection.active_signature, 1);
        assert_eq!(selection.active_parameter, 1);
        assert_eq!(
            selection.signatures[1].qualified_owner.as_deref(),
            Some("Client")
        );
        assert_eq!(
            selection.signatures[1].documentation.as_deref(),
            Some("Sends a value with a label.")
        );
        assert_eq!(
            selection.signatures[1].parameter_documentation[1].as_deref(),
            Some("label to attach")
        );
    }

    #[test]
    fn signature_selection_rejects_unresolved_owner_and_clamps_zero_parameters() {
        let mut signatures = collect_function_signatures(
            "impl Client { def ping(self) -> unit {} }\ndef free() -> unit {}\n",
        );
        qualify_function_signatures(&mut signatures, "");
        let unresolved = ActiveCallSite {
            callee: "ping".into(),
            qualifier: Some("unknown".into()),
            separator: Some(CallSeparator::Member),
            argument_index: 0,
            arguments: vec![String::new()],
        };
        assert!(select_signature_help(&unresolved, &signatures, None).is_none());

        let free = active_call_site("free(", 5).unwrap();
        let selection = select_signature_help(&free, &signatures, None).unwrap();
        assert_eq!(selection.active_parameter, 0);
    }

    #[test]
    fn signature_selection_prefers_known_argument_types_before_source_order() {
        let mut signatures = collect_function_signatures(
            "def parse(value: i64) -> unit {}\ndef parse(value: String) -> unit {}\n",
        );
        qualify_function_signatures(&mut signatures, "");
        signatures.reverse();
        let call = active_call_site("parse(42", 8).unwrap();
        let selection = select_signature_help(&call, &signatures, None).unwrap();
        assert_eq!(
            selection.signatures[selection.active_signature].param_types,
            vec!["i64"]
        );

        let call = active_call_site("parse(\"forty two\"", 17).unwrap();
        let selection = select_signature_help(&call, &signatures, None).unwrap();
        assert_eq!(
            selection.signatures[selection.active_signature].param_types,
            vec!["String"]
        );
    }

    #[test]
    fn call_walker_handles_turbofish_comparisons_and_multiline_strings() {
        let turbofish = "foo::<Pair<i64, i64>>(1, ";
        let call = active_call_site(turbofish, turbofish.len()).unwrap();
        assert_eq!(call.callee, "foo");
        assert_eq!(call.argument_index, 1);

        let comparison = "foo(a < b, next";
        let call = active_call_site(comparison, comparison.len()).unwrap();
        assert_eq!(call.argument_index, 1);

        let compact_comparison = "foo(a<b,c>d,next";
        let call = active_call_site(compact_comparison, compact_comparison.len()).unwrap();
        assert_eq!(call.argument_index, 2);

        let uppercase_comparison = "foo(MAX<value,other>MIN,next";
        let call = active_call_site(uppercase_comparison, uppercase_comparison.len()).unwrap();
        assert_eq!(call.argument_index, 2);

        let comparison_chain = "foo(a<=b,c>=d,next";
        let call = active_call_site(comparison_chain, comparison_chain.len()).unwrap();
        assert_eq!(call.argument_index, 2);

        let typed_literal = "foo(Vec<A,B>{},next";
        let call = active_call_site(typed_literal, typed_literal.len()).unwrap();
        assert_eq!(call.argument_index, 1);

        let lowercase_typed_literal = "foo(vec<a,b>{},next";
        let call =
            active_call_site(lowercase_typed_literal, lowercase_typed_literal.len()).unwrap();
        assert_eq!(call.argument_index, 1);

        let multiline = "foo(\"\"\"line one, )\nline two\"\"\", next";
        let call = active_call_site(multiline, multiline.len()).unwrap();
        assert_eq!(call.argument_index, 1);
    }

    #[test]
    fn overload_type_match_precedes_arity_and_uses_str_literal_type() {
        let mut signatures = collect_function_signatures(
            "def choose(value: i64) -> unit {}\ndef choose(value: &str, extra: i64) -> unit {}\n",
        );
        qualify_function_signatures(&mut signatures, "");
        let call = active_call_site("choose(\"text\"", 13).unwrap();
        let selection = select_signature_help(&call, &signatures, None).unwrap();
        assert_eq!(
            selection.signatures[selection.active_signature].param_types[0],
            "&str"
        );
    }

    #[test]
    fn documentation_crosses_attributes_before_callable() {
        let signatures = collect_function_signatures(
            "/// Runs the test.\n/// @param value test value\n#[test]\ndef checked(value: i64) -> unit {}\n",
        );
        assert_eq!(
            signatures[0].documentation.as_deref(),
            Some("Runs the test.")
        );
        assert_eq!(
            signatures[0].parameter_documentation[0].as_deref(),
            Some("test value")
        );
    }
}
