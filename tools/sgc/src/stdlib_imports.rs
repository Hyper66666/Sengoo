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
    "assert",
    "ffi",
    "status",
    "collections",
    "json",
    "strconv",
    "fmt",
    "regex",
    "log",
    "config",
    "hash",
    "encoding",
    "compress",
    "file",
    "dir",
    "fs",
    "io",
    "stream",
    "env",
    "time",
    "random",
    "path",
    "process",
    "args",
    "db",
    "lua54",
    "net",
    "http",
    "proto",
    "async",
];

fn is_virtual_stdlib_module(module: &str) -> bool {
    module == "reflect"
}

fn source_module_needs_result_family(module: &str) -> bool {
    matches!(
        module,
        "option"
            | "result"
            | "math"
            | "collections"
            | "json"
            | "status"
            | "strconv"
            | "db"
            | "ffi"
            | "file"
            | "dir"
            | "io"
            | "stream"
            | "env"
            | "path"
            | "process"
            | "args"
            | "fmt"
            | "regex"
            | "log"
            | "config"
            | "hash"
            | "encoding"
            | "compress"
            | "fs"
            | "http"
            | "lua54"
            | "net"
            | "proto"
            | "async"
    )
}

fn source_module_direct_dependencies(module: &str) -> &'static [&'static str] {
    match module {
        "collections" | "json" | "status" => &["ffi", "string"],
        "math" => &["option", "status"],
        "string" => &["ffi"],
        "file" | "io" | "env" | "process" | "args" | "strconv" | "time" => &["status"],
        "stream" => &["status", "ffi", "io", "file", "net"],
        "path" | "dir" => &["status", "string"],
        "fmt" => &["strconv", "status"],
        "regex" | "log" | "config" | "hash" | "encoding" | "compress" | "fs" => &["status"],
        "async" => &["status", "result"],
        "http" | "net" => &["ffi", "status", "string", "async"],
        "db" | "lua54" | "proto" => &["ffi"],
        "assert" => &[],
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

    #[test]
    fn math_import_expands_option_and_status_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::math;\ndef main() -> i64 { 0 }\n")
                .expect("math stdlib import should expand");

        assert!(expanded.contains("struct Option"));
        assert!(expanded.contains("def STATUS_OVERFLOW()"));
        assert!(expanded.contains("def checked_add(self, rhs: i64) -> Option<i64>"));
        assert!(expanded.contains("def saturating_mul(self, rhs: i64) -> i64"));
    }

    #[test]
    fn file_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::file;\ndef main() -> i64 { 0 }\n")
                .expect("file stdlib import should expand");

        assert!(expanded.contains("def file_exists"));
        assert!(expanded.contains("def file_copy"));
        assert!(expanded.contains("def file_move"));
        assert!(expanded.contains("def file_kind"));
        assert!(expanded.contains("def file_size"));
        assert!(expanded.contains("def file_modified_unix_ms"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn env_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::env;\ndef main() -> i64 { 0 }\n")
                .expect("env stdlib import should expand");

        assert!(expanded.contains("def env_var_copy"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn time_import_expands_source_module() {
        let expanded =
            expand_stdlib_imports_for_source("import std::time;\ndef main() -> i64 { 0 }\n")
                .expect("time stdlib import should expand");

        assert!(expanded.contains("def time_unix_ms"));
        assert!(expanded.contains("def time_sleep_ms"));
        assert!(expanded.contains("def time_format_utc_ms"));
    }

    #[test]
    fn assert_import_expands_source_module() {
        let expanded =
            expand_stdlib_imports_for_source("import std::assert;\ndef main() -> i64 { 0 }\n")
                .expect("assert stdlib import should expand");

        assert!(expanded.contains("def assert_eq_i64"));
        assert!(expanded.contains("sengoo_assert_failure_v1"));
    }

    #[test]
    fn regex_import_expands_status_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::regex;\ndef main() -> i64 { 0 }\n")
                .expect("regex stdlib import should expand");

        assert!(expanded.contains("def regex_compile"));
        assert!(expanded.contains("def STATUS_PARSE()"));
    }

    #[test]
    fn random_import_expands_source_module() {
        let expanded =
            expand_stdlib_imports_for_source("import std::random;\ndef main() -> i64 { 0 }\n")
                .expect("random stdlib import should expand");

        assert!(expanded.contains("def random_seed"));
        assert!(expanded.contains("def random_range_i64"));
    }

    #[test]
    fn path_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::path;\ndef main() -> i64 { 0 }\n")
                .expect("path stdlib import should expand");

        assert!(expanded.contains("def path_join"));
        assert!(expanded.contains("def path_normalize"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn process_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::process;\ndef main() -> i64 { 0 }\n")
                .expect("process stdlib import should expand");

        assert!(expanded.contains("struct ProcessCommand"));
        assert!(expanded.contains("struct ProcessOutput"));
        assert!(expanded.contains("def process_command"));
        assert!(expanded.contains("def process_id"));
        assert!(expanded.contains("def process_current_dir_copy"));
        assert!(expanded.contains("def process_run"));
        assert!(expanded.contains("def process_run_3"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn dir_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::dir;\ndef main() -> i64 { 0 }\n")
                .expect("dir stdlib import should expand");

        assert!(expanded.contains("def dir_exists"));
        assert!(expanded.contains("def dir_create_all"));
        assert!(expanded.contains("def dir_entry_count"));
        assert!(expanded.contains("def dir_entry_name"));
        assert!(expanded.contains("struct DirWalk"));
        assert!(expanded.contains("def dir_walk"));
        assert!(expanded.contains("def next(&self, buffer: Buffer) -> Result<i64, i64>"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn io_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::io;\ndef main() -> i64 { 0 }\n")
                .expect("io stdlib import should expand");

        assert!(expanded.contains("def io_stdin_read"));
        assert!(expanded.contains("def io_stderr_write"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn args_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::args;\ndef main() -> i64 { 0 }\n")
                .expect("args stdlib import should expand");

        assert!(expanded.contains("def args_len"));
        assert!(expanded.contains("def arg_copy"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn strconv_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::strconv;\ndef main() -> i64 { 0 }\n")
                .expect("strconv stdlib import should expand");

        assert!(expanded.contains("def strconv_parse_i64"));
        assert!(expanded.contains("def strconv_format_i64"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn collections_import_expands_ffi_for_text_collection_buffers() {
        let expanded =
            expand_stdlib_imports_for_source("import std::collections;\ndef main() -> i64 { 0 }\n")
                .expect("collections stdlib import should expand");

        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("def ffi_buffer_new"));
        assert!(expanded.contains("struct TextList"));
        assert!(expanded.contains("def text_list_new() -> TextList"));
        assert!(expanded.contains("def string_map_i64_new() -> StringMapI64"));
        assert!(expanded.contains("def string_map_bool_new() -> StringMapBool"));
        assert!(expanded.contains("def hashmap_new_string_i64() -> HashMap<String, i64>"));
    }

    #[test]
    fn status_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::status;\ndef main() -> i64 { 0 }\n")
                .expect("status stdlib import should expand");

        assert!(expanded.contains("def STATUS_UNKNOWN"));
        assert!(expanded.contains("def status_name_copy"));
        assert!(expanded.contains("def status_message_copy"));
        assert!(expanded.contains("def status_from_raw_ffi"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }

    #[test]
    fn json_import_expands_ffi_and_result_dependencies() {
        let expanded =
            expand_stdlib_imports_for_source("import std::json;\ndef main() -> i64 { 0 }\n")
                .expect("json stdlib import should expand");

        assert!(expanded.contains("struct JsonDoc"));
        assert!(expanded.contains("struct JsonValue"));
        assert!(expanded.contains("def json_parse"));
        assert!(expanded.contains("def json_doc_object"));
        assert!(expanded.contains("struct Buffer"));
        assert!(expanded.contains("struct Result"));
    }
}
