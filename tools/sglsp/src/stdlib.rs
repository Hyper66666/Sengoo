use super::signatures::{collect_function_signatures, FunctionSignatureInfo};
use super::symbols::{collect_ast_symbols, AstSymbol};
use std::collections::HashSet;
use tower_lsp::lsp_types::{Location, Url};

const STDLIB_SOURCES: &[(&str, &str)] = &[
    ("args", include_str!("../../stdlib/args.sg")),
    ("assert", include_str!("../../stdlib/assert.sg")),
    ("collections", include_str!("../../stdlib/collections.sg")),
    ("compress", include_str!("../../stdlib/compress.sg")),
    ("config", include_str!("../../stdlib/config.sg")),
    ("db", include_str!("../../stdlib/db.sg")),
    ("dir", include_str!("../../stdlib/dir.sg")),
    ("encoding", include_str!("../../stdlib/encoding.sg")),
    ("env", include_str!("../../stdlib/env.sg")),
    ("error", include_str!("../../stdlib/error.sg")),
    ("ffi", include_str!("../../stdlib/ffi.sg")),
    ("file", include_str!("../../stdlib/file.sg")),
    ("fmt", include_str!("../../stdlib/fmt.sg")),
    ("fs", include_str!("../../stdlib/fs.sg")),
    ("hash", include_str!("../../stdlib/hash.sg")),
    ("http", include_str!("../../stdlib/http.sg")),
    ("io", include_str!("../../stdlib/io.sg")),
    ("json", include_str!("../../stdlib/json.sg")),
    ("log", include_str!("../../stdlib/log.sg")),
    ("lua54", include_str!("../../stdlib/lua54.sg")),
    ("math", include_str!("../../stdlib/math.sg")),
    ("net", include_str!("../../stdlib/net.sg")),
    ("option", include_str!("../../stdlib/option.sg")),
    ("path", include_str!("../../stdlib/path.sg")),
    ("process", include_str!("../../stdlib/process.sg")),
    ("proto", include_str!("../../stdlib/proto.sg")),
    ("random", include_str!("../../stdlib/random.sg")),
    ("regex", include_str!("../../stdlib/regex.sg")),
    ("result", include_str!("../../stdlib/result.sg")),
    ("string", include_str!("../../stdlib/string.sg")),
    ("strconv", include_str!("../../stdlib/strconv.sg")),
    ("status", include_str!("../../stdlib/status.sg")),
    ("time", include_str!("../../stdlib/time.sg")),
];

fn stdlib_source(module: &str) -> Option<&'static str> {
    STDLIB_SOURCES
        .iter()
        .find_map(|(name, source)| (*name == module).then_some(*source))
}

fn stdlib_dependencies(module: &str) -> &'static [&'static str] {
    match module {
        "collections" => &["ffi"],
        "string" => &["ffi"],
        "option" => &["result"],
        "result" => &["option"],
        "ffi" => &["option", "result"],
        "json" | "status" => &["ffi"],
        "fmt" => &["strconv", "status"],
        "regex" | "log" | "config" | "hash" | "encoding" | "compress" | "fs" | "time" => {
            &["status"]
        }
        "http" => &["ffi", "status"],
        "net" => &["ffi", "status", "string"],
        "file" | "dir" | "io" | "env" | "path" | "process" | "args" | "strconv" => &["status"],
        "db" | "lua54" | "proto" => &["ffi"],
        "assert" => &[],
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

    fn assert_symbols_for_content(content: &str, expected: &[&str]) {
        let symbols = stdlib_symbols_for_content(content);
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        for name in expected {
            assert!(
                names.contains(name),
                "missing symbol {name}; symbols: {names:#?}"
            );
        }
    }

    fn assert_signatures_for_content(content: &str, expected: &[&str]) {
        let signatures = stdlib_signatures_for_content(content);
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();

        for label in expected {
            assert!(
                labels.contains(label),
                "missing signature {label}; signatures: {labels:#?}"
            );
        }
    }

    #[test]
    fn realworld_cli_json_audit_imports_expose_completion_and_signatures() {
        let content = "\
import std::args;
import std::collections;
import std::dir;
import std::file;
import std::json;
import std::log;
import std::status;
";

        assert_symbols_for_content(
            content,
            &[
                "arg_exists",
                "dir_create_all",
                "file_read_into",
                "file_write_str",
                "JsonDoc",
                "JsonValue",
                "json_parse",
                "LOG_INFO",
                "log_write",
                "STATUS_OK",
                "status_name_copy",
                "Vec",
                "vec_new_i64",
                "Buffer",
                "Result",
            ],
        );
        assert_signatures_for_content(
            content,
            &[
                "def arg_copy(index: i64, buffer: Buffer) -> Result<i64, i64>",
                "def dir_create_all(path: &str) -> Result<bool, i64>",
                "def file_read_into(path: &str, buffer: Buffer) -> Result<i64, i64>",
                "def file_write_str(path: &str, data: &str) -> Result<i64, i64>",
                "def json_parse(text: &str) -> Result<JsonDoc, i64>",
                "def number_i64(self) -> Result<i64, i64> [impl JsonValue]",
                "def log_write(level: i64, message: &str) -> Result<bool, i64>",
                "def status_name_copy(code: i64, buffer: Buffer) -> Result<i64, i64>",
                "def vec_new_i64() -> Vec<i64>",
                "def push(self, value: i64) -> bool [impl Vec<i64>]",
            ],
        );
    }

    #[test]
    fn realworld_http_client_status_imports_expose_completion_and_signatures() {
        let content = "\
import std::http;
import std::json;
import std::log;
import std::status;
";

        assert_symbols_for_content(
            content,
            &[
                "HttpResponse",
                "http_client_get",
                "json_doc_object",
                "JsonDoc",
                "LOG_INFO",
                "log_test_sink_copy",
                "STATUS_UNSUPPORTED",
                "status_name_copy",
                "Buffer",
                "Result",
            ],
        );
        assert_signatures_for_content(
            content,
            &[
                "def http_client_get(url: &str, timeout_ms: i64) -> Result<HttpResponse, i64>",
                "def close(self) -> bool [impl HttpResponse]",
                "def json_doc_object() -> Result<JsonDoc, i64>",
                "def serialize(self, buffer: Buffer) -> Result<i64, i64> [impl JsonDoc]",
                "def log_test_sink_copy(buffer: Buffer) -> Result<i64, i64>",
                "def STATUS_UNSUPPORTED() -> i64",
                "def status_name_copy(code: i64, buffer: Buffer) -> Result<i64, i64>",
            ],
        );
    }

    #[test]
    fn realworld_workspace_doc_loop_imports_expose_completion_and_signatures() {
        let content = "\
import std::env;
import std::process;
";

        assert_symbols_for_content(
            content,
            &[
                "env_is_windows",
                "process_current_dir_len",
                "process_current_dir_copy",
                "process_id",
                "process_run_2",
                "process_run_3",
                "ProcessCommand",
                "ProcessOutput",
                "Buffer",
                "Result",
            ],
        );
        assert_signatures_for_content(
            content,
            &[
                "def env_is_windows() -> bool",
                "def process_current_dir_len() -> Result<i64, i64>",
                "def process_current_dir_copy(buffer: Buffer) -> Result<i64, i64>",
                "def process_id() -> i64",
                "def process_run_2(executable: &str, arg0: &str, arg1: &str) -> Result<i64, i64>",
                "def process_run_3(executable: &str, arg0: &str, arg1: &str, arg2: &str) -> Result<i64, i64>",
            ],
        );
    }

    #[test]
    fn realworld_http_client_status_imports_expose_hover_detail_and_definition() {
        let content = "\
import std::http;
import std::json;
import std::log;
import std::status;
";

        let symbol = stdlib_symbol_detail_for_content(content, "http_client_get")
            .expect("realworld HTTP import set should expose hover detail");
        assert_eq!(symbol.name, "http_client_get");
        assert_eq!(symbol.detail, "function");

        let definition = stdlib_definition_for_content(content, "http_client_get")
            .expect("realworld HTTP import set should expose definition");
        assert_eq!(definition.uri.scheme(), "sengoo-stdlib");
        assert!(definition.uri.as_str().ends_with("/http.sg"));
        assert!(definition.range.start.line > 0);
    }

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
    fn stdlib_symbols_follow_string_buffer_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::string;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"string_new"));
        assert!(names.contains(&"string_from_str"));

        let signatures = stdlib_signatures_for_content("import std::string;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"def string_new() -> String"));
        assert!(labels.contains(
            &"def copy_to_buffer(self, buffer: Buffer) -> Result<i64, i64> [impl String]"
        ));
    }

    #[test]
    fn stdlib_symbols_follow_collections_buffer_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::collections;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"TextList"));
        assert!(names.contains(&"text_list_new"));
        assert!(names.contains(&"StringMapI64"));
        assert!(names.contains(&"string_map_i64_new"));
        assert!(names.contains(&"StringMapBool"));
        assert!(names.contains(&"string_map_bool_new"));

        let signatures = stdlib_signatures_for_content("import std::collections;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"def text_list_new() -> TextList"));
        assert!(labels.contains(&"def string_map_i64_new() -> StringMapI64"));
        assert!(labels.contains(&"def string_map_bool_new() -> StringMapBool"));
        assert!(
            labels.contains(
                &"def get_copy(self, index: i64, buffer: Buffer) -> Result<i64, i64> [impl TextList]"
            ),
            "labels: {labels:#?}"
        );
        assert!(
            labels.contains(&"def iter_keys(self) -> StringMapKeyIter [impl StringMapI64]"),
            "labels: {labels:#?}"
        );
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
    fn stdlib_symbols_follow_http_server_request_surface() {
        let content = "import std::net;\n";

        assert_symbols_for_content(
            content,
            &[
                "HttpServer",
                "HttpServerRequest",
                "HttpServerNextRequestOutcome",
                "http_server_bind",
                "Buffer",
                "Result",
                "String",
            ],
        );

        let signatures = stdlib_signatures_for_content(content);
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        for expected in [
            "def http_server_bind(host: &str, port: i64) -> Result<HttpServer, i64>",
            "def next_request(self, timeout_ms: i64) -> Result<HttpServerRequest, i64> [impl HttpServer]",
            "def next_request_async(self, timeout_ms: i64) -> Future<HttpServerNextRequestOutcome> [impl HttpServer]",
            "def method_string(self) -> Result<String, i64> [impl HttpServerRequest]",
            "def path_string(self) -> Result<String, i64> [impl HttpServerRequest]",
            "def query_string(self) -> Result<String, i64> [impl HttpServerRequest]",
            "def version_string(self) -> Result<String, i64> [impl HttpServerRequest]",
            "def header_string(self, name: &str) -> Result<String, i64> [impl HttpServerRequest]",
            "def body_len(self) -> Result<i64, i64> [impl HttpServerRequest]",
            "def body_copy(self, buffer: Buffer) -> Result<i64, i64> [impl HttpServerRequest]",
            "def respond(self, status: i64, body: &str) -> Result<bool, i64> [impl HttpServerRequest]",
            "def respond_with_content_type(self, status: i64, content_type: &str, body: &str) -> Result<bool, i64> [impl HttpServerRequest]",
            "def close(self) -> bool [impl HttpServerRequest]",
        ] {
            assert!(
                labels.contains(&expected),
                "missing signature {expected}; labels: {labels:#?}"
            );
        }

        let definition = stdlib_definition_for_content(content, "HttpServerRequest")
            .expect("net import should expose HttpServerRequest definition");
        assert_eq!(definition.uri.scheme(), "sengoo-stdlib");
        assert!(definition.uri.as_str().ends_with("/net.sg"));
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
        assert!(names.contains(&"file_copy"));
        assert!(names.contains(&"file_move"));
        assert!(names.contains(&"PATH_KIND_FILE"));
        assert!(names.contains(&"PATH_KIND_DIR"));
        assert!(names.contains(&"file_kind"));
        assert!(names.contains(&"file_size"));
        assert!(names.contains(&"file_modified_unix_ms"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::file;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(
            &"def file_copy(source: &str, destination: &str, overwrite: bool) -> Result<i64, i64>"
        ));
        assert!(labels.contains(
            &"def file_move(source: &str, destination: &str, overwrite: bool) -> Result<bool, i64>"
        ));
        assert!(labels.contains(&"def PATH_KIND_FILE() -> i64"));
        assert!(labels.contains(&"def file_kind(path: &str) -> Result<i64, i64>"));
        assert!(labels.contains(&"def file_size(path: &str) -> Result<i64, i64>"));
        assert!(labels.contains(&"def file_modified_unix_ms(path: &str) -> Result<i64, i64>"));
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

    #[test]
    fn stdlib_symbols_follow_path_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::path;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"path_separator"));
        assert!(names.contains(&"path_join"));
        assert!(names.contains(&"path_normalize"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::path;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(
            &"def path_join(left: &str, right: &str, buffer: Buffer) -> Result<i64, i64>"
        ));
        assert!(labels.contains(&"def path_is_absolute(path: &str) -> bool"));
    }

    #[test]
    fn stdlib_symbols_follow_process_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::process;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"process_id"));
        assert!(names.contains(&"process_current_dir_copy"));
        assert!(names.contains(&"process_command"));
        assert!(names.contains(&"process_exit_code"));
        assert!(names.contains(&"process_run"));
        assert!(names.contains(&"process_run_1"));
        assert!(names.contains(&"process_run_2"));
        assert!(names.contains(&"process_run_3"));
        assert!(names.contains(&"process_run_raw"));
        assert!(names.contains(&"ProcessCommand"));
        assert!(names.contains(&"ProcessOutput"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::process;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(
            labels.contains(&"def process_current_dir_copy(buffer: Buffer) -> Result<i64, i64>")
        );
        assert!(labels
            .contains(&"def process_command(executable: &str) -> Result<ProcessCommand, i64>"));
        assert!(labels.contains(&"def process_id() -> i64"));
        assert!(labels.contains(&"def process_run(executable: &str) -> Result<i64, i64>"));
        assert!(labels.contains(
            &"def process_run_3(executable: &str, arg0: &str, arg1: &str, arg2: &str) -> Result<i64, i64>"
        ));
    }

    #[test]
    fn stdlib_symbols_follow_dir_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::dir;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"dir_exists"));
        assert!(names.contains(&"dir_create"));
        assert!(names.contains(&"dir_create_all"));
        assert!(names.contains(&"dir_remove"));
        assert!(names.contains(&"dir_entry_count"));
        assert!(names.contains(&"dir_entry_name"));
        assert!(names.contains(&"DirWalk"));
        assert!(names.contains(&"dir_walk"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::dir;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"def dir_exists(path: &str) -> bool"));
        assert!(labels.contains(&"def dir_create_all(path: &str) -> Result<bool, i64>"));
        assert!(labels.contains(&"def dir_remove(path: &str) -> Result<bool, i64>"));
        assert!(labels.contains(&"def dir_entry_count(path: &str) -> Result<i64, i64>"));
        assert!(labels.contains(
            &"def dir_entry_name(path: &str, index: i64, buffer: Buffer) -> Result<i64, i64>"
        ));
        assert!(
            labels.contains(&"def dir_walk(root: &str, max_depth: i64) -> Result<DirWalk, i64>")
        );
        assert!(
            labels.contains(&"def next(self, buffer: Buffer) -> Result<i64, i64> [impl DirWalk]")
        );
        assert!(labels.contains(&"def close(self) -> bool [impl DirWalk]"));
    }

    #[test]
    fn stdlib_symbols_follow_io_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::io;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"io_stdin_read"));
        assert!(names.contains(&"io_stdin_read_line"));
        assert!(names.contains(&"io_stdout_write"));
        assert!(names.contains(&"io_stderr_write"));
        assert!(names.contains(&"io_stdout_flush"));
        assert!(names.contains(&"io_stderr_flush"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::io;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"def io_stdin_read(buffer: Buffer) -> Result<i64, i64>"));
        assert!(labels.contains(&"def io_stdout_write(data: &str) -> Result<i64, i64>"));
        assert!(labels.contains(&"def io_stderr_flush() -> Result<bool, i64>"));
    }

    #[test]
    fn stdlib_symbols_follow_args_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::args;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"args_len"));
        assert!(names.contains(&"arg_exists"));
        assert!(names.contains(&"arg_len"));
        assert!(names.contains(&"arg_copy"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::args;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"def arg_len(index: i64) -> Result<i64, i64>"));
        assert!(labels.contains(&"def arg_copy(index: i64, buffer: Buffer) -> Result<i64, i64>"));
    }

    #[test]
    fn stdlib_symbols_follow_strconv_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::strconv;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"strconv_parse_i64"));
        assert!(names.contains(&"strconv_parse_i64_buffer"));
        assert!(names.contains(&"strconv_format_i64"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::strconv;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"def strconv_parse_i64(value: &str) -> Result<i64, i64>"));
        assert!(labels
            .contains(&"def strconv_format_i64(value: i64, buffer: Buffer) -> Result<i64, i64>"));
    }

    #[test]
    fn stdlib_symbols_follow_status_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::status;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"STATUS_UNKNOWN"));
        assert!(names.contains(&"STATUS_INVALID_ARGUMENT"));
        assert!(names.contains(&"status_name_copy"));
        assert!(names.contains(&"status_message_copy"));
        assert!(names.contains(&"status_from_raw_ffi"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::status;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"def STATUS_UNKNOWN() -> i64"));
        assert!(
            labels.contains(&"def status_name_copy(code: i64, buffer: Buffer) -> Result<i64, i64>")
        );
        assert!(labels.contains(&"def status_from_raw_ffi(code: i64) -> i64"));
    }

    #[test]
    fn stdlib_symbols_follow_json_dependencies() {
        let symbols = stdlib_symbols_for_content("import std::json;\n");
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"JsonDoc"));
        assert!(names.contains(&"JsonValue"));
        assert!(names.contains(&"json_parse"));
        assert!(names.contains(&"json_doc_object"));
        assert!(names.contains(&"Buffer"));
        assert!(names.contains(&"Result"));

        let signatures = stdlib_signatures_for_content("import std::json;\n");
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"def json_parse(text: &str) -> Result<JsonDoc, i64>"));
        assert!(labels.contains(
            &"def json_parse_buffer(buffer: Buffer, input_len: i64) -> Result<JsonDoc, i64>"
        ));
        assert!(labels.contains(&"def json_doc_object() -> Result<JsonDoc, i64>"));
        assert!(labels.contains(&"def number_f64(self) -> Result<f64, i64> [impl JsonValue]"));
    }
}
