use super::symbols::{collect_ast_symbols, AstSymbol};
use std::collections::HashSet;

const STDLIB_SOURCES: &[(&str, &str)] = &[
    ("collections", include_str!("../../stdlib/collections.sg")),
    ("db", include_str!("../../stdlib/db.sg")),
    ("error", include_str!("../../stdlib/error.sg")),
    ("ffi", include_str!("../../stdlib/ffi.sg")),
    ("lua54", include_str!("../../stdlib/lua54.sg")),
    ("math", include_str!("../../stdlib/math.sg")),
    ("net", include_str!("../../stdlib/net.sg")),
    ("option", include_str!("../../stdlib/option.sg")),
    ("proto", include_str!("../../stdlib/proto.sg")),
    ("result", include_str!("../../stdlib/result.sg")),
    ("string", include_str!("../../stdlib/string.sg")),
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
    fn stdlib_symbol_detail_resolves_imported_symbol() {
        let symbol = stdlib_symbol_detail_for_content("import std::option;\n", "option_some")
            .expect("imported stdlib symbol should resolve");

        assert_eq!(symbol.name, "option_some");
        assert_eq!(symbol.detail, "function");
    }
}
