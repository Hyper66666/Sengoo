use super::signatures::{collect_function_signatures, FunctionSignatureInfo};
use super::symbols::{collect_ast_symbols, AstSymbol};
use std::collections::HashSet;
use tower_lsp::lsp_types::{Location, Url};

const STDLIB_SOURCES: &[(&str, &str)] = &[
    ("collections", include_str!("../../stdlib/collections.sg")),
    ("db", include_str!("../../stdlib/db.sg")),
    ("env", include_str!("../../stdlib/env.sg")),
    ("error", include_str!("../../stdlib/error.sg")),
    ("ffi", include_str!("../../stdlib/ffi.sg")),
    ("file", include_str!("../../stdlib/file.sg")),
    ("lua54", include_str!("../../stdlib/lua54.sg")),
    ("math", include_str!("../../stdlib/math.sg")),
    ("net", include_str!("../../stdlib/net.sg")),
    ("option", include_str!("../../stdlib/option.sg")),
    ("proto", include_str!("../../stdlib/proto.sg")),
    ("random", include_str!("../../stdlib/random.sg")),
    ("result", include_str!("../../stdlib/result.sg")),
    ("string", include_str!("../../stdlib/string.sg")),
    ("time", include_str!("../../stdlib/time.sg")),
];

fn stdlib_source(module: &str) -> Option<&'static str> {
    STDLIB_SOURCES
        .iter()
        .find_map(|(name, source)| (*name == module).then_some(*source))
}

fn stdlib_dependencies(module: &str) -> &'static [&'static str] {
    match module {
        "collections" => &["option"],
        "option" => &["result"],
        "result" => &["option"],
        "ffi" => &["option", "result"],
        "file" | "env" => &["ffi"],
        "db" | "lua54" | "net" | "proto" => &["ffi"],
        _ => &[],
    }
}

fn imported_stdlib_modules(content: &str) -> Vec<String> {
    let mut modules = Vec::new();
    let mut seen = HashSet::new();

    for line in content.lines() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix("import std::") else {
            continue;
        };
        let module = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>();
        if stdlib_source(&module).is_some() && seen.insert(module.clone()) {
            modules.push(module);
        }
    }

    modules
}

fn add_stdlib_module_symbols(
    module: &str,
    seen_modules: &mut HashSet<String>,
    seen_symbols: &mut HashSet<String>,
    out: &mut Vec<AstSymbol>,
) {
    if !seen_modules.insert(module.to_string()) {
        return;
    }

    if let Some(source) = stdlib_source(module) {
        for symbol in collect_ast_symbols(source) {
            if seen_symbols.insert(symbol.name.clone()) {
                out.push(symbol);
            }
        }
    }

    for dependency in stdlib_dependencies(module) {
        add_stdlib_module_symbols(dependency, seen_modules, seen_symbols, out);
    }
}

fn add_stdlib_module_signatures(
    module: &str,
    seen_modules: &mut HashSet<String>,
    out: &mut Vec<FunctionSignatureInfo>,
) {
    if !seen_modules.insert(module.to_string()) {
        return;
    }

    if let Some(source) = stdlib_source(module) {
        out.extend(collect_function_signatures(source));
    }

    for dependency in stdlib_dependencies(module) {
        add_stdlib_module_signatures(dependency, seen_modules, out);
    }
}

fn stdlib_module_uri(module: &str) -> Option<Url> {
    Url::parse(&format!("sengoo-stdlib:/{module}.sg")).ok()
}

fn stdlib_definition_in_module(
    module: &str,
    symbol: &str,
    seen_modules: &mut HashSet<String>,
) -> Option<Location> {
    if !seen_modules.insert(module.to_string()) {
        return None;
    }

    if let Some(source) = stdlib_source(module) {
        if let Some(found) = collect_ast_symbols(source)
            .into_iter()
            .find(|item| item.name == symbol)
        {
            return Some(Location::new(stdlib_module_uri(module)?, found.range));
        }
    }

    for dependency in stdlib_dependencies(module) {
        if let Some(location) = stdlib_definition_in_module(dependency, symbol, seen_modules) {
            return Some(location);
        }
    }

    None
}

pub(super) fn stdlib_symbols_for_content(content: &str) -> Vec<AstSymbol> {
    let mut seen_modules = HashSet::new();
    let mut seen_symbols = HashSet::new();
    let mut symbols = Vec::new();

    for module in imported_stdlib_modules(content) {
        add_stdlib_module_symbols(&module, &mut seen_modules, &mut seen_symbols, &mut symbols);
    }

    symbols
}

pub(super) fn stdlib_symbol_detail_for_content(content: &str, symbol: &str) -> Option<AstSymbol> {
    stdlib_symbols_for_content(content)
        .into_iter()
        .find(|item| item.name == symbol)
}

pub(super) fn stdlib_definition_for_content(content: &str, symbol: &str) -> Option<Location> {
    let mut seen_modules = HashSet::new();

    for module in imported_stdlib_modules(content) {
        if let Some(location) = stdlib_definition_in_module(&module, symbol, &mut seen_modules) {
            return Some(location);
        }
    }

    None
}

pub(super) fn stdlib_signatures_for_content(content: &str) -> Vec<FunctionSignatureInfo> {
    let mut seen_modules = HashSet::new();
    let mut signatures = Vec::new();

    for module in imported_stdlib_modules(content) {
        add_stdlib_module_signatures(&module, &mut seen_modules, &mut signatures);
    }

    signatures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdlib_symbols_follow_imported_modules() {
        let symbols = stdlib_symbols_for_content("import std::option;\nimport std::result;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"Option"));
        assert!(names.contains(&"option_some"));
        assert!(names.contains(&"Result"));
        assert!(names.contains(&"result_ok_with"));
    }

    #[test]
    fn stdlib_symbols_include_impl_and_trait_methods() {
        let option_symbols = stdlib_symbols_for_content("import std::option;\n");
        let option_names = option_symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.detail.as_str()))
            .collect::<Vec<_>>();

        assert!(option_names.contains(&("unwrap", "method")));
        assert!(option_names.contains(&("expect", "method")));
        assert!(option_names.contains(&("ok_or", "method")));

        let collection_symbols = stdlib_symbols_for_content("import std::collections;\n");
        let collection_names = collection_symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.detail.as_str()))
            .collect::<Vec<_>>();

        assert!(collection_names.contains(&("Iterator", "trait")));
        assert!(collection_names.contains(&("next", "trait method")));
    }

    #[test]
    fn stdlib_symbol_detail_resolves_imported_symbol() {
        let symbol = stdlib_symbol_detail_for_content("import std::option;\n", "option_some")
            .expect("imported stdlib symbol should resolve");

        assert_eq!(symbol.name, "option_some");
        assert_eq!(symbol.detail, "function");
    }

    #[test]
    fn stdlib_definition_resolves_imported_methods() {
        let location = stdlib_definition_for_content("import std::option;\n", "unwrap")
            .expect("imported stdlib method definition should resolve");

        assert_eq!(location.uri.scheme(), "sengoo-stdlib");
        assert!(location.uri.as_str().ends_with("/option.sg"));
        assert!(location.range.start.line > 0);
    }

    #[test]
    fn stdlib_signatures_follow_imported_modules() {
        let signatures = stdlib_signatures_for_content("import std::option;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();

        assert!(labels.contains(&"def option_some<T>(value: T) -> Option<T>"));
        assert!(labels
            .contains(&"def result_ok_with<T, E>(value: T, error_placeholder: E) -> Result<T, E>"));
    }

    #[test]
    fn stdlib_symbols_follow_ffi_result_family_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::net;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"TcpStream"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));
        assert!(names.contains(&"result_ok_with"));

        let signatures = stdlib_signatures_for_content("import std::ffi;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels
            .contains(&"def result_ok_with<T, E>(value: T, error_placeholder: E) -> Result<T, E>"));
    }

    #[test]
    fn stdlib_symbols_follow_file_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::file;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"file_exists"));
        assert!(names.contains(&"file_write_str"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));
    }

    #[test]
    fn stdlib_symbols_follow_env_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::env;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"env_var_copy"));
        assert!(names.contains(&"env_has_var"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));
    }

    #[test]
    fn stdlib_symbols_include_time_helpers() {
        let symbols = stdlib_symbols_for_content("import std::time;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"time_unix_ms"));
        assert!(names.contains(&"time_sleep_ms"));
        assert!(names.contains(&"time_elapsed_ms"));
    }

    #[test]
    fn stdlib_symbols_include_random_helpers() {
        let symbols = stdlib_symbols_for_content("import std::random;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"random_seed"));
        assert!(names.contains(&"random_i64"));
        assert!(names.contains(&"random_range_i64"));
        assert!(names.contains(&"random_bool"));
    }
}
