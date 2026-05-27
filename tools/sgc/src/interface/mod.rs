use sengoo_compiler::Span;

mod function_fingerprints;
mod function_signatures;
mod generic_instances;
mod generic_items;
mod signature;

pub(crate) use self::function_fingerprints::{
    function_fingerprints_for_module, function_fingerprints_for_program,
};
pub(crate) use self::function_signatures::function_signatures_for_module;
pub(crate) use self::generic_instances::{
    generic_fingerprints_for_module, generic_fingerprints_for_program,
};
pub(crate) use self::signature::{ast_interface_signature, interface_fingerprint_from_program};

fn source_span_slice(source: &str, span: Span) -> Option<&str> {
    source.get(span.lo as usize..span.hi as usize)
}

fn function_symbol(module_path: &str, scope: &[String], name: &str) -> String {
    let mut parts = Vec::with_capacity(scope.len() + 2);
    parts.push(module_path.to_string());
    parts.extend(scope.iter().cloned());
    parts.push(name.to_string());
    parts.join("::")
}
