use crate::find_stdlib_root;
use miette::{IntoDiagnostic, Result};
use sengoo_compiler::ast::{DeclKind, ImportKind};
use sengoo_compiler::Parser;
use std::collections::BTreeSet;
use std::fs;

const STDLIB_SOURCE_ORDER: &[&str] = &[
    "option",
    "result",
    "math",
    "string",
    "error",
    "collections",
    "ffi",
    "db",
    "lua54",
    "net",
    "proto",
];

fn is_virtual_stdlib_module(module: &str) -> bool {
    module == "reflect"
}

fn source_module_needs_result_family(module: &str) -> bool {
    matches!(
        module,
        "option" | "result" | "collections" | "db" | "ffi" | "lua54" | "net" | "proto"
    )
}

fn source_module_direct_dependencies(module: &str) -> &'static [&'static str] {
    match module {
        "db" | "lua54" | "net" | "proto" => &["ffi"],
        _ => &[],
    }
}

fn collect_requested_stdlib_modules(source: &str) -> BTreeSet<String> {
    if !source.contains("import") || !source.contains("std") {
        return BTreeSet::new();
    }

    let Ok(program) = Parser::parse(source) else {
        return BTreeSet::new();
    };

    let mut modules = BTreeSet::new();
    for decl in program.decls {
        let DeclKind::Import(import_decl) = decl.kind else {
            continue;
        };

        let Some(first) = import_decl.path.segments.first() else {
            continue;
        };
        if first.name != "std" {
            continue;
        }

        if let Some(module) = import_decl.path.segments.get(1) {
            modules.insert(module.name.clone());
            continue;
        }

        if let ImportKind::Selective(names) = import_decl.kind {
            for name in names {
                modules.insert(name.name);
            }
        }
    }

    modules
}

fn expand_transitive_source_modules(modules: &BTreeSet<String>) -> BTreeSet<String> {
    let mut expanded = modules.clone();

    let mut stack = modules.iter().cloned().collect::<Vec<_>>();
    while let Some(module) = stack.pop() {
        for dependency in source_module_direct_dependencies(&module) {
            if expanded.insert((*dependency).to_string()) {
                stack.push((*dependency).to_string());
            }
        }

        if source_module_needs_result_family(&module) {
            for dependency in ["option", "result"] {
                if expanded.insert(dependency.to_string()) {
                    stack.push(dependency.to_string());
                }
            }
        }
    }
    expanded
}

pub(crate) fn expand_stdlib_imports_for_source(source: &str) -> Result<String> {
    let requested_modules = collect_requested_stdlib_modules(source);
    let requested_source_modules = requested_modules
        .iter()
        .filter(|module| !is_virtual_stdlib_module(module))
        .cloned()
        .collect::<BTreeSet<_>>();
    if requested_source_modules.is_empty() {
        return Ok(source.to_string());
    }

    let Some(stdlib_root) = find_stdlib_root() else {
        miette::bail!("standard library root not found");
    };

    let expanded_modules = expand_transitive_source_modules(&requested_source_modules);
    let mut ordered_modules = STDLIB_SOURCE_ORDER
        .iter()
        .filter(|module| expanded_modules.contains(**module))
        .map(|module| (*module).to_string())
        .collect::<Vec<_>>();
    for module in expanded_modules {
        if !ordered_modules.contains(&module) {
            ordered_modules.push(module);
        }
    }

    let mut sources = Vec::new();
    for module in ordered_modules {
        let path = stdlib_root.join(format!("{module}.sg"));
        if !path.exists() {
            miette::bail!("unresolved standard library import 'std::{}'", module);
        }
        let source = fs::read_to_string(&path).into_diagnostic().map_err(|err| {
            miette::miette!("failed to read stdlib module {}: {}", path.display(), err)
        })?;
        sources.push(source);
    }

    if sources.is_empty() {
        return Ok(source.to_string());
    }

    sources.push(source.to_string());
    Ok(sources.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_stdlib_module() {
        let err =
            expand_stdlib_imports_for_source("import std::missing;\ndef main() -> i64 { 0 }\n")
                .expect_err("unknown stdlib module should be rejected");

        assert!(err
            .to_string()
            .contains("unresolved standard library import 'std::missing'"));
    }

    #[test]
    fn preserves_virtual_reflection_import() {
        let source = "import std::reflect;\ndef main() -> i64 { 0 }\n";

        assert_eq!(expand_stdlib_imports_for_source(source).unwrap(), source);
    }
}
