#[path = "runtime_hardening_tests.rs"]
mod runtime_hardening_tests;

use super::{
    append_native_runtime_inputs, bench_root_dir, build_cache_key, build_graph_v2_for_source,
    build_metadata_matches, build_reflection_metadata, cache_key, cache_mismatch_reasons,
    can_reuse_artifacts_for_unreachable_impl_only_changes, can_skip_codegen_via_generic_cache,
    can_use_incremental_link_with_metadata, can_use_incremental_link_with_run_metadata,
    classify_edit_impact, cmd_build, collect_bench_cases, collect_impl_only_impacted_symbols,
    collect_module_graph_snapshot, compile_ir_to_object, compile_native_binary, compile_source,
    compile_source_to_llvm_file_with_phase_timings_with_mode, compile_source_with_phase_timings,
    daemon_request_build, derive_build_workset_plan, derive_cached_native_recovery_plan,
    derive_codegen_workset_manifest, derive_generic_instance_plan, derive_run_workset_plan,
    dispatch_build_via_daemon, edit_class_label, ensure_runtime_objects,
    expand_stdlib_imports_for_source, find_clang, find_runtime_c, format_edit_impact_lines,
    generic_fingerprints_for_module, generic_instance_hit_ratio, handle_daemon_client,
    link_native_binary_from_objects, maybe_emit_reflection_sidecar, metadata_matches,
    module_dependency_levels, module_fingerprints_for_source, module_invalidation_stats,
    native_library_link_args, parse_frontend_jobs_arg, parse_linker_mode,
    reflection_options_from_cli, reflection_sidecar_path_for_artifact, resolve_bench_suite_path,
    resolve_daemon_addr, resolve_engine, runtime_bundle_fingerprint, runtime_source_bundle,
    select_reflection_i64_zero_arity_symbol, send_daemon_request, signature_is_zero_arity_i64,
    validate_reflection_metadata, BuildCacheMetadata, BuildGraphNodeV2, BuildGraphV2,
    BuildWorksetPlan, CachedNativeRecoveryPlan, ContractChecksMode, DaemonDispatchOutcome,
    EditClass, EditImpact, FrontendFallbackScope, FrontendJobs, FrontendMemoryMode,
    FrontendProbeMode, FunctionFingerprint, GenericInstanceCacheEntry,
    GenericInstanceCacheMetadata, GenericInstanceFingerprint, GenericInstancePlanStats,
    GenericItemFingerprint, LinkerMode, ModuleFingerprint, ReflectionMetadata, ReflectionMode,
    RunCacheMetadata, RunEngine, RuntimeSourceIdentity, BUILD_GRAPH_SCHEMA_VERSION,
    DAEMON_PROTOCOL_VERSION, DEFAULT_DAEMON_ADDR, DEFAULT_SYMBOL_FINGERPRINT_MAX_SOURCE_BYTES,
    FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES, GENERIC_INSTANCE_CACHE_SCHEMA_VERSION,
    LOW_MEMORY_HINT_AVAILABLE_BYTES,
};
use crate::cli::Cli;
use crate::cross_compile::NativeBuildTarget;
use clap::Parser as _;
use sengoo_compiler::{
    compile_to_ir as compile_compiler_ir, compile_to_mir, CompileWarning, DebugInfoConfig,
    JITCodegen,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;

fn fp(path: &str, interface_hash: u64, hash: u64) -> ModuleFingerprint {
    ModuleFingerprint {
        path: path.to_string(),
        interface_hash,
        hash,
    }
}

fn metadata_for_test() -> RunCacheMetadata {
    RunCacheMetadata {
        source_hash: 123,
        root_interface_hash: 101,
        root_implementation_hash: 123,
        module_fingerprints: vec![fp("tests/mod_a.sg", 11, 11)],
        opt_level: 1,
        contract_checks: false,
        debug_info: false,
        requested_engine: RunEngine::Auto,
        resolved_engine: RunEngine::Native,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(777),
        llvm_ir_path: "tests/build/a.ll".to_string(),
        executable_path: Some("tests/build/a.exe".to_string()),
        llvm_ir_hash: 999,
        object_path: Some("tests/build/a.obj".to_string()),
        build_graph_v2: None,
    }
}

fn temp_object_file(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sengoo-sgc-{}-{}.{}",
        name,
        std::process::id(),
        if cfg!(windows) { "obj" } else { "o" }
    ));
    fs::write(&path, b"obj").unwrap();
    path
}

fn temp_artifact(name: &str, ext: &str) -> std::path::PathBuf {
    let stem = format!("sengoo-sgc-{}-{}", name, std::process::id());
    if ext.is_empty() {
        std::env::temp_dir().join(stem)
    } else {
        std::env::temp_dir().join(format!("{}.{}", stem, ext))
    }
}

fn temp_sg_module(name: &str, source: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sengoo-sgc-reflect-{}-{}.sg",
        name,
        std::process::id()
    ));
    fs::write(&path, source).unwrap();
    path
}

fn reflection_graph_for_module(path: &Path) -> BuildGraphV2 {
    let module_id = super::canonical_or_lossy(path);
    BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: module_id.clone(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: module_id,
            interface_hash: 1,
            implementation_hash: 1,
            depends_on: Vec::new(),
            object_path: None,
            functions: Vec::new(),
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    }
}

fn classify_root_edit(before: &str, after: &str) -> super::EditImpact {
    let before_interface = super::interface_fingerprint(before);
    let before_impl = super::implementation_fingerprint(before);
    let after_interface = super::interface_fingerprint(after);
    let after_impl = super::implementation_fingerprint(after);
    let mut edges = BTreeMap::new();
    edges.insert(
        super::canonical_or_lossy(Path::new("tests/main.sg")),
        Vec::new(),
    );
    let graph = build_graph_v2_for_source(
        Path::new("tests/main.sg"),
        &[],
        &edges,
        None,
        after_interface,
        after_impl,
    );
    classify_edit_impact(
        before_interface,
        before_impl,
        after_interface,
        after_impl,
        &[],
        &[],
        None,
        &graph,
    )
}

#[test]
fn auto_prefers_native_when_available() {
    let resolved = resolve_engine(RunEngine::Auto, true, true).unwrap();
    assert_eq!(resolved, RunEngine::Native);
}

#[test]
fn auto_falls_back_to_lli_when_native_unavailable() {
    let resolved = resolve_engine(RunEngine::Auto, false, true).unwrap();
    assert_eq!(resolved, RunEngine::Lli);
}

#[test]
fn explicit_engine_is_validated() {
    assert!(resolve_engine(RunEngine::Native, false, true).is_err());
    assert!(resolve_engine(RunEngine::Lli, true, false).is_err());
    assert!(resolve_engine(RunEngine::Native, true, false).is_ok());
    assert!(resolve_engine(RunEngine::Lli, false, true).is_ok());
}

#[test]
fn linker_mode_defaults_to_auto() {
    assert_eq!(parse_linker_mode(None), LinkerMode::Auto);
    assert_eq!(parse_linker_mode(Some("")), LinkerMode::Auto);
    assert_eq!(parse_linker_mode(Some("unknown")), LinkerMode::Auto);
}

#[test]
fn linker_mode_parses_lld_and_system() {
    assert_eq!(parse_linker_mode(Some("lld")), LinkerMode::Lld);
    assert_eq!(parse_linker_mode(Some("system")), LinkerMode::System);
    assert_eq!(parse_linker_mode(Some(" LLD ")), LinkerMode::Lld);
}

#[test]
fn cache_miss_when_opt_level_changes() {
    let metadata = metadata_for_test();
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 11)],
        2,
        false,
        false,
        RunEngine::Auto,
        RunEngine::Native,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(777)),
    );
    assert!(!metadata_matches(&metadata, &key));
}

#[test]
fn cache_miss_when_engine_changes() {
    let metadata = metadata_for_test();
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 11)],
        1,
        false,
        false,
        RunEngine::Auto,
        RunEngine::Lli,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(777)),
    );
    assert!(!metadata_matches(&metadata, &key));
}

#[test]
fn cache_hit_when_key_matches() {
    let metadata = metadata_for_test();
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 11)],
        1,
        false,
        false,
        RunEngine::Auto,
        RunEngine::Native,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(777)),
    );
    assert!(metadata_matches(&metadata, &key));
}

#[test]
fn cache_miss_when_debug_info_changes() {
    let metadata = metadata_for_test();
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 11)],
        1,
        false,
        true,
        RunEngine::Auto,
        RunEngine::Native,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(777)),
    );
    assert!(!metadata_matches(&metadata, &key));
    assert!(cache_mismatch_reasons(&metadata, &key)
        .iter()
        .any(|reason| reason.contains("debug info changed")));
}

#[test]
fn cache_miss_when_runtime_source_fingerprint_changes() {
    let mut metadata = metadata_for_test();
    metadata.runtime_c_fingerprint = Some(11);
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 11)],
        1,
        false,
        false,
        RunEngine::Auto,
        RunEngine::Native,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(22)),
    );

    assert!(!metadata_matches(&metadata, &key));
    assert!(cache_mismatch_reasons(&metadata, &key)
        .iter()
        .any(|reason| reason == "runtime source changed"));
}

#[test]
fn cache_miss_when_runtime_source_fingerprint_is_missing() {
    let mut metadata = metadata_for_test();
    metadata.runtime_c_fingerprint = None;
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 11)],
        1,
        false,
        false,
        RunEngine::Auto,
        RunEngine::Native,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(777)),
    );

    assert!(!metadata_matches(&metadata, &key));
}

#[test]
fn legacy_cache_metadata_defaults_missing_runtime_source_fingerprint() {
    let mut run_json = serde_json::to_value(metadata_for_test()).unwrap();
    run_json
        .as_object_mut()
        .unwrap()
        .remove("runtime_c_fingerprint");
    let run_metadata: RunCacheMetadata = serde_json::from_value(run_json).unwrap();
    assert_eq!(run_metadata.runtime_c_fingerprint, None);

    let build_metadata: BuildCacheMetadata = serde_json::from_value(serde_json::json!({
        "source_hash": 123,
        "module_fingerprints": [],
        "opt_level": 1,
        "contract_checks": false,
        "emit_llvm": false,
        "runtime_c": "tools/stdlib/runtime.c",
        "llvm_ir_path": "tests/build/a.ll",
        "output_path": "tests/build/a.exe"
    }))
    .unwrap();
    assert_eq!(build_metadata.runtime_c_fingerprint, None);
}

#[test]
fn cache_miss_when_module_dependency_changes() {
    let metadata = metadata_for_test();
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 99)],
        1,
        false,
        false,
        RunEngine::Auto,
        RunEngine::Native,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(777)),
    );
    assert!(!metadata_matches(&metadata, &key));
}

#[test]
fn cache_mismatch_reasons_include_module_changes() {
    let metadata = metadata_for_test();
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 99)],
        1,
        false,
        false,
        RunEngine::Auto,
        RunEngine::Native,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(777)),
    );
    let reasons = cache_mismatch_reasons(&metadata, &key);
    assert!(reasons
        .iter()
        .any(|r| r.contains("module implementations changed")));
}

#[test]
fn benchmark_scaffold_exists() {
    let root = bench_root_dir();
    assert!(root.join("baseline.json").exists());
    assert!(root.join("suites/runtime/basic_loop.sg").exists());
    assert!(root.join("suites/compile/mod_tree_root.sg").exists());
    assert!(root.join("suites/incremental/change_impl_root.sg").exists());
    assert!(root.join("suites/incremental/math_util.sg").exists());
    assert!(root
        .join("templates/generic-benchmark-report-template.md")
        .exists());
    assert!(root.join("scripts/sprint01-generic-gate.py").exists());
}

#[test]
fn example_reference_docs_cover_core_cases() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);
    let guide = fs::read_to_string(workspace_root.join("docs/DEVELOPMENT_GUIDE.md")).unwrap();
    let readme = fs::read_to_string(workspace_root.join("examples/README.md")).unwrap();
    for case_name in [
        "examples/01_hello.sg",
        "examples/05_loop.sg",
        "examples/08_struct.sg",
        "examples/09_method_call.sg",
    ] {
        assert!(
            guide.contains(case_name) || readme.contains(case_name),
            "missing {case_name} in current example docs"
        );
    }
}

#[test]
fn stdlib_runtime_exports_vec_and_hashmap_core_operations() {
    let runtime_c = load_runtime_bundle_source_for_tests();

    for symbol in [
        "sengoo_vec_new_i64",
        "sengoo_vec_free_i64",
        "sengoo_vec_len_i64",
        "sengoo_vec_clear_i64_status",
        "sengoo_vec_push_i64",
        "sengoo_vec_get_i64",
        "sengoo_vec_set_i64",
        "sengoo_vec_insert_i64",
        "sengoo_vec_pop_i64",
        "sengoo_vec_get_or_default_i64",
        "sengoo_vec_contains_i64",
        "sengoo_vec_remove_i64",
        "sengoo_vec_remove_or_default_i64",
        "sengoo_vec_pop_or_default_i64",
        "sengoo_vec_string_set",
        "sengoo_vec_string_insert",
        "sengoo_hashmap_new_i64",
        "sengoo_hashmap_free_i64",
        "sengoo_hashmap_len_i64",
        "sengoo_hashmap_clear_i64",
        "sengoo_hashmap_clear_i64_status",
        "sengoo_hashmap_insert_i64",
        "sengoo_hashmap_get_i64",
        "sengoo_hashmap_get_or_default_i64",
        "sengoo_hashmap_contains_i64",
        "sengoo_hashmap_remove_i64",
        "sengoo_hashmap_iter_new_i64",
        "sengoo_hashmap_iter_done_i64",
        "sengoo_hashmap_iter_next_i64",
        "sengoo_hashmap_iter_next_or_default_i64",
        "sengoo_hashmap_iter_free_i64_status",
        "sengoo_hashmap_iter_reset_i64_status",
    ] {
        assert!(
            runtime_c.contains(symbol),
            "runtime stdlib missing symbol: {symbol}"
        );
    }
}

#[test]
fn stdlib_runtime_exports_text_collection_operations() {
    let runtime_c = load_runtime_bundle_source_for_tests();

    for symbol in [
        "sengoo_text_list_new",
        "sengoo_text_list_len",
        "sengoo_text_list_clear_status",
        "sengoo_text_list_free_status",
        "sengoo_text_list_push",
        "sengoo_text_list_get_copy",
        "sengoo_text_list_set",
        "sengoo_text_list_remove_copy",
        "sengoo_text_list_iter_new",
        "sengoo_text_list_iter_done",
        "sengoo_text_list_iter_next_copy",
        "sengoo_text_list_iter_reset_status",
        "sengoo_text_list_iter_free_status",
        "sengoo_string_map_new",
        "sengoo_string_map_len",
        "sengoo_string_map_clear_status",
        "sengoo_string_map_free_status",
        "sengoo_string_map_insert_i64",
        "sengoo_string_map_get_or_default_i64",
        "sengoo_string_map_insert_bool",
        "sengoo_string_map_get_or_default_bool",
        "sengoo_string_map_contains",
        "sengoo_string_map_remove",
        "sengoo_string_map_key_iter_new",
        "sengoo_string_map_key_iter_done",
        "sengoo_string_map_key_iter_next_copy",
        "sengoo_string_map_key_iter_reset_status",
        "sengoo_string_map_key_iter_free_status",
    ] {
        assert!(
            runtime_c.contains(symbol),
            "runtime stdlib missing text collection symbol: {symbol}"
        );
    }
}

#[test]
fn stdlib_runtime_exports_iterator_and_option_result_adapters() {
    let runtime_c = load_runtime_bundle_source_for_tests();

    for symbol in [
        "sengoo_vec_iter_new_i64",
        "sengoo_vec_iter_done_i64",
        "sengoo_vec_iter_next_i64",
        "sengoo_vec_iter_next_or_default_i64",
        "sengoo_vec_iter_map_add_i64",
        "sengoo_vec_iter_filter_even_i64",
        "sengoo_option_some_i64",
        "sengoo_option_none_i64",
        "sengoo_option_map_add_i64",
        "sengoo_option_and_then_mul_i64",
        "sengoo_result_ok_i64",
        "sengoo_result_err_i64",
        "sengoo_result_map_add_i64",
        "sengoo_result_and_then_mul_i64",
        "sengoo_result_map_err_add_i64",
    ] {
        assert!(
            runtime_c.contains(symbol),
            "runtime stdlib missing symbol: {symbol}"
        );
    }
}

#[test]
fn stdlib_runtime_exports_managed_buffer_helpers() {
    let runtime_c = load_runtime_bundle_source_for_tests();

    for symbol in [
        "sengoo_ffi_last_error_code",
        "sengoo_ffi_last_error_len",
        "sengoo_ffi_last_error_copy",
        "sengoo_ffi_last_error_clear",
        "sengoo_ffi_buffer_new",
        "sengoo_ffi_buffer_from_bytes",
        "sengoo_ffi_buffer_len",
        "sengoo_ffi_buffer_ptr",
        "sengoo_ffi_buffer_copy_out",
        "sengoo_ffi_buffer_copy_in",
        "sengoo_ffi_buffer_free",
    ] {
        assert!(
            runtime_c.contains(symbol),
            "runtime stdlib missing Buffer helper: {symbol}"
        );
    }
}

#[test]
fn runtime_source_bundle_discovers_anchor_and_existing_split_sources() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-runtime-bundle-discovery-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    for file in [
        "runtime.c",
        "runtime_breadth.c",
        "runtime_collections.c",
        "runtime_json.c",
        "runtime_process.c",
        "runtime_shared.h",
    ] {
        fs::write(root.join(file), b"/* test runtime bundle input */\n").unwrap();
    }

    let anchor = root.join("runtime.c");
    let bundle = runtime_source_bundle(&anchor.to_string_lossy())
        .expect("runtime source bundle should be discoverable");
    let file_names = bundle
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        file_names,
        vec![
            "runtime.c",
            "runtime_breadth.c",
            "runtime_collections.c",
            "runtime_json.c",
            "runtime_process.c",
        ]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn runtime_bundle_fingerprint_changes_when_split_source_or_header_changes() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-runtime-bundle-fingerprint-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("runtime.c"),
        b"long long anchor(void) { return 1; }\n",
    )
    .unwrap();
    fs::write(
        root.join("runtime_json.c"),
        b"long long split(void) { return 2; }\n",
    )
    .unwrap();
    fs::write(root.join("runtime_shared.h"), b"#define VALUE 1\n").unwrap();

    let anchor = root.join("runtime.c");
    let before = runtime_bundle_fingerprint(&anchor.to_string_lossy())
        .expect("runtime bundle fingerprint should hash split inputs");
    fs::write(
        root.join("runtime_json.c"),
        b"long long split(void) { return 3; }\n",
    )
    .unwrap();
    let after_source = runtime_bundle_fingerprint(&anchor.to_string_lossy())
        .expect("runtime bundle fingerprint should change after split source edit");
    fs::write(root.join("runtime_shared.h"), b"#define VALUE 4\n").unwrap();
    let after_header = runtime_bundle_fingerprint(&anchor.to_string_lossy())
        .expect("runtime bundle fingerprint should change after shared header edit");

    assert_ne!(before, after_source);
    assert_ne!(after_source, after_header);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn native_runtime_bundle_links_split_sources_for_full_and_object_link_paths() {
    let Some(clang) = find_clang() else {
        return;
    };

    let root =
        std::env::temp_dir().join(format!("sengoo-runtime-bundle-link-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let anchor = root.join("runtime.c");
    fs::write(
        &anchor,
        b"/* anchor runtime source intentionally empty */\n",
    )
    .unwrap();
    fs::write(root.join("runtime_shared.h"), b"/* shared header */\n").unwrap();
    fs::write(
        root.join("runtime_json.c"),
        b"long long sengoo_runtime_split_probe(void) { return 42; }\n",
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_runtime_split_probe() -> i64;
}

def main() -> i64 {
    sengoo_runtime_split_probe()
}
"#;
    let llvm_ir = compile_source(source, 1).expect("split runtime probe should compile");
    let ll_path = temp_artifact("runtime-bundle-link", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let anchor_str = anchor.to_string_lossy().to_string();

    let full_exe = temp_artifact(
        "runtime-bundle-link-full",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(
        &clang,
        &ll_path,
        &full_exe,
        Some(&anchor_str),
        1,
        None,
        None,
    )
    .unwrap();
    let full_output = Command::new(&full_exe)
        .output()
        .expect("full runtime bundle executable should run");
    assert_eq!(full_output.status.code(), Some(42));

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact("runtime-bundle-link-main", obj_ext);
    compile_ir_to_object(&clang, &ll_path, &main_obj, 1, None, false).unwrap();
    let inc_exe = temp_artifact(
        "runtime-bundle-link-objects",
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![main_obj.clone()];
    object_paths.extend(ensure_runtime_objects(&clang, &anchor_str, 1, None).unwrap());
    link_native_binary_from_objects(&clang, &object_paths, &inc_exe, None, None).unwrap();
    let inc_output = Command::new(&inc_exe)
        .output()
        .expect("object runtime bundle executable should run");
    assert_eq!(inc_output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&full_exe);
    let _ = fs::remove_file(&main_obj);
    let _ = fs::remove_file(&inc_exe);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn native_runtime_bundle_exports_tcp_readiness_fallback_for_async_runtime() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
extern "C" {
    fn sengoo_tcp_poll_readable(handle: i64) -> i64;
}

def main() -> i64 {
    sengoo_tcp_poll_readable(0)
}
"#;
    let llvm_ir = compile_source(source, 1).expect("tcp readiness probe should compile");
    let ll_path = temp_artifact("runtime-tcp-readiness-fallback", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact("runtime-tcp-readiness-fallback-main", obj_ext);
    compile_ir_to_object(&clang, &ll_path, &main_obj, 1, None, false).unwrap();

    let exe_path = temp_artifact(
        "runtime-tcp-readiness-fallback",
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![main_obj.clone()];
    object_paths.extend(ensure_runtime_objects(&clang, &runtime_c, 1, None).unwrap());
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("tcp readiness fallback probe should run");
    assert_eq!(output.status.code(), Some(0));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&main_obj);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn openspec_acceptance_scripts_target_real_test_filters() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);

    let ps = fs::read_to_string(workspace_root.join("scripts/openspec-acceptance.ps1"))
        .expect("powershell acceptance script should exist");
    let sh = fs::read_to_string(workspace_root.join("scripts/openspec-acceptance.sh"))
        .expect("shell acceptance script should exist");

    for needle in [
        "cargo test -p sgc edit_classifier_detects_",
        "cargo test -p sgc interface_change_propagates_",
        "cargo test -p sengoo-runtime --features python python_",
        "cargo test -p sengoo-compiler stdlib_surface_",
        "cargo test -p sgc stdlib_surface_runtime_",
    ] {
        assert!(
            ps.contains(needle),
            "ps1 missing updated acceptance command: {needle}"
        );
        assert!(
            sh.contains(needle),
            "sh missing updated acceptance command: {needle}"
        );
    }
}

#[test]
fn openspec_acceptance_scripts_cover_all_capabilities() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);

    let ps = fs::read_to_string(workspace_root.join("scripts/openspec-acceptance.ps1"))
        .expect("powershell acceptance script should exist");
    let sh = fs::read_to_string(workspace_root.join("scripts/openspec-acceptance.sh"))
        .expect("shell acceptance script should exist");

    for capability in [
        "lsp-tooling-sglsp",
        "formatter-tooling-sgfmt",
        "package-management-sgpm",
        "generics-core",
        "async-concurrency-model",
        "macro-system",
        "incremental-compilation-accuracy",
        "jit-aot-execution-modes",
        "python-interop-embedding",
        "docs-and-api-reference",
        "stdlib-core-collections",
    ] {
        assert!(
            ps.contains(capability),
            "ps1 missing capability: {capability}"
        );
        assert!(
            sh.contains(capability),
            "sh missing capability: {capability}"
        );
    }
}
#[test]
fn advanced_pipeline_memory_buckets_cover_100k_and_1000k() {
    let root = bench_root_dir();
    let script = fs::read_to_string(root.join("advanced_pipeline_bench.py")).unwrap();
    assert!(script.contains("MEMORY_LOC_BUCKETS = [10000, 100000, 1000000]"));
    assert!(script.contains("LADDER_STRETCH_LOC = 2500000"));
}

#[test]
fn advanced_kpi_gate_requires_100k_and_1000k_memory_buckets() {
    let root = bench_root_dir();
    let gate = fs::read_to_string(root.join("scripts/advanced-kpi-gate.py")).unwrap();
    assert!(gate.contains("DEFAULT_REQUIRED_MEMORY_LOCS = (\"10000\", \"100000\", \"1000000\")"));
    assert!(gate.contains("DEFAULT_MAX_RSS_RATIO_100K = 1.5"));
    assert!(gate.contains("DEFAULT_MAX_FRONTEND_SHARE_100K_PCT = 70.0"));
    assert!(gate.contains("DEFAULT_MAX_RSS_RATIO_1000K = 1.8"));
    assert!(gate.contains("DEFAULT_MAX_FRONTEND_SHARE_1000K_PCT = 65.0"));
    assert!(gate.contains("DEFAULT_MAX_RSS_RATIO_2500K = 2.0"));
    assert!(gate.contains("DEFAULT_LADDER_STRETCH_LOC = \"2500000\""));
    assert!(gate.contains("ladder_stretch"));
    assert!(gate.contains("DEFAULT_MAX_FRONTEND_SHARE_1000K_REGRESSION_PP = 5.0"));
    assert!(gate.contains("DEFAULT_MAX_RSS_1000K_REGRESSION_PCT = 10.0"));
}

#[test]
fn sprint01_generic_gate_covers_required_cases() {
    let root = bench_root_dir();
    let gate = fs::read_to_string(root.join("scripts/sprint01-generic-gate.py")).unwrap();
    assert!(gate.contains("generic_body_change_root.sg"));
    assert!(gate.contains("generic_new_instantiation_root.sg"));
    assert!(gate.contains("generic_signature_change_root.sg"));
    assert!(gate.contains("choices=(\"soft\", \"hard\")"));
}

#[test]
fn generic_benchmark_template_mentions_full_incremental_and_memory() {
    let root = bench_root_dir();
    let template =
        fs::read_to_string(root.join("templates/generic-benchmark-report-template.md")).unwrap();
    assert!(template.contains("Full Compile Snapshot"));
    assert!(template.contains("Incremental Generic Scenarios"));
    assert!(template.contains("Compile Memory"));
}

#[test]
fn bench_subcommands_parse() {
    assert!(Cli::try_parse_from(["sgc", "bench", "run", "runtime"]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "bench", "compile", "compile"]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "bench", "incremental", "incremental"]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "bench", "reflection", "runtime"]).is_ok());
}

#[test]
fn build_force_rebuild_flag_parses() {
    assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--force-rebuild"]).is_ok());
}

#[test]
fn build_output_flag_parses() {
    assert!(
        Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--output", "dist/app",]).is_ok()
    );
}

#[test]
fn build_emit_llvm_flag_parses() {
    assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--emit-llvm",]).is_ok());
}

#[test]
fn check_subcommand_parses() {
    assert!(Cli::try_parse_from(["sgc", "check", "tests/demo.sg"]).is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn check_command_compiles_relative_imported_module_symbols() {
    let root = std::env::temp_dir().join(format!("sengoo-sgc-local-import-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("util.sg"),
        "def imported_value() -> i64 {\n    42\n}\n",
    )
    .unwrap();
    let input = root.join("main.sg");
    fs::write(
        &input,
        "import util;\n\ndef main() -> i64 {\n    imported_value()\n}\n",
    )
    .unwrap();

    let result = super::cmd_check(input.to_string_lossy().as_ref()).await;

    let _ = fs::remove_dir_all(&root);
    result.expect("relative imported module symbols should compile");
}

#[test]
fn doc_subcommand_parses() {
    assert!(
        Cli::try_parse_from(["sgc", "doc", "tests/demo.sg", "--output", "target/api-docs",])
            .is_ok()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn doc_command_generates_rustdoc_like_layout() {
    let root = std::env::temp_dir().join(format!("sengoo-sgc-doc-gen-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let input = root.join("api_demo.sg");
    fs::write(
        &input,
        "def main() -> i64 {\n    0\n}\n\ndef helper(x: i64) -> i64 {\n    x\n}\n",
    )
    .unwrap();
    let out_dir = root.join("docs-out");

    super::cmd_doc(
        input.to_string_lossy().as_ref(),
        out_dir.to_string_lossy().as_ref(),
    )
    .await
    .unwrap();

    let index_path = out_dir.join("index.html");
    let module_path = out_dir.join("api_demo.html");
    let search_index = out_dir.join("search-index.json");
    assert!(index_path.exists(), "index page should be generated");
    assert!(module_path.exists(), "module page should be generated");
    assert!(search_index.exists(), "search index should be generated");

    let index_html = fs::read_to_string(&index_path).unwrap();
    let module_html = fs::read_to_string(&module_path).unwrap();
    assert!(index_html.contains("Sengoo API Docs"));
    assert!(module_html.contains("main"));
    assert!(module_html.contains("helper"));
}

#[test]
fn dump_ast_render_parses_source_file() {
    let root = std::env::temp_dir().join(format!("sengoo-sgc-dump-ast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let input = root.join("main.sg");
    fs::write(&input, "def answer() -> i64 {\n    42\n}\n").unwrap();

    let dump =
        super::render_ast_dump(input.to_str().unwrap()).expect("dump_ast should parse source");
    assert!(dump.contains("Program"), "{dump}");
    assert!(dump.contains("answer"), "{dump}");
    assert!(!dump.contains("not implemented"), "{dump}");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn repl_session_checks_expressions_and_declarations() {
    let transcript = super::render_repl_session(":help\n1 + 2\ndef answer() -> i64 { 42 }\nexit\n");

    assert!(transcript.contains("Sengoo REPL"), "{transcript}");
    assert!(transcript.contains("Commands:"), "{transcript}");
    assert!(
        transcript.contains("ok: expression compiled"),
        "{transcript}"
    );
    assert!(
        transcript.contains("ok: declaration compiled"),
        "{transcript}"
    );
    assert!(transcript.contains("bye"), "{transcript}");
    assert!(!transcript.contains("not implemented"), "{transcript}");
}

#[test]
fn repl_session_reports_compile_errors_without_stopping() {
    let transcript = super::render_repl_session("unknown_name\n1 + 1\nexit\n");

    assert!(transcript.contains("error:"), "{transcript}");
    assert!(transcript.contains("unknown_name"), "{transcript}");
    assert!(
        transcript.contains("ok: expression compiled"),
        "{transcript}"
    );
    assert!(transcript.contains("bye"), "{transcript}");
}

#[test]
fn run_supported_engine_flags_parse() {
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--engine", "auto",]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--engine", "native",]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--engine", "lli",]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--cranelift-fast-jit",]).is_ok());
}

#[test]
fn low_memory_flag_parses_for_build_and_run() {
    assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--low-memory"]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--low-memory"]).is_ok());
}

#[test]
fn contract_checks_flag_parses_for_build_and_run() {
    assert!(
        Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--contract-checks", "auto",])
            .is_ok()
    );
    assert!(
        Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--contract-checks", "on",]).is_ok()
    );
    assert!(
        Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--contract-checks", "off",]).is_ok()
    );
}

#[test]
fn debug_info_flag_parses_for_build_and_run() {
    assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--debug-info"]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--debug-info"]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "-g"]).is_ok());
}

#[test]
fn error_format_flag_parses_for_build_and_run() {
    assert!(
        Cli::try_parse_from(["sgc", "--error-format", "json", "build", "tests/demo.sg",]).is_ok()
    );
    assert!(
        Cli::try_parse_from(["sgc", "--error-format", "text", "run", "tests/demo.sg",]).is_ok()
    );
}

#[test]
fn split_compiler_error_stage_detects_common_prefixes() {
    let (stage_parse, msg_parse) = super::split_compiler_error_stage("parse failed: boom");
    assert_eq!(stage_parse, "parse");
    assert_eq!(msg_parse, "boom");

    let (stage_typeck, msg_typeck) =
        super::split_compiler_error_stage("typecheck failed: mismatch");
    assert_eq!(stage_typeck, "typecheck");
    assert_eq!(msg_typeck, "mismatch");

    let (stage_fallback, msg_fallback) = super::split_compiler_error_stage("unexpected failure");
    assert_eq!(stage_fallback, "compile");
    assert_eq!(msg_fallback, "unexpected failure");
}

#[test]
fn render_compile_error_json_contains_expected_fields() {
    let raw = "typecheck failed: expected i64, found bool";
    let json = super::render_compile_error_json(Some("tests/demo.sg"), raw);
    let value: Value = serde_json::from_str(&json).expect("json payload should be valid");

    assert_eq!(value["ok"], false);
    assert_eq!(value["kind"], "compile_error");
    assert_eq!(value["stage"], "typecheck");
    assert_eq!(value["message"], "expected i64, found bool");
    assert_eq!(value["input"], "tests/demo.sg");
}

#[test]
fn render_compile_error_json_extracts_stable_diagnostic_code() {
    let raw = "typecheck failed: [non-exhaustive-match] match is not exhaustive: missing Blue";
    let json = super::render_compile_error_json(Some("tests/match.sg"), raw);
    let value: Value = serde_json::from_str(&json).expect("json payload should be valid");

    assert_eq!(value["code"], "non-exhaustive-match");
    assert!(value["message"]
        .as_str()
        .unwrap_or("")
        .contains("non-exhaustive-match"));
}

#[test]
fn render_compile_error_json_extracts_dyn_stable_diagnostic_codes() {
    for code in ["dyn-multi-trait-unsupported", "dyn-box-unsupported"] {
        let raw = format!("typecheck failed: [{code}] unsupported dyn form");
        let json = super::render_compile_error_json(Some("tests/dyn.sg"), &raw);
        let value: Value = serde_json::from_str(&json).expect("json payload should be valid");

        assert_eq!(value["code"], code);
        assert!(value["message"].as_str().unwrap_or("").contains(code));
    }
}

#[test]
fn render_compile_error_json_extracts_attribute_code() {
    let raw = "parse error: unsupported attribute: unsupported cfg predicate `target_arch`";
    let json = super::render_compile_error_json(Some("tests/attrs.sg"), raw);
    let value: Value = serde_json::from_str(&json).expect("json payload should be valid");

    assert_eq!(value["stage"], "parse");
    assert_eq!(value["code"], "attributes::unsupported_attribute");
    assert!(value["message"]
        .as_str()
        .unwrap_or("")
        .contains("unsupported cfg predicate"));
}

#[test]
fn render_compile_error_json_extracts_user_future_contract_code() {
    for raw in [
        "MIR lowering failed: Poll<T> must contain `is_ready: bool` followed by `value: T`",
        "MIR lowering failed: Future<T>::poll must return Poll<T>",
        "type check error: Future<T>::poll must use `&mut self` receiver",
    ] {
        let json = super::render_compile_error_json(Some("tests/async_future.sg"), raw);
        let value: Value = serde_json::from_str(&json).expect("json payload should be valid");

        assert_eq!(value["code"], "async::user_future_contract");
        assert!(
            value["message"]
                .as_str()
                .unwrap_or("")
                .contains("Future<T>")
                || value["message"].as_str().unwrap_or("").contains("Poll<T>")
        );
    }
}

#[test]
fn render_compile_error_json_keeps_multiline_details() {
    let raw = "parse failed: unexpected token\nline 1, col 8\nnote: expected `}`";
    let json = super::render_compile_error_json(Some("tests/broken.sg"), raw);
    let value: Value = serde_json::from_str(&json).expect("json payload should be valid");
    let details = value["details"]
        .as_array()
        .expect("details should be array");

    assert_eq!(value["stage"], "parse");
    assert_eq!(value["message"], "unexpected token");
    assert_eq!(details.len(), 2);
    assert_eq!(details[0], "line 1, col 8");
    assert_eq!(details[1], "note: expected `}`");
}
#[test]
fn render_compile_error_json_with_location_serializes_structured_fields() {
    let raw = "parse error: unexpected token";
    let location = super::CompilerErrorLocationJson {
        line: Some(3),
        column: Some(9),
        span: Some(super::CompilerErrorSpanJson { lo: 24, hi: 25 }),
    };
    let json = super::render_compile_error_json_with_location(
        Some("tests/broken.sg"),
        raw,
        Some(location),
    );
    let value: Value = serde_json::from_str(&json).expect("json payload should be valid");

    assert_eq!(value["stage"], "parse");
    assert_eq!(value["location"]["line"], 3);
    assert_eq!(value["location"]["column"], 9);
    assert_eq!(value["location"]["span"]["lo"], 24);
    assert_eq!(value["location"]["span"]["hi"], 25);
}

#[test]
fn render_compile_warning_json_with_span_serializes_structured_location() {
    let warning = CompileWarning::deprecated_use("fn", "old_main", None, Some((42, 50)));
    let json = super::render_compile_warning_json(&warning);
    let value: Value = serde_json::from_str(&json).expect("warning json should be valid");

    assert_eq!(value["kind"], "compile_warning");
    assert_eq!(value["severity"], "warning");
    assert_eq!(value["code"], "attributes::deprecated_use");
    assert_eq!(value["location"]["span"]["lo"], 42);
    assert_eq!(value["location"]["span"]["hi"], 50);
}

#[test]
fn location_from_compile_error_extracts_invalid_pattern_span() {
    let src = "def main() -> i64 {\n    let = 1;\n}\n";
    let error = super::compile_to_ir(src).expect_err("source should fail parsing");
    let location = super::location_from_compile_error(src, &error)
        .expect("parse errors should include location");

    assert!(location.line.unwrap_or(0) > 0);
    assert!(location.column.unwrap_or(0) > 0);
    let span = location.span.expect("location span should exist");
    assert!(span.hi >= span.lo);
}

#[test]
fn split_compiler_error_stage_understands_compile_error_prefixes() {
    let (stage_parse, msg_parse) = super::split_compiler_error_stage("parse error: bad token");
    assert_eq!(stage_parse, "parse");
    assert_eq!(msg_parse, "bad token");

    let (stage_type, msg_type) =
        super::split_compiler_error_stage("type error: expected i64, found bool");
    assert_eq!(stage_type, "typecheck");
    assert_eq!(msg_type, "expected i64, found bool");
}

#[test]
fn frontend_memory_mode_wire_supports_low_memory_aliases() {
    assert_eq!(
        super::parse_frontend_memory_mode_wire("low-memory"),
        FrontendMemoryMode::LowMemory
    );
    assert_eq!(
        super::parse_frontend_memory_mode_wire("low_memory"),
        FrontendMemoryMode::LowMemory
    );
    assert_eq!(
        super::parse_frontend_memory_mode_wire("low"),
        FrontendMemoryMode::LowMemory
    );
}

#[test]
fn frontend_memory_mode_auto_uses_legacy_for_small_and_stream_for_large_sources() {
    assert_eq!(
        super::resolve_frontend_memory_mode(64),
        FrontendMemoryMode::Legacy
    );
    assert_eq!(
        super::resolve_frontend_memory_mode(FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES * 8),
        FrontendMemoryMode::Stream
    );
}

#[test]
fn low_memory_hint_recommendation_requires_large_source_and_low_available_memory() {
    assert!(super::low_memory_hint_should_recommend(
        LOW_MEMORY_HINT_AVAILABLE_BYTES,
        FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES
    ));
    assert!(!super::low_memory_hint_should_recommend(
        LOW_MEMORY_HINT_AVAILABLE_BYTES + 1,
        FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES
    ));
    assert!(!super::low_memory_hint_should_recommend(
        LOW_MEMORY_HINT_AVAILABLE_BYTES,
        FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES - 1
    ));
}

#[test]
fn symbol_fingerprint_collection_uses_size_and_mode_guards() {
    assert!(super::should_collect_symbol_fingerprints(64, false, false));
    assert!(super::should_collect_symbol_fingerprints(64, true, false));
    assert!(!super::should_collect_symbol_fingerprints(
        DEFAULT_SYMBOL_FINGERPRINT_MAX_SOURCE_BYTES + 1,
        false,
        false
    ));
    assert!(!super::should_collect_symbol_fingerprints(64, false, true));
}

#[test]
fn frontend_jobs_parser_accepts_auto_and_positive_int() {
    assert_eq!(parse_frontend_jobs_arg("auto").unwrap(), FrontendJobs::Auto);
    assert_eq!(
        parse_frontend_jobs_arg(" 4 ").unwrap(),
        FrontendJobs::Fixed(4)
    );
    assert!(parse_frontend_jobs_arg("0").is_err());
    assert!(parse_frontend_jobs_arg("abc").is_err());
}

#[test]
fn frontend_jobs_flag_parses_for_build_and_run() {
    assert!(
        Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--frontend-jobs", "auto",]).is_ok()
    );
    assert!(Cli::try_parse_from([
        "sgc",
        "run",
        "tests/demo.sg",
        "--frontend-jobs",
        "1",
        "--frontend-trace",
    ])
    .is_ok());
}

#[test]
fn daemon_subcommand_parses() {
    assert!(Cli::try_parse_from(["sgc", "daemon"]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "daemon", "--addr", "127.0.0.1:50000"]).is_ok());
}

#[test]
fn build_and_run_daemon_flags_parse() {
    assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--daemon"]).is_ok());
    assert!(Cli::try_parse_from([
        "sgc",
        "run",
        "tests/demo.sg",
        "--daemon",
        "--daemon-addr",
        "127.0.0.1:50000",
    ])
    .is_ok());
}

#[test]
fn reflection_flags_parse_for_build_and_run() {
    assert!(Cli::try_parse_from([
        "sgc",
        "build",
        "tests/demo.sg",
        "--reflect",
        "--reflect-module",
        "tests/demo.sg",
        "--reflect-symbol",
        "tests/demo.sg::main",
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "sgc",
        "run",
        "tests/demo.sg",
        "--reflect",
        "--reflect-symbol",
        "tests/demo.sg::main",
    ])
    .is_ok());
    assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--reflect=off",]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--reflect=auto",]).is_ok());
}

#[test]
fn source_requests_reflection_detects_common_import_forms() {
    assert!(super::source_requests_reflection(
        "import reflect;\ndef main() -> i64 { 0 }\n"
    ));
    assert!(super::source_requests_reflection(
        "import std::reflect;\ndef main() -> i64 { 0 }\n"
    ));
    assert!(super::source_requests_reflection(
        "import std{io, reflect};\ndef main() -> i64 { 0 }\n"
    ));
    assert!(!super::source_requests_reflection(
        "import std::io;\ndef main() -> i64 { 0 }\n"
    ));
}

#[test]
fn reflection_auto_mode_enables_when_dependency_imports_reflect() {
    let root = std::env::temp_dir().join(format!("sengoo-sgc-reflect-auto-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let root_module = root.join("main.sg");
    let dep_module = root.join("util.sg");
    let std_dir = root.join("std");
    fs::create_dir_all(&std_dir).unwrap();
    let std_reflect = std_dir.join("reflect.sg");

    fs::write(
        &root_module,
        "import util;\ndef main() -> i64 { util_value() }\n",
    )
    .unwrap();
    fs::write(
        &dep_module,
        "import std::reflect;\ndef util_value() -> i64 { 1 }\n",
    )
    .unwrap();
    fs::write(&std_reflect, "def meta_probe() -> i64 { 1 }\n").unwrap();

    let root_source = fs::read_to_string(&root_module).unwrap();
    let snapshot = super::collect_module_graph_snapshot(
        &root_module,
        &root_source,
        None,
        None,
        super::FrontendProbeMode::FastNoVerify,
        super::FrontendJobs::Auto,
        false,
        true,
    );
    let dep_id = super::canonical_or_lossy(&dep_module);
    assert!(snapshot.reflection_import_modules.contains(&dep_id));

    let auto = super::resolve_reflection_options_for_snapshot(
        super::reflection_options_from_cli(ReflectionMode::Auto, &[], &[]),
        &snapshot,
    );
    assert!(auto.enabled);

    let forced_off = super::resolve_reflection_options_for_snapshot(
        super::reflection_options_from_cli(ReflectionMode::Off, &[], &[]),
        &snapshot,
    );
    assert!(!forced_off.enabled);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn reflection_signature_parser_detects_zero_arity_i64() {
    assert!(signature_is_zero_arity_i64(
        "pub|main|async=false|self=-|tp=[]|params=[]|ret=i64"
    ));
    assert!(!signature_is_zero_arity_i64(
        "pub|main|async=false|self=-|tp=[]|params=[a:i64]|ret=i64"
    ));
    assert!(!signature_is_zero_arity_i64(
        "pub|main|async=false|self=-|tp=[]|params=[]|ret=bool"
    ));
}

#[test]
fn reflection_symbol_selector_prefers_reflect_probe_over_main() {
    let symbols = vec![
        sengoo_runtime::ReflectionSymbolMetadata {
            symbol: "tests/demo.sg::main".to_string(),
            signature: "pub|main|async=false|self=-|tp=[]|params=[]|ret=i64".to_string(),
            native_symbol: Some("main".to_string()),
        },
        sengoo_runtime::ReflectionSymbolMetadata {
            symbol: "tests/demo.sg::reflect_probe".to_string(),
            signature: "pub|reflect_probe|async=false|self=-|tp=[]|params=[]|ret=i64".to_string(),
            native_symbol: Some("reflect_probe".to_string()),
        },
    ];
    let picked = select_reflection_i64_zero_arity_symbol(&symbols);
    assert_eq!(picked.as_deref(), Some("reflect_probe"));
}

#[test]
fn reflection_symbol_selector_falls_back_to_main() {
    let symbols = vec![sengoo_runtime::ReflectionSymbolMetadata {
        symbol: "tests/demo.sg::main".to_string(),
        signature: "pub|main|async=false|self=-|tp=[]|params=[]|ret=i64".to_string(),
        native_symbol: Some("main".to_string()),
    }];
    let picked = select_reflection_i64_zero_arity_symbol(&symbols);
    assert_eq!(picked.as_deref(), Some("main"));
}

#[test]
fn reflection_symbol_selector_returns_none_without_supported_signature() {
    let symbols = vec![sengoo_runtime::ReflectionSymbolMetadata {
        symbol: "tests/demo.sg::flag".to_string(),
        signature: "pub|flag|async=false|self=-|tp=[]|params=[]|ret=bool".to_string(),
        native_symbol: Some("flag".to_string()),
    }];
    assert!(select_reflection_i64_zero_arity_symbol(&symbols).is_none());
}

#[test]
fn daemon_addr_prefers_explicit_value() {
    let addr = resolve_daemon_addr(Some("127.0.0.1:50001"));
    assert_eq!(addr, "127.0.0.1:50001");
}

#[test]
fn root_hashes_follow_semantic_source() {
    let source = "def main() -> i64 { 1 }";
    let (interface_hash, impl_hash) = super::resolve_root_hashes_for_request(source, None, None);
    assert_eq!(interface_hash, super::interface_fingerprint(source));
    assert_eq!(impl_hash, super::implementation_fingerprint(source));
}

#[test]
fn root_hashes_reuse_previous_interface_when_semantic_source_is_unchanged() {
    let source = "def main() -> i64 { 1 }";
    let impl_hash = super::implementation_fingerprint(source);
    let (interface_hash, actual_impl_hash) =
        super::resolve_root_hashes_for_request(source, Some(impl_hash), Some(333));
    assert_eq!(actual_impl_hash, impl_hash);
    assert_eq!(interface_hash, 333);
}

#[test]
fn root_hashes_recompute_interface_when_semantic_source_changes() {
    let source = "def main() -> i64 { 1 }";
    let source_impl_hash = super::implementation_fingerprint(source);
    let (interface_hash, impl_hash) =
        super::resolve_root_hashes_for_request(source, Some(source_impl_hash ^ 1), Some(333));
    assert_eq!(impl_hash, source_impl_hash);
    assert_eq!(interface_hash, super::interface_fingerprint(source));
}

#[test]
fn reflection_metadata_generation_filters_symbols() {
    let module_path = temp_sg_module(
        "meta-filter",
        "def add(a: i64, b: i64) -> i64 { a + b }\ndef sub(a: i64, b: i64) -> i64 { a - b }\n",
    );
    let module_id = super::canonical_or_lossy(&module_path);
    let graph = reflection_graph_for_module(&module_path);
    let options =
        reflection_options_from_cli(ReflectionMode::On, &[], &[format!("{}::add", module_id)]);
    let metadata = build_reflection_metadata(&graph, &options, None)
        .unwrap()
        .expect("reflection metadata");
    assert_eq!(metadata.schema_version, 1);
    assert_eq!(metadata.modules.len(), 1);
    assert_eq!(metadata.modules[0].symbols.len(), 1);
    assert_eq!(
        metadata.modules[0].symbols[0].symbol,
        format!("{}::add", module_id)
    );
    validate_reflection_metadata(&metadata).unwrap();
    let _ = fs::remove_file(module_path);
}

#[test]
fn reflection_metadata_generation_accepts_short_symbol_selector() {
    let module_path = temp_sg_module(
        "meta-short-filter",
        "def add(a: i64, b: i64) -> i64 { a + b }\ndef sub(a: i64, b: i64) -> i64 { a - b }\n",
    );
    let module_id = super::canonical_or_lossy(&module_path);
    let graph = reflection_graph_for_module(&module_path);
    let options = reflection_options_from_cli(ReflectionMode::On, &[], &[String::from("add")]);
    let metadata = build_reflection_metadata(&graph, &options, None)
        .unwrap()
        .expect("reflection metadata");
    assert_eq!(metadata.modules.len(), 1);
    assert_eq!(metadata.modules[0].symbols.len(), 1);
    assert_eq!(
        metadata.modules[0].symbols[0].symbol,
        format!("{}::add", module_id)
    );
    let _ = fs::remove_file(module_path);
}

#[test]
fn reflection_metadata_assigns_native_symbol_when_llvm_symbol_available() {
    let module_path = temp_sg_module(
        "meta-native-symbol",
        "def add(a: i64, b: i64) -> i64 { a + b }\ndef sub(a: i64, b: i64) -> i64 { a - b }\n",
    );
    let module_id = super::canonical_or_lossy(&module_path);
    let graph = reflection_graph_for_module(&module_path);
    let options =
        reflection_options_from_cli(ReflectionMode::On, &[], &[format!("{}::add", module_id)]);
    let llvm_defined = HashSet::from([String::from("add")]);
    let metadata = build_reflection_metadata(&graph, &options, Some(&llvm_defined))
        .unwrap()
        .expect("reflection metadata");

    assert_eq!(metadata.modules.len(), 1);
    assert_eq!(metadata.modules[0].symbols.len(), 1);
    assert_eq!(
        metadata.modules[0].symbols[0].native_symbol.as_deref(),
        Some("add")
    );
    let _ = fs::remove_file(module_path);
}

#[test]
fn reflection_metadata_rejects_symbol_missing_from_llvm_ir() {
    let module_path = temp_sg_module(
        "meta-missing-llvm",
        "def add(a: i64, b: i64) -> i64 { a + b }\n",
    );
    let module_id = super::canonical_or_lossy(&module_path);
    let graph = reflection_graph_for_module(&module_path);
    let options =
        reflection_options_from_cli(ReflectionMode::On, &[], &[format!("{}::add", module_id)]);
    let llvm_defined = HashSet::<String>::new();
    let err = build_reflection_metadata(&graph, &options, Some(&llvm_defined)).unwrap_err();
    assert!(err
        .to_string()
        .contains("is not emitted in LLVM IR (native symbol: add)"));
    let _ = fs::remove_file(module_path);
}

#[test]
fn reflection_metadata_rejects_unknown_symbol() {
    let module_path = temp_sg_module("meta-unknown", "def add(a: i64, b: i64) -> i64 { a + b }\n");
    let module_id = super::canonical_or_lossy(&module_path);
    let graph = reflection_graph_for_module(&module_path);
    let options = reflection_options_from_cli(
        ReflectionMode::On,
        &[],
        &[format!("{}::missing", module_id)],
    );
    let err = build_reflection_metadata(&graph, &options, None).unwrap_err();
    assert!(err
        .to_string()
        .contains("reflection symbol(s) not found in selected modules"));
    let _ = fs::remove_file(module_path);
}

#[test]
fn reflection_sidecar_emit_and_disabled_cleanup() {
    let module_path = temp_sg_module("sidecar", "def main() -> i64 { 1 }\n");
    let graph = reflection_graph_for_module(&module_path);
    let artifact = temp_artifact("reflect-sidecar", "exe");

    let options = reflection_options_from_cli(ReflectionMode::On, &[], &[]);
    maybe_emit_reflection_sidecar(&artifact, &graph, &options, None).unwrap();
    let sidecar_path = reflection_sidecar_path_for_artifact(&artifact);
    assert!(sidecar_path.exists());
    let metadata: ReflectionMetadata =
        serde_json::from_slice(&fs::read(&sidecar_path).unwrap()).unwrap();
    validate_reflection_metadata(&metadata).unwrap();

    let disabled = reflection_options_from_cli(ReflectionMode::Off, &[], &[]);
    maybe_emit_reflection_sidecar(&artifact, &graph, &disabled, None).unwrap();
    assert!(!sidecar_path.exists());

    let _ = fs::remove_file(module_path);
    let _ = fs::remove_file(artifact);
}

#[test]
fn daemon_build_request_uses_protocol_and_version() {
    let request = daemon_request_build(
        "tests/demo.sg",
        None,
        2,
        ContractChecksMode::Auto,
        false,
        false,
        false,
        FrontendJobs::Auto,
        false,
        ReflectionMode::Off,
        &[],
        &[],
        false,
    );
    assert_eq!(request.protocol_version, DAEMON_PROTOCOL_VERSION);
    assert_eq!(request.client_version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn daemon_default_addr_constant_has_host_and_port() {
    assert!(DEFAULT_DAEMON_ADDR.contains(':'));
}

#[tokio::test(flavor = "current_thread")]
async fn daemon_happy_path_handles_build_request() {
    let root = std::env::temp_dir().join(format!("sengoo-sgc-daemon-happy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let input = root.join("main.sg");
    fs::write(&input, "def main() -> i64 {\n    1\n}\n").unwrap();
    let input_string = input.to_string_lossy().to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        handle_daemon_client(stream).await.unwrap();
    });

    let request = daemon_request_build(
        &input_string,
        None,
        2,
        ContractChecksMode::Auto,
        true,
        false,
        false,
        FrontendJobs::Auto,
        false,
        ReflectionMode::Off,
        &[],
        &[],
        false,
    );
    let response = send_daemon_request(&addr.to_string(), &request)
        .await
        .unwrap();
    assert!(response.ok, "{}", response.message);

    server.await.unwrap();
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "current_thread")]
async fn daemon_and_oneshot_build_emit_same_workset_manifest() {
    let root =
        std::env::temp_dir().join(format!("sengoo-sgc-daemon-parity-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let input = root.join("main.sg");
    let input_text = "def main() -> i64 {\n    1\n}\n";
    fs::write(&input, input_text).unwrap();
    let input_string = input.to_string_lossy().to_string();
    let stem = input
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let manifest_path = root
        .join("build")
        .join("workset")
        .join(format!("{}.build.workset.json", stem));

    cmd_build(
        &input_string,
        None,
        2,
        ContractChecksMode::Auto,
        true,
        false,
        false,
        FrontendJobs::Auto,
        false,
        super::ReflectionCliOptions::default(),
        None,
        None,
        false,
    )
    .await
    .unwrap();
    let direct_manifest = fs::read_to_string(&manifest_path).unwrap();

    fs::remove_dir_all(root.join("build")).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        handle_daemon_client(stream).await.unwrap();
    });

    let request = daemon_request_build(
        &input_string,
        None,
        2,
        ContractChecksMode::Auto,
        true,
        false,
        false,
        FrontendJobs::Auto,
        false,
        ReflectionMode::Off,
        &[],
        &[],
        false,
    );
    let response = send_daemon_request(&addr.to_string(), &request)
        .await
        .unwrap();
    assert!(response.ok, "{}", response.message);
    server.await.unwrap();

    let daemon_manifest = fs::read_to_string(&manifest_path).unwrap();
    assert_eq!(direct_manifest, daemon_manifest);

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "current_thread")]
async fn daemon_client_fallback_when_server_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let input = bench_root_dir().join("tests").join("simple_array.sg");

    let outcome = dispatch_build_via_daemon(
        &addr.to_string(),
        input.to_string_lossy().as_ref(),
        None,
        2,
        ContractChecksMode::Auto,
        false,
        false,
        false,
        FrontendJobs::Auto,
        false,
        ReflectionMode::Off,
        &[],
        &[],
        false,
    )
    .await
    .unwrap();
    assert_eq!(outcome, DaemonDispatchOutcome::Fallback);
}

#[test]
fn build_graph_v2_contains_root_and_dependency_nodes() {
    let input = Path::new("tests/main.sg");
    let deps = vec![fp("tests/a.sg", 1, 11), fp("tests/b.sg", 2, 22)];
    let mut edges = BTreeMap::new();
    edges.insert(
        super::canonical_or_lossy(input),
        vec!["tests/a.sg".to_string(), "tests/b.sg".to_string()],
    );
    edges.insert("tests/a.sg".to_string(), Vec::new());
    edges.insert("tests/b.sg".to_string(), Vec::new());
    let graph = build_graph_v2_for_source(input, &deps, &edges, None, 88, 99);
    assert_eq!(graph.schema_version, BUILD_GRAPH_SCHEMA_VERSION);
    assert_eq!(graph.nodes.len(), 3);
    let root_module = super::canonical_or_lossy(input);
    let root = graph
        .nodes
        .iter()
        .find(|node| node.module_path == root_module)
        .expect("root node");
    assert_eq!(root.interface_hash, 88);
    assert_eq!(root.implementation_hash, 99);
    assert_eq!(root.depends_on.len(), 2);
    assert!(root.depends_on.contains(&"tests/a.sg".to_string()));
    assert!(root.depends_on.contains(&"tests/b.sg".to_string()));
}

#[test]
fn build_cache_schema_mismatch_forces_metadata_miss() {
    let key = build_cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 11)],
        1,
        false,
        false,
        false,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(777)),
        "tests/build/a.exe".to_string(),
    );
    let metadata = BuildCacheMetadata {
        cache_schema_version: 1,
        source_hash: 123,
        root_interface_hash: 101,
        root_implementation_hash: 123,
        module_fingerprints: vec![fp("tests/mod_a.sg", 11, 11)],
        opt_level: 1,
        contract_checks: false,
        debug_info: false,
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(777),
        llvm_ir_path: "tests/build/a.ll".to_string(),
        output_path: "tests/build/a.exe".to_string(),
        llvm_ir_hash: 777,
        object_path: Some("tests/build/a.obj".to_string()),
        build_graph_v2: None,
    };
    assert!(!build_metadata_matches(&metadata, &key));
}

#[test]
fn build_cache_miss_when_runtime_source_fingerprint_changes() {
    let key = build_cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 11)],
        1,
        false,
        false,
        false,
        RuntimeSourceIdentity::new(Some("tools/stdlib/runtime.c".to_string()), Some(22)),
        "tests/build/a.exe".to_string(),
    );
    let metadata = BuildCacheMetadata {
        cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        source_hash: 123,
        root_interface_hash: 101,
        root_implementation_hash: 123,
        module_fingerprints: vec![fp("tests/mod_a.sg", 11, 11)],
        opt_level: 1,
        contract_checks: false,
        debug_info: false,
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(11),
        llvm_ir_path: "tests/build/a.ll".to_string(),
        output_path: "tests/build/a.exe".to_string(),
        llvm_ir_hash: 777,
        object_path: Some("tests/build/a.obj".to_string()),
        build_graph_v2: None,
    };

    assert!(!build_metadata_matches(&metadata, &key));
    assert!(super::build_cache_mismatch_reasons(&metadata, &key)
        .iter()
        .any(|reason| reason == "runtime source changed"));
}

#[test]
fn incremental_link_reuse_requires_matching_ir_hash() {
    let object_path = temp_object_file("ir-hash");
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 0,
            implementation_hash: 0,
            depends_on: vec![],
            object_path: Some(object_path.to_string_lossy().to_string()),
            functions: vec![],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };
    let metadata = BuildCacheMetadata {
        cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        source_hash: 1,
        root_interface_hash: 1,
        root_implementation_hash: 1,
        module_fingerprints: vec![],
        opt_level: 2,
        contract_checks: false,
        debug_info: false,
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(777),
        llvm_ir_path: "tests/build/main.ll".to_string(),
        output_path: "tests/build/main.exe".to_string(),
        llvm_ir_hash: 10,
        object_path: Some(object_path.to_string_lossy().to_string()),
        build_graph_v2: Some(graph.clone()),
    };

    let err = can_use_incremental_link_with_metadata(
        &metadata,
        11,
        &object_path,
        "tests/build/main.exe",
        Some("tools/stdlib/runtime.c"),
        2,
        false,
        false,
        &graph,
    )
    .unwrap_err();
    assert!(err.contains("LLVM IR changed"));

    let _ = fs::remove_file(&object_path);
}

#[test]
fn run_incremental_link_reuse_accepts_matching_metadata() {
    let object_path = temp_object_file("run-ok");
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 0,
            implementation_hash: 0,
            depends_on: vec![],
            object_path: Some(object_path.to_string_lossy().to_string()),
            functions: vec![],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };
    let metadata = RunCacheMetadata {
        source_hash: 1,
        root_interface_hash: 1,
        root_implementation_hash: 1,
        module_fingerprints: vec![],
        opt_level: 2,
        contract_checks: false,
        debug_info: false,
        requested_engine: RunEngine::Native,
        resolved_engine: RunEngine::Native,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(777),
        llvm_ir_path: "tests/build/main.ll".to_string(),
        executable_path: Some("tests/build/main.exe".to_string()),
        llvm_ir_hash: 44,
        object_path: Some(object_path.to_string_lossy().to_string()),
        build_graph_v2: Some(graph.clone()),
    };

    assert!(can_use_incremental_link_with_run_metadata(
        &metadata,
        44,
        &object_path,
        Some("tools/stdlib/runtime.c"),
        2,
        false,
        false,
        RunEngine::Native,
        RunEngine::Native,
        &graph,
    )
    .is_ok());

    let _ = fs::remove_file(&object_path);
}

#[test]
fn cached_native_recovery_prefers_existing_object() {
    let plan = derive_cached_native_recovery_plan(true, true);
    assert_eq!(plan, Some(CachedNativeRecoveryPlan::RelinkFromObject));
}

#[test]
fn cached_native_recovery_can_rebuild_object_from_ir() {
    let plan = derive_cached_native_recovery_plan(true, false);
    assert_eq!(
        plan,
        Some(CachedNativeRecoveryPlan::RebuildObjectFromCachedIr)
    );
}

#[test]
fn cached_native_recovery_requires_cached_ir_or_object() {
    let plan = derive_cached_native_recovery_plan(false, false);
    assert_eq!(plan, None);
}

#[test]
fn async_native_runtime_executes_async_main_end_to_end() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}

async def main() -> i64 {
    let f = add_one(40);
    let a = await f;
    let b = await add_one(a);
    b + 1
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async source should compile to LLVM IR");
    let ll_path = temp_artifact("async-native-main", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-native-main", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async native executable should run");
    assert_eq!(output.status.code(), Some(43));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_awaits_user_future_impl() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
struct Poll<T> {
    is_ready: bool,
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T> {
        Poll { is_ready: false, value: 0 }
    }
}

struct ImmediateFuture {
    value: i64,
}

impl Future<i64> for ImmediateFuture {
    def poll(&mut self, ctx: AsyncContext) -> Poll<i64> {
        Poll { is_ready: true, value: self.value }
    }
}

async def main() -> i64 {
    let future = ImmediateFuture { value: 42 };
    await future
}
"#;

    let llvm_ir = compile_source(source, 1).expect("user Future source should compile");
    let ll_path = temp_artifact("async-user-future", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact("async-user-future", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("user Future executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_user_future_local_parameter_return_flow() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
struct Poll<T> {
    is_ready: bool,
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T> {
        Poll { is_ready: false, value: 0 }
    }
}

struct ImmediateFuture {
    value: i64,
}

impl Future<i64> for ImmediateFuture {
    def poll(&mut self, ctx: AsyncContext) -> Poll<i64> {
        Poll { is_ready: true, value: self.value }
    }
}

def make_future(value: i64) -> ImmediateFuture {
    ImmediateFuture { value: value }
}

async def consume_future(future: ImmediateFuture) -> i64 {
    await future
}

async def main() -> i64 {
    let local_future = make_future(10);
    let first = await local_future;
    let second = await consume_future(make_future(20));
    let returned_future = make_future(12);
    let third = await returned_future;
    first + second + third
}
"#;

    let llvm_ir = compile_source(source, 1).expect("user Future flow source should compile");
    let ll_path = temp_artifact("async-user-future-flow", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact(
        "async-user-future-flow",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("user Future flow executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_does_not_repoll_user_future_after_ready() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-user-future-ready-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\n{}",
            runtime_source,
            r#"
static long long sengoo_test_user_future_poll_calls = 0;

long long sengoo_test_user_future_tick(void) {
    sengoo_test_user_future_poll_calls += 1;
    return sengoo_test_user_future_poll_calls;
}

long long sengoo_test_user_future_calls(void) {
    return sengoo_test_user_future_poll_calls;
}
"#
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_user_future_tick() -> i64;
    fn sengoo_test_user_future_calls() -> i64;
}

struct Poll<T> {
    is_ready: bool,
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T> {
        Poll { is_ready: false, value: 0 }
    }
}

struct ImmediateFuture {
    value: i64,
}

impl Future<i64> for ImmediateFuture {
    def poll(&mut self, ctx: AsyncContext) -> Poll<i64> {
        let calls = sengoo_test_user_future_tick();
        Poll { is_ready: true, value: self.value }
    }
}

async def main() -> i64 {
    let value = await ImmediateFuture { value: 40 };
    value + sengoo_test_user_future_calls()
}
"#;

    let llvm_ir = compile_source(source, 1).expect("ready user Future source should compile");
    let ll_path = temp_artifact("async-user-future-ready-once", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact(
        "async-user-future-ready-once",
        if cfg!(windows) { "exe" } else { "" },
    );
    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("ready user Future executable should run");
    assert_eq!(output.status.code(), Some(41));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_preserves_inline_user_future_across_pending() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-user-future-pending-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\n{}",
            runtime_source,
            r#"
long long sengoo_test_user_future_tick(void) {
    static long long calls = 0;
    calls += 1;
    return calls;
}
"#
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_user_future_tick() -> i64;
}

struct Poll<T> {
    is_ready: bool,
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T> {
        Poll { is_ready: false, value: 0 }
    }
}

struct PendingOnceFuture {
    value: i64,
}

impl Future<i64> for PendingOnceFuture {
    def poll(&mut self, ctx: AsyncContext) -> Poll<i64> {
        let calls = sengoo_test_user_future_tick();
        if calls < 2 {
            Poll { is_ready: false, value: 0 }
        } else {
            Poll { is_ready: true, value: self.value }
        }
    }
}

async def main() -> i64 {
    await PendingOnceFuture { value: 77 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("pending user Future source should compile");
    let ll_path = temp_artifact("async-user-future-pending", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact(
        "async-user-future-pending",
        if cfg!(windows) { "exe" } else { "" },
    );
    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("pending user Future executable should run");
    assert_eq!(output.status.code(), Some(77));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_executes_async_block_with_capture() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def main() -> i64 {
    let base = 40;
    let fut = async { base + 3 };
    await fut
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async block source should compile to LLVM IR");
    let ll_path = temp_artifact("async-block-capture", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-block-capture",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async block native executable should run");
    assert_eq!(output.status.code(), Some(43));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_executes_sleep_builtin() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def main() -> i64 {
    await sleep(20);
    42
}
"#;

    let llvm_ir = compile_source(source, 1).expect("sleep source should compile to LLVM IR");
    let ll_path = temp_artifact("async-sleep", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-sleep", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let run_start = SystemTime::now();
    let output = Command::new(&exe_path)
        .output()
        .expect("sleep native executable should run");
    let elapsed_ms = run_start.elapsed().unwrap().as_millis();

    assert_eq!(output.status.code(), Some(42));
    assert!(
        elapsed_ms >= 10,
        "sleep should delay execution measurably, only took {}ms",
        elapsed_ms
    );

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_waits_for_spawned_sleep_future_before_exit() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def main() -> i64 {
    let background = spawn(sleep(20));
    await sleep(1);
    42
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("spawned sleep source should compile to LLVM IR");
    let ll_path = temp_artifact("async-spawn-sleep", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-spawn-sleep", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let run_start = SystemTime::now();
    let output = Command::new(&exe_path)
        .output()
        .expect("spawned sleep native executable should run");
    let elapsed_ms = run_start.elapsed().unwrap().as_millis();

    assert_eq!(output.status.code(), Some(42));
    assert!(
        elapsed_ms >= 10,
        "spawned sleep future should keep the runtime alive, only took {}ms",
        elapsed_ms
    );

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_timeout_reports_not_ready_then_allows_await() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def child() -> i64 {
    await sleep(20);
    7
}

async def main() -> i64 {
    let fut = child();
    let ready = await timeout(fut, 1);
    if ready {
        0
    } else {
        await fut
    }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("timeout source should compile to LLVM IR");
    let ll_path = temp_artifact("async-timeout", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-timeout", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("timeout native executable should run");
    assert_eq!(output.status.code(), Some(7));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_timeout_ready_then_still_allows_await() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def child() -> i64 {
    9
}

async def main() -> i64 {
    let fut = child();
    let ready = await timeout(fut, 50);
    if ready {
        await fut
    } else {
        0
    }
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("timeout-ready source should compile to LLVM IR");
    let ll_path = temp_artifact("async-timeout-ready", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-timeout-ready",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("timeout-ready native executable should run");
    assert_eq!(output.status.code(), Some(9));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_timeout_future_binding_can_be_awaited() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def child() -> i64 {
    await sleep(20);
    7
}

async def main() -> i64 {
    let fut = timeout(child(), 1);
    if await fut {
        0
    } else {
        42
    }
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("bound timeout source should compile to LLVM IR");
    let ll_path = temp_artifact("async-timeout-bound-fut", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-timeout-bound-fut",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("bound timeout native executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_executes_spawned_future() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def add_one(x: i64) -> i64 { x + 1 }
async def slow_step() -> i64 { 0 }
async def slow() -> i64 {
    let first = await slow_step();
    let second = await slow_step();
    0
}
async def main() -> i64 {
    let task = spawn(add_one(41));
    let waited = await slow();
    await task
}
"#;

    let llvm_ir = compile_source(source, 1).expect("spawn source should compile to LLVM IR");
    let ll_path = temp_artifact("async-spawn", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-spawn", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("spawn native executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_spawn_task_reports_completed_status() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def child() -> i64 {
    await sleep(5);
    7
}

async def main() -> i64 {
    let task = spawn_task(child());
    let pending = task_status(task);
    await sleep(15);
    let done = task_status(task);
    if pending == 1 {
        if done == 2 { 42 } else { 0 }
    } else {
        0
    }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("spawn_task source should compile to LLVM IR");
    let ll_path = temp_artifact("async-spawn-task-status", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-spawn-task-status",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("spawn_task status executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_cancel_task_marks_canceled_status() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def main() -> i64 {
    let task = spawn_task(sleep(20));
    let canceled = cancel_task(task);
    let status = task_status(task);
    if canceled {
        if status == 3 { 42 } else { 0 }
    } else {
        0
    }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("cancel_task source should compile to LLVM IR");
    let ll_path = temp_artifact("async-cancel-task-status", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-cancel-task-status",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("cancel_task executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_cancel_task_prevents_post_await_code() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-cancel-post-await-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_cancel_post_await_counter = 0;\nlong long sengoo_test_cancel_post_await_reset(void) {{ sengoo_test_cancel_post_await_counter = 0; return 0; }}\nlong long sengoo_test_cancel_post_await_mark(void) {{ sengoo_test_cancel_post_await_counter += 1; return sengoo_test_cancel_post_await_counter; }}\nlong long sengoo_test_cancel_post_await_get(void) {{ return sengoo_test_cancel_post_await_counter; }}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_cancel_post_await_reset() -> i64;
    fn sengoo_test_cancel_post_await_mark() -> i64;
    fn sengoo_test_cancel_post_await_get() -> i64;
}

async def child() -> i64 {
    await sleep(20);
    sengoo_test_cancel_post_await_mark();
    7
}

async def main() -> i64 {
    sengoo_test_cancel_post_await_reset();
    let task = spawn_task(child());
    await sleep(1);
    let canceled = cancel_task(task);
    await sleep(40);
    let status = task_status(task);
    let marks = sengoo_test_cancel_post_await_get();
    if canceled {
        if status == 3 {
            if marks == 0 { 42 } else { 1 }
        } else {
            2
        }
    } else {
        3
    }
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("cancel post-await source should compile to LLVM IR");
    let ll_path = temp_artifact("async-cancel-post-await", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-cancel-post-await",
        if cfg!(windows) { "exe" } else { "" },
    );
    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("cancel post-await executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_polls_spawned_future_while_parent_waits() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-spawn-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_spawn_counter = 0;\nlong long sengoo_test_spawn_reset(void) {{ sengoo_test_spawn_counter = 0; return 0; }}\nlong long sengoo_test_spawn_mark(void) {{ sengoo_test_spawn_counter += 1; return sengoo_test_spawn_counter; }}\nlong long sengoo_test_spawn_get(void) {{ return sengoo_test_spawn_counter; }}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_spawn_reset() -> i64;
    fn sengoo_test_spawn_mark() -> i64;
    fn sengoo_test_spawn_get() -> i64;
}

async def child() -> i64 {
    sengoo_test_spawn_mark();
    7
}

async def slow_step() -> i64 { 0 }
async def slow() -> i64 {
    let first = await slow_step();
    let second = await slow_step();
    0
}

async def main() -> i64 {
    sengoo_test_spawn_reset();
    let task = spawn(child());
    let waited = await slow();
    if sengoo_test_spawn_get() == 1 {
        await task
    } else {
        0
    }
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("spawn polling source should compile to LLVM IR");
    let ll_path = temp_artifact("async-spawn-polling", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();

    let exe_path = temp_artifact(
        "async-spawn-polling",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("spawn polling executable should run");
    assert_eq!(output.status.code(), Some(7));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_join_waits_for_all_spawned_futures() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-join-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_join_counter = 0;\nlong long sengoo_test_join_reset(void) {{ sengoo_test_join_counter = 0; return 0; }}\nlong long sengoo_test_join_mark(long long bit) {{ sengoo_test_join_counter |= bit; return sengoo_test_join_counter; }}\nlong long sengoo_test_join_get(void) {{ return sengoo_test_join_counter; }}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_join_reset() -> i64;
    fn sengoo_test_join_mark(bit: i64) -> i64;
    fn sengoo_test_join_get() -> i64;
}

async def slow_step() -> i64 { 0 }

async def child_one() -> i64 {
    let waited = await slow_step();
    sengoo_test_join_mark(1)
}

async def child_two() -> i64 {
    let waited = await slow_step();
    sengoo_test_join_mark(2)
}

async def main() -> i64 {
    sengoo_test_join_reset();
    let first = spawn(child_one());
    let second = spawn(child_two());
    join(first, second);
    if sengoo_test_join_get() == 3 {
        7
    } else {
        0
    }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("join source should compile to LLVM IR");
    let ll_path = temp_artifact("async-join", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();

    let exe_path = temp_artifact("async-join", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("join native executable should run");
    assert_eq!(output.status.code(), Some(7));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_select_returns_first_completed_value() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-select-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_select_fast_mark(void) {{ return 7; }}\nlong long sengoo_test_select_slow_mark(void) {{ return 9; }}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_select_fast_mark() -> i64;
    fn sengoo_test_select_slow_mark() -> i64;
}

async def slow_step() -> i64 { 0 }

async def fast() -> i64 {
    sengoo_test_select_fast_mark()
}

async def slow() -> i64 {
    let waited = await slow_step();
    sengoo_test_select_slow_mark()
}

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    select(first, second)
}
"#;

    let llvm_ir = compile_source(source, 1).expect("select source should compile to LLVM IR");
    let ll_path = temp_artifact("async-select", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();

    let exe_path = temp_artifact("async-select", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("select native executable should run");
    assert_eq!(output.status.code(), Some(7));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_select_cancel_prevents_loser_post_await_code() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-select-cancel-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_select_cancel_counter = 0;\nlong long sengoo_test_select_cancel_reset(void) {{ sengoo_test_select_cancel_counter = 0; return 0; }}\nlong long sengoo_test_select_cancel_mark(void) {{ sengoo_test_select_cancel_counter += 1; return sengoo_test_select_cancel_counter; }}\nlong long sengoo_test_select_cancel_get(void) {{ return sengoo_test_select_cancel_counter; }}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_select_cancel_reset() -> i64;
    fn sengoo_test_select_cancel_mark() -> i64;
    fn sengoo_test_select_cancel_get() -> i64;
}

async def fast() -> i64 {
    7
}

async def slow() -> i64 {
    await sleep(20);
    sengoo_test_select_cancel_mark();
    9
}

async def main() -> i64 {
    sengoo_test_select_cancel_reset();
    let first = spawn(fast());
    let second = spawn(slow());
    let picked = select_cancel(first, second);
    await sleep(40);
    let marks = sengoo_test_select_cancel_get();

    sengoo_test_select_cancel_reset();
    let mixed = select_cancel(spawn(fast()), slow());
    await sleep(40);
    let mixed_marks = sengoo_test_select_cancel_get();

    let shared = spawn(fast());
    let alias = select_cancel(shared, shared);

    if picked == 7 {
        if marks == 0 {
            if mixed == 7 {
                if mixed_marks == 0 {
                    if alias == 7 { 42 } else { 5 }
                } else {
                    1
                }
            } else {
                2
            }
        } else {
            3
        }
    } else {
        4
    }
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("select_cancel source should compile to LLVM IR");
    let ll_path = temp_artifact("async-select-cancel", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();

    let exe_path = temp_artifact(
        "async-select-cancel",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("select_cancel native executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_select_cancel_handles_three_and_eight_operands() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-select-cancel-n-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_select_cancel_n_counter = 0;\nlong long sengoo_test_select_cancel_n_reset(void) {{ sengoo_test_select_cancel_n_counter = 0; return 0; }}\nlong long sengoo_test_select_cancel_n_mark(void) {{ sengoo_test_select_cancel_n_counter += 1; return sengoo_test_select_cancel_n_counter; }}\nlong long sengoo_test_select_cancel_n_get(void) {{ return sengoo_test_select_cancel_n_counter; }}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_select_cancel_n_reset() -> i64;
    fn sengoo_test_select_cancel_n_mark() -> i64;
    fn sengoo_test_select_cancel_n_get() -> i64;
}

async def fast() -> i64 { 7 }

async def slow() -> i64 {
    await sleep(20);
    sengoo_test_select_cancel_n_mark();
    9
}

async def main() -> i64 {
    sengoo_test_select_cancel_n_reset();
    let three = select_cancel(spawn(slow()), spawn(fast()), spawn(slow()));
    await sleep(40);
    let three_marks = sengoo_test_select_cancel_n_get();

    sengoo_test_select_cancel_n_reset();
    let eight = select_cancel(
        spawn(slow()),
        spawn(slow()),
        spawn(slow()),
        spawn(slow()),
        spawn(slow()),
        spawn(slow()),
        spawn(slow()),
        spawn(fast()),
    );
    await sleep(40);
    let eight_marks = sengoo_test_select_cancel_n_get();

    if three == 7 {
        if three_marks == 0 {
            if eight == 7 {
                if eight_marks == 0 { 42 } else { 1 }
            } else {
                2
            }
        } else {
            3
        }
    } else {
        4
    }
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("select_cancel n-ary source should compile to LLVM IR");
    let ll_path = temp_artifact("async-select-cancel-n", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();

    let exe_path = temp_artifact(
        "async-select-cancel-n",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("select_cancel n-ary native executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_select_returns_first_completed_bool_value() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def fast() -> bool { true }
async def slow_step() -> i64 { 0 }
async def slow() -> bool {
    let waited = await slow_step();
    false
}

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    if select(first, second) { 1 } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("bool select source should compile to LLVM IR");
    let ll_path = temp_artifact("async-select-bool", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact("async-select-bool", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("bool select executable should run");
    assert_eq!(output.status.code(), Some(1));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_select_returns_first_completed_f64_value() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def fast() -> f64 { 3.5 }
async def slow_step() -> i64 { 0 }
async def slow() -> f64 {
    let waited = await slow_step();
    1.5
}

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    if select(first, second) > 3.0 { 1 } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("f64 select source should compile to LLVM IR");
    let ll_path = temp_artifact("async-select-f64", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact("async-select-f64", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("f64 select executable should run");
    assert_eq!(output.status.code(), Some(1));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_select_returns_first_completed_struct_value() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
struct Choice {
    value: i64,
    ready: bool,
}

async def fast() -> Choice {
    Choice { value: 7, ready: true }
}

async def slow_step() -> i64 { 0 }

async def slow() -> Choice {
    let waited = await slow_step();
    Choice { value: 9, ready: false }
}

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    let picked = select(first, second);
    if picked.ready { picked.value } else { 0 }
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("struct select source should compile to LLVM IR");
    let ll_path = temp_artifact("async-select-struct", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact(
        "async-select-struct",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("struct select executable should run");
    assert_eq!(output.status.code(), Some(7));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_select_returns_first_completed_generic_struct_value() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
struct Wrap<T> {
    value: T,
    bonus: i64,
}

async def fast() -> Wrap<i64> {
    Wrap { value: 40, bonus: 2 }
}

async def slow_step() -> i64 { 0 }

async def slow() -> Wrap<i64> {
    let waited = await slow_step();
    Wrap { value: 9, bonus: 1 }
}

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    let picked = select(first, second);
    picked.value + picked.bonus
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("generic struct select source should compile to LLVM IR");
    let ll_path = temp_artifact("async-select-generic-struct", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact(
        "async-select-generic-struct",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("generic struct select executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_select_returns_first_completed_tuple_value() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def fast() -> (i64, bool) {
    (7, true)
}

async def slow_step() -> i64 { 0 }

async def slow() -> (i64, bool) {
    let waited = await slow_step();
    (9, false)
}

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    let picked = select(first, second);
    if picked.1 { picked.0 } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("tuple select source should compile to LLVM IR");
    let ll_path = temp_artifact("async-select-tuple", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact("async-select-tuple", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("tuple select executable should run");
    assert_eq!(output.status.code(), Some(7));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_preserves_live_locals_across_resume() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def step1() -> i64 { 10 }
async def step2(x: i64) -> i64 { x + 20 }

async def main() -> i64 {
    let a = await step1();
    let b = await step2(a);
    a + b
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async source should compile to LLVM IR");
    let ll_path = temp_artifact("async-live-local", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-live-local", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async native executable should run");
    assert_eq!(output.status.code(), Some(40));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_preserves_payloadless_enum_across_resume() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
enum State { Cold, Hot }

async def step() -> i64 { 1 }

async def main() -> i64 {
    let state = State::Hot;
    let waited = await step();
    match state {
        State::Cold => 0,
        State::Hot => waited + 41,
    }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async enum source should compile to LLVM IR");
    let ll_path = temp_artifact("async-payloadless-enum", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-payloadless-enum",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async enum executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_executes_if_structured_multi_await_body() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def step1() -> i64 { 1 }
async def step2(x: i64) -> i64 { x + 1 }

async def main() -> i64 {
    let flag: bool = false;
    let seed = if flag { 40 } else { 41 };
    let a = await step1();
    let b = await step2(a + seed);
    b
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("if-structured async source should compile to LLVM IR");
    let ll_path = temp_artifact("async-if-structured", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-if-structured",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("if-structured async executable should run");
    assert_eq!(output.status.code(), Some(43));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_executes_loop_with_await_body() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def step() -> i64 { 1 }

async def main() -> i64 {
    let mut x = 0;
    while x < 3 {
        let y = await step();
        x = x + y;
    }
    x
}
"#;

    let llvm_ir = compile_source(source, 1).expect("loop async source should compile to LLVM IR");
    let ll_path = temp_artifact("async-loop-body", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-loop-body", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("loop async executable should run");
    assert_eq!(output.status.code(), Some(3));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_executes_sleep_inside_loop_body() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def main() -> i64 {
    let mut ticks = 0;
    while ticks < 3 {
        await sleep(1);
        ticks = ticks + 1;
    }
    ticks
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("sleep-loop async source should compile to LLVM IR");
    let ll_path = temp_artifact("async-sleep-loop-body", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        "async-sleep-loop-body",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("sleep-loop async executable should run");
    assert_eq!(output.status.code(), Some(3));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_executes_match_with_await_arms() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def a() -> i64 { 10 }
async def b() -> i64 { 20 }

async def main() -> i64 {
    let x = 0;
    let y = match x {
        0 => await a(),
        _ => await b(),
    };
    y + 1
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("match-shaped async source should compile to LLVM IR");
    let ll_path = temp_artifact("async-match-arms", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-match-arms", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("match-shaped async executable should run");
    assert_eq!(output.status.code(), Some(11));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_preserves_bool_locals_across_resume() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }

async def main() -> i64 {
    let keep: bool = true;
    let first = await step1();
    let value = await step2(first);
    if keep { value } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async bool source should compile to LLVM IR");
    let ll_path = temp_artifact("async-bool-local", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-bool-local", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async bool executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_preserves_ref_locals_across_resume() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def step1() -> i64 { 0 }
async def step2(x: i64) -> i64 { x + 42 }

async def main() -> i64 {
    let base = 41;
    let keep = &base;
    let first = await step1();
    let value = await step2(first);
    if *keep == 41 { value } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async ref source should compile to LLVM IR");
    let ll_path = temp_artifact("async-ref-local", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-ref-local", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async ref executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_preserves_f64_locals_across_resume() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }

async def main() -> i64 {
    let keep: f64 = 3.14;
    let first = await step1();
    let value = await step2(first);
    if keep > 3.0 { value } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async f64 source should compile to LLVM IR");
    let ll_path = temp_artifact("async-f64-local", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-f64-local", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async f64 executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_preserves_f32_locals_across_resume() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-f32-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nfloat sengoo_test_get_f32(void) {{ return 3.25f; }}\nfloat sengoo_test_get_f32_threshold(void) {{ return 3.0f; }}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_get_f32() -> f32;
    fn sengoo_test_get_f32_threshold() -> f32;
}

async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }

async def main() -> i64 {
    let keep = sengoo_test_get_f32();
    let first = await step1();
    let value = await step2(first);
    if keep > sengoo_test_get_f32_threshold() { value } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async f32 source should compile to LLVM IR");
    let ll_path = temp_artifact("async-f32-local", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();
    let exe_path = temp_artifact("async-f32-local", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async f32 executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn native_runtime_alloc_respects_requested_alignment() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("alloc-align-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_alloc_alignment(long long size, long long align) {{\n    void* ptr = sengoo_alloc(size, align);\n    if (!ptr) {{ return 0; }}\n    unsigned long long addr = (unsigned long long)(uintptr_t)ptr;\n    long long ok = (align <= 1) ? 1 : ((addr % (unsigned long long)align) == 0 ? 1 : 0);\n    sengoo_free(ptr, size, align);\n    return ok;\n}}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_alloc_alignment(size: i64, align: i64) -> i64;
}

def main() -> i64 {
    if sengoo_test_alloc_alignment(64, 16) == 1 {
        if sengoo_test_alloc_alignment(96, 32) == 1 { 1 } else { 0 }
    } else {
        0
    }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("alignment probe should compile to LLVM IR");
    let ll_path = temp_artifact("alloc-align", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();
    let exe_path = temp_artifact("alloc-align", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("alignment probe executable should run");
    assert_eq!(output.status.code(), Some(1));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn native_runtime_invalid_async_frame_access_aborts_in_debug_builds() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-frame-guard-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_invalid_async_frame_load_reaches_end(void) {{\n    long long frame = sengoo_async_frame_alloc(1);\n    if (frame == 0) {{ return 0; }}\n    (void)sengoo_async_frame_load(frame, 1);\n    sengoo_async_frame_free(frame);\n    return 1;\n}}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_invalid_async_frame_load_reaches_end() -> i64;
}

def main() -> i64 {
    sengoo_test_invalid_async_frame_load_reaches_end()
}
"#;

    let llvm_ir = compile_source(source, 1).expect("frame guard probe should compile to LLVM IR");
    let ll_path = temp_artifact("async-frame-guard", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();
    let exe_path = temp_artifact("async-frame-guard", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("frame guard probe executable should run");
    assert_ne!(
        output.status.code(),
        Some(1),
        "invalid async frame access should not reach the end of the probe function"
    );

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn native_runtime_invalid_async_frame_free_aborts_in_debug_builds() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let runtime_source = fs::read_to_string(&runtime_c).expect("runtime.c should be readable");
    let custom_runtime_c = temp_artifact("async-frame-free-guard-runtime", "c");
    fs::write(
        &custom_runtime_c,
        format!(
            "{}\n\nlong long sengoo_test_invalid_async_frame_free_reaches_end(void) {{\n    sengoo_async_frame_free(0);\n    return 1;\n}}\n",
            runtime_source
        ),
    )
    .unwrap();

    let source = r#"
extern "C" {
    fn sengoo_test_invalid_async_frame_free_reaches_end() -> i64;
}

def main() -> i64 {
    sengoo_test_invalid_async_frame_free_reaches_end()
}
"#;

    let llvm_ir =
        compile_source(source, 1).expect("frame free guard probe should compile to LLVM IR");
    let ll_path = temp_artifact("async-frame-free-guard", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let custom_runtime_c_str = custom_runtime_c.to_string_lossy().to_string();
    let exe_path = temp_artifact(
        "async-frame-free-guard",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(
        &clang,
        &ll_path,
        &exe_path,
        Some(&custom_runtime_c_str),
        1,
        None,
        None,
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("frame free guard probe executable should run");
    assert_ne!(
        output.status.code(),
        Some(1),
        "invalid async frame free should not reach the end of the probe function"
    );

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&custom_runtime_c);
}

#[test]
fn async_native_runtime_preserves_struct_locals_across_resume() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
struct Point { x: i64, y: i64 }

async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }

async def main() -> i64 {
    let point = Point { x: 1, y: 2 };
    let first = await step1();
    let value = await step2(first);
    if point.x + point.y == 3 { value } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async struct source should compile to LLVM IR");
    let ll_path = temp_artifact("async-struct-local", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-struct-local", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async struct executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_preserves_array_locals_across_resume() {
    let Some(clang) = find_clang() else {
        return;
    };

    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = r#"
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }

async def main() -> i64 {
    let values = [1, 2, 3];
    let first = await step1();
    let value = await step2(first);
    if values[0] + values[2] == 4 { value } else { 0 }
}
"#;

    let llvm_ir = compile_source(source, 1).expect("async array source should compile to LLVM IR");
    let ll_path = temp_artifact("async-array-local", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-array-local", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async array executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_concurrent_thread_pool_and_spawn_blocking() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = format!(
        "{}\n\n{}",
        load_async_runtime_stdlib(),
        r#"
def heavy() -> i64 { 55 }

async def main() -> i64 {
    let enabled = runtime_enable_thread_pool(2);
    if !enabled.is_ok { return 0; }
    let fut = spawn_blocking_future_i64(| | heavy());
    await fut
}
"#
    );

    let llvm_ir =
        compile_source(&source, 1).expect("concurrent spawn_blocking source should compile");
    let ll_path = temp_artifact("async-concurrent-blocking", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact(
        "async-concurrent-blocking",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();
    let output = Command::new(&exe_path)
        .output()
        .expect("executable should run");
    assert_eq!(output.status.code(), Some(55));
    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_spawn_blocking_reports_unsupported_without_pool() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = format!(
        "{}\n\n{}",
        load_async_runtime_stdlib(),
        r#"
def heavy() -> i64 { 1 }

async def main() -> i64 {
    let spawned = spawn_blocking_i64(| | heavy());
    if spawned.is_ok { 0 } else { spawned.error }
}
"#
    );

    let llvm_ir = compile_source(&source, 1).expect("unsupported spawn_blocking should compile");
    let ll_path = temp_artifact("async-concurrent-unsupported", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact(
        "async-concurrent-unsupported",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();
    let output = Command::new(&exe_path)
        .output()
        .expect("executable should run");
    assert_eq!(output.status.code(), Some(8));
    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_native_runtime_concurrent_channel_round_trip() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = format!(
        "{}\n\n{}",
        load_async_runtime_stdlib(),
        r#"
async def main() -> i64 {
    let pair = channel_bounded(4);
    if pair.is_ok {
        let sender = channel_pair_sender(pair.value);
        let receiver = channel_pair_receiver(pair.value);
        let send_fut = channel_send_i64(sender, 17);
        let send_outcome = await send_fut;
        if send_outcome.is_ok {
            let recv_fut = channel_recv_i64(receiver);
            let recv_outcome = await recv_fut;
            if recv_outcome.is_ok { recv_outcome.value } else { 12 }
        } else {
            11
        }
    } else {
        10
    }
}
"#
    );

    let llvm_ir = compile_source(&source, 1).expect("concurrent channel source should compile");
    let ll_path = temp_artifact("async-concurrent-channel", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact(
        "async-concurrent-channel",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();
    let output = Command::new(&exe_path)
        .output()
        .expect("executable should run");
    assert_eq!(output.status.code(), Some(17));
    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_stdlib_arc_i64_bool_runtime_counts_and_reads() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let source = format!(
        "{}\n\n{}",
        load_async_runtime_stdlib(),
        r#"
def consume_i64_arc(value: Arc<i64>) {
    let observed = value.get();
}

def consume_bool_arc(value: Arc<bool>) {
    let observed = value.get();
}

def main() -> i64 {
    let shared = arc_new_i64(40);
    let cloned = shared.clone_arc();
    let before_drop = shared.strong_count();
    let value = cloned.get();
    consume_i64_arc(cloned);
    let after_drop = shared.strong_count();
    let flag = arc_new_bool(true);
    let flag_clone = flag.clone_arc();
    let flag_count = flag.strong_count();
    consume_bool_arc(flag_clone);
    let flag_after_drop = flag.strong_count();
    let ok = value == 40
        && before_drop == 2
        && after_drop == 1
        && flag.get()
        && flag_count == 2
        && flag_after_drop == 1;
    if ok {
        42
    } else {
        1
    }
}
"#
    );

    let llvm_ir = compile_source(&source, 1).expect("Arc stdlib source should compile");
    let ll_path = temp_artifact("async-arc", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let exe_path = temp_artifact("async-arc", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1, None, None).unwrap();
    let output = Command::new(&exe_path)
        .output()
        .expect("Arc executable should run");
    assert_eq!(output.status.code(), Some(42));
    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_stdlib_generic_arc_mutex_joins_cross_thread_workers() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "async-generic-arc-mutex",
        r#"
import std::async;

async def main() -> i64 {
    let enabled = runtime_enable_thread_pool(4);
    if !enabled.is_ok { return enabled.error; }

    let counter: Arc<Mutex<i64>> = arc_new(mutex_new(2));
    let first = spawn_shared_counter_i64(counter.clone_arc(), 1, 5);
    let second = spawn_shared_counter_i64(counter.clone_arc(), 1, 5);
    let third = spawn_shared_counter_i64(counter.clone_arc(), 1, 5);
    let fourth = spawn_shared_counter_i64(counter.clone_arc(), 1, 5);
    let fifth = spawn_shared_counter_i64(counter.clone_arc(), 1, 5);
    let sixth = spawn_shared_counter_i64(counter.clone_arc(), 1, 5);
    let seventh = spawn_shared_counter_i64(counter.clone_arc(), 1, 5);
    let eighth = spawn_shared_counter_i64(counter.clone_arc(), 1, 5);
    if !first.is_ok || !second.is_ok || !third.is_ok || !fourth.is_ok
        || !fifth.is_ok || !sixth.is_ok || !seventh.is_ok || !eighth.is_ok {
        return 1;
    }

    first.value.join();
    second.value.join();
    third.value.join();
    fourth.value.join();
    fifth.value.join();
    sixth.value.join();
    seventh.value.join();
    eighth.value.join();

    let locked = await mutex_lock_guard(counter.borrow());
    if !locked.is_ok { return locked.error; }
    locked.value.get()
}
"#,
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn async_stdlib_generic_arc_mutex_locks_fresh_payload() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "async-generic-arc-mutex-fresh-lock",
        r#"
import std::async;

async def main() -> i64 {
    let counter: Arc<Mutex<i64>> = arc_new(mutex_new(2));
    let locked = await mutex_lock_guard(counter.borrow());
    if !locked.is_ok { return locked.error; }
    locked.value.get()
}
"#,
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn async_stdlib_generic_mutex_failed_lock_copy_leaves_output_untouched() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "async-generic-mutex-failed-copy",
        r#"
import std::async;

async def main() -> i64 {
    let invalid: Mutex<i64> = Mutex { handle: 0, marker: 0 };
    let locked = await mutex_lock_guard(&invalid);
    if locked.is_ok { return 1; }

    let mut output = 9;
    if mutex_guard_copy_into(&locked.value, &mut output) { return 2; }
    if output == 9 { 42 } else { 3 }
}
"#,
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn async_stdlib_shared_counter_joins_cross_thread_workers() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "async-shared-counter",
        r#"
import std::async;

def main() -> i64 {
    let enabled = runtime_enable_thread_pool(4);
    if !enabled.is_ok { return enabled.error; }

    let counter = arc_mutex_new_i64(2);
    let first = spawn_shared_counter_i64(counter.clone_arc_mutex(), 1, 5);
    let second = spawn_shared_counter_i64(counter.clone_arc_mutex(), 1, 5);
    let third = spawn_shared_counter_i64(counter.clone_arc_mutex(), 1, 5);
    let fourth = spawn_shared_counter_i64(counter.clone_arc_mutex(), 1, 5);
    let fifth = spawn_shared_counter_i64(counter.clone_arc_mutex(), 1, 5);
    let sixth = spawn_shared_counter_i64(counter.clone_arc_mutex(), 1, 5);
    let seventh = spawn_shared_counter_i64(counter.clone_arc_mutex(), 1, 5);
    let eighth = spawn_shared_counter_i64(counter.clone_arc_mutex(), 1, 5);
    if !first.is_ok || !second.is_ok || !third.is_ok || !fourth.is_ok
        || !fifth.is_ok || !sixth.is_ok || !seventh.is_ok || !eighth.is_ok {
        return 1;
    }

    first.value.join();
    second.value.join();
    third.value.join();
    fourth.value.join();
    fifth.value.join();
    sixth.value.join();
    seventh.value.join();
    eighth.value.join();
    counter.get()
}
"#,
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn async_stdlib_mutex_guard_i64_releases_and_writes_back_on_drop() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "async-mutex-guard",
        r#"
import std::async;

async def update(mutex: MutexI64) -> i64 {
    let locked = await mutex_lock_guard_i64(mutex);
    if !locked.is_ok { return locked.error; }
    let guard = locked.value;
    guard.set(17);
    0
}

async def read_after_update(mutex: MutexI64) -> i64 {
    let locked = await mutex_lock_guard_i64(mutex);
    if !locked.is_ok { return locked.error; }
    let guard = locked.value;
    guard.get()
}

async def main() -> i64 {
    let mutex = mutex_new_i64(5);
    let wrote = await update(mutex);
    let observed = await read_after_update(mutex);

    mutex_close(mutex);
    let rejected = await mutex_lock_guard_i64(mutex);
    mutex_drop(mutex);

    if wrote == 0 && observed == 17 && !rejected.is_ok {
        42
    } else {
        1
    }
}
"#,
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn async_stdlib_rwlock_i64_guards_release_and_write_back_on_drop() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "async-rwlock-guards",
        r#"
import std::async;

def read_pair(lock: RwLockI64) -> i64 {
    let first_result = rwlock_try_read_guard_i64(lock);
    if !first_result.is_ok { return first_result.error; }
    let first = first_result.value;
    let second_result = rwlock_try_read_guard_i64(lock);
    if !second_result.is_ok { return second_result.error; }
    let second = second_result.value;
    first.get() + second.get()
}

def write_value(lock: RwLockI64, value: i64) -> i64 {
    let result = rwlock_try_write_guard_i64(lock);
    if !result.is_ok { return result.error; }
    let guard = result.value;
    guard.set(value);
    guard.get()
}

def read_value(lock: RwLockI64) -> i64 {
    let result = rwlock_try_read_guard_i64(lock);
    if !result.is_ok { return result.error; }
    let guard = result.value;
    guard.get()
}

def main() -> i64 {
    let lock = rwlock_new_i64(5);
    let before = read_pair(lock);
    let wrote = write_value(lock, 17);
    let after = read_value(lock);
    rwlock_close(lock);
    let rejected = rwlock_try_read_guard_i64(lock);
    rwlock_drop(lock);

    if before == 10 && wrote == 17 && after == 17 && !rejected.is_ok {
        42
    } else {
        1
    }
}
"#,
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn incremental_link_output_matches_full_link_output() {
    let Some(clang) = find_clang() else {
        return;
    };

    let source = "def main() -> i64 { 0 }\n";
    let llvm_ir = compile_source(source, 2).unwrap();
    let ll_path = temp_artifact("equiv-main", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let full_exe = temp_artifact("equiv-full", if cfg!(windows) { "exe" } else { "" });
    let inc_exe = temp_artifact("equiv-inc", if cfg!(windows) { "exe" } else { "" });
    let obj_path = temp_artifact("equiv-main", if cfg!(windows) { "obj" } else { "o" });

    let runtime_c = find_runtime_c();
    compile_native_binary(
        &clang,
        &ll_path,
        &full_exe,
        runtime_c.as_deref(),
        2,
        None,
        None,
    )
    .unwrap();
    compile_ir_to_object(&clang, &ll_path, &obj_path, 2, None, false).unwrap();

    let mut object_paths = vec![obj_path.clone()];
    if let Some(runtime_c) = runtime_c.as_deref() {
        object_paths.extend(ensure_runtime_objects(&clang, runtime_c, 2, None).unwrap());
    }
    link_native_binary_from_objects(&clang, &object_paths, &inc_exe, None, None).unwrap();

    let full_out = Command::new(&full_exe).output().unwrap();
    let inc_out = Command::new(&inc_exe).output().unwrap();
    assert_eq!(full_out.status.code(), inc_out.status.code());
    assert_eq!(full_out.stdout, inc_out.stdout);
    assert_eq!(full_out.stderr, inc_out.stderr);

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&full_exe);
    let _ = fs::remove_file(&inc_exe);
}

static STDLIB_RUNTIME_C_READY: OnceLock<bool> = OnceLock::new();
fn stdlib_runtime_c_is_compilable(clang: &str, runtime_c: &Path) -> bool {
    *STDLIB_RUNTIME_C_READY.get_or_init(|| {
        ensure_runtime_objects(clang, &runtime_c.to_string_lossy(), 2, None).is_ok()
    })
}

fn load_stdlib_surface_source() -> String {
    load_stdlib_modules(&[
        "option.sg",
        "result.sg",
        "ffi.sg",
        "string.sg",
        "collections.sg",
    ])
}

fn load_async_runtime_stdlib() -> String {
    load_stdlib_modules(&["option.sg", "result.sg", "ffi.sg", "status.sg", "async.sg"])
}

fn load_stdlib_modules(modules: &[&str]) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);
    let stdlib_root = workspace_root.join("tools/stdlib");
    modules
        .iter()
        .map(|module| {
            fs::read_to_string(stdlib_root.join(module))
                .unwrap_or_else(|err| panic!("stdlib module {} should exist: {err}", module))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn workspace_root_for_tests() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir)
        .to_path_buf()
}

fn load_runtime_bundle_source_for_tests() -> String {
    let runtime_c = find_runtime_c().unwrap_or_else(|| {
        workspace_root_for_tests()
            .join("tools/stdlib/runtime.c")
            .to_string_lossy()
            .to_string()
    });
    runtime_source_bundle(&runtime_c)
        .expect("runtime bundle should be discoverable")
        .into_iter()
        .map(|path| {
            fs::read_to_string(&path).unwrap_or_else(|err| {
                panic!(
                    "runtime source {} should be readable: {err}",
                    path.display()
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn read_example_source(relative_path: &str) -> String {
    let path = workspace_root_for_tests().join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("example {} should exist: {err}", path.display()))
}

fn assert_example_file(relative_path: &str) {
    let source = read_example_source(relative_path);
    let line_limit = match relative_path {
        "examples/stdlib/02_math.sg" => 180,
        "examples/stdlib/25_formatting.sg" => 100,
        _ => 60,
    };
    assert!(
        source.lines().count() <= line_limit,
        "{relative_path} should stay at or below {line_limit} lines"
    );
    assert!(
        source.starts_with("//"),
        "{relative_path} should start with a comment block"
    );
    assert!(
        source.contains("Run:") && source.contains("Expected output:"),
        "{relative_path} should document Run and Expected output"
    );
}

fn compile_and_run_example_with_args(
    tag: &str,
    relative_path: &str,
    extra_c_inputs: &[&str],
    args: &[&str],
    strict_native: bool,
) -> Option<std::process::Output> {
    let source = super::expand_stdlib_imports_for_source(&read_example_source(relative_path))
        .unwrap_or_else(|err| {
            panic!("example {relative_path} stdlib imports should expand: {err}")
        });
    let llvm_ir = compile_source(&source, 1)
        .unwrap_or_else(|err| panic!("example {relative_path} should compile: {err}"));

    let clang = find_clang().unwrap_or_else(|| {
        if strict_native {
            panic!("core conformance requires clang");
        }
        String::new()
    });
    if clang.is_empty() {
        return None;
    }
    let runtime_c = find_runtime_c().unwrap_or_else(|| {
        if strict_native {
            panic!("core conformance requires the stdlib runtime sources");
        }
        String::new()
    });
    if runtime_c.is_empty() {
        return None;
    }
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        if strict_native {
            panic!("core conformance stdlib runtime sources must compile");
        }
        return None;
    }

    let ll_path = temp_artifact(&format!("examples-smoke-{tag}"), "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact(&format!("examples-smoke-{tag}-main"), obj_ext);
    if let Err(error) = compile_ir_to_object(&clang, &ll_path, &main_obj, 1, None, false) {
        let _ = fs::remove_file(&ll_path);
        let _ = fs::remove_file(&main_obj);
        if strict_native {
            panic!("example {relative_path} LLVM IR should compile: {error}");
        }
        return None;
    }

    let runtime_objects = match ensure_runtime_objects(&clang, &runtime_c, 1, None) {
        Ok(objects) => objects,
        Err(error) => {
            let _ = fs::remove_file(&ll_path);
            let _ = fs::remove_file(&main_obj);
            if strict_native {
                panic!("example {relative_path} runtime objects should compile: {error}");
            }
            return None;
        }
    };

    let mut object_paths = vec![main_obj.clone()];
    object_paths.extend(runtime_objects);
    let workspace_root = workspace_root_for_tests();
    let mut extra_objects = Vec::new();
    for extra_input in extra_c_inputs {
        let extra_obj = temp_artifact(
            &format!(
                "examples-smoke-{tag}-{}",
                extra_input.replace(['/', '\\', '.'], "-")
            ),
            obj_ext,
        );
        if let Err(error) = compile_ir_to_object(
            &clang,
            &workspace_root.join(extra_input),
            &extra_obj,
            1,
            None,
            false,
        ) {
            let _ = fs::remove_file(&ll_path);
            let _ = fs::remove_file(&main_obj);
            for object in &extra_objects {
                let _ = fs::remove_file(object);
            }
            if strict_native {
                panic!("example {relative_path} extra input {extra_input} should compile: {error}");
            }
            return None;
        }
        object_paths.push(extra_obj.clone());
        extra_objects.push(extra_obj);
    }

    let exe_path = temp_artifact(
        &format!("examples-smoke-{tag}"),
        if cfg!(windows) { "exe" } else { "" },
    );
    if let Err(error) =
        link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None)
    {
        let _ = fs::remove_file(&ll_path);
        let _ = fs::remove_file(&main_obj);
        for object in &extra_objects {
            let _ = fs::remove_file(object);
        }
        let _ = fs::remove_file(&exe_path);
        if strict_native {
            panic!("example {relative_path} should link: {error}");
        }
        return None;
    }

    let output = Command::new(&exe_path)
        .args(args)
        .output()
        .expect("example executable should run");

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&main_obj);
    for object in &extra_objects {
        let _ = fs::remove_file(object);
    }
    let _ = fs::remove_file(&exe_path);
    Some(output)
}

fn assert_example_output(tag: &str, relative_path: &str, expected_stdout: &str) {
    assert_example_output_with_c_inputs(tag, relative_path, &[], expected_stdout);
}

fn assert_example_output_with_c_inputs(
    tag: &str,
    relative_path: &str,
    extra_c_inputs: &[&str],
    expected_stdout: &str,
) {
    assert_example_output_with_c_inputs_and_args(
        tag,
        relative_path,
        extra_c_inputs,
        &[],
        expected_stdout,
    );
}

fn assert_example_output_with_args(
    tag: &str,
    relative_path: &str,
    args: &[&str],
    expected_stdout: &str,
) {
    assert_example_output_with_c_inputs_and_args(tag, relative_path, &[], args, expected_stdout);
}

fn assert_example_output_with_c_inputs_and_args(
    tag: &str,
    relative_path: &str,
    extra_c_inputs: &[&str],
    args: &[&str],
    expected_stdout: &str,
) {
    let output = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name(format!("compile-example-{tag}"))
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, || {
                compile_and_run_example_with_args(tag, relative_path, extra_c_inputs, args, false)
            })
            .expect("example compiler worker should spawn")
            .join()
            .expect("example compiler worker should complete")
    });
    let Some(output) = output else {
        return;
    };
    assert!(
        output.status.success(),
        "{relative_path} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        expected_stdout,
        "{relative_path} stdout mismatch"
    );
}

#[test]
fn immutable_assignment_json_reports_stable_code_and_target_span() {
    let source = r#"
def main() -> i64 {
    let value = 1;
    value = value + 1;
    value
}
"#;
    let error = sengoo_compiler::compile_to_ir(source).expect_err("assignment should fail");
    let location = super::location_from_compile_error(source, &error);
    let json = super::render_compile_error_json_with_location(
        Some("tests/immutable.sg"),
        &error.to_string(),
        location,
    );
    let value: Value = serde_json::from_str(&json).expect("json payload should be valid");
    let target_lo = source.find("value = value").expect("assignment target") as u64;

    assert_eq!(value["stage"], "typecheck");
    assert_eq!(value["code"], "immutable-assignment");
    assert_eq!(value["location"]["span"]["lo"], target_lo);
    assert_eq!(
        value["location"]["span"]["hi"],
        target_lo + "value".len() as u64
    );
}

#[test]
fn use_after_move_json_reports_stable_code_and_target_span() {
    let source = r#"
struct String { handle: i64 }

def main() -> i64 {
    let a: String = String { handle: 1 };
    let b = a;
    a.handle
}
"#;
    let error = sengoo_compiler::compile_to_ir(source).expect_err("use after move should fail");
    let location = super::location_from_compile_error(source, &error);
    let json = super::render_compile_error_json_with_location(
        Some("tests/use_after_move.sg"),
        &error.to_string(),
        location,
    );
    let value: Value = serde_json::from_str(&json).expect("json payload should be valid");
    let target_lo = source.rfind("a.handle").expect("moved value use") as u64;

    assert_eq!(value["stage"], "typecheck");
    assert_eq!(value["code"], "use-after-move");
    assert_eq!(value["location"]["span"]["lo"], target_lo);
}

#[test]
fn unsatisfied_trait_bound_json_reports_stable_code_and_target_span() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

def consume<T: Showable>(x: T) -> i64 {
    0
}

def main() -> i64 {
    consume(42)
}
"#;
    let error = sengoo_compiler::compile_to_ir(source)
        .expect_err("unsatisfied generic trait bound should fail");
    let location = super::location_from_compile_error(source, &error);
    let json = super::render_compile_error_json_with_location(
        Some("tests/unsatisfied_bound.sg"),
        &error.to_string(),
        location,
    );
    let value: Value = serde_json::from_str(&json).expect("json payload should be valid");
    let target_lo = source.rfind("consume(").expect("call target") as u64;

    assert_eq!(value["stage"], "typecheck");
    assert_eq!(value["code"], "unsatisfied-trait-bound");
    assert_eq!(value["location"]["span"]["lo"], target_lo);
    assert_eq!(
        value["location"]["span"]["hi"],
        target_lo + "consume(".len() as u64
    );
    assert!(value["message"]
        .as_str()
        .unwrap_or_default()
        .contains("Showable"));
}

fn assert_example_result(
    tag: &str,
    relative_path: &str,
    expected_exit_code: i32,
    expected_stdout: &str,
) {
    let Some(output) = compile_and_run_example_with_args(tag, relative_path, &[], &[], true) else {
        return;
    };
    assert_eq!(
        output.status.code(),
        Some(expected_exit_code),
        "{relative_path} exit mismatch: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        expected_stdout,
        "{relative_path} stdout mismatch"
    );
}

#[test]
fn stdlib_auto_drop_releases_all_generation_handles() {
    let Some(output) = compile_and_run_example_with_args(
        "auto-drop-live-handles",
        "tools/sgc/tests/fixtures/auto_drop_live_handles.sg",
        &[],
        &[],
        true,
    ) else {
        return;
    };
    assert_eq!(
        output.status.code(),
        Some(0),
        "auto-drop live-handle probe failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn core_conformance_examples_compile_link_and_run() {
    let cases = [
        (
            "core-scalars-control",
            "examples/conformance/01_scalars_control.sg",
            9,
            "core",
        ),
        (
            "core-recursion",
            "examples/conformance/02_recursion.sg",
            13,
            "",
        ),
        ("core-struct", "examples/08_struct.sg", 30, ""),
        ("core-method", "examples/09_method_call.sg", 43, ""),
        ("core-array-read", "examples/04_array.sg", 20, ""),
        ("core-array-for", "examples/05_loop.sg", 15, ""),
        (
            "core-array-write",
            "examples/conformance/03_array_write.sg",
            42,
            "",
        ),
        ("core-closure", "examples/06_lambda.sg", 15, ""),
        (
            "core-closure-multi",
            "examples/conformance/04_closure_multi_capture.sg",
            18,
            "",
        ),
        (
            "core-enum-value",
            "examples/ergonomics/03_enum_match.sg",
            2,
            "",
        ),
        (
            "core-enum-payload",
            "examples/conformance/05_enum_payload.sg",
            42,
            "",
        ),
        (
            "core-enum-multi-payload",
            "examples/conformance/06_enum_multi_payload.sg",
            42,
            "",
        ),
        (
            "core-enum-return",
            "examples/conformance/07_enum_return.sg",
            42,
            "",
        ),
    ];

    for (tag, path, exit_code, stdout) in cases {
        assert_example_result(tag, path, exit_code, stdout);
    }
}

#[test]
fn jit_enum_match_ir_is_accepted_by_clang() {
    let Some(clang) = find_clang() else {
        return;
    };

    let source = r#"
enum Maybe { Empty, Value(i64) }

def main() -> i64 {
    let value = Maybe::Value(42);
    match value {
        Maybe::Empty => 0,
        Maybe::Value(inner) => inner,
    }
}
"#;
    let mir = compile_to_mir(source).expect("enum match should compile to MIR");
    let ir = JITCodegen::new()
        .generate(&mir)
        .expect("JIT enum match should generate LLVM IR");
    let ll_path = temp_artifact("jit-enum-match", "ll");
    let obj_path = temp_artifact("jit-enum-match", if cfg!(windows) { "obj" } else { "o" });
    fs::write(&ll_path, ir).expect("JIT IR should be writable");

    let output = Command::new(&clang)
        .arg("-c")
        .arg(&ll_path)
        .arg("-o")
        .arg(&obj_path)
        .output()
        .expect("clang should run");

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&obj_path);
    assert!(
        output.status.success(),
        "clang rejected JIT enum-match IR: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn examples_catalog_lists_expanded_categories() {
    let examples = [
        "examples/async/01_sleep_spawn.sg",
        "examples/async/02_select_two.sg",
        "examples/async/03_spawn_task_lifecycle.sg",
        "examples/generics/01_vec_i64.sg",
        "examples/generics/02_option_unwrap.sg",
        "examples/generics/03_result_chain.sg",
        "examples/generics/04_stdlib_collections.sg",
        "examples/generics/05_bound_and_dyn.sg",
        "examples/stdlib/01_strings.sg",
        "examples/stdlib/02_math.sg",
        "examples/stdlib/03_error.sg",
        "examples/stdlib/04_option_result.sg",
        "examples/stdlib/05_file.sg",
        "examples/stdlib/11_args.sg",
        "examples/stdlib/12_dir.sg",
        "examples/stdlib/13_io.sg",
        "examples/stdlib/14_strconv.sg",
        "examples/stdlib/15_dir_listing.sg",
        "examples/stdlib/16_file_copy_move.sg",
        "examples/stdlib/17_process_run.sg",
        "examples/stdlib/18_status_buffer.sg",
        "examples/stdlib/18_json.sg",
        "examples/stdlib/19_process_capture.sg",
        "examples/stdlib/20_owned_string.sg",
        "examples/stdlib/21_assert.sg",
        "examples/stdlib/22_regex_log.sg",
        "examples/stdlib/23_config_hash.sg",
        "examples/stdlib/25_formatting.sg",
        "examples/traits/01_iterator_basic.sg",
        "examples/traits/02_method_specialization.sg",
        "examples/ffi/sengoo_calls_c.sg",
        "examples/ffi/sengoo_exports.sg",
    ];

    for example in examples {
        assert_example_file(example);
    }

    let workspace_root = workspace_root_for_tests();
    for readme in [
        "examples/README.md",
        "examples/async/README.md",
        "examples/generics/README.md",
        "examples/stdlib/README.md",
        "examples/traits/README.md",
        "examples/ffi/README.md",
    ] {
        let content = fs::read_to_string(workspace_root.join(readme))
            .unwrap_or_else(|err| panic!("{readme} should exist: {err}"));
        assert!(
            !content.contains("éˆ") && !content.contains("æµ"),
            "{readme} should not contain mojibake"
        );
    }

    assert!(
        workspace_root.join("examples/ffi/Makefile").exists(),
        "FFI examples should include a Makefile"
    );

    let index = fs::read_to_string(workspace_root.join("examples/README.md")).unwrap();
    for category in [
        "async/",
        "generics/",
        "stdlib/",
        "traits/",
        "ffi/",
        "reflection/",
    ] {
        assert!(
            index.contains(category),
            "examples index should link {category}"
        );
    }

    for readme in ["README.md", "README.zh-CN.md"] {
        let content = fs::read_to_string(workspace_root.join(readme)).unwrap();
        assert!(
            content.contains("examples/README.md"),
            "{readme} should link the examples index"
        );
    }
}

#[test]
fn stdlib_json_example_uses_public_json_wrappers() {
    let source = read_example_source("examples/stdlib/18_json.sg");

    assert!(
        !source.contains("sengoo_json_"),
        "JSON example should demonstrate std::json wrappers instead of raw C bridge calls"
    );
    for needle in ["json_parse(", "json_doc_object(", ".root()", ".serialize("] {
        assert!(
            source.contains(needle),
            "JSON example should include wrapper usage: {needle}"
        );
    }
}

#[test]
fn stdlib_status_buffer_example_imports_ffi_explicitly() {
    let source = read_example_source("examples/stdlib/18_status_buffer.sg");

    assert!(
        source.contains("import std::ffi;"),
        "status/buffer example should import std::ffi explicitly before using Buffer helpers"
    );
}

#[test]
fn examples_smoke_async_sleep_spawn() {
    assert_example_output(
        "async-sleep-spawn",
        "examples/async/01_sleep_spawn.sg",
        "42",
    );
}

#[test]
fn examples_smoke_async_select_two() {
    assert_example_output("async-select-two", "examples/async/02_select_two.sg", "43");
}

#[test]
fn examples_smoke_async_spawn_task_lifecycle() {
    assert_example_output(
        "async-spawn-task-lifecycle",
        "examples/async/03_spawn_task_lifecycle.sg",
        "42",
    );
}

#[test]
fn examples_smoke_generics_vec_i64() {
    assert_example_output("generics-vec-i64", "examples/generics/01_vec_i64.sg", "60");
}

#[test]
fn examples_smoke_generics_option_unwrap() {
    assert_example_output(
        "generics-option-unwrap",
        "examples/generics/02_option_unwrap.sg",
        "9",
    );
}

#[test]
fn examples_smoke_generics_result_chain() {
    assert_example_output(
        "generics-result-chain",
        "examples/generics/03_result_chain.sg",
        "18",
    );
}

#[test]
fn examples_smoke_generics_stdlib_collections_import() {
    assert_example_output(
        "generics-stdlib-collections",
        "examples/generics/04_stdlib_collections.sg",
        "60",
    );
}

#[test]
fn examples_smoke_generics_bound_and_dyn() {
    assert_example_output(
        "generics-bound-and-dyn",
        "examples/generics/05_bound_and_dyn.sg",
        "29",
    );
}

#[test]
fn examples_smoke_stdlib_strings_import() {
    assert_example_output("stdlib-strings", "examples/stdlib/01_strings.sg", "8");
}

#[test]
fn examples_smoke_stdlib_math_import() {
    assert_example_output("stdlib-math", "examples/stdlib/02_math.sg", "50");
}

#[test]
fn examples_smoke_stdlib_error_import() {
    assert_example_output("stdlib-error", "examples/stdlib/03_error.sg", "7");
}

#[test]
fn examples_smoke_stdlib_option_result_import() {
    assert_example_output(
        "stdlib-option-result",
        "examples/stdlib/04_option_result.sg",
        "7",
    );
}

#[test]
fn examples_smoke_stdlib_file_import() {
    assert_example_output("stdlib-file", "examples/stdlib/05_file.sg", "15");
}

#[test]
fn examples_smoke_stdlib_env_time_import() {
    assert_example_output("stdlib-env-time", "examples/stdlib/06_env_time.sg", "6");
}

#[test]
fn examples_smoke_stdlib_random_import() {
    assert_example_output("stdlib-random", "examples/stdlib/07_random.sg", "8");
}

#[test]
fn examples_smoke_stdlib_path_import() {
    assert_example_output("stdlib-path", "examples/stdlib/08_path.sg", "9");
}

#[test]
fn examples_smoke_stdlib_process_import() {
    assert_example_output("stdlib-process", "examples/stdlib/09_process.sg", "10");
}

#[test]
fn examples_smoke_stdlib_collections_import() {
    let Some(output) = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("compile-example-stdlib-collections".to_string())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, || {
                compile_and_run_example_with_args(
                    "stdlib-collections",
                    "examples/stdlib/10_collections.sg",
                    &[],
                    &[],
                    true,
                )
            })
            .expect("collections compiler worker should spawn")
            .join()
            .expect("collections compiler worker should complete")
    }) else {
        return;
    };
    assert!(
        output.status.success(),
        "collections example should succeed"
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "60");
}

#[test]
fn examples_smoke_stdlib_args_import() {
    assert_example_output_with_args(
        "stdlib-args",
        "examples/stdlib/11_args.sg",
        &["alpha", "beta"],
        "11",
    );
}

#[test]
fn examples_smoke_stdlib_dir_import() {
    assert_example_output("stdlib-dir", "examples/stdlib/12_dir.sg", "12");
}

#[test]
fn examples_smoke_stdlib_io_import() {
    assert_example_output("stdlib-io", "examples/stdlib/13_io.sg", "13");
}

#[test]
fn examples_smoke_stdlib_strconv_import() {
    assert_example_output("stdlib-strconv", "examples/stdlib/14_strconv.sg", "14");
}

#[test]
fn examples_smoke_stdlib_dir_listing_import() {
    assert_example_output(
        "stdlib-dir-listing",
        "examples/stdlib/15_dir_listing.sg",
        "15",
    );
}

#[test]
fn examples_smoke_stdlib_file_copy_move_import() {
    assert_example_output(
        "stdlib-file-copy-move",
        "examples/stdlib/16_file_copy_move.sg",
        "16",
    );
}

#[test]
fn examples_smoke_stdlib_process_run_import() {
    assert_example_output(
        "stdlib-process-run",
        "examples/stdlib/17_process_run.sg",
        "17",
    );
}

#[test]
fn examples_smoke_stdlib_status_buffer_import() {
    assert_example_output(
        "stdlib-status-buffer",
        "examples/stdlib/18_status_buffer.sg",
        "18",
    );
}

#[test]
fn examples_smoke_stdlib_json_import() {
    assert_example_output("stdlib-json", "examples/stdlib/18_json.sg", "18");
}

#[test]
fn examples_smoke_stdlib_process_capture_import() {
    assert_example_output(
        "stdlib-process-capture",
        "examples/stdlib/19_process_capture.sg",
        "19",
    );
}

#[test]
fn examples_smoke_stdlib_owned_string_import() {
    assert_example_output(
        "stdlib-owned-string",
        "examples/stdlib/20_owned_string.sg",
        "20",
    );
}

#[test]
fn examples_smoke_stdlib_assert_import() {
    assert_example_output("stdlib-assert", "examples/stdlib/21_assert.sg", "21");
}

#[test]
fn examples_smoke_stdlib_regex_log_import() {
    assert_example_output("stdlib-regex-log", "examples/stdlib/22_regex_log.sg", "22");
}

#[test]
fn examples_smoke_stdlib_config_hash_import() {
    assert_example_output(
        "stdlib-config-hash",
        "examples/stdlib/23_config_hash.sg",
        "23",
    );
}

#[test]
fn examples_smoke_stdlib_formatting_import() {
    assert_example_output(
        "stdlib-formatting",
        "examples/stdlib/25_formatting.sg",
        "25",
    );
}

#[test]
fn stdlib_format_width_right_aligns_runtime_output() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "format-width-right-align",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let rendered = format("{:>4}", 7);
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = rendered.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if copied == 4 && wrote == 4 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "   7");
}

#[test]
fn stdlib_format_precision_renders_f64_output() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "format-precision-f64",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let rendered = format("{:.2}", 3.14159);
    let buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let copied = rendered.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if copied == 4 && wrote == 4 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3.14");
}

#[test]
fn stdlib_format_debug_renders_struct_output() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "format-debug-struct",
        r#"
import std::ffi;
import std::io;
import std::string;

struct Point {
    x: i64,
    ok: bool,
}

def main() -> i64 {
    let point = Point { x: 7, ok: true };
    let rendered = format("{:?}", point);
    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let copied = rendered.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if copied == 24 && wrote == 24 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Point { x: 7, ok: true }"
    );
}

#[test]
fn stdlib_string_push_char_appends_unicode_scalar() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-push-char",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let text = string_from_str("hi").unwrap_or(String { handle: 0 });
    let pushed = text.push_char('!').unwrap_or(false);
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = text.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if pushed && copied == 3 && wrote == 3 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi!");
}

#[test]
fn stdlib_string_compare_orders_owned_strings() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-compare",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let eq_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let eq_r = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let ne_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let ne_r = string_from_str("beta").unwrap_or(String { handle: 0 });
    let lt_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let lt_r = string_from_str("beta").unwrap_or(String { handle: 0 });
    let le_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let le_r = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let gt_l = string_from_str("beta").unwrap_or(String { handle: 0 });
    let gt_r = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let ge_l = string_from_str("beta").unwrap_or(String { handle: 0 });
    let ge_r = string_from_str("beta").unwrap_or(String { handle: 0 });
    let cmp_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let cmp_r = string_from_str("alphabet").unwrap_or(String { handle: 0 });
    let rendered = string_from_str("ok").unwrap_or(String { handle: 0 });
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = rendered.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if eq_l.eq(eq_r)
        && ne_l.ne(ne_r)
        && lt_l.lt(lt_r)
        && le_l.le(le_r)
        && gt_l.gt(gt_r)
        && ge_l.ge(ge_r)
        && cmp_l.compare(cmp_r) < 0
        && copied == 2 && wrote == 2 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
}

#[test]
fn stdlib_string_hash_uses_runtime_byte_state() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-hash-runtime-state",
        r#"
import std::string;

def main() -> i64 {
    let mut left = hasher_new();
    left.write_str("ab");
    let left_hash = left.finish();
    let mut right = hasher_new();
    right.write_str("ac");
    let right_hash = right.finish();
    if left_hash == right_hash {
        2
    } else {
        0
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_string_comparison_operators_order_owned_strings() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-compare-operators",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let eq_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let eq_r = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let ne_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let ne_r = string_from_str("beta").unwrap_or(String { handle: 0 });
    let lt_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let lt_r = string_from_str("beta").unwrap_or(String { handle: 0 });
    let le_l = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let le_r = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let gt_l = string_from_str("beta").unwrap_or(String { handle: 0 });
    let gt_r = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let ge_l = string_from_str("beta").unwrap_or(String { handle: 0 });
    let ge_r = string_from_str("beta").unwrap_or(String { handle: 0 });
    let rendered = string_from_str("ok").unwrap_or(String { handle: 0 });
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = rendered.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if !(eq_l == eq_r) { return 10; }
    if !(ne_l != ne_r) { return 11; }
    if !(lt_l < lt_r) { return 12; }
    if !(le_l <= le_r) { return 13; }
    if !(gt_l > gt_r) { return 14; }
    if !(ge_l >= ge_r) { return 15; }
    if copied != 2 { return 16; }
    if wrote != 2 { return 17; }
    0
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
}

#[test]
fn stdlib_str_comparison_operators_order_borrowed_strings() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "str-compare-operators",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let rendered = string_from_str("ok").unwrap_or(String { handle: 0 });
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = rendered.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if !("alpha" < "beta") { return 10; }
    if !("alpha" <= "alpha") { return 11; }
    if !("beta" > "alpha") { return 12; }
    if !("beta" >= "beta") { return 13; }
    if copied != 2 { return 14; }
    if wrote != 2 { return 15; }
    0
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
}

#[test]
fn stdlib_string_get_checks_utf8_boundaries() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-get-utf8",
        r#"
import std::ffi;
import std::io;
import std::status;
import std::string;

def main() -> i64 {
    let first = str_get("héllo", 0, 1).unwrap_or(String { handle: 0 });
    let accent = str_get("héllo", 1, 3).unwrap_or(String { handle: 0 });
    let bad_borrowed = str_get("héllo", 1, 2);
    let owned = string_from_str("héllo").unwrap_or(String { handle: 0 });
    let bad_owned = owned.get(1, 2);
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = accent.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if first.len() == 1
        && accent.len() == 2
        && bad_borrowed.err().unwrap_or(0) == STATUS_INVALID_ARGUMENT()
        && bad_owned.err().unwrap_or(0) == STATUS_INVALID_ARGUMENT()
        && copied == 2 && wrote == 2 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "é");
}

#[test]
fn stdlib_string_range_index_returns_owned_slice() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-range-index",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let borrowed = "hello"[1..4];
    let owned_text = string_from_str("hello").unwrap_or(String { handle: 0 });
    let owned = owned_text[1..4];
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = borrowed.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if borrowed.len() == 3 && owned.len() == 3 && copied == 3 && wrote == 3 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ell");
}

#[test]
fn stdlib_string_iterators_decode_bytes_and_chars() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-iterators",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let bytes_text = string_from_str("hé").unwrap_or(String { handle: 0 });
    let chars_text = string_from_str("hé").unwrap_or(String { handle: 0 });
    let mut bytes = bytes_text.bytes();
    let mut chars = chars_text.chars();
    let mut byte_count = 0;
    let mut saw_non_ascii_byte = false;
    let mut next_byte = bytes.next();
    while next_byte.is_some() {
        let value = next_byte.unwrap_or(0);
        if value > 127 {
            saw_non_ascii_byte = true;
        }
        byte_count = byte_count + 1;
        next_byte = bytes.next();
    }
    let mut char_count = 0;
    let mut saw_non_ascii_char = false;
    let mut next_char = chars.next();
    while next_char.is_some() {
        let value = next_char.unwrap_or('\0');
        if value != 'h' {
            saw_non_ascii_char = true;
        }
        char_count = char_count + 1;
        next_char = chars.next();
    }
    let bytes_freed = bytes.free();
    let chars_freed = chars.free();
    let rendered = string_from_str("ok").unwrap_or(String { handle: 0 });
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = rendered.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if byte_count <= char_count { return 10; }
    if saw_non_ascii_byte == false { return 11; }
    if char_count != 2 { return 12; }
    if saw_non_ascii_char == false { return 13; }
    if bytes_freed == false { return 18; }
    if chars_freed == false { return 19; }
    if copied != 2 { return 20; }
    if wrote != 2 { return 21; }
    0
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
}

#[test]
fn stdlib_string_split_iterator_returns_owned_segments() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-split",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let text = string_from_str("a,b,").unwrap_or(String { handle: 0 });
    let mut parts = text.split(",");
    let first = parts.next().unwrap_or(String { handle: 0 });
    let second = parts.next().unwrap_or(String { handle: 0 });
    let third = parts.next().unwrap_or(String { handle: 0 });
    let fourth = parts.next();
    let freed = parts.free();
    let first_buffer = ffi_buffer_new(4).unwrap_or(Buffer { handle: 0 });
    let second_buffer = ffi_buffer_new(4).unwrap_or(Buffer { handle: 0 });
    let first_len = first.copy_to_buffer(first_buffer).unwrap_or(0);
    let second_len = second.copy_to_buffer(second_buffer).unwrap_or(0);
    let first_written = io_stdout_write_raw(first_buffer.ptr(), first_len).unwrap_or(0);
    let second_written = io_stdout_write_raw(second_buffer.ptr(), second_len).unwrap_or(0);
    if first.handle == 0 { return 20; }
    if second.handle == 0 { return 21; }
    if first_len != 1 { return 10; }
    if second_len != 1 { return 11; }
    if third.len() != 0 { return 12; }
    if fourth.is_some() { return 13; }
    if freed == false { return 14; }
    if first_written != 1 { return 15; }
    if second_written != 1 { return 16; }
    0
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ab");
}

#[test]
fn stdlib_string_plus_str_returns_owned_string() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "string-plus-str",
        r#"
import std::ffi;
import std::io;
import std::string;

def main() -> i64 {
    let base = string_from_str("hi").unwrap_or(String { handle: 0 });
    let joined = base + "!";
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let copied = joined.copy_to_buffer(buffer).unwrap_or(0);
    let wrote = io_stdout_write_raw(buffer.ptr(), copied).unwrap_or(0);
    if copied == 3 && wrote == 3 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi!");
}

#[test]
fn eprintln_builtin_writes_to_stderr_with_native_runtime() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "builtin-eprintln",
        r#"
def main() -> i64 {
    eprintln("err");
    eprintln(42);
    0
}
"#,
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "eprintln should not write stdout, got:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr).replace("\r\n", "\n");
    assert_eq!(stderr.trim(), "err\n42");
}

#[test]
fn display_impl_prints_to_stdout_and_stderr() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "display-print",
        r#"
import std::string;

struct Tag {
    id: i64,
}

impl Display for Tag {
    def to_string(&self) -> String {
        string_from_str("Tag").value
    }
}

def main() -> i64 {
    let out = Tag { id: 1 };
    let err = Tag { id: 2 };
    print(out);
    eprintln(err);
    0
}
"#,
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "Tag");
    assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "Tag");
}

#[test]
fn stdlib_runtime_release_functions_are_idempotent_for_core_handles() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "release-idempotence",
        r#"
import std::ffi;
import std::json;
import std::process;
import std::string;
import std::collections;

extern "C" {
    fn sengoo_opaque_live_handle_count() -> i64;
}

def main() -> i64 {
    let opaque_before = sengoo_opaque_live_handle_count();
    let text = string_from_str("release").unwrap_or(String { handle: 0 });
    let text_handle = text.handle;
    let text_first = sengoo_string_free_status(text_handle) >= 0;
    let text_second = sengoo_string_free_status(text_handle) >= 0;

    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let buffer_handle = buffer.handle;
    let buffer_first = sengoo_ffi_buffer_free(buffer_handle) == 0;
    let buffer_second = sengoo_ffi_buffer_free(buffer_handle) == 0;

    let doc = json_parse("{}").unwrap_or(JsonDoc { handle: 0 });
    let doc_handle = doc.handle;
    let doc_first = sengoo_json_doc_close(doc_handle) == 0;
    let doc_second = sengoo_json_doc_close(doc_handle) == 0;

    let command = process_command("sengoo-no-such-release-idempotence").unwrap_or(ProcessCommand { handle: 0 });
    let command_handle = command.handle;
    let command_first = sengoo_process_command_close(command_handle) == 0;
    let command_second = sengoo_process_command_close(command_handle) == 0;

    let vec = vec_new_i64();
    let vec_handle = vec.handle;
    let vec_first = sengoo_vec_free_i64_status(vec_handle) == 1;
    let vec_second = sengoo_vec_free_i64_status(vec_handle) == 1;

    let map = hashmap_new_i64_i64();
    let map_handle = map.handle;
    let map_first = sengoo_hashmap_free_i64_status(map_handle) == 1;
    let map_second = sengoo_hashmap_free_i64_status(map_handle) == 1;

    let list = text_list_new();
    let list_handle = list.handle;
    let list_first = sengoo_text_list_free_status(list_handle) == 1;
    let list_second = sengoo_text_list_free_status(list_handle) == 1;

    let text_map = string_map_i64_new();
    let text_map_handle = text_map.handle;
    let text_map_first = sengoo_string_map_free_status(text_map_handle) == 1;
    let text_map_second = sengoo_string_map_free_status(text_map_handle) == 1;

    let string_vec = vec_new_string();
    let string_vec_handle = string_vec.handle;
    let string_vec_first = sengoo_vec_string_free_status(string_vec_handle) == 1;
    let string_vec_second = sengoo_vec_string_free_status(string_vec_handle) == 1;

    let string_string_map = string_map_string_new();
    let string_string_map_handle = string_string_map.handle;
    let string_string_map_first = sengoo_string_map_string_free_status(string_string_map_handle) == 1;
    let string_string_map_second = sengoo_string_map_string_free_status(string_string_map_handle) == 1;
    let opaque_after = sengoo_opaque_live_handle_count();

    if opaque_before == 0 and opaque_after == 0 and text_first and text_second and buffer_first and buffer_second and doc_first and doc_second and command_first and command_second and vec_first and vec_second and map_first and map_second and list_first and list_second and text_map_first and text_map_second and string_vec_first and string_vec_second and string_string_map_first and string_string_map_second {
        42
    } else {
        1
    }
}
"#,
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(42),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_c_fallback_net_release_functions_are_idempotent() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "fallback-net-release-idempotence",
        r#"
import std::net;

def main() -> i64 {
    let closes = sengoo_tcp_close(0) == 1
        and sengoo_udp_close(0) == 1
        and sengoo_http_close(0) == 1
        and sengoo_http_server_close(0) == 1
        and sengoo_http_request_close(0) == 1
        and sengoo_ws_close(0) == 1;
    if closes {
        42
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(42),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_runtime_string_trim_and_ascii_case_return_owned_strings() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "string-trim-case",
        r#"
import std::string;

def main() -> i64 {
    let trimmed = str_trim("  Sengoo\n").unwrap_or(String { handle: 0 });
    let expected_trim = string_from_str("Sengoo").unwrap_or(String { handle: 0 });
    let upper = str_to_ascii_upper("SenGoo").unwrap_or(String { handle: 0 });
    let expected_upper = string_from_str("SENGOO").unwrap_or(String { handle: 0 });
    let lower = str_to_ascii_lower("SenGoo").unwrap_or(String { handle: 0 });
    let expected_lower = string_from_str("sengoo").unwrap_or(String { handle: 0 });

    if trimmed.eq(expected_trim) and upper.eq(expected_upper) and lower.eq(expected_lower) {
        42
    } else {
        1
    }
}
"#,
    ) else {
        return;
    };

    assert_eq!(
        output.status.code(),
        Some(42),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_http_import_links_native_runtime_and_maps_errors() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "http-status",
        r#"
import std::http;

def main() -> i64 {
    let ftp_result = http_client_get("ftp://127.0.0.1/", 1);
    let ftp_closed = if ftp_result.is_ok { ftp_result.value.close(); } else { true; };
    let ftp_unsupported = if ftp_result.is_ok { false; } else { ftp_result.error == STATUS_UNSUPPORTED(); };
    if ftp_unsupported and ftp_closed {
        0
    } else if ftp_result.is_ok {
        99
    } else {
        ftp_result.error
    }
}
"#,
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn stdlib_http_server_pulls_and_answers_localhost_request() {
    use std::io::{BufRead, BufReader, Read};

    let source = expand_stdlib_imports_for_source(
        r#"
import std::net;
import std::io;
import std::strconv;

def main() -> i64 {
    let bind_result = http_server_bind("127.0.0.1", 0);
    if bind_result.is_ok == false {
        10
    } else {
        let server = bind_result.value;
        let port = server.local_port().unwrap_or(0);
        let port_buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
        let port_len = strconv_format_i64(port, port_buffer).unwrap_or(0);
        io_stdout_write_raw(port_buffer.ptr(), port_len);
        io_stdout_write("\n");
        io_stdout_flush();
        port_buffer.free();

        let request_result = server.next_request(15000);
        if request_result.is_ok == false {
            server.close();
            11
        } else {
            let request = request_result.value;
            let method_ok = request.method_len().unwrap_or(0) == 4;
            let path_ok = request.path_len().unwrap_or(0) == 5;
            let query_ok = request.query_len().unwrap_or(0) == 7;
            let header_ok = request.header_len("X-Trace").unwrap_or(0) == 3;
            let body_buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
            let copied = request.body_copy(body_buffer).unwrap_or(0);
            let responded = if method_ok && path_ok && query_ok && header_ok && copied == 4 {
                request.respond_raw(200, body_buffer.ptr(), copied).unwrap_or(false)
            } else {
                request.respond(500, "mismatch").unwrap_or(false)
            };
            body_buffer.free();
            server.close();
            if responded { 0 } else { 12 }
        }
    }
}

"#,
    )
    .expect("http server stdlib imports should expand");
    let llvm_ir =
        compile_source(&source, 1).expect("http server stdlib program should compile to LLVM IR");

    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let ll_path = temp_artifact("stdlib-http-server-pull", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact("stdlib-http-server-pull-main", obj_ext);
    compile_ir_to_object(&clang, &ll_path, &main_obj, 2, None, false).unwrap();
    let exe_path = temp_artifact(
        "stdlib-http-server-pull",
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![main_obj.clone()];
    append_native_runtime_inputs(&clang, &mut object_paths, Some(&runtime_c), 2, None).unwrap();
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let mut child = Command::new(&exe_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("http server fixture should spawn");

    let stdout = child.stdout.take().expect("child stdout should be piped");
    let mut reader = BufReader::new(stdout);
    let mut port_line = String::new();
    reader
        .read_line(&mut port_line)
        .expect("server should print its port");
    let port: u16 = match port_line.trim().parse() {
        Ok(port) => port,
        Err(_) => {
            let status = child.wait().expect("server fixture should be waitable");
            let mut stderr_text = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut stderr_text);
            }
            panic!(
                "port line should be numeric, got {port_line:?}; exit: {status:?}; stderr:\n{stderr_text}"
            );
        }
    };

    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))
        .expect("client should connect to the Sengoo server");
    stream
        .write_all(
            b"POST /echo?mode=up HTTP/1.1\r\nHost: localhost\r\nX-Trace: abc\r\nContent-Length: 4\r\nConnection: close\r\n\r\nping",
        )
        .expect("client request should be writable");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("client should read the response");
    let response_text = String::from_utf8_lossy(&response);
    assert!(
        response_text.starts_with("HTTP/1.1 200 OK"),
        "response: {response_text}"
    );
    assert!(
        response_text.ends_with("ping"),
        "response body should echo client bytes: {response_text}"
    );

    let status = child.wait().expect("server fixture should exit");
    assert!(
        status.success(),
        "server fixture should exit cleanly, got {status:?}"
    );

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&main_obj);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn stdlib_http_server_async_awaits_and_answers_localhost_request() {
    use std::io::{BufRead, BufReader, Read};

    let source = expand_stdlib_imports_for_source(
        r#"
import std::net;
import std::io;
import std::strconv;

def request_is_live(request: HttpServerRequest) -> bool {
    request.handle > 0
}

async def main() -> i64 {
    let bind_result = http_server_bind("127.0.0.1", 0);
    if bind_result.is_ok == false {
        10
    } else {
        let server = bind_result.value;
        let port = server.local_port().unwrap_or(0);
        let port_buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
        let port_len = strconv_format_i64(port, port_buffer).unwrap_or(0);
        io_stdout_write_raw(port_buffer.ptr(), port_len);
        io_stdout_write("\n");
        io_stdout_flush();
        port_buffer.free();

        let outcome = await server.next_request_async(15000);
        if outcome.is_ok == false {
            server.close();
            11
        } else {
            let request = outcome.value;
            let method_ok = request_is_live(request) && request.handle > 0 && request.method_len().unwrap_or(0) == 4;
            let path_ok = request.path_len().unwrap_or(0) == 5;
            let query_ok = request.query_len().unwrap_or(0) == 7;
            let header_ok = request.header_len("X-Trace").unwrap_or(0) == 3;
            let body_buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
            let copied = request.body_copy(body_buffer).unwrap_or(0);
            let responded = if method_ok && path_ok && query_ok && header_ok && copied == 4 {
                request.respond_raw(200, body_buffer.ptr(), copied).unwrap_or(false)
            } else {
                request.respond(500, "mismatch").unwrap_or(false)
            };
            body_buffer.free();
            server.close();
            if responded { 0 } else { 12 }
        }
    }
}
"#,
    )
    .expect("async HTTP server stdlib imports should expand");
    let llvm_ir = compile_source(&source, 1)
        .expect("async HTTP server stdlib program should compile to LLVM IR");

    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let ll_path = temp_artifact("stdlib-http-server-async", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact("stdlib-http-server-async-main", obj_ext);
    compile_ir_to_object(&clang, &ll_path, &main_obj, 2, None, false).unwrap();
    let exe_path = temp_artifact(
        "stdlib-http-server-async",
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![main_obj.clone()];
    append_native_runtime_inputs(&clang, &mut object_paths, Some(&runtime_c), 2, None).unwrap();
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let mut child = Command::new(&exe_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("async HTTP server fixture should spawn");

    let stdout = child.stdout.take().expect("child stdout should be piped");
    let mut reader = BufReader::new(stdout);
    let mut port_line = String::new();
    reader
        .read_line(&mut port_line)
        .expect("async server should print its port");
    let port: u16 = match port_line.trim().parse() {
        Ok(port) => port,
        Err(_) => {
            let status = child
                .wait()
                .expect("async server fixture should be waitable");
            let mut stderr_text = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut stderr_text);
            }
            panic!(
                "port line should be numeric, got {port_line:?}; exit: {status:?}; stderr:\n{stderr_text}"
            );
        }
    };

    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))
        .expect("client should connect to the async Sengoo server");
    stream
        .write_all(
            b"POST /echo?mode=up HTTP/1.1\r\nHost: localhost\r\nX-Trace: abc\r\nContent-Length: 4\r\nConnection: close\r\n\r\nping",
        )
        .expect("client request should be writable");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("client should read the async response");
    let response_text = String::from_utf8_lossy(&response);
    assert!(
        response_text.starts_with("HTTP/1.1 200 OK"),
        "response: {response_text}"
    );
    assert!(
        response_text.ends_with("ping"),
        "response body should echo client bytes: {response_text}"
    );

    let status = child.wait().expect("async server fixture should exit");
    assert!(
        status.success(),
        "async server fixture should exit cleanly, got {status:?}"
    );

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&main_obj);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn stdlib_io_runtime_reads_stdin_and_writes_streams() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "io-stdin",
        r#"
import std::io;

def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let read = io_stdin_read_line(buffer).unwrap_or(0);
    let wrote = io_stdout_write("out").unwrap_or(0);
    let err = io_stderr_write("err").unwrap_or(0);
    let flushed = io_stdout_flush().unwrap_or(false) && io_stderr_flush().unwrap_or(false);
    buffer.free();

    if read == 4 && wrote == 3 && err == 3 && flushed {
        0
    } else {
        1
    }
}
"#,
        "abc\nxyz",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "out");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "err");
}

#[test]
fn stdlib_dir_runtime_lists_entries_in_deterministic_order() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "dir-listing",
        r#"
import std::dir;
import std::file;
import std::io;

def main() -> i64 {
    let root = "sengoo_tmp_dir_listing_runtime";
    let a = "sengoo_tmp_dir_listing_runtime/a.txt";
    let b = "sengoo_tmp_dir_listing_runtime/b.txt";
    file_remove(a);
    file_remove(b);
    dir_remove(root);

    let created = dir_create(root).unwrap_or(false);
    let empty_count = dir_entry_count(root).unwrap_or(-1);
    let wrote_b = file_write_str(b, "b").unwrap_or(0);
    let wrote_a = file_write_str(a, "a").unwrap_or(0);
    let buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let small = ffi_buffer_new(3).unwrap_or(Buffer { handle: 0 });
    let count = dir_entry_count(root).unwrap_or(0);
    let first = dir_entry_name(root, 0, buffer).unwrap_or(0);
    let wrote_name = io_stdout_write_raw(buffer.ptr(), first).unwrap_or(0);
    let too_small = dir_entry_name(root, 0, small).is_err();
    let missing = dir_entry_name(root, 2, buffer).is_err();

    small.free();
    buffer.free();
    file_remove(a);
    file_remove(b);
    let removed = dir_remove(root).unwrap_or(false);

    if created && empty_count == 0 && wrote_a == 1 && wrote_b == 1 && count == 2 && first == 5 && wrote_name == 5 && too_small && missing && removed {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "a.txt");
}

#[test]
fn stdlib_file_metadata_and_dir_walk_runtime_cover_statuses_and_bounded_order() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "file-metadata-dir-walk",
        r#"
import std::dir;
import std::file;
import std::io;

def main() -> i64 {
    let root = "sengoo_tmp_dir_walk_runtime";
    let nested = "sengoo_tmp_dir_walk_runtime/nested";
    let deep = "sengoo_tmp_dir_walk_runtime/nested/deep";
    let a = "sengoo_tmp_dir_walk_runtime/a.txt";
    let b = "sengoo_tmp_dir_walk_runtime/b.txt";
    let c = "sengoo_tmp_dir_walk_runtime/nested/c.txt";
    let d = "sengoo_tmp_dir_walk_runtime/nested/deep/d.txt";
    let missing = "sengoo_tmp_dir_walk_runtime/missing.txt";

    file_remove(a);
    file_remove(b);
    file_remove(c);
    file_remove(d);
    dir_remove(deep);
    dir_remove(nested);
    dir_remove(root);

    let created = dir_create_all(deep).unwrap_or(false);
    let wrote_a = file_write_str(a, "a").unwrap_or(0);
    let wrote_b = file_write_str(b, "bb").unwrap_or(0);
    let wrote_c = file_write_str(c, "ccc").unwrap_or(0);
    let wrote_d = file_write_str(d, "dddd").unwrap_or(0);

    let file_kind_result = file_kind(a);
    let dir_kind_result = file_kind(root);
    let missing_kind = file_kind(missing);
    let size_result = file_size(c);
    let unsupported_size = file_size(root);
    let modified_result = file_modified_unix_ms(a);

    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let small = ffi_buffer_new(3).unwrap_or(Buffer { handle: 0 });
    let walk = dir_walk(root, 1).unwrap_or(DirWalk { handle: 0 });
    let first = walk.next(buffer);
    let first_len = first.unwrap_or(0);
    let first_out = io_stdout_write_raw(buffer.ptr(), first_len).unwrap_or(0);
    let sep0 = io_stdout_write("|").unwrap_or(0);
    let second = walk.next(buffer);
    let second_len = second.unwrap_or(0);
    let second_out = io_stdout_write_raw(buffer.ptr(), second_len).unwrap_or(0);
    let sep1 = io_stdout_write("|").unwrap_or(0);
    let third = walk.next(buffer);
    let third_len = third.unwrap_or(0);
    let third_out = io_stdout_write_raw(buffer.ptr(), third_len).unwrap_or(0);
    let sep2 = io_stdout_write("|").unwrap_or(0);
    let fourth = walk.next(buffer);
    let fourth_len = fourth.unwrap_or(0);
    let fourth_out = io_stdout_write_raw(buffer.ptr(), fourth_len).unwrap_or(0);
    let sep3 = io_stdout_write("|").unwrap_or(0);
    let fifth = walk.next(buffer);
    let fifth_len = fifth.unwrap_or(0);
    let fifth_out = io_stdout_write_raw(buffer.ptr(), fifth_len).unwrap_or(0);
    let done_len = walk.next(buffer).unwrap_or(-1);
    let closed = walk.close();

    let small_walk = dir_walk(root, 0).unwrap_or(DirWalk { handle: 0 });
    let too_small = small_walk.next(small);
    let small_closed = small_walk.close();
    let bad_depth = dir_walk(root, -1);
    let missing_walk = dir_walk(missing, 0);

    small.free();
    buffer.free();
    file_remove(a);
    file_remove(b);
    file_remove(c);
    file_remove(d);
    let removed_deep = dir_remove(deep).unwrap_or(false);
    let removed_nested = dir_remove(nested).unwrap_or(false);
    let removed_root = dir_remove(root).unwrap_or(false);

    let modified_ok = if modified_result.is_ok {
        modified_result.value > 0
    } else {
        modified_result.error == 8
    };

    if created
        && wrote_a == 1
        && wrote_b == 2
        && wrote_c == 3
        && wrote_d == 4
        && file_kind_result.is_ok
        && file_kind_result.value == PATH_KIND_FILE()
        && dir_kind_result.is_ok
        && dir_kind_result.value == PATH_KIND_DIR()
        && missing_kind.is_err()
        && missing_kind.error == 5
        && size_result.is_ok
        && size_result.value == 3
        && unsupported_size.is_err()
        && unsupported_size.error == 8
        && modified_ok
        && first_len == 5
        && second_len == 5
        && third_len == 6
        && fourth_len == 12
        && fifth_len == 11
        && first_out == 5
        && second_out == 5
        && third_out == 6
        && fourth_out == 12
        && fifth_out == 11
        && sep0 == 1
        && sep1 == 1
        && sep2 == 1
        && sep3 == 1
        && done_len == 0
        && closed
        && too_small.is_err()
        && too_small.error == 4
        && small_closed
        && bad_depth.is_err()
        && bad_depth.error == 2
        && missing_walk.is_err()
        && missing_walk.error == 5
        && removed_deep
        && removed_nested
        && removed_root {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a.txt|b.txt|nested|nested/c.txt|nested/deep"
    );
}

#[test]
fn stdlib_file_runtime_copies_moves_and_requires_explicit_overwrite() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "file-copy-move",
        r#"
import std::file;

def main() -> i64 {
    let source = "sengoo_tmp_file_transfer_source.txt";
    let copy = "sengoo_tmp_file_transfer_copy.txt";
    let moved = "sengoo_tmp_file_transfer_moved.txt";
    file_remove(source);
    file_remove(copy);
    file_remove(moved);

    let wrote = file_write_str(source, "alpha").unwrap_or(0);
    let copied = file_copy(source, copy, false).unwrap_or(0);
    let source_kept = file_exists(source);
    let copy_exists = file_exists(copy);
    let reject_copy = file_copy(source, copy, false).is_err();
    let overwrote = file_copy(source, copy, true).unwrap_or(0);
    let reject_same_file_copy = file_copy(source, source, true).is_err();
    let source_len = file_len(source).unwrap_or(0);

    file_write_str(moved, "old");
    let reject_move = file_move(copy, moved, false).is_err();
    let moved_ok = file_move(copy, moved, true).unwrap_or(false);
    let copy_gone = !file_exists(copy);
    let moved_exists = file_exists(moved);

    file_remove(source);
    file_remove(copy);
    file_remove(moved);

    if wrote == 5 && copied == 5 && source_kept && copy_exists && reject_copy && overwrote == 5 && reject_same_file_copy && source_len == 5 && reject_move && moved_ok && copy_gone && moved_exists {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_process_runtime_runs_literal_argument_and_reports_exit_code() {
    let Some(clang) = find_clang() else {
        return;
    };

    let child_c = temp_artifact("process-run-child", "c");
    let child_exe = temp_artifact("process-run-child", if cfg!(windows) { "exe" } else { "" });
    fs::write(
        &child_c,
        r#"
int main(int argc, char** argv) {
    const char* expected = "hello world";
    int index = 0;
    if (argc != 2) {
        return 8;
    }
    while (expected[index] != '\0' && argv[1][index] == expected[index]) {
        index++;
    }
    return expected[index] == '\0' && argv[1][index] == '\0' ? 7 : 8;
}
"#,
    )
    .unwrap();
    let status = Command::new(&clang)
        .arg(&child_c)
        .arg("-o")
        .arg(&child_exe)
        .status()
        .expect("process-run child fixture should compile");
    assert!(status.success(), "process-run child fixture should compile");

    let executable = child_exe.to_string_lossy().replace('\\', "/");
    let source = format!(
        r#"
import std::process;

def main() -> i64 {{
    let code = process_run_1("{executable}", "hello world").unwrap_or(-1);
    let rejected_empty = process_run("").is_err();
    if code == 7 && rejected_empty {{
        0
    }} else {{
        1
    }}
}}
"#
    );
    let output = compile_and_run_stdlib_import_program_with_stdin("process-run", &source, "");

    let _ = fs::remove_file(&child_c);
    let _ = fs::remove_file(&child_exe);

    let Some(output) = output else {
        return;
    };
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_process_command_captures_output_and_controls_child() {
    let Some(clang) = find_clang() else {
        return;
    };

    let child_c = temp_artifact("process-command-child", "c");
    let child_exe = temp_artifact(
        "process-command-child",
        if cfg!(windows) { "exe" } else { "" },
    );
    fs::write(
        &child_c,
        r#"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <direct.h>
#include <windows.h>
#define getcwd _getcwd
static void sleep_ms(unsigned long ms) { Sleep(ms); }
#else
#include <unistd.h>
static void sleep_ms(unsigned long ms) { usleep(ms * 1000); }
#endif

static int path_eq(const char* left, const char* right) {
    if (!left || !right) {
        return 0;
    }
    while (*left != '\0' && *right != '\0') {
        char lhs = *left++;
        char rhs = *right++;
#ifdef _WIN32
        if (lhs == '\\') {
            lhs = '/';
        }
        if (rhs == '\\') {
            rhs = '/';
        }
#endif
        if (lhs != rhs) {
            return 0;
        }
    }
    return *left == '\0' && *right == '\0';
}

int main(int argc, char** argv) {
    if (argc < 2) {
        return 90;
    }

    if (strcmp(argv[1], "literal") == 0) {
        return argc == 3 && strcmp(argv[2], "literal spaces &^%$!") == 0 ? 7 : 8;
    }

    if (strcmp(argv[1], "inherit") == 0) {
        fprintf(stdout, "inherit-stdout");
        fprintf(stderr, "inherit-stderr");
        fflush(stdout);
        fflush(stderr);
        return 23;
    }

    if (strcmp(argv[1], "inspect") == 0) {
        char cwd[1024];
        const char* flag = getenv("SENGOO_FLAG");
        const char* removed = getenv("SENGOO_REMOVE_ME");
        const char* path = getenv("PATH");
        if (argc != 5) {
            return 30;
        }
        if (!getcwd(cwd, sizeof(cwd))) {
            return 31;
        }
        fprintf(stdout, "123");
        fprintf(stderr, "456");
        fflush(stdout);
        fflush(stderr);
        if (strcmp(argv[2], "hello world &^%$!") != 0) {
            return 32;
        }
        if (!path_eq(cwd, argv[3])) {
            return 33;
        }
        if (!flag || strcmp(flag, argv[4]) != 0) {
            return 34;
        }
        if (removed != NULL) {
            return 35;
        }
        if (path != NULL) {
            return 36;
        }
        return 19;
    }

    if (strcmp(argv[1], "timeout") == 0) {
        fprintf(stdout, "12");
        fprintf(stderr, "34");
        fflush(stdout);
        fflush(stderr);
        sleep_ms(300);
        return 5;
    }

    if (strcmp(argv[1], "pipe-producer") == 0) {
        fprintf(stdout, "789");
        fflush(stdout);
        return 0;
    }

    if (strcmp(argv[1], "pipe-consumer") == 0) {
        char input[16] = {0};
        size_t read_count = fread(input, 1, sizeof(input) - 1, stdin);
        if (read_count != 3 || strcmp(input, "789") != 0) {
            return 40;
        }
        fprintf(stdout, "%s", input);
        fflush(stdout);
        return 0;
    }

    return 91;
}
"#,
    )
    .unwrap();
    let status = Command::new(&clang)
        .arg(&child_c)
        .arg("-o")
        .arg(&child_exe)
        .status()
        .expect("process-command child fixture should compile");
    assert!(
        status.success(),
        "process-command child fixture should compile"
    );

    let cwd_dir = temp_artifact("process-command-cwd", "dir");
    let _ = fs::remove_dir_all(&cwd_dir);
    fs::create_dir_all(&cwd_dir).unwrap();

    let executable = child_exe.to_string_lossy().replace('\\', "/");
    let expected_cwd = cwd_dir.to_string_lossy().replace('\\', "/");
    let source = format!(
        r#"
import std::ffi;
import std::process;
import std::strconv;

def main() -> i64 {{
    let fixed = process_run_2("{executable}", "literal", "literal spaces &^%$!").unwrap_or(-1);

    let inherited = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let inherited_arg = inherited.arg("inherit").unwrap_or(false);
    let inherited_output = inherited.run().unwrap_or(ProcessOutput {{ handle: 0 }});
    let inherited_code = inherited_output.exit_code().unwrap_or(-1);
    let inherited_stdout_len = inherited_output.stdout_len().unwrap_or(-1);
    let inherited_stderr_len = inherited_output.stderr_len().unwrap_or(-1);
    let inherited_output_closed = inherited_output.close();
    let inherited_output_reused = inherited_output.stdout_len().is_err();
    let inherited_command_closed = inherited.close();
    let inherited_command_reused = inherited.arg("again").is_err();

    let stdout_buffer = ffi_buffer_new(8).unwrap_or(Buffer {{ handle: 0 }});
    let stderr_buffer = ffi_buffer_new(8).unwrap_or(Buffer {{ handle: 0 }});
    let command = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let inspect_arg0 = command.arg("inspect").unwrap_or(false);
    let inspect_arg1 = command.arg("hello world &^%$!").unwrap_or(false);
    let inspect_arg2 = command.arg("{expected_cwd}").unwrap_or(false);
    let inspect_arg3 = command.arg("kept").unwrap_or(false);
    let inspect_cwd = command.cwd("{expected_cwd}").unwrap_or(false);
    let inspect_clear = command.env_clear().unwrap_or(false);
    let inspect_set = command.env_set("SENGOO_FLAG", "kept").unwrap_or(false);
    let inspect_set_removed = command.env_set("SENGOO_REMOVE_ME", "present").unwrap_or(false);
    let inspect_remove = command.env_remove("SENGOO_REMOVE_ME").unwrap_or(false);
    let inspect_capture_stdout = command.capture_stdout(true).unwrap_or(false);
    let inspect_capture_stderr = command.capture_stderr(true).unwrap_or(false);
    let output = command.run().unwrap_or(ProcessOutput {{ handle: 0 }});
    let code = output.exit_code().unwrap_or(-1);
    let timed_out = output.timed_out();
    let stdout_len = output.stdout_len().unwrap_or(-1);
    let stderr_len = output.stderr_len().unwrap_or(-1);
    let stdout_copied = output.stdout_copy(stdout_buffer).unwrap_or(-1);
    let stderr_copied = output.stderr_copy(stderr_buffer).unwrap_or(-1);
    let stdout_value = strconv_parse_i64_buffer(stdout_buffer, stdout_copied).unwrap_or(-1);
    let stderr_value = strconv_parse_i64_buffer(stderr_buffer, stderr_copied).unwrap_or(-1);
    let output_closed = output.close();
    let output_reused = output.stderr_len().is_err();
    let command_closed = command.close();
    let command_reused = command.run().is_err();

    let timeout_stdout = ffi_buffer_new(8).unwrap_or(Buffer {{ handle: 0 }});
    let timeout_stderr = ffi_buffer_new(8).unwrap_or(Buffer {{ handle: 0 }});
    let timeout = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let timeout_arg = timeout.arg("timeout").unwrap_or(false);
    let timeout_capture_stdout = timeout.capture_stdout(true).unwrap_or(false);
    let timeout_capture_stderr = timeout.capture_stderr(true).unwrap_or(false);
    let timeout_set = timeout.timeout_ms(50).unwrap_or(false);
    let timeout_output = timeout.run().unwrap_or(ProcessOutput {{ handle: 0 }});
    let timeout_exit = timeout_output.exit_code();
    let timeout_timed_out = timeout_output.timed_out();
    let timeout_stdout_len = timeout_output.stdout_len().unwrap_or(-1);
    let timeout_stderr_len = timeout_output.stderr_len().unwrap_or(-1);
    let timeout_stdout_copied = timeout_output.stdout_copy(timeout_stdout).unwrap_or(-1);
    let timeout_stderr_copied = timeout_output.stderr_copy(timeout_stderr).unwrap_or(-1);
    let timeout_stdout_value = strconv_parse_i64_buffer(timeout_stdout, timeout_stdout_copied).unwrap_or(-1);
    let timeout_stderr_value = strconv_parse_i64_buffer(timeout_stderr, timeout_stderr_copied).unwrap_or(-1);
    let timeout_output_closed = timeout_output.close();
    let timeout_command_closed = timeout.close();

    let pipeline_buffer = ffi_buffer_new(8).unwrap_or(Buffer {{ handle: 0 }});
    let producer = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let producer_arg = producer.arg("pipe-producer").unwrap_or(false);
    let consumer = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let consumer_arg = consumer.arg("pipe-consumer").unwrap_or(false);
    let consumer_capture = consumer.capture_stdout(true).unwrap_or(false);
    let pipeline = producer.pipe_stdout_to(consumer).unwrap_or(ProcessCommand {{ handle: 0 }});
    let pipeline_output = pipeline.run().unwrap_or(ProcessOutput {{ handle: 0 }});
    let pipeline_exit = pipeline_output.exit_code().unwrap_or(-1);
    let pipeline_len = pipeline_output.stdout_len().unwrap_or(-1);
    let pipeline_copied = pipeline_output.stdout_copy(pipeline_buffer).unwrap_or(-1);
    let pipeline_value = strconv_parse_i64_buffer(pipeline_buffer, pipeline_copied).unwrap_or(-1);
    let pipeline_output_closed = pipeline_output.close();
    let pipeline_command_closed = pipeline.close();

    let wait_command = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let wait_arg0 = wait_command.arg("literal").unwrap_or(false);
    let wait_arg1 = wait_command.arg("literal spaces &^%$!").unwrap_or(false);
    let wait_handle = wait_command.spawn().unwrap_or(ProcessHandle {{ handle: 0 }});
    let wait_code = wait_handle.wait_cancellable(2000).unwrap_or(-1);
    let wait_exit = wait_handle.exit_code().unwrap_or(-1);
    let wait_closed = wait_handle.close();

    let kill_command = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let kill_arg = kill_command.arg("timeout").unwrap_or(false);
    let kill_handle = kill_command.spawn().unwrap_or(ProcessHandle {{ handle: 0 }});
    let killed = kill_handle.kill().unwrap_or(false);
    let canceled_wait = kill_handle.wait_cancellable(2000);
    let kill_closed = kill_handle.close();

    let ok =
        fixed == 7
        && inherited_arg
        && inherited_code == 23
        && inherited_stdout_len == 0
        && inherited_stderr_len == 0
        && inherited_output_closed
        && inherited_output_reused
        && inherited_command_closed
        && inherited_command_reused
        && inspect_arg0
        && inspect_arg1
        && inspect_arg2
        && inspect_arg3
        && inspect_cwd
        && inspect_clear
        && inspect_set
        && inspect_set_removed
        && inspect_remove
        && inspect_capture_stdout
        && inspect_capture_stderr
        && code == 19
        && !timed_out
        && stdout_len == 3
        && stderr_len == 3
        && stdout_copied == 3
        && stderr_copied == 3
        && stdout_value == 123
        && stderr_value == 456
        && output_closed
        && output_reused
        && command_closed
        && command_reused
        && timeout_arg
        && timeout_capture_stdout
        && timeout_capture_stderr
        && timeout_set
        && timeout_exit.is_err()
        && timeout_exit.error == 11
        && timeout_timed_out
        && timeout_stdout_len == 2
        && timeout_stderr_len == 2
        && timeout_stdout_copied == 2
        && timeout_stderr_copied == 2
        && timeout_stdout_value == 12
        && timeout_stderr_value == 34
        && timeout_output_closed
        && timeout_command_closed
        && producer_arg
        && consumer_arg
        && consumer_capture
        && pipeline_exit == 0
        && pipeline_len == 3
        && pipeline_copied == 3
        && pipeline_value == 789
        && pipeline_output_closed
        && pipeline_command_closed
        && wait_arg0
        && wait_arg1
        && wait_code == 7
        && wait_exit == 7
        && wait_closed
        && kill_arg
        && killed
        && canceled_wait.is_err()
        && canceled_wait.error == 19
        && kill_closed;

    stdout_buffer.free();
    stderr_buffer.free();
    timeout_stdout.free();
    timeout_stderr.free();
    pipeline_buffer.free();

    if ok {{
        0
    }} else if !producer_arg || !consumer_arg || !consumer_capture {{
        41
    }} else if pipeline_exit != 0 {{
        pipeline_exit
    }} else if pipeline_len != 3 || pipeline_copied != 3 {{
        43
    }} else if pipeline_value != 789 {{
        44
    }} else if wait_code != 7 || wait_exit != 7 {{
        45
    }} else if !canceled_wait.is_err() || canceled_wait.error != 19 {{
        46
    }} else {{
        1
    }}
}}
"#
    );
    let output = compile_and_run_stdlib_import_program_with_stdin("process-command", &source, "");

    let _ = fs::remove_file(&child_c);
    let _ = fs::remove_file(&child_exe);
    let _ = fs::remove_dir_all(&cwd_dir);

    let Some(output) = output else {
        return;
    };
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("inherit-stdout"),
        "stdout should include inherited child stdout, got:\n{stdout}"
    );
    assert!(
        stderr.contains("inherit-stderr"),
        "stderr should include inherited child stderr, got:\n{stderr}"
    );
}

#[test]
fn stdlib_process_wait_cancellable_returns_promptly_after_kill() {
    let Some(clang) = find_clang() else {
        return;
    };

    let child_c = temp_artifact("process-wait-cancellable-child", "c");
    let child_exe = temp_artifact(
        "process-wait-cancellable-child",
        if cfg!(windows) { "exe" } else { "" },
    );
    fs::write(
        &child_c,
        r#"
#include <string.h>

#ifdef _WIN32
#include <windows.h>
static void sleep_ms(unsigned long ms) { Sleep(ms); }
#else
#include <unistd.h>
static void sleep_ms(unsigned long ms) { usleep(ms * 1000); }
#endif

int main(int argc, char** argv) {
    if (argc == 2 && strcmp(argv[1], "sleep") == 0) {
        sleep_ms(5000);
        return 5;
    }
    return 7;
}
"#,
    )
    .unwrap();
    let status = Command::new(&clang)
        .arg(&child_c)
        .arg("-o")
        .arg(&child_exe)
        .status()
        .expect("wait-cancellable child fixture should compile");
    assert!(
        status.success(),
        "wait-cancellable child fixture should compile"
    );

    let executable = child_exe.to_string_lossy().replace('\\', "/");
    let source = format!(
        r#"
import std::process;

def main() -> i64 {{
    let command = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let arg = command.arg("sleep").unwrap_or(false);
    let handle = command.spawn().unwrap_or(ProcessHandle {{ handle: 0 }});
    let killed = handle.kill().unwrap_or(false);
    let waited = handle.wait_cancellable(5000);
    let closed = handle.close();

    let timeout_command = process_command("{executable}").unwrap_or(ProcessCommand {{ handle: 0 }});
    let timeout_arg = timeout_command.arg("sleep").unwrap_or(false);
    let timeout_handle = timeout_command.spawn().unwrap_or(ProcessHandle {{ handle: 0 }});
    let timed_wait = timeout_handle.wait_cancellable(1);
    let timeout_killed = timeout_handle.kill().unwrap_or(false);
    let timeout_closed = timeout_handle.close();

    if arg
        && killed
        && waited.is_err()
        && waited.error == 19
        && closed
        && timeout_arg
        && timed_wait.is_err()
        && timed_wait.error == 11
        && timeout_killed
        && timeout_closed {{
        0
    }} else {{
        1
    }}
}}
"#
    );

    let source = expand_stdlib_imports_for_source(&source)
        .unwrap_or_else(|err| panic!("stdlib imports should expand: {err}"));
    let llvm_ir = compile_source(&source, 1)
        .unwrap_or_else(|err| panic!("wait-cancellable source should compile: {err}"));
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return;
    }

    let ll_path = temp_artifact("process-wait-cancellable", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();
    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact("process-wait-cancellable-main", obj_ext);
    compile_ir_to_object(&clang, &ll_path, &main_obj, 2, None, false).unwrap();

    let exe_path = temp_artifact(
        "process-wait-cancellable",
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![main_obj.clone()];
    object_paths.extend(ensure_runtime_objects(&clang, &runtime_c, 2, None).unwrap());
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let started = std::time::Instant::now();
    let output = Command::new(&exe_path)
        .output()
        .expect("wait-cancellable binary should run");
    let elapsed = started.elapsed();

    let _ = fs::remove_file(&child_c);
    let _ = fs::remove_file(&child_exe);
    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&main_obj);
    let _ = fs::remove_file(&exe_path);

    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "wait_cancellable should not wait for the 5s child sleep; elapsed={elapsed:?}"
    );
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_strconv_runtime_parses_and_formats_i64_values() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "strconv-i64",
        r#"
import std::io;
import std::strconv;

def main() -> i64 {
    let input = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let out = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let read = io_stdin_read_line(input).unwrap_or(0);
    let parsed = strconv_parse_i64_buffer(input, read).unwrap_or(0);
    let literal = strconv_parse_i64("  -5\n").unwrap_or(0);
    let invalid = strconv_parse_i64("12x").unwrap_or(99);
    let overflow = strconv_parse_i64("9223372036854775808").unwrap_or(77);
    let formatted = strconv_format_i64(parsed + literal, out).unwrap_or(0);
    let wrote = io_stdout_write_raw(out.ptr(), formatted).unwrap_or(0);
    input.free();
    out.free();

    if parsed == 19 && literal == -5 && invalid == 99 && overflow == 77 && formatted == 2 && wrote == 2 {
        0
    } else {
        1
    }
}
"#,
        "19\n",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "14");
}

#[test]
fn stdlib_strconv_runtime_parses_and_formats_f64_values() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "strconv-f64",
        r#"
import std::io;
import std::strconv;

def main() -> i64 {
    let out = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let parsed = strconv_parse_f64(" 3.25\n").unwrap_or(0.0);
    let invalid = strconv_parse_f64("3.2x").err().unwrap_or(0);
    let formatted = strconv_format_f64(parsed, 2, out).unwrap_or(0);
    let wrote = io_stdout_write_raw(out.ptr(), formatted).unwrap_or(0);
    out.free();

    if invalid == STATUS_PARSE() && formatted == 4 && wrote == 4 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3.25");
}

#[test]
fn stdlib_math_runtime_float_predicates_cover_nan_and_infinity() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "math-float-predicates",
        r#"
import std::math;

def main() -> i64 {
    let nan64 = 0.0 / 0.0;
    let inf64 = 1.0 / 0.0;
    let finite64 = sqrt_f64(9.0);

    let nan32 = 0.0f32 / 0.0f32;
    let inf32 = 1.0f32 / 0.0f32;
    let finite32 = sqrt_f32(9.0f32);

    if is_nan_f64(nan64)
        && is_infinite_f64(inf64)
        && !is_finite_f64(inf64)
        && is_finite_f64(finite64)
        && is_nan_f32(nan32)
        && is_infinite_f32(inf32)
        && !is_finite_f32(inf32)
        && is_finite_f32(finite32) {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_integer_overflow_traps_in_debug_and_wraps_in_release() {
    let source = r#"
def main() -> i64 {
    let max = 9223372036854775807;
    let value = max + 1;
    if value < 0 {
        0
    } else {
        1
    }
}
"#;

    let Some(debug_output) = compile_and_run_program_with_opt_level("integer-overflow", source, 0)
    else {
        return;
    };
    assert!(
        !debug_output.status.success(),
        "O0 overflow should trap before returning successfully; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&debug_output.stdout),
        String::from_utf8_lossy(&debug_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&debug_output.stderr).contains("Integer overflow"),
        "O0 overflow should report the runtime overflow trap, got stderr:\n{}",
        String::from_utf8_lossy(&debug_output.stderr)
    );

    let Some(release_output) =
        compile_and_run_program_with_opt_level("integer-overflow", source, 2)
    else {
        return;
    };
    assert!(
        release_output.status.success(),
        "O2 overflow should wrap and take the negative branch; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&release_output.stdout),
        String::from_utf8_lossy(&release_output.stderr)
    );
}

#[test]
fn native_integer_division_by_zero_traps_in_debug() {
    let source = r#"
def main() -> i64 {
    let zero = 0;
    let value = 84 / zero;
    value
}
"#;

    let Some(debug_output) = compile_and_run_program_with_opt_level("integer-div-zero", source, 0)
    else {
        return;
    };
    assert!(
        !debug_output.status.success(),
        "O0 division by zero should trap before returning successfully; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&debug_output.stdout),
        String::from_utf8_lossy(&debug_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&debug_output.stderr).contains("Division by zero"),
        "O0 division by zero should report the runtime divisor trap, got stderr:\n{}",
        String::from_utf8_lossy(&debug_output.stderr)
    );
}

#[test]
fn stdlib_legacy_fallible_wrappers_return_status_categories() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "legacy-status-categories",
        r#"
import std::dir;
import std::file;
import std::process;
import std::status;
import std::strconv;

def main() -> i64 {
    let small = ffi_buffer_new(1).unwrap_or(Buffer { handle: 0 });
    let missing_file = file_len("__sengoo_missing_status_file__").err().unwrap_or(0);
    let missing_dir = dir_entry_count("__sengoo_missing_status_dir__").err().unwrap_or(0);
    let invalid_process = process_run("").err().unwrap_or(0);
    let cwd_too_small = process_current_dir_copy(small).err().unwrap_or(0);
    let parse_error = strconv_parse_i64("12x").err().unwrap_or(0);
    let invalid_slice = strconv_parse_i64_buffer(small, 2).err().unwrap_or(0);
    small.free();

    let missing_file_ok = missing_file == STATUS_NOT_FOUND() || missing_file == STATUS_IO();
    let missing_dir_ok = missing_dir == STATUS_NOT_FOUND() || missing_dir == STATUS_IO();

    if missing_file_ok
        && missing_dir_ok
        && invalid_process == STATUS_INVALID_ARGUMENT()
        && cwd_too_small == STATUS_BUFFER_TOO_SMALL()
        && parse_error == STATUS_PARSE()
        && invalid_slice == STATUS_BUFFER_TOO_SMALL() {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_compress_runtime_round_trips_gzip_buffers_and_maps_errors() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "compress-gzip-roundtrip",
        r#"
import std::compress;
import std::io;
import std::status;

def main() -> i64 {
    let input = ffi_buffer_from_bytes("[1,2,3]").unwrap_or(Buffer { handle: 0 });
    let gzip_a = ffi_buffer_new(128).unwrap_or(Buffer { handle: 0 });
    let gzip_b = ffi_buffer_new(128).unwrap_or(Buffer { handle: 0 });
    let restored = ffi_buffer_new(128).unwrap_or(Buffer { handle: 0 });
    let tiny = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let invalid = ffi_buffer_from_bytes("not-gzip").unwrap_or(Buffer { handle: 0 });

    let gzip_a_result = compress_gzip_buffer(input, input.used_len(), gzip_a);
    let gzip_b_result = compress_gzip_buffer(input, input.used_len(), gzip_b);
    let tiny_error = compress_gzip_buffer(input, input.used_len(), tiny).err().unwrap_or(0);
    let restored_len = if gzip_a_result.is_ok {
        decompress_gzip_buffer(gzip_a, gzip_a_result.value, restored).unwrap_or(0)
    } else {
        0
    };
    let truncated_error = if gzip_a_result.is_ok {
        decompress_gzip_buffer(gzip_a, gzip_a_result.value - 1, restored).err().unwrap_or(0)
    } else {
        0
    };
    let invalid_error = decompress_gzip_buffer(invalid, invalid.used_len(), restored).err().unwrap_or(0);

    let ok = gzip_a_result.is_ok
        && gzip_b_result.is_ok
        && gzip_a_result.value == gzip_b_result.value
        && restored_len == input.used_len()
        && tiny_error == STATUS_BUFFER_TOO_SMALL()
        && truncated_error == STATUS_PARSE()
        && invalid_error == STATUS_PARSE();

    if ok {
        print(gzip_a_result.value);
        let _newline = io_stdout_write("\n").unwrap_or(0);
        let _wrote_a = io_stdout_write_raw(gzip_a.ptr(), gzip_a_result.value).unwrap_or(0);
        let _wrote_b = io_stdout_write_raw(gzip_b.ptr(), gzip_b_result.value).unwrap_or(0);
        let _wrote_restored = io_stdout_write_raw(restored.ptr(), restored_len).unwrap_or(0);
    }

    invalid.free();
    tiny.free();
    restored.free();
    gzip_b.free();
    gzip_a.free();
    input.free();

    if ok { 0 } else { 1 }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout bytes:\n{:?}\nstderr:\n{}",
        output.status,
        output.stdout,
        String::from_utf8_lossy(&output.stderr)
    );

    let newline = output
        .stdout
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("stdout should prefix gzip length");
    let gzip_len = std::str::from_utf8(&output.stdout[..newline])
        .expect("gzip length should be utf8")
        .trim()
        .parse::<usize>()
        .expect("gzip length should parse");
    let mut payload_start = newline + 1;
    while payload_start < output.stdout.len() && output.stdout[payload_start].is_ascii_whitespace()
    {
        payload_start += 1;
    }
    let bytes = &output.stdout[payload_start..];
    assert!(
        bytes.len() >= gzip_len * 2,
        "stdout should contain two gzip payloads and restored JSON, got {} bytes for gzip length {gzip_len}",
        bytes.len()
    );
    let gzip_a = &bytes[..gzip_len];
    let gzip_b = &bytes[gzip_len..gzip_len * 2];
    let restored = &bytes[gzip_len * 2..];

    assert_eq!(gzip_a, gzip_b, "gzip output should be deterministic");
    assert!(gzip_a.starts_with(&[0x1f, 0x8b, 0x08, 0x00]));
    assert_eq!(
        &gzip_a[4..8],
        &[0, 0, 0, 0],
        "gzip mtime must be normalized"
    );
    assert_eq!(gzip_a[9], 255, "gzip OS byte must be normalized");
    assert_eq!(restored, b"[1,2,3]");
}

#[test]
fn stdlib_compress_runtime_enforces_v1_input_and_output_limits() {
    let source = r#"
import std::compress;
import std::io;
import std::status;

def main() -> i64 {
    let input = ffi_buffer_new(1048577).unwrap_or(Buffer { handle: 0 });
    let read = io_stdin_read(input).unwrap_or(0);
    let large_out = ffi_buffer_new(1100000).unwrap_or(Buffer { handle: 0 });
    let restored = ffi_buffer_new(1048576).unwrap_or(Buffer { handle: 0 });
    let tiny_out = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let too_large_gzip = ffi_buffer_new(1048680).unwrap_or(Buffer { handle: 0 });
    let sample = ffi_buffer_from_bytes("small-buffer-check").unwrap_or(Buffer { handle: 0 });
    let sample_gzip = ffi_buffer_new(128).unwrap_or(Buffer { handle: 0 });

    let exact_len = if read > 1048576 { 1048576 } else { read };
    let exact = compress_gzip_buffer(input, exact_len, large_out);
    let exact_roundtrip = if exact.is_ok {
        decompress_gzip_buffer(large_out, exact.value, restored).unwrap_or(0)
    } else {
        0
    };
    let one_over = compress_gzip_buffer(input, 1048577, large_out).err().unwrap_or(0);
    let gunzip_one_over = decompress_gzip_buffer(too_large_gzip, 1048680, large_out).err().unwrap_or(0);
    let sample_exact = compress_gzip_buffer(sample, sample.used_len(), sample_gzip);
    let small_out = if sample_exact.is_ok {
        decompress_gzip_buffer(sample_gzip, sample_exact.value, tiny_out).err().unwrap_or(0)
    } else {
        0
    };

    sample_gzip.free();
    sample.free();
    too_large_gzip.free();
    tiny_out.free();
    restored.free();
    large_out.free();
    input.free();

    let ok = exact.is_ok
        && exact_roundtrip == exact_len
        && one_over == STATUS_OVERFLOW()
        && gunzip_one_over == STATUS_OVERFLOW()
        && small_out == STATUS_BUFFER_TOO_SMALL();
    if !ok {
        print(read);
        print(exact_len);
        print(if exact.is_ok { exact.value } else { 0 });
        print(exact_roundtrip);
        print(one_over);
        print(gunzip_one_over);
        print(if sample_exact.is_ok { sample_exact.value } else { 0 });
        print(small_out);
    }

    if ok {
        0
    } else {
        1
    }
}
"#;
    let oversized_input = "a".repeat(1_048_577);
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "compress-gzip-limits",
        source,
        &oversized_input,
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_fallible_wrappers_do_not_use_legacy_generic_error_literal() {
    let workspace_root = workspace_root_for_tests();
    for module in [
        "tools/stdlib/args.sg",
        "tools/stdlib/dir.sg",
        "tools/stdlib/env.sg",
        "tools/stdlib/file.sg",
        "tools/stdlib/io.sg",
        "tools/stdlib/path.sg",
        "tools/stdlib/process.sg",
        "tools/stdlib/strconv.sg",
    ] {
        let source = fs::read_to_string(workspace_root.join(module))
            .unwrap_or_else(|err| panic!("{module} should be readable: {err}"));
        assert!(
            !source.contains("error: 1"),
            "{module} should map fallible wrapper errors through std::status instead of legacy error: 1"
        );
    }
}

#[test]
fn stdlib_json_runtime_parses_queries_builds_and_serializes_values() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "json-runtime",
        r#"
import std::io;
import std::json;

def main() -> i64 {
    let parsed_input = ffi_buffer_from_bytes("[null,false,42,9223372036854775807,2.5]").unwrap_or(Buffer { handle: 0 });
    let parsed = json_parse_buffer(parsed_input, parsed_input.len()).unwrap_or(JsonDoc { handle: 0 });
    let root = parsed.root();
    let first = root.array_get(0).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let second = root.array_get(1).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let count = root.array_get(2).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let big = root.array_get(3).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let ratio = root.array_get(4).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });

    let built = json_doc_object().unwrap_or(JsonDoc { handle: 0 });
    let built_root = built.root();
    let built_items = built.new_array().unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let built_name = built.new_string("sengoo").unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let built_ok = built.new_bool(true).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let built_null = built.new_null().unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let built_false = built.new_bool(false).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let built_x = built.new_string("x").unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let output_buffer = ffi_buffer_new(256).unwrap_or(Buffer { handle: 0 });
    let name_buffer = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });

    let set_name = built_root.object_set("name", built_name).unwrap_or(false);
    let set_ok = built_root.object_set("ok", built_ok).unwrap_or(false);
    let ratio_result = ratio.number_f64();
    let built_ratio = built.new_number(ratio_result.value).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let set_ratio = built_root.object_set("ratio", built_ratio).unwrap_or(false);
    let push_null = built_items.array_push(built_null).unwrap_or(false);
    let push_false = built_items.array_push(built_false).unwrap_or(false);
    let push_x = built_items.array_push(built_x).unwrap_or(false);
    let set_items = built_root.object_set("items", built_items).unwrap_or(false);

    let root_kind = root.kind().unwrap_or(0);
    let has_name = built_root.object_has("name");
    let missing = built_root.object_get("missing").is_err();
    let item_len = root.array_len().unwrap_or(0);
    let bad_index = root.array_get(5).is_err();
    let built_name_value = built_root.object_get("name").unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let copied_name = built_name_value.string_copy(name_buffer).unwrap_or(0);
    let wrote_name = io_stdout_write_raw(name_buffer.ptr(), copied_name).unwrap_or(0);
    let wrote_sep = io_stdout_write("|").unwrap_or(0);
    let count_i64 = count.number_i64().unwrap_or(0);
    let big_i64 = big.number_i64().unwrap_or(0);
    let ratio_i64_err = ratio.number_i64().is_err();
    let ratio_f64_ok = ratio_result.is_ok();
    let first_is_null = first.is_null();
    let second_bool = second.bool_value().unwrap_or(true);
    let serialized = built.serialize(output_buffer).unwrap_or(0);
    let wrote_json = io_stdout_write_raw(output_buffer.ptr(), serialized).unwrap_or(0);

    parsed_input.free();
    name_buffer.free();
    output_buffer.free();
    let built_closed = built.close();
    let parsed_closed = parsed.close();

    if root_kind == JSON_KIND_ARRAY()
        && has_name
        && missing
        && item_len == 5
        && bad_index
        && copied_name == 6
        && wrote_name == 6
        && wrote_sep == 1
        && count_i64 == 42
        && big_i64 == 9223372036854775807
        && ratio_i64_err
        && ratio_f64_ok
        && first_is_null
        && !second_bool
        && set_name
        && set_ok
        && set_ratio
        && push_null
        && push_false
        && push_x
        && set_items
        && serialized > 0
        && wrote_json == serialized
        && built_closed
        && parsed_closed {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "sengoo|{\"name\":\"sengoo\",\"ok\":true,\"ratio\":2.5,\"items\":[null,false,\"x\"]}"
    );
}

#[test]
fn stdlib_json_runtime_reports_parse_errors_and_limits() {
    let too_deep = format!("{}0{}", "[".repeat(70), "]".repeat(70));
    let too_many_nodes = format!("[{}]", vec!["0"; 5000].join(","));
    let too_deep = too_deep.replace('\\', "\\\\").replace('"', "\\\"");
    let too_many_nodes = too_many_nodes.replace('\\', "\\\\").replace('"', "\\\"");

    let source = format!(
        r#"
import std::json;
import std::io;

def main() -> i64 {{
    let message = ffi_buffer_new(128).unwrap_or(Buffer {{ handle: 0 }});
    let invalid = json_parse("[1, ]").is_err();
    let invalid_code = json_last_error_code();
    let invalid_offset = json_last_error_offset();
    let invalid_message = json_last_error_copy(message).unwrap_or(0);
    let deep = json_parse("{too_deep}").is_err();
    let deep_code = json_last_error_code();
    let deep_offset = json_last_error_offset();
    let too_many = json_parse("{too_many_nodes}").is_err();
    let too_many_code = json_last_error_code();
    let oversize_buf = ffi_buffer_new(2000000).unwrap_or(Buffer {{ handle: 0 }});
    let oversize_len = io_stdin_read(oversize_buf).unwrap_or(0);
    let too_big = json_parse_buffer(oversize_buf, oversize_len).is_err();
    let too_big_code = json_last_error_code();
    oversize_buf.free();
    let empty_doc = JsonDoc {{ handle: 0 }};
    let empty_close = !empty_doc.close();
    message.free();

    if !invalid {{
        1
    }} else if invalid_code != 10 {{
        2
    }} else if invalid_offset < 0 {{
        3
    }} else if invalid_message <= 0 {{
        4
    }} else if !deep {{
        5
    }} else if deep_code != 10 {{
        6
    }} else if deep_offset < 0 {{
        7
    }} else if !too_many {{
        8
    }} else if too_many_code != 10 && too_many_code != 14 {{
        20 + too_many_code
    }} else if !too_big {{
        10
    }} else if too_big_code != 10 {{
        11
    }} else if !empty_close {{
        12
    }} else {{
        0
    }}
}}
"#
    );

    let oversized_json = format!("\"{}\"", "a".repeat(1_100_000));
    let Some(output) =
        compile_and_run_stdlib_import_program_with_stdin("json-errors", &source, &oversized_json)
    else {
        return;
    };

    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_json_bool_wrong_kind_updates_last_error_code() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "json-bool-wrong-kind",
        r#"
import std::json;
import std::status;

def main() -> i64 {
    let parsed = json_parse("[1]").unwrap_or(JsonDoc { handle: 0 });
    let root = parsed.root();
    let value = root.array_get(0).unwrap_or(JsonValue { doc_handle: 0, node_id: 0 });
    let wrong_kind = value.bool_value().err().unwrap_or(0);
    let last = json_last_error_code();
    let closed = parsed.close();

    if wrong_kind == STATUS_INVALID_ARGUMENT() && last == STATUS_INVALID_ARGUMENT() && closed {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_args_pipeline_ir_declares_runtime_calls() {
    let source = super::expand_stdlib_imports_for_source(
        "import std::args;\n\ndef main() -> i64 {\n    arg_copy(0, ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 })).unwrap_or(0)\n}\n",
    )
    .expect("args import should expand");

    let (ir, _) =
        compile_source_with_phase_timings(&source, 1).expect("args source should compile");

    assert!(ir.contains("declare void @sengoo_args_init(i64, i64)"));
    assert!(ir.contains("declare i64 @sengoo_args_len()"));
    assert!(ir.contains("declare i64 @sengoo_arg_len(i64)"));
    assert!(ir.contains("declare i64 @sengoo_arg_copy(i64, i64, i64)"));
}

#[test]
fn examples_smoke_traits_iterator_basic() {
    assert_example_output(
        "traits-iterator-basic",
        "examples/traits/01_iterator_basic.sg",
        "6",
    );
}

#[test]
fn examples_smoke_traits_method_specialization() {
    assert_example_output(
        "traits-method-specialization",
        "examples/traits/02_method_specialization.sg",
        "42",
    );
}

#[test]
fn examples_smoke_ffi_sengoo_calls_c() {
    assert_example_output_with_c_inputs(
        "ffi-sengoo-calls-c",
        "examples/ffi/sengoo_calls_c.sg",
        &["examples/ffi/c_add.c"],
        "42",
    );
}

#[test]
fn native_link_metadata_reaches_linker_arguments() {
    let source = r#"
        #[link(name = "sample")]
        extern "C" {
            pub fn sample_ping() -> i64;
        }

        def main() -> i64 {
            return sample_ping();
        }
    "#;
    let llvm_ir = compile_source(source, 1).expect("native link metadata should compile");
    assert!(llvm_ir.contains("declare i64 @sample_ping()"));

    let target = NativeBuildTarget::host();
    let args = native_library_link_args(&["sample".to_string()], &target, &[]);
    if cfg!(windows) {
        assert!(
            args.iter().any(|arg| arg == "sample.lib"),
            "expected sample.lib in linker args, got {:?}",
            args
        );
    } else {
        assert_eq!(args, vec!["-lsample".to_string()]);
    }
}

#[test]
fn native_link_graph_collection_unions_imported_modules() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-sgc-native-link-graph-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("dep")).unwrap();
    fs::write(
        root.join("dep").join("ffi.sg"),
        r#"
            #[link(name = "sample")]
            extern "C" {
                pub fn sample_ping() -> i64;
            }
        "#,
    )
    .unwrap();
    fs::write(
        root.join("main.sg"),
        r#"
            import dep::ffi;

            def main() -> i64 {
                return sample_ping();
            }
        "#,
    )
    .unwrap();

    let main_source = fs::read_to_string(root.join("main.sg")).unwrap();
    let libraries =
        super::collect_native_link_libraries_for_graph(&root.join("main.sg"), &main_source)
            .expect("native link graph collection should succeed");
    assert_eq!(libraries, vec!["sample".to_string()]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn package_native_c_sources_discovered_from_module_graph() {
    let root = workspace_root_for_tests().join("packages/sgplatform");
    if !root.join("Sengoo.toml").is_file() {
        return;
    }
    let main_source = fs::read_to_string(root.join("tests/platform_smoke.sg")).unwrap();
    let sources = super::collect_package_native_c_sources(
        &root.join("tests/platform_smoke.sg"),
        &main_source,
    );
    assert!(
        sources.iter().any(
            |path| path.file_name().and_then(|name| name.to_str()) == Some("sgplatform_shim.c")
        ),
        "expected sgplatform native shim in {:?}",
        sources
    );
}

#[test]
fn examples_smoke_ffi_c_calls_sengoo_export() {
    let source = read_example_source("examples/ffi/sengoo_exports.sg");
    let llvm_ir = compile_source(&source, 1).expect("FFI export example should compile");

    let clang = match find_clang() {
        Some(clang) => clang,
        None => return,
    };

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let ll_path = temp_artifact("examples-smoke-ffi-export", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let sengoo_obj = temp_artifact("examples-smoke-ffi-export-sengoo", obj_ext);
    let c_obj = temp_artifact("examples-smoke-ffi-export-c", obj_ext);
    let exe_path = temp_artifact(
        "examples-smoke-ffi-export",
        if cfg!(windows) { "exe" } else { "" },
    );
    if compile_ir_to_object(&clang, &ll_path, &sengoo_obj, 1, None, false).is_err()
        || compile_ir_to_object(
            &clang,
            &workspace_root_for_tests().join("examples/ffi/c_calls_sengoo.c"),
            &c_obj,
            1,
            None,
            false,
        )
        .is_err()
        || link_native_binary_from_objects(
            &clang,
            &[sengoo_obj.clone(), c_obj.clone()],
            &exe_path,
            None,
            None,
        )
        .is_err()
    {
        let _ = fs::remove_file(&ll_path);
        let _ = fs::remove_file(&sengoo_obj);
        let _ = fs::remove_file(&c_obj);
        let _ = fs::remove_file(&exe_path);
        return;
    }

    let output = Command::new(&exe_path)
        .output()
        .expect("C caller executable should run");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&sengoo_obj);
    let _ = fs::remove_file(&c_obj);
    let _ = fs::remove_file(&exe_path);
}

fn compile_and_run_stdlib_program(tag: &str, source: &str) -> Option<std::process::Output> {
    let clang = find_clang()?;

    let combined = format!("{}\n\n{}", load_stdlib_surface_source(), source);
    let llvm_ir = compile_compiler_ir(&combined).expect("stdlib source should compile");
    let ll_path = temp_artifact(&format!("stdlib-runtime-{}", tag), "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(
        &format!("stdlib-runtime-{}", tag),
        if cfg!(windows) { "exe" } else { "" },
    );
    let obj_path = temp_artifact(
        &format!("stdlib-runtime-{}", tag),
        if cfg!(windows) { "obj" } else { "o" },
    );
    compile_ir_to_object(&clang, &ll_path, &obj_path, 2, None, false).unwrap();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);
    let runtime_c = workspace_root.join("tools/stdlib/runtime.c");
    if !stdlib_runtime_c_is_compilable(&clang, &runtime_c) {
        let _ = fs::remove_file(&ll_path);
        let _ = fs::remove_file(&obj_path);
        let _ = fs::remove_file(&exe_path);
        return None;
    }
    let mut object_paths = vec![obj_path.clone()];
    object_paths
        .extend(ensure_runtime_objects(&clang, &runtime_c.to_string_lossy(), 2, None).unwrap());
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("stdlib binary should run");

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    Some(output)
}

fn compile_and_run_stdlib_import_program_with_stdin(
    tag: &str,
    source: &str,
    stdin: &str,
) -> Option<std::process::Output> {
    let source = expand_stdlib_imports_for_source(source)
        .unwrap_or_else(|err| panic!("stdlib imports should expand: {err}"));
    let llvm_ir = compile_source(&source, 1)
        .unwrap_or_else(|err| panic!("stdlib source should compile: {err}"));

    let clang = find_clang()?;
    let runtime_c = find_runtime_c()?;
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return None;
    }

    let ll_path = temp_artifact(&format!("stdlib-import-runtime-{tag}"), "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact(&format!("stdlib-import-runtime-{tag}-main"), obj_ext);
    compile_ir_to_object(&clang, &ll_path, &main_obj, 2, None, false).unwrap();

    let exe_path = temp_artifact(
        &format!("stdlib-import-runtime-{tag}"),
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![main_obj.clone()];
    object_paths.extend(ensure_runtime_objects(&clang, &runtime_c, 2, None).unwrap());
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let mut child = Command::new(&exe_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("stdlib binary should spawn");
    if let Some(mut input) = child.stdin.take() {
        input
            .write_all(stdin.as_bytes())
            .expect("stdin should be writable");
    }
    let output = child
        .wait_with_output()
        .expect("stdlib binary should run to completion");

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&main_obj);
    let _ = fs::remove_file(&exe_path);
    Some(output)
}

fn compile_and_run_program_with_opt_level(
    tag: &str,
    source: &str,
    opt_level: u8,
) -> Option<std::process::Output> {
    let llvm_ir = compile_source(source, opt_level)
        .unwrap_or_else(|err| panic!("source should compile at O{opt_level}: {err}"));

    let clang = find_clang()?;
    let runtime_c = find_runtime_c()?;
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return None;
    }

    let ll_path = temp_artifact(&format!("native-opt-{tag}-O{opt_level}"), "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact(&format!("native-opt-{tag}-O{opt_level}-main"), obj_ext);
    compile_ir_to_object(&clang, &ll_path, &main_obj, opt_level, None, false).unwrap();

    let exe_path = temp_artifact(
        &format!("native-opt-{tag}-O{opt_level}"),
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![main_obj.clone()];
    object_paths.extend(ensure_runtime_objects(&clang, &runtime_c, opt_level, None).unwrap());
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("native binary should run");

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&main_obj);
    let _ = fs::remove_file(&exe_path);
    Some(output)
}

fn compile_and_run_stdlib_import_program_with_native_runtime(
    tag: &str,
    source: &str,
) -> Option<std::process::Output> {
    let source = expand_stdlib_imports_for_source(source)
        .unwrap_or_else(|err| panic!("stdlib imports should expand: {err}"));
    let llvm_ir = compile_source(&source, 1)
        .unwrap_or_else(|err| panic!("stdlib source should compile: {err}"));

    let clang = find_clang()?;
    let runtime_c = find_runtime_c()?;
    if !stdlib_runtime_c_is_compilable(&clang, Path::new(&runtime_c)) {
        return None;
    }

    let ll_path = temp_artifact(&format!("stdlib-import-native-runtime-{tag}"), "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let main_obj = temp_artifact(&format!("stdlib-import-native-runtime-{tag}-main"), obj_ext);
    compile_ir_to_object(&clang, &ll_path, &main_obj, 2, None, false).unwrap();

    let exe_path = temp_artifact(
        &format!("stdlib-import-native-runtime-{tag}"),
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![main_obj.clone()];
    append_native_runtime_inputs(&clang, &mut object_paths, Some(&runtime_c), 2, None).unwrap();
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("stdlib binary should run");

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&main_obj);
    let _ = fs::remove_file(&exe_path);
    Some(output)
}

fn compile_reflection_example_via_std_imports(example: &str) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);
    let example_source = fs::read_to_string(workspace_root.join(example))
        .unwrap_or_else(|err| panic!("reflection example {example} should exist: {err}"));
    let combined = expand_stdlib_imports_for_source(&example_source).unwrap_or_else(|err| {
        panic!("reflection example {example} stdlib imports should expand: {err}")
    });
    compile_compiler_ir(&combined).unwrap_or_else(|err| {
        panic!("reflection example {example} should compile through its stdlib imports: {err}")
    })
}

#[tokio::test(flavor = "current_thread")]
async fn build_emit_llvm_loads_stdlib_collection_imports() {
    let root =
        std::env::temp_dir().join(format!("sengoo-sgc-stdlib-imports-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let main_path = root.join("main.sg");
    let output_path = root.join("main.ll");
    fs::write(
        &main_path,
        r#"
import std::collections;

def main() -> i64 {
    let values = vec_new_i64();
    values.push(41);
    let answer = values.get(0).unwrap_or(0) + 1;
    values.free();
    answer
}
"#,
    )
    .unwrap();

    let result = cmd_build(
        &main_path.to_string_lossy(),
        Some(&output_path.to_string_lossy()),
        1,
        ContractChecksMode::Auto,
        true,
        true,
        false,
        FrontendJobs::Fixed(1),
        false,
        super::ReflectionCliOptions::default(),
        None,
        None,
        false,
    )
    .await;

    assert!(
        result.is_ok(),
        "sgc build should compile source-level stdlib imports: {result:?}"
    );
    let llvm_ir = fs::read_to_string(&output_path).unwrap();
    assert!(llvm_ir.contains("vec_new_i64"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn stdlib_collections_import_preloads_buffer_and_text_collection_symbols() {
    let source = r#"
import std::collections;

def main() -> i64 {
    let list = text_list_new();
    let map = string_map_i64_new();
    let buffer = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let copied = list.get_copy(0, buffer).unwrap_or(0);
    buffer.free();
    list.free();
    map.free();
    copied
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("collections stdlib import should expand with transitive dependencies");
    let ir = compile_compiler_ir(&combined)
        .expect("collections import should make Buffer-backed text collections usable");

    assert!(ir.contains("sengoo_ffi_buffer_new"));
    assert!(ir.contains("sengoo_text_list_new"));
    assert!(ir.contains("sengoo_text_list_get_copy"));
    assert!(ir.contains("sengoo_string_map_new"));
}

#[test]
fn examples_smoke_reflection_db_open_query() {
    let ir = compile_reflection_example_via_std_imports("examples/reflection/db_open_query.sg");
    assert!(ir.contains("sengoo_db_open"));
}

#[test]
fn examples_smoke_reflection_lua54_eval() {
    let ir = compile_reflection_example_via_std_imports("examples/reflection/lua54_eval.sg");
    assert!(ir.contains("sengoo_lua54_open"));
    assert!(ir.contains("sengoo_lua54_call_i64_value"));
}

#[test]
fn examples_smoke_reflection_proto_encode_decode() {
    let ir =
        compile_reflection_example_via_std_imports("examples/reflection/proto_encode_decode.sg");
    assert!(ir.contains("sengoo_proto_user_event_encode"));
}

#[test]
fn stdlib_proto_import_preloads_buffer_wrapper() {
    let source = r#"
import std::proto;

def main() -> i64 {
    let event = proto_user_event(7, "alice", 42);
    let buffer = ffi_buffer_new(128).unwrap_or(Buffer { handle: 0 });
    let encoded = proto_user_event_encode(event, buffer);
    buffer.free();
    encoded.unwrap_or(0)
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("proto stdlib import should expand with transitive dependencies");
    let ir = compile_compiler_ir(&combined)
        .expect("proto import should make the managed Buffer encode wrapper usable");

    assert!(ir.contains("sengoo_ffi_buffer_new"));
    assert!(ir.contains("sengoo_proto_user_event_encode"));
}

#[test]
fn stdlib_proto_import_preloads_owned_decode_wrapper() {
    let source = r#"
import std::proto;

def main() -> i64 {
    let encoded = ffi_buffer_new(128).unwrap_or(Buffer { handle: 0 });
    let name = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let decoded = proto_user_event_decode(encoded, 16).unwrap_or(ProtoDecodedUserEvent { handle: 0 });
    let id = decoded.id();
    let copied = decoded.name_copy(name).unwrap_or(0);
    decoded.close();
    encoded.free();
    name.free();
    id + copied
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("proto stdlib import should expand with transitive dependencies");
    let ir = compile_compiler_ir(&combined)
        .expect("proto import should make the owned decode wrapper usable");

    assert!(ir.contains("sengoo_proto_user_event_decode_open"));
    assert!(ir.contains("sengoo_proto_user_event_decoded_name_copy"));
    assert!(ir.contains("sengoo_proto_user_event_decoded_close"));
}

#[test]
fn stdlib_net_import_preloads_buffer_wrapper() {
    let source = r#"
import std::net;

def main() -> i64 {
    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let copied = net_error_name_copy(0, buffer);
    buffer.free();
    copied.unwrap_or(0)
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("net stdlib import should expand with transitive dependencies");
    let ir = compile_compiler_ir(&combined)
        .expect("net import should make managed Buffer output wrappers usable");

    assert!(ir.contains("sengoo_ffi_buffer_new"));
    assert!(ir.contains("sengoo_net_error_name_copy"));
}

#[test]
fn stdlib_net_import_preloads_http_server_wrappers() {
    let source = r#"
import std::net;

def main() -> i64 {
    let server = http_server_bind("127.0.0.1", 0).unwrap_or(HttpServer { handle: 0 });
    let routed = server.add_route("GET", "/health", 200, "ok").unwrap_or(false);
    let served = server.serve_once(1).unwrap_or(false);
    server.close();
    if routed && !served { 1 } else { 0 }
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("net stdlib import should expand with HTTP server wrappers");
    let ir = compile_compiler_ir(&combined).expect("net import should expose HTTP server wrappers");

    assert!(ir.contains("sengoo_http_server_bind"));
    assert!(ir.contains("sengoo_http_server_add_route"));
    assert!(ir.contains("sengoo_http_server_serve_once"));
}

#[test]
fn stdlib_net_import_preloads_http_server_request_wrappers() {
    let source = r#"
import std::net;

def main() -> i64 {
    let server = http_server_bind("127.0.0.1", 0).unwrap_or(HttpServer { handle: 0 });
    let request = server.next_request(1).unwrap_or(HttpServerRequest { handle: 0 });
    let buffer = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let method = request.method_string().is_ok;
    let path = request.path_string().is_ok;
    let query = request.query_string().is_ok;
    let version = request.version_string().is_ok;
    let header = request.header_string("x-test").is_ok;
    let body_len = request.body_len().unwrap_or(0);
    let copied = request.body_copy(buffer).unwrap_or(0);
    let typed = request.respond_with_content_type(200, "text/plain", "ok").unwrap_or(false);
    let answered = request.respond(200, "ok").unwrap_or(false);
    request.close();
    buffer.free();
    server.close();
    if method && path && query && version && header && typed && answered && body_len >= 0 && copied >= 0 { 1 } else { 0 }
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("net stdlib import should expand with HTTP server request wrappers");
    let ir = compile_compiler_ir(&combined)
        .expect("net import should expose HTTP server request wrappers");

    assert!(ir.contains("sengoo_http_server_next_request"));
    assert!(ir.contains("sengoo_http_request_method_len"));
    assert!(ir.contains("sengoo_http_request_method_copy"));
    assert!(ir.contains("sengoo_http_request_path_copy"));
    assert!(ir.contains("sengoo_http_request_query_copy"));
    assert!(ir.contains("sengoo_http_request_version_copy"));
    assert!(ir.contains("sengoo_http_request_header_len"));
    assert!(ir.contains("sengoo_http_request_header_copy"));
    assert!(ir.contains("sengoo_http_request_body_len"));
    assert!(ir.contains("sengoo_http_request_body_copy"));
    assert!(ir.contains("sengoo_http_request_respond"));
    assert!(ir.contains("sengoo_http_request_respond_with_content_type"));
    assert!(ir.contains("sengoo_http_request_close"));
}

#[test]
fn stdlib_db_import_preloads_buffer_wrapper() {
    let source = r#"
import std::db;

def main() -> i64 {
    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let copied = db_last_error_copy(buffer);
    buffer.free();
    copied.unwrap_or(0)
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("db stdlib import should expand with transitive dependencies");
    let ir = compile_compiler_ir(&combined)
        .expect("db import should make managed Buffer output wrappers usable");

    assert!(ir.contains("sengoo_ffi_buffer_new"));
    assert!(ir.contains("sengoo_db_last_error_copy"));
}

#[test]
fn stdlib_lua54_import_preloads_buffer_wrapper() {
    let source = r#"
import std::lua54;

def main() -> i64 {
    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let copied = lua54_last_error_copy(buffer);
    buffer.free();
    copied.unwrap_or(0)
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("lua54 stdlib import should expand with transitive dependencies");
    let ir = compile_compiler_ir(&combined)
        .expect("lua54 import should make managed Buffer output wrappers usable");

    assert!(ir.contains("sengoo_ffi_buffer_new"));
    assert!(ir.contains("sengoo_lua54_last_error_copy"));
}

#[test]
fn stdlib_ffi_and_lua54_imports_preload_value_call_wrappers() {
    let source = r#"
import std::lua54;

def main() -> i64 {
    let lib = CLib { handle: 0 };
    let ffi_value = lib.call_i64_2("add", 20, 22).unwrap_or(0);
    let object = lib.object_create_1("counter_new", 5, "counter_drop").unwrap_or(CppObject { handle: 0 });
    let object_value = object.call_i64_1("counter_add", 7).unwrap_or(0);
    let lua = Lua54 { handle: 0 };
    let lua_value = lua.call_i64_2("add", 2, 5).unwrap_or(0);
    ffi_value + object_value + lua_value
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("lua54 stdlib import should expand with its ffi dependency");
    let ir = compile_compiler_ir(&combined)
        .expect("ffi and lua54 imports should make value call wrappers usable");

    assert!(ir.contains("sengoo_ffi_c_call_i64_value"));
    assert!(ir.contains("sengoo_ffi_object_create_value"));
    assert!(ir.contains("sengoo_ffi_object_call_i64_value"));
    assert!(ir.contains("sengoo_lua54_call_i64_value"));
}

#[test]
fn stdlib_file_import_preloads_buffer_wrapper() {
    let source = r#"
import std::file;

def main() -> i64 {
    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let wrote = file_write_str("target/sgc-file-smoke.txt", "hello").unwrap_or(0);
    let read = file_read_into("target/sgc-file-smoke.txt", buffer).unwrap_or(0);
    let len = file_len("target/sgc-file-smoke.txt").unwrap_or(0);
    let exists = file_exists("target/sgc-file-smoke.txt");
    buffer.free();
    file_remove("target/sgc-file-smoke.txt");
    if exists { wrote + read + len } else { 0 }
}
"#;
    let combined = expand_stdlib_imports_for_source(source)
        .expect("file stdlib import should expand with its ffi dependency");
    let ir = compile_compiler_ir(&combined)
        .expect("file import should make managed Buffer file wrappers usable");

    assert!(ir.contains("sengoo_file_write_str"));
    assert!(ir.contains("sengoo_file_read_into"));
    assert!(ir.contains("sengoo_file_len"));
}

#[test]
fn examples_smoke_reflection_net_tcp_echo() {
    let ir = compile_reflection_example_via_std_imports("examples/reflection/net_tcp_echo.sg");
    assert!(ir.contains("sengoo_tcp_connect"));
}

#[test]
fn examples_smoke_reflection_net_http_server() {
    let ir = compile_reflection_example_via_std_imports("examples/reflection/net_http_server.sg");
    assert!(ir.contains("sengoo_http_server_bind"));
    assert!(ir.contains("sengoo_http_server_add_route"));
    assert!(ir.contains("sengoo_http_server_add_middleware_require_header"));
}

#[test]
fn examples_smoke_reflection_ffi_load_call() {
    let ir = compile_reflection_example_via_std_imports("examples/reflection/ffi_load_call.sg");
    assert!(ir.contains("sengoo_ffi_c_open"));
    assert!(ir.contains("sengoo_ffi_c_call_i64_value"));
}

macro_rules! require_stdlib_runtime_output {
    ($tag:expr, $source:expr $(,)?) => {{
        let Some(output) = compile_and_run_stdlib_program($tag, $source) else {
            return;
        };
        output
    }};
}

#[tokio::test(flavor = "current_thread")]
async fn stdlib_surface_cross_module_generic_type_imports_probe_successfully() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-stdlib-generic-cross-module-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let main_path = root.join("main.sg");
    let util_path = root.join("util.sg");
    let collections_path = root.join("collections.sg");
    fs::write(&collections_path, load_stdlib_surface_source()).unwrap();
    fs::write(
        &util_path,
        r#"
import collections;

def util_ok_flag() -> Option<bool> {
    let ok_result: Result<bool, i64> = Result { is_ok: true, value: true, error: 9 };
    ok_result.ok()
}

def util_err_flag() -> Option<bool> {
    let err_result: Result<i64, bool> = Result { is_ok: false, value: 0, error: true };
    err_result.err()
}
"#,
    )
    .unwrap();
    fs::write(
        &main_path,
        r#"
import collections;
import util;

def main() -> i64 {
    let imported: Option<bool> = util_ok_flag();
    if imported.unwrap_or(false) {
        1
    } else {
        0
    }
}
"#,
    )
    .unwrap();

    let main_source = fs::read_to_string(&main_path).unwrap();
    let snapshot = collect_module_graph_snapshot(
        &main_path,
        &main_source,
        None,
        None,
        FrontendProbeMode::VerifyAll,
        FrontendJobs::Auto,
        false,
        true,
    );
    let root_module = super::canonical_or_lossy(&main_path);
    let root_deps = snapshot
        .dependency_edges
        .get(&root_module)
        .expect("main module should appear in dependency graph");
    assert!(
        snapshot.module_fingerprints.len() >= 2 && !root_deps.is_empty(),
        "expected imported files to participate in the frontend dependency graph"
    );
    assert!(
        snapshot.diagnostics.is_empty(),
        "expanded frontend probes should not report false diagnostics: {:?}",
        snapshot.diagnostics
    );

    super::frontend_probe_module_full(&main_path.to_string_lossy(), &main_source)
        .expect("cross-module imported stdlib generic types should probe successfully");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn runtime_function_value_parameter_executes_non_capturing_lambda() {
    let output = require_stdlib_runtime_output!(
        "fn-value-call",
        r#"
def apply_twice(x: i64, f: fn(i64) -> i64) -> i64 {
    f(f(x))
}

def main() -> i64 {
    let add1 = |y| y + 1;
    apply_twice(40, add1)
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_handles_boundary_values_and_resource_methods() {
    let output = require_stdlib_runtime_output!(
        "boundary",
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    let empty_pop = vec.pop().unwrap_or(11);
    vec.push(5);
    vec.push(9);
    let missing_index = vec.get(9).unwrap_or(22);

    let iter = vec.iter();
    let first = iter.next().unwrap_or(0);
    iter.reset();
    let first_again = iter.next().unwrap_or(0);

    let map = hashmap_new_i64_i64();
    let missing_key = map.get(7).unwrap_or(33);

    iter.free();
    map.free();
    vec.free();

    empty_pop + missing_index + first + first_again + missing_key
}
"#,
    );

    assert_eq!(output.status.code(), Some(76));
}

#[test]
fn stdlib_surface_runtime_vec_remove_shifts_tail_elements() {
    let output = require_stdlib_runtime_output!(
        "vec-remove-contains",
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(3);
    vec.push(5);
    vec.push(7);

    let removed = vec.remove(1).unwrap_or(0);
    let tail = vec.get(1).unwrap_or(0);
    vec.free();
    removed + tail
}
"#,
    );

    assert_eq!(output.status.code(), Some(12));
}

#[test]
fn stdlib_surface_runtime_clear_and_is_empty_are_correct() {
    let output = require_stdlib_runtime_output!(
        "clear-empty",
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.push(2);
    vec.clear();

    let map = hashmap_new_i64_i64();
    map.insert(1, 2);
    map.insert(3, 4);
    map.clear();

    if vec.is_empty() && map.is_empty() {
        1
    } else {
        0
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn stdlib_surface_runtime_rc_clone_counts_until_last_drop() {
    let output = require_stdlib_runtime_output!(
        "rc-shared-count",
        r#"
def main() -> i64 {
    let first = rc_new_i64(40);
    let second = first.clone();
    let count = first.strong_count();
    let value = second.get();
    let flag = rc_new_bool(true);
    if count == 2 and value == 40 and flag.get() {
        42
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_rc_string_clones_until_last_drop() {
    let output = require_stdlib_runtime_output!(
        "rc-string-shared",
        r#"
def main() -> i64 {
    let text = string_from_str("hello").unwrap_or(String { handle: 0 });
    let first = rc_new_string(text);
    let second = first.clone();
    let copy = second.get();
    if first.strong_count() == 2 and copy.len() == 5 {
        42
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_rc_value_trait_constructs_shared_handles() {
    let output = require_stdlib_runtime_output!(
        "rc-value-trait-shared",
        r#"
def share<T: RcValue>(value: T) -> Rc<T> {
    value.rc()
}

def main() -> i64 {
    let first = share(40);
    let second = first.clone();
    let flag = share(true);
    let ok = first.strong_count() == 2 and second.get() == 40 and flag.get();
    if ok {
        42
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_rc_generic_payload_drops_once_after_last_release() {
    let output = require_stdlib_runtime_output!(
        "rc-generic-payload-drop",
        r#"
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
}

struct Pair {
    text: String,
}

def make_shared_pair() -> i64 {
    let text = string_from_str("hello").unwrap_or(String { handle: 0 });
    let pair = Pair { text: text };
    let first = rc_new(pair);
    let second = first.clone();
    first.strong_count() + second.strong_count()
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let count = make_shared_pair();
    let after = sengoo_string_live_handle_count();
    if count == 4 and after == before { 42 } else { 1 }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_generic_filter_skips_rejected_items() {
    let output = require_stdlib_runtime_output!(
        "generic-filter-i64",
        r#"
def main() -> i64 {
    let values: Vec<i64> = vec_new();
    values.push(10);
    values.push(20);
    values.push(30);
    let keep: fn(&i64) -> bool = |value| *value == 20;
    let iter = values.into_iter().filter(keep);
    let first: Option<i64> = iter.next();
    if first.is_some && first.value == 20 { 42 } else { 1 }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_generic_filter_drops_owned_items_once() {
    let output = require_stdlib_runtime_output!(
        "generic-filter-owned-drop",
        r#"
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
}

struct Payload {
    text: String,
    keep: bool,
}

impl Payload {
    def should_keep(&self) -> bool { self.keep }
}

def exercise_filter() -> i64 {
    let values: Vec<Payload> = vec_new();
    values.push(Payload {
        text: string_from_str("rejected").unwrap_or(String { handle: 0 }),
        keep: false,
    });
    values.push(Payload {
        text: string_from_str("accepted").unwrap_or(String { handle: 0 }),
        keep: true,
    });
    values.push(Payload {
        text: string_from_str("remaining").unwrap_or(String { handle: 0 }),
        keep: true,
    });
    let predicate: fn(&Payload) -> bool = |payload| payload.should_keep();
    let iter = values.into_iter().filter(predicate);
    let accepted: Option<Payload> = iter.next();
    if accepted.is_some { accepted.value.text.len() } else { 0 }
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let accepted_len = exercise_filter();
    let after = sengoo_string_live_handle_count();
    if accepted_len == 8 && after == before { 42 } else { 1 }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_generic_collect_preserves_owned_items() {
    let output = require_stdlib_runtime_output!(
        "generic-collect-owned",
        r#"
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
}

struct Payload {
    text: String,
    keep: bool,
}

impl Payload {
    def should_keep(&self) -> bool { self.keep }
}

def exercise_collect() -> i64 {
    let values: Vec<Payload> = vec_new();
    values.push(Payload {
        text: string_from_str("skip").unwrap_or(String { handle: 0 }),
        keep: false,
    });
    values.push(Payload {
        text: string_from_str("accepted").unwrap_or(String { handle: 0 }),
        keep: true,
    });
    let predicate: fn(&Payload) -> bool = |payload| payload.should_keep();
    let collected: Vec<Payload> = values.into_iter().filter(predicate).collect();
    let accepted: Option<Payload> = collected.pop();
    if accepted.is_some { accepted.value.text.len() } else { 0 }
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let accepted_len = exercise_collect();
    let after = sengoo_string_live_handle_count();
    if accepted_len == 8 && after == before { 42 } else { 1 }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_generic_count_and_fold_execute() {
    let output = require_stdlib_runtime_output!(
        "generic-count-fold",
        r#"
def main() -> i64 {
    let values: Vec<i64> = vec_new();
    values.push(10);
    values.push(20);
    values.push(30);
    let add: fn(i64, i64) -> i64 = |total, value| total + value;
    let total = values.into_iter().fold(0, add);

    let flags: Vec<i64> = vec_new();
    flags.push(1);
    flags.push(2);
    flags.push(3);
    let keep: fn(&i64) -> bool = |value| *value >= 2;
    let count = flags.into_iter().filter(keep).count();
    if total == 60 && count == 2 { 42 } else { 1 }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_generic_sum_and_explicit_collection_sinks_execute() {
    let output = require_stdlib_runtime_output!(
        "generic-sum-collection-sinks",
        r#"
#[derive(Hash, PartialEq, Eq)]
struct Key {
    value: i64,
}

def keep(value: &i64) -> bool { *value >= 2 }
def to_key(value: i64) -> Key { Key { value: value } }
def identity(value: i64) -> i64 { value }
def to_entry(value: i64) -> MapEntry<Key, i64> {
    MapEntry { key: Key { value: value }, value: value + 10 }
}

def main() -> i64 {
    let sum_values: Vec<i64> = vec_new();
    sum_values.push(1);
    sum_values.push(2);
    sum_values.push(3);
    sum_values.push(4);
    let total = sum_values.into_iter().skip(1).take(2).sum();

    let set_values: Vec<i64> = vec_new();
    set_values.push(1);
    set_values.push(2);
    set_values.push(2);
    set_values.push(3);
    let predicate: fn(&i64) -> bool = keep;
    let mapper: fn(i64) -> Key = to_key;
    let set: HashSet<Key> = set_values.into_iter().filter(predicate).map(mapper).collect_hashset();

    let map_values: Vec<i64> = vec_new();
    map_values.push(4);
    map_values.push(5);
    let projector: fn(i64) -> MapEntry<Key, i64> = to_entry;
    let map: HashMap<Key, i64> = map_values.into_iter().collect_hashmap(projector);

    let chain_values: Vec<i64> = vec_new();
    chain_values.push(1);
    chain_values.push(2);
    chain_values.push(3);
    let identity_fn: fn(i64) -> i64 = identity;
    let chain_count = chain_values.into_iter().map(identity_fn).take(2).skip(1).count();

    let indexed_values: Vec<i64> = vec_new();
    indexed_values.push(1);
    indexed_values.push(2);
    indexed_values.push(3);
    let indexed = indexed_values.into_iter().filter(predicate).skip(1).take(1).enumerate();
    let first: Option<EnumeratedItem<i64>> = indexed.next();

    if total != 5 { 11 }
    else if set.len() != 2 { 12 }
    else if map.len() != 2 { 13 }
    else if chain_count != 1 { 14 }
    else if first.is_none() { 15 }
    else if first.value.index != 0 { 16 }
    else if first.value.value != 3 { 17 }
    else { 42 }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_owned_dyn_scope_exit_drop_releases_handle() {
    let output = require_stdlib_runtime_output!(
        "owned-dyn-scope-exit-drop",
        r#"
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
    fn sengoo_string_free_status(handle: i64) -> i64;
}

trait Speak {
    def speak(&self) -> i64 {
        0
    }
}

struct Guard {
    handle: i64,
}

impl Drop for Guard {
    def drop(&mut self) {
        sengoo_string_free_status(self.handle);
    }
}

impl Speak for Guard {
    def speak(&self) -> i64 {
        1
    }
}

def scoped(handle: i64) -> i64 {
    let g = Guard { handle: handle };
    let s: dyn Speak = g;
    s.speak()
}

def main() -> i64 {
    let text = string_from_str("hello").unwrap_or(String { handle: 0 });
    let handle = text.handle;
    let before = sengoo_string_live_handle_count();
    let spoke = scoped(handle);
    let after = sengoo_string_live_handle_count();
    if spoke == 1 and after == before - 1 {
        42
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_owned_dyn_explicit_drop_releases_handle() {
    let output = require_stdlib_runtime_output!(
        "owned-dyn-explicit-drop",
        r#"
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
    fn sengoo_string_free_status(handle: i64) -> i64;
}

trait Speak {
    def speak(&self) -> i64 {
        0
    }
}

struct Guard {
    handle: i64,
}

impl Drop for Guard {
    def drop(&mut self) {
        sengoo_string_free_status(self.handle);
    }
}

impl Speak for Guard {
    def speak(&self) -> i64 {
        1
    }
}

def main() -> i64 {
    let text = string_from_str("hello").unwrap_or(String { handle: 0 });
    let handle = text.handle;
    let before = sengoo_string_live_handle_count();
    let g = Guard { handle: handle };
    let s: dyn Speak = g;
    s.drop();
    let after = sengoo_string_live_handle_count();
    if after == before - 1 {
        42
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_derived_hash_matches_manual_hash_into() {
    let output = require_stdlib_runtime_output!(
        "derived-hash-runtime-state-consistency",
        r#"
#[derive(Hash)]
struct Point {
    x: i64,
    y: i64,
    flag: bool,
}

def main() -> i64 {
    let p = Point { x: 7, y: 11, flag: true };
    let derived = p.hash();
    let mut h = hasher_new();
    h.write_i64(7);
    h.write_i64(11);
    h.write_bool(true);
    let manual = h.finish();
    if derived == manual and derived != 0 {
        42
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_rc_generic_borrow_reads_shared_payload() {
    let output = require_stdlib_runtime_output!(
        "rc-generic-payload-borrow",
        r#"
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
}

def observe(first: &Rc<i64>, second: &Rc<i64>) -> i64 {
    let a = first.borrow();
    let b = second.borrow();
    (*a) + (*b)
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let payload = 21;
    let first = rc_new(payload);
    let second = first.clone();
    let observed = observe(&first, &second);
    let count = first.strong_count();
    if observed == 42 and count == 2 {
        let mid = sengoo_string_live_handle_count();
        if mid == before {
            42
        } else {
            2
        }
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_rc_generic_payload_accepts_temporary_value() {
    let output = require_stdlib_runtime_output!(
        "rc-generic-payload-temp",
        r#"
def main() -> i64 {
    let first = rc_new(21);
    let second = first.clone();
    let a = first.borrow();
    let b = second.borrow();
    if (*a) + (*b) == 42 and first.strong_count() == 2 {
        42
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_rc_generic_borrow_reads_aggregate_field() {
    let output = require_stdlib_runtime_output!(
        "rc-generic-payload-borrow-field",
        r#"
struct Pair {
    value: i64,
}

def observe(first: &Rc<Pair>, second: &Rc<Pair>) -> i64 {
    let a = first.borrow();
    let b = second.borrow();
    (*a).value + (*b).value
}

def main() -> i64 {
    let first = rc_new(Pair { value: 21 });
    let second = first.clone();
    if observe(&first, &second) == 42 {
        42
    } else {
        1
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_hashmap_iter_sums_all_values() {
    let output = require_stdlib_runtime_output!(
        "hashmap-iter",
        r#"
def main() -> i64 {
    let map = hashmap_new_i64_i64();
    map.insert(1, 10);
    map.insert(9, 20);
    map.insert(17, 30);

    let iter = map.iter();
    let mut item = iter.next();
    let mut total = 0;
    while item.is_some {
        total = total + item.value;
        item = iter.next();
    }
    iter.free();
    map.free();
    total
}
"#,
    );

    assert_eq!(output.status.code(), Some(60));
}

#[test]
fn stdlib_surface_runtime_iterator_map_with_executes_non_capturing_lambda() {
    let output = require_stdlib_runtime_output!(
        "iter-higher-order",
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    vec.push(4);

    let add1 = |x| x + 1;

    let iter = vec.iter();
    let mapped = iter.map_with(add1).unwrap_or(0);
    iter.free();
    vec.free();
    mapped
}
"#,
    );

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn stdlib_surface_runtime_iterator_filter_even_progresses() {
    let output = require_stdlib_runtime_output!(
        "iter-filter-even-progresses",
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.push(2);
    vec.push(3);

    let iter = vec.iter();
    let first_even = iter.filter_even().unwrap_or(0);
    let done_after_match = iter.next().unwrap_or(0);
    iter.free();
    vec.free();

    first_even + done_after_match
}
"#,
    );

    assert_eq!(output.status.code(), Some(5));
}

#[test]
fn stdlib_surface_runtime_hashmap_remove_preserves_other_keys_in_probe_chain() {
    let output = require_stdlib_runtime_output!(
        "hashmap-probe-chain",
        r#"
def main() -> i64 {
    let map = hashmap_new_i64_i64();
    map.insert(1, 10);
    map.insert(9, 20);
    map.insert(17, 30);

    map.remove(1);

    let a = map.get(9).unwrap_or(0);
    let b = map.get(17).unwrap_or(0);
    map.free();
    a + b
}
"#,
    );

    assert_eq!(output.status.code(), Some(50));
}

#[test]
fn stdlib_surface_runtime_option_and_result_values_are_correct() {
    let output = require_stdlib_runtime_output!(
        "option-result",
        r#"
def main() -> i64 {
    let option_value = option_some_i64(2).map_add(3).and_then_mul(4).unwrap_or(0);
    let result_ok = result_ok_i64(5).map_add(1).and_then_mul(3).unwrap_or(0);
    let result_err = result_err_i64(7).map_err_add(2).unwrap_or(11);
    option_value + result_ok + result_err
}
"#,
    );

    assert_eq!(output.status.code(), Some(49));
}

#[test]
fn runtime_suite_name_prefers_bench_directory() {
    let suite_path = resolve_bench_suite_path("runtime", "runtime").unwrap();
    assert!(suite_path.ends_with(Path::new("bench").join("suites").join("runtime")));
    let cases = collect_bench_cases(&suite_path).unwrap();
    assert!(!cases.is_empty());
}

#[test]
fn frontend_probe_module_full_accepts_async_sources() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}

async def main() -> i64 {
    let f = add_one(41);
    await f
}
"#;

    let result = super::frontend_probe_module_full("tests/async.sg", source);
    assert!(
        result.is_ok(),
        "frontend probe should accept async sources, got: {:?}",
        result.err()
    );
}

#[test]
fn compile_phase_timings_include_expected_keys() {
    let source = "def main() -> i64 { 0 }";
    let (_, phases) = compile_source_with_phase_timings(source, 2).unwrap();
    assert!(phases.contains_key("parse"));
    assert!(phases.contains_key("typeck"));
    assert!(phases.contains_key("mir"));
    assert!(phases.contains_key("mir_prune"));
    assert!(phases.contains_key("codegen"));
    assert!(phases.contains_key("link"));
    // Frontend hotspot profiling: the `mir` bucket is sub-split so callers can
    // attribute cost to HIR lowering vs MIR lowering vs MIR optimization.
    assert!(phases.contains_key("hir_lower"));
    assert!(phases.contains_key("mir_lower"));
    assert!(phases.contains_key("mir_opt"));
}

#[test]
fn debug_info_metadata_is_emitted_to_llvm_ir() {
    let source =
        "def helper(value: i64) -> i64 {\n    let doubled = value * 2;\n    doubled\n}\n\n\
def main() -> i64 { helper(1) }\n";
    let llvm_path = temp_artifact("debug-info", "ll");
    compile_source_to_llvm_file_with_phase_timings_with_mode(
        source,
        1,
        &llvm_path,
        None,
        None,
        None,
        Some(DebugInfoConfig::for_source(
            "examples/debug/main.sg",
            source.to_string(),
        )),
    )
    .unwrap();

    let llvm_ir = fs::read_to_string(&llvm_path).unwrap();
    assert!(llvm_ir.contains("!llvm.dbg.cu"), "{llvm_ir}");
    assert!(llvm_ir.contains("!DICompileUnit"), "{llvm_ir}");
    assert!(
        llvm_ir.contains("!DISubprogram(name: \"main\""),
        "{llvm_ir}"
    );
    assert!(
        llvm_ir.contains("!DILocalVariable(name: \"value\", arg: 1"),
        "{llvm_ir}"
    );
    assert!(
        llvm_ir.contains("!DILocalVariable(name: \"doubled\""),
        "{llvm_ir}"
    );
    assert!(llvm_ir.contains("@llvm.dbg.value"), "{llvm_ir}");
    assert!(llvm_ir.contains("@llvm.dbg.declare"), "{llvm_ir}");
    assert!(llvm_ir.contains("define i64 @main() !dbg !"), "{llvm_ir}");
    assert!(llvm_ir.contains("ret i64"), "{llvm_ir}");
    assert!(llvm_ir.contains(", !dbg !"), "{llvm_ir}");

    let _ = fs::remove_file(llvm_path);
}

fn find_llvm_dwarfdump() -> Option<&'static str> {
    ["llvm-dwarfdump", "llvm-dwarfdump.exe"]
        .into_iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
        })
}

fn dwarf_debug_test_target() -> NativeBuildTarget {
    NativeBuildTarget {
        triple: crate::cross_compile::REFERENCE_TARGET_LINUX_GNU.to_string(),
    }
}

fn compile_dwarf_debug_test_object(clang: &str, llvm_path: &Path, object_path: &Path) {
    let output = Command::new(clang)
        .arg(format!(
            "--target={}",
            crate::cross_compile::REFERENCE_TARGET_LINUX_GNU
        ))
        .args(["-c", "-x", "ir", "-g", "-O0", "-Wno-override-module"])
        .arg(llvm_path)
        .arg("-o")
        .arg(object_path)
        .output()
        .expect("clang should compile DWARF test IR");
    assert!(
        output.status.success(),
        "clang failed to compile DWARF test IR:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn parse_dwarf_line_rows(dump: &str) -> Vec<(u64, u64)> {
    dump.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("0x") {
                return None;
            }
            let columns = trimmed.split_whitespace().collect::<Vec<_>>();
            if columns.len() < 4 {
                return None;
            }
            let line_number = columns[1].parse().ok()?;
            let file_number = columns[3].parse().ok()?;
            Some((line_number, file_number))
        })
        .collect()
}

fn source_line_number(source: &str, needle: &str) -> u64 {
    source
        .lines()
        .position(|line| line.contains(needle))
        .map(|index| index as u64 + 1)
        .unwrap_or_else(|| panic!("source should contain {needle}"))
}

fn dwarfdump_has_subprogram_decl_line(dump: &str, name: &str, decl_line: u64) -> bool {
    let name_needle = format!("(\"{name}\")");
    let line_needle = format!("DW_AT_decl_line\t({decl_line})");
    dump.split("DW_TAG_subprogram")
        .any(|block| block.contains(&name_needle) && block.contains(&line_needle))
}

fn dwarfdump_has_named_debug_entry(dump: &str, tag: &str, name: &str) -> bool {
    dwarfdump_named_debug_entry_contains(dump, tag, name, &[])
}

fn dwarfdump_named_debug_entry_contains(
    dump: &str,
    tag: &str,
    name: &str,
    required: &[&str],
) -> bool {
    let name_needle = format!("(\"{name}\")");
    let mut current = String::new();
    let mut matches_tag = false;

    for line in dump.lines() {
        if line.contains("DW_TAG_") {
            if matches_tag
                && current.contains("DW_AT_name")
                && current.contains(&name_needle)
                && required.iter().all(|needle| current.contains(needle))
            {
                return true;
            }
            current.clear();
            matches_tag = line.contains(tag);
        }
        if matches_tag {
            current.push_str(line);
            current.push('\n');
        }
    }

    matches_tag
        && current.contains("DW_AT_name")
        && current.contains(&name_needle)
        && required.iter().all(|needle| current.contains(needle))
}

#[test]
fn debug_info_line_table_survives_object_compilation() {
    let Some(clang) = find_clang() else {
        eprintln!("skipping debug-info line table test: clang not found");
        return;
    };
    let Some(dwarfdump) = find_llvm_dwarfdump() else {
        eprintln!("skipping debug-info line table test: llvm-dwarfdump not found");
        return;
    };

    let target = dwarf_debug_test_target();
    let source = "def helper() -> i64 { 1 }\n\ndef main() -> i64 { helper() }\n";
    let llvm_path = temp_artifact("debug-info-line-table", "ll");
    let object_path = temp_artifact("debug-info-line-table", target.object_extension());
    compile_source_to_llvm_file_with_phase_timings_with_mode(
        source,
        0,
        &llvm_path,
        None,
        None,
        Some(&target.triple),
        Some(DebugInfoConfig::for_source(
            "examples/debug/main.sg",
            source.to_string(),
        )),
    )
    .unwrap();

    compile_dwarf_debug_test_object(&clang, &llvm_path, &object_path);

    let output = Command::new(dwarfdump)
        .arg("--debug-line")
        .arg(&object_path)
        .output()
        .expect("llvm-dwarfdump should run");
    assert!(
        output.status.success(),
        "llvm-dwarfdump failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let dump = String::from_utf8_lossy(&output.stdout);
    assert!(
        dump.contains("main.sg"),
        "DWARF line table should reference the Sengoo source file, got:\n{dump}"
    );

    let _ = fs::remove_file(llvm_path);
    let _ = fs::remove_file(object_path);
}

#[test]
fn debug_info_preserves_statement_and_local_declaration_lines() {
    let Some(clang) = find_clang() else {
        eprintln!("skipping debug-info statement-line test: clang not found");
        return;
    };
    let Some(dwarfdump) = find_llvm_dwarfdump() else {
        eprintln!("skipping debug-info statement-line test: llvm-dwarfdump not found");
        return;
    };

    let target = dwarf_debug_test_target();
    let source = r#"struct Pair {
    left: i64,
    enabled: bool,
}

def debug_probe(value: i64) -> i64 {
    let doubled = value * 2;
    let pair = Pair { left: value, enabled: true };
    let tuple_value = (doubled, pair.enabled);
    let stepped = tuple_value.0 + 1;
    stepped
}

def main() -> i64 {
    debug_probe(21)
}
"#;
    let llvm_path = temp_artifact("debug-info-statements", "ll");
    let object_path = temp_artifact("debug-info-statements", target.object_extension());
    compile_source_to_llvm_file_with_phase_timings_with_mode(
        source,
        0,
        &llvm_path,
        None,
        None,
        Some(&target.triple),
        Some(DebugInfoConfig::for_source(
            "examples/debug/debugger_probe.sg",
            source,
        )),
    )
    .unwrap();

    compile_dwarf_debug_test_object(&clang, &llvm_path, &object_path);

    let line_output = Command::new(dwarfdump)
        .arg("--debug-line")
        .arg(&object_path)
        .output()
        .expect("llvm-dwarfdump --debug-line should run");
    assert!(
        line_output.status.success(),
        "llvm-dwarfdump --debug-line failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&line_output.stdout),
        String::from_utf8_lossy(&line_output.stderr)
    );
    let line_dump = String::from_utf8_lossy(&line_output.stdout);
    let line_rows = parse_dwarf_line_rows(&line_dump)
        .into_iter()
        .filter(|(_, file_number)| *file_number == 1)
        .map(|(line, _)| line)
        .collect::<HashSet<_>>();
    let expected_lines = [
        source_line_number(source, "let doubled ="),
        source_line_number(source, "let pair ="),
        source_line_number(source, "let tuple_value ="),
        source_line_number(source, "let stepped ="),
        source_line_number(source, "    stepped"),
    ];
    let missing = expected_lines
        .iter()
        .filter(|line| !line_rows.contains(line))
        .copied()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "DWARF line table should retain every debug_probe statement line, missing {missing:?}:\n{line_dump}"
    );

    let info_output = Command::new(dwarfdump)
        .arg("--debug-info")
        .arg(&object_path)
        .output()
        .expect("llvm-dwarfdump --debug-info should run");
    assert!(
        info_output.status.success(),
        "llvm-dwarfdump --debug-info failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&info_output.stdout),
        String::from_utf8_lossy(&info_output.stderr)
    );
    let info_dump = String::from_utf8_lossy(&info_output.stdout);
    for (name, needle) in [
        ("doubled", "let doubled ="),
        ("pair", "let pair ="),
        ("tuple_value", "let tuple_value ="),
        ("stepped", "let stepped ="),
    ] {
        let line = source_line_number(source, needle);
        assert!(
            dwarfdump_named_debug_entry_contains(
                &info_dump,
                "DW_TAG_variable",
                name,
                &[&format!("DW_AT_decl_line\t({line})"), "DW_AT_location"],
            ),
            "DWARF local `{name}` should retain declaration line {line} and a location:\n{info_dump}"
        );
    }

    let _ = fs::remove_file(llvm_path);
    let _ = fs::remove_file(object_path);
}

#[test]
fn debug_info_emits_parameter_and_local_variable_dies() {
    let Some(clang) = find_clang() else {
        eprintln!("skipping debug-info variable test: clang not found");
        return;
    };
    let Some(dwarfdump) = find_llvm_dwarfdump() else {
        eprintln!("skipping debug-info variable test: llvm-dwarfdump not found");
        return;
    };

    let target = dwarf_debug_test_target();
    let source = r#"
def helper(value: i64) -> i64 {
    let doubled = value * 2;
    doubled
}

def main() -> i64 {
    helper(21)
}
"#;
    let llvm_path = temp_artifact("debug-info-vars", "ll");
    let object_path = temp_artifact("debug-info-vars", target.object_extension());
    compile_source_to_llvm_file_with_phase_timings_with_mode(
        source,
        0,
        &llvm_path,
        None,
        None,
        Some(&target.triple),
        Some(DebugInfoConfig::for_source(
            "examples/debug/vars.sg",
            source.to_string(),
        )),
    )
    .unwrap();

    compile_dwarf_debug_test_object(&clang, &llvm_path, &object_path);

    let output = Command::new(dwarfdump)
        .arg("--debug-info")
        .arg(&object_path)
        .output()
        .expect("llvm-dwarfdump --debug-info should run");
    assert!(
        output.status.success(),
        "llvm-dwarfdump --debug-info failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let dump = String::from_utf8_lossy(&output.stdout);
    assert!(
        dwarfdump_has_named_debug_entry(&dump, "DW_TAG_formal_parameter", "value"),
        "DWARF should expose helper(value: i64) as a named formal parameter:\n{dump}"
    );
    assert!(
        dwarfdump_has_named_debug_entry(&dump, "DW_TAG_variable", "doubled"),
        "DWARF should expose `let doubled` as a named local variable:\n{dump}"
    );
    assert!(
        dump.contains("DW_TAG_base_type") && dump.contains("(\"i64\")"),
        "DWARF should include the i64 base type used by the parameter/local:\n{dump}"
    );

    let _ = fs::remove_file(llvm_path);
    let _ = fs::remove_file(object_path);
}

#[test]
fn debug_info_emits_struct_member_names_types_and_offsets() {
    let Some(clang) = find_clang() else {
        eprintln!("skipping debug-info struct test: clang not found");
        return;
    };
    let Some(dwarfdump) = find_llvm_dwarfdump() else {
        eprintln!("skipping debug-info struct test: llvm-dwarfdump not found");
        return;
    };

    let target = dwarf_debug_test_target();
    let source = r#"
struct Pair {
    left: i64,
    enabled: bool,
}

def inspect_pair() -> i64 {
    let pair = Pair { left: 21, enabled: true };
    if pair.enabled { pair.left } else { 0 }
}

def main() -> i64 {
    inspect_pair()
}
"#;
    let llvm_path = temp_artifact("debug-info-struct", "ll");
    let object_path = temp_artifact("debug-info-struct", target.object_extension());
    compile_source_to_llvm_file_with_phase_timings_with_mode(
        source,
        0,
        &llvm_path,
        None,
        None,
        Some(&target.triple),
        Some(DebugInfoConfig::for_source(
            "examples/debug/struct.sg",
            source.to_string(),
        )),
    )
    .unwrap();

    compile_dwarf_debug_test_object(&clang, &llvm_path, &object_path);
    let output = Command::new(dwarfdump)
        .arg("--debug-info")
        .arg(&object_path)
        .output()
        .expect("llvm-dwarfdump --debug-info should run");
    assert!(
        output.status.success(),
        "llvm-dwarfdump --debug-info failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let dump = String::from_utf8_lossy(&output.stdout);
    assert!(
        dump.contains("DW_TAG_structure_type") && dump.contains("(\"Pair\")"),
        "DWARF should expose the Pair composite type:\n{dump}"
    );
    assert!(
        dwarfdump_has_named_debug_entry(&dump, "DW_TAG_member", "left"),
        "DWARF should expose Pair.left as a member:\n{dump}"
    );
    assert!(
        dwarfdump_has_named_debug_entry(&dump, "DW_TAG_member", "enabled"),
        "DWARF should expose Pair.enabled as a member:\n{dump}"
    );
    assert!(
        dump.contains("(\"i64\")") && dump.contains("(\"bool\")"),
        "DWARF should retain Pair member base types:\n{dump}"
    );

    let _ = fs::remove_file(llvm_path);
    let _ = fs::remove_file(object_path);
}

#[test]
fn debug_info_emits_enum_tuple_string_and_vec_composite_layouts() {
    let Some(clang) = find_clang() else {
        eprintln!("skipping debug-info composite test: clang not found");
        return;
    };
    let Some(dwarfdump) = find_llvm_dwarfdump() else {
        eprintln!("skipping debug-info composite test: llvm-dwarfdump not found");
        return;
    };

    let target = dwarf_debug_test_target();
    let stdlib = load_stdlib_surface_source();
    let program = r#"
enum Choice { Empty, Value(i64) }

def inspect_composites() -> i64 {
    let tuple_value = (21, true);
    let text = string_new();
    let values = vec_new_i64();
    let picked = Choice::Value(7);
    if tuple_value.1 { tuple_value.0 + text.len() + values.len() } else { 0 }
}

def main() -> i64 {
    inspect_composites()
}
"#;
    let source = format!("{stdlib}\n\n{program}");
    let llvm_path = temp_artifact("debug-info-composites", "ll");
    let object_path = temp_artifact("debug-info-composites", target.object_extension());
    compile_source_to_llvm_file_with_phase_timings_with_mode(
        &source,
        0,
        &llvm_path,
        None,
        None,
        Some(&target.triple),
        Some(DebugInfoConfig::for_source(
            "examples/debug/composites.sg",
            source.clone(),
        )),
    )
    .unwrap();

    compile_dwarf_debug_test_object(&clang, &llvm_path, &object_path);
    let output = Command::new(dwarfdump)
        .arg("--debug-info")
        .arg(&object_path)
        .output()
        .expect("llvm-dwarfdump --debug-info should run");
    assert!(
        output.status.success(),
        "llvm-dwarfdump --debug-info failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let dump = String::from_utf8_lossy(&output.stdout);

    for (local, debug_type) in [
        ("tuple_value", "tuple"),
        ("text", "String"),
        ("values", "Vec_i64"),
        ("picked", "enum"),
    ] {
        assert!(
            dwarfdump_named_debug_entry_contains(
                &dump,
                "DW_TAG_variable",
                local,
                &[debug_type, "DW_AT_location"]
            ),
            "DWARF should retain the location and type of local `{local}` as `{debug_type}`:\n{dump}"
        );
    }

    for composite in ["tuple", "String", "Vec_i64", "enum"] {
        assert!(
            dwarfdump_has_named_debug_entry(&dump, "DW_TAG_structure_type", composite),
            "DWARF should expose the `{composite}` composite type:\n{dump}"
        );
    }
    for member in ["0", "1", "handle", "marker", "discriminant", "payload"] {
        assert!(
            dwarfdump_has_named_debug_entry(&dump, "DW_TAG_member", member),
            "DWARF should expose composite member `{member}`:\n{dump}"
        );
    }
    assert!(
        dwarfdump_named_debug_entry_contains(
            &dump,
            "DW_TAG_member",
            "1",
            &["bool", "DW_AT_data_member_location\t(0x08)"]
        ),
        "tuple field 1 should retain its bool type at byte offset 8:\n{dump}"
    );
    assert!(
        dwarfdump_named_debug_entry_contains(
            &dump,
            "DW_TAG_base_type",
            "bool",
            &["DW_AT_byte_size\t(0x01)"]
        ),
        "bool debug metadata should describe its one-byte storage size:\n{dump}"
    );
    assert!(
        dwarfdump_named_debug_entry_contains(
            &dump,
            "DW_TAG_member",
            "handle",
            &["i64", "DW_AT_data_member_location\t(0x00)"]
        ),
        "String/Vec handles should retain their i64 type at byte offset 0:\n{dump}"
    );
    assert!(
        dwarfdump_named_debug_entry_contains(
            &dump,
            "DW_TAG_member",
            "discriminant",
            &["i64", "DW_AT_data_member_location\t(0x00)"]
        ),
        "enum discriminant should be an i64 member at byte offset 0:\n{dump}"
    );
    assert!(
        dwarfdump_named_debug_entry_contains(
            &dump,
            "DW_TAG_member",
            "payload",
            &["DW_AT_data_member_location\t(0x08)"]
        ),
        "enum payload storage should begin at byte offset 8:\n{dump}"
    );
    assert!(
        dump.contains("DW_TAG_array_type")
            && dump.contains("DW_TAG_subrange_type")
            && dump.contains("(\"u8\")"),
        "enum payload bytes should retain u8 array/subrange metadata:\n{dump}"
    );
    assert!(
        dwarfdump_named_debug_entry_contains(
            &dump,
            "DW_TAG_member",
            "marker",
            &["i64", "DW_AT_data_member_location\t(0x08)"]
        ),
        "Vec<i64>.marker should retain its i64 type at byte offset 8:\n{dump}"
    );

    let _ = fs::remove_file(llvm_path);
    let _ = fs::remove_file(object_path);
}

#[test]
fn debug_info_tracks_multi_surface_function_entry_lines() {
    let Some(clang) = find_clang() else {
        eprintln!("skipping debug-info surface test: clang not found");
        return;
    };
    let Some(dwarfdump) = find_llvm_dwarfdump() else {
        eprintln!("skipping debug-info surface test: llvm-dwarfdump not found");
        return;
    };

    let target = dwarf_debug_test_target();
    let stdlib = load_stdlib_surface_source();
    let program = r#"struct Pair {
    left: i64,
    right: i64,
}

enum Choice { Empty, Value(i64) }

def scalar_surface() -> i64 {
    let base = 2;
    let scaled = base * 3;
    scaled + 1
}

def struct_surface() -> i64 {
    let pair = Pair { left: 4, right: 5 };
    pair.left + pair.right
}

def enum_surface() -> i64 {
    let picked = Choice::Value(7);
    match picked {
        Choice::Empty => 0,
        Choice::Value(value) => value,
    }
}

def string_surface() -> i64 {
    let greeting = string_from_str("hi");
    if greeting.is_ok == false {
        return 0;
    }
    let owned = greeting.value;
    let appended = owned.push_str("!");
    if appended.is_ok == false {
        return 0;
    }
    owned.len()
}

def vec_surface() -> i64 {
    let values = vec_new_i64();
    values.push(1);
    values.push(2);
    let total = values.get(0).unwrap_or(0) + values.get(1).unwrap_or(0);
    values.free();
    total
}

def call_surface(value: i64) -> i64 {
    scalar_surface() + value
}

def closure_surface() -> i64 {
    let add = |x| call_surface(x);
    add(3)
}

def main() -> i64 {
    struct_surface()
        + enum_surface()
        + string_surface()
        + vec_surface()
        + closure_surface()
}
"#;
    let source = format!("{stdlib}\n\n{program}");
    let llvm_path = temp_artifact("debug-info-surfaces", "ll");
    let object_path = temp_artifact("debug-info-surfaces", target.object_extension());
    compile_source_to_llvm_file_with_phase_timings_with_mode(
        &source,
        0,
        &llvm_path,
        None,
        None,
        Some(&target.triple),
        Some(DebugInfoConfig::for_source(
            "examples/debug/main.sg",
            source.clone(),
        )),
    )
    .unwrap();

    compile_dwarf_debug_test_object(&clang, &llvm_path, &object_path);

    let line_output = Command::new(dwarfdump)
        .arg("--debug-line")
        .arg(&object_path)
        .output()
        .expect("llvm-dwarfdump --debug-line should run");
    assert!(
        line_output.status.success(),
        "llvm-dwarfdump --debug-line failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&line_output.stdout),
        String::from_utf8_lossy(&line_output.stderr)
    );
    let line_dump = String::from_utf8_lossy(&line_output.stdout);
    let surfaced_lines = parse_dwarf_line_rows(&line_dump)
        .into_iter()
        .filter(|(_, file_number)| *file_number == 1)
        .map(|(line_number, _)| line_number)
        .collect::<HashSet<_>>();
    let expected_functions = [
        (
            "scalar_surface",
            source_line_number(&source, "def scalar_surface() -> i64 {"),
        ),
        (
            "struct_surface",
            source_line_number(&source, "def struct_surface() -> i64 {"),
        ),
        (
            "enum_surface",
            source_line_number(&source, "def enum_surface() -> i64 {"),
        ),
        (
            "string_surface",
            source_line_number(&source, "def string_surface() -> i64 {"),
        ),
        (
            "vec_surface",
            source_line_number(&source, "def vec_surface() -> i64 {"),
        ),
        (
            "call_surface",
            source_line_number(&source, "def call_surface(value: i64) -> i64 {"),
        ),
        (
            "closure_surface",
            source_line_number(&source, "def closure_surface() -> i64 {"),
        ),
        ("main", source_line_number(&source, "def main() -> i64 {")),
    ];
    let missing_line_rows = expected_functions
        .iter()
        .filter_map(|(name, line_number)| {
            (!surfaced_lines.contains(line_number)).then_some(format!("{name}@{line_number}"))
        })
        .collect::<Vec<_>>();
    assert!(
        missing_line_rows.is_empty(),
        "DWARF line table should preserve entry lines for scalar/struct/enum/string/Vec/call/closure surfaces, missing {missing_line_rows:?}\n{line_dump}"
    );

    let debug_info_output = Command::new(dwarfdump)
        .arg("--debug-info")
        .arg(&object_path)
        .output()
        .expect("llvm-dwarfdump --debug-info should run");
    assert!(
        debug_info_output.status.success(),
        "llvm-dwarfdump --debug-info failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&debug_info_output.stdout),
        String::from_utf8_lossy(&debug_info_output.stderr)
    );
    let debug_info_dump = String::from_utf8_lossy(&debug_info_output.stdout);
    let missing_decl_lines = expected_functions
        .iter()
        .filter_map(|(name, line_number)| {
            (!dwarfdump_has_subprogram_decl_line(&debug_info_dump, name, *line_number))
                .then_some(format!("{name}@{line_number}"))
        })
        .collect::<Vec<_>>();
    assert!(
        missing_decl_lines.is_empty(),
        "DWARF subprogram DIEs should preserve decl lines for surface probes, missing {missing_decl_lines:?}\n{debug_info_dump}"
    );

    let _ = fs::remove_file(llvm_path);
    let _ = fs::remove_file(object_path);
}

#[test]
fn compile_source_prunes_unreachable_functions_from_ir() {
    let source = r#"
def live() -> i64 { 1 }
def unused_xyz_dead() -> i64 { 42 }
def main() -> i64 { live() }
"#;
    let llvm_ir = compile_source(source, 2).unwrap();
    assert!(llvm_ir.contains("live"));
    assert!(llvm_ir.contains("main"));
    assert!(
        !llvm_ir.contains("unused_xyz_dead"),
        "unreachable function should be pruned from LLVM IR"
    );
}

#[test]
fn compile_source_without_main_keeps_functions() {
    let source = r#"
def keep_alpha() -> i64 { 1 }
def keep_beta() -> i64 { keep_alpha() + 1 }
"#;
    let llvm_ir = compile_source(source, 2).unwrap();
    assert!(llvm_ir.contains("keep_alpha"));
    assert!(llvm_ir.contains("keep_beta"));
}

#[test]
fn edit_classifier_detects_noop_for_comment_only_change() {
    let before = "def main() -> i64 {\n    0\n}\n";
    let after = "def main() -> i64 {\n    0\n}\n// comment-only change\n";
    let impact = classify_root_edit(before, after);
    assert_eq!(
        impact.class,
        EditClass::Noop,
        "{}",
        edit_class_label(impact.class)
    );
}

#[test]
fn edit_classifier_detects_impl_only_for_loop_body_change() {
    let before = r#"
def main() -> i64 {
    let i = 0
    let acc = 0
    while i < 10 {
        acc = acc + i
        i = i + 1
    }
    acc
}
"#;
    let after = r#"
def main() -> i64 {
    let i = 0
    let acc = 0
    while i < 10 {
        acc = acc + i + 1
        i = i + 1
    }
    acc
}
"#;
    let impact = classify_root_edit(before, after);
    assert_eq!(
        impact.class,
        EditClass::ImplOnly,
        "{}",
        edit_class_label(impact.class)
    );
}

#[test]
fn edit_classifier_detects_interface_change_for_signature_change() {
    let before = "def add(x: i64) -> i64 { x + 1 }\ndef main() -> i64 { add(1) }\n";
    let after = "def add(x: i64, k: i64) -> i64 { x + k }\ndef main() -> i64 { add(1, 1) }\n";
    let impact = classify_root_edit(before, after);
    assert_eq!(impact.class, EditClass::InterfaceChange);
}

#[test]
fn edit_classifier_detects_interface_change_for_add_new_function() {
    let before = "def main() -> i64 { 0 }\n";
    let after = "def extra(x: i64) -> i64 { x + 1 }\ndef main() -> i64 { extra(0) }\n";
    let impact = classify_root_edit(before, after);
    assert_eq!(impact.class, EditClass::InterfaceChange);
}

#[test]
fn interface_change_propagates_to_dependents() {
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![
            BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 7,
                implementation_hash: 9,
                depends_on: vec!["tests/dep.sg".to_string()],
                object_path: None,
                functions: vec![],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
            BuildGraphNodeV2 {
                module_path: "tests/dep.sg".to_string(),
                interface_hash: 1,
                implementation_hash: 11,
                depends_on: vec![],
                object_path: None,
                functions: vec![],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
        ],
    };
    let before = vec![fp("tests/dep.sg", 1, 11)];
    let after = vec![fp("tests/dep.sg", 2, 11)];
    let impact = classify_edit_impact(7, 9, 7, 9, &before, &after, Some(&graph), &graph);
    assert_eq!(impact.class, EditClass::InterfaceChange);
    assert!(impact
        .impacted_modules
        .contains(&"tests/dep.sg".to_string()));
    assert!(impact
        .impacted_modules
        .contains(&"tests/main.sg".to_string()));
}

#[test]
fn impl_only_change_does_not_propagate_to_dependents() {
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![
            BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 7,
                implementation_hash: 9,
                depends_on: vec!["tests/dep.sg".to_string()],
                object_path: None,
                functions: vec![],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
            BuildGraphNodeV2 {
                module_path: "tests/dep.sg".to_string(),
                interface_hash: 1,
                implementation_hash: 11,
                depends_on: vec![],
                object_path: None,
                functions: vec![],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
        ],
    };
    let before = vec![fp("tests/dep.sg", 1, 11)];
    let after = vec![fp("tests/dep.sg", 1, 12)];
    let impact = classify_edit_impact(7, 9, 7, 9, &before, &after, Some(&graph), &graph);
    assert_eq!(impact.class, EditClass::ImplOnly);
    assert_eq!(impact.impacted_modules, vec!["tests/dep.sg".to_string()]);
}

#[test]
fn function_level_evidence_downgrades_false_module_interface_change() {
    let previous_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![
            BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 7,
                implementation_hash: 9,
                depends_on: vec!["tests/dep.sg".to_string()],
                object_path: None,
                functions: vec![FunctionFingerprint {
                    symbol: "tests/main.sg::main".to_string(),
                    abi_hash: 1,
                    body_hash: 10,
                    calls: vec![],
                    module_imports: vec![],
                }],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
            BuildGraphNodeV2 {
                module_path: "tests/dep.sg".to_string(),
                interface_hash: 1,
                implementation_hash: 11,
                depends_on: vec![],
                object_path: None,
                functions: vec![FunctionFingerprint {
                    symbol: "tests/dep.sg::helper".to_string(),
                    abi_hash: 2,
                    body_hash: 20,
                    calls: vec![],
                    module_imports: vec![],
                }],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
        ],
    };

    let current_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![
            BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 7,
                implementation_hash: 9,
                depends_on: vec!["tests/dep.sg".to_string()],
                object_path: None,
                functions: vec![FunctionFingerprint {
                    symbol: "tests/main.sg::main".to_string(),
                    abi_hash: 1,
                    body_hash: 10,
                    calls: vec![],
                    module_imports: vec![],
                }],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
            BuildGraphNodeV2 {
                module_path: "tests/dep.sg".to_string(),
                interface_hash: 2,
                implementation_hash: 12,
                depends_on: vec![],
                object_path: None,
                functions: vec![FunctionFingerprint {
                    symbol: "tests/dep.sg::helper".to_string(),
                    abi_hash: 2,
                    body_hash: 21,
                    calls: vec![],
                    module_imports: vec![],
                }],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
        ],
    };

    // Simulate coarse module hash drift that flags dep as interface-changed,
    // while function ABI evidence indicates body-only change.
    let before = vec![fp("tests/dep.sg", 1, 11)];
    let after = vec![fp("tests/dep.sg", 2, 12)];

    let impact = classify_edit_impact(
        7,
        9,
        7,
        9,
        &before,
        &after,
        Some(&previous_graph),
        &current_graph,
    );
    assert_eq!(impact.class, EditClass::ImplOnly);
    assert!(impact.changed_modules.contains(&"tests/dep.sg".to_string()));
    assert!(!impact
        .impacted_modules
        .contains(&"tests/main.sg".to_string()));
}

#[test]
fn missing_previous_function_state_does_not_force_interface_change() {
    let previous_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 7,
            implementation_hash: 9,
            depends_on: vec![],
            object_path: None,
            functions: vec![],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };
    let current_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 7,
            implementation_hash: 10,
            depends_on: vec![],
            object_path: None,
            functions: vec![FunctionFingerprint {
                symbol: "tests/main.sg::main".to_string(),
                abi_hash: 11,
                body_hash: 101,
                calls: vec![],
                module_imports: vec![],
            }],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };

    let impact = classify_edit_impact(7, 9, 7, 10, &[], &[], Some(&previous_graph), &current_graph);
    assert_eq!(impact.class, EditClass::ImplOnly);
    assert!(impact.changed_functions.is_empty());
    assert!(impact.impacted_functions.is_empty());
}

#[test]
fn function_state_drift_without_module_interface_change_stays_impl_only() {
    let previous_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 7,
            implementation_hash: 9,
            depends_on: vec![],
            object_path: None,
            functions: vec![
                FunctionFingerprint {
                    symbol: "tests/main.sg::main".to_string(),
                    abi_hash: 11,
                    body_hash: 101,
                    calls: vec![],
                    module_imports: vec![],
                },
                FunctionFingerprint {
                    symbol: "tests/main.sg::helper".to_string(),
                    abi_hash: 12,
                    body_hash: 102,
                    calls: vec![],
                    module_imports: vec![],
                },
            ],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };
    let current_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 7,
            implementation_hash: 10,
            depends_on: vec![],
            object_path: None,
            // Simulate symbol-collection drift: helper missing in current snapshot.
            functions: vec![FunctionFingerprint {
                symbol: "tests/main.sg::main".to_string(),
                abi_hash: 11,
                body_hash: 103,
                calls: vec![],
                module_imports: vec![],
            }],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };

    let impact = classify_edit_impact(7, 9, 7, 10, &[], &[], Some(&previous_graph), &current_graph);
    assert_eq!(impact.class, EditClass::ImplOnly);
}

#[test]
fn format_edit_impact_lines_truncates_large_symbol_lists() {
    let symbols = (0..80)
        .map(|i| format!("tests/main.sg::f{}", i))
        .collect::<Vec<_>>();
    let impact = EditImpact {
        class: EditClass::InterfaceChange,
        changed_modules: vec!["tests/main.sg".to_string()],
        impacted_modules: vec!["tests/main.sg".to_string()],
        changed_functions: symbols.clone(),
        impacted_functions: symbols,
    };

    let lines = format_edit_impact_lines(&impact);
    assert!(lines
        .iter()
        .any(|line| line.starts_with("changed functions: ") && line.contains("(truncated)")));
    assert!(lines
        .iter()
        .any(|line| line == "changed functions total: 80"));
    assert!(lines
        .iter()
        .any(|line| line.starts_with("impacted functions: ") && line.contains("(truncated)")));
    assert!(lines
        .iter()
        .any(|line| line == "impacted functions total: 80"));
}

#[test]
fn dependency_levels_follow_topological_order() {
    let mut edges = BTreeMap::new();
    edges.insert("main".to_string(), vec!["a".to_string(), "b".to_string()]);
    edges.insert("a".to_string(), vec!["c".to_string()]);
    edges.insert("b".to_string(), vec!["d".to_string()]);
    edges.insert("c".to_string(), Vec::new());
    edges.insert("d".to_string(), Vec::new());

    let levels = module_dependency_levels(&edges);
    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0], vec!["c".to_string(), "d".to_string()]);
    assert_eq!(levels[1], vec!["a".to_string(), "b".to_string()]);
    assert_eq!(levels[2], vec!["main".to_string()]);
}

#[test]
fn dependency_levels_keep_cycle_output_deterministic() {
    let mut edges = BTreeMap::new();
    edges.insert("a".to_string(), vec!["b".to_string()]);
    edges.insert("b".to_string(), vec!["a".to_string()]);
    edges.insert("c".to_string(), Vec::new());

    let levels = module_dependency_levels(&edges);
    assert_eq!(levels.len(), 2);
    assert_eq!(levels[0], vec!["c".to_string()]);
    assert_eq!(levels[1], vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn dependency_fingerprints_are_deterministic_with_parallel_collection() {
    let root_dir = std::env::temp_dir().join(format!("sengoo-mod-fp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root_dir);
    fs::create_dir_all(&root_dir).unwrap();

    let main_path = root_dir.join("main.sg");
    let dep_a = root_dir.join("dep_a.sg");
    let dep_b = root_dir.join("dep_b.sg");

    fs::write(
        &main_path,
        "import dep_b;\nimport dep_a;\ndef main() -> i64 {\n    0\n}\n",
    )
    .unwrap();
    fs::write(&dep_a, "def a() -> i64 {\n    1\n}\n").unwrap();
    fs::write(&dep_b, "def b() -> i64 {\n    2\n}\n").unwrap();

    let source = fs::read_to_string(&main_path).unwrap();
    let first = module_fingerprints_for_source(&main_path, &source);
    for _ in 0..5 {
        let current = module_fingerprints_for_source(&main_path, &source);
        assert_eq!(current, first);
    }

    let _ = fs::remove_dir_all(&root_dir);
}

#[test]
fn incremental_fixture_retains_dependency_fingerprint_on_impl_change() {
    let root = bench_root_dir();
    let case = root.join("suites/incremental/change_impl_root.sg");
    let original = fs::read_to_string(&case).unwrap();
    let before = module_fingerprints_for_source(&case, &original);

    let mut mutated = original.clone();
    mutated.push_str("\n// test-mut\n");
    let after = module_fingerprints_for_source(&case, &mutated);
    let stats = module_invalidation_stats(&before, &after);
    assert!(
        stats.reused_modules >= 1,
        "expected at least one reused dependency module"
    );
}

#[test]
fn impl_only_change_does_not_rebuild_all_modules() {
    let before = vec![
        fp("tests/mod_a.sg", 100, 1000),
        fp("tests/mod_b.sg", 200, 2000),
    ];
    let after = vec![
        // implementation changed, interface unchanged
        fp("tests/mod_a.sg", 100, 1999),
        // unchanged module should be reused
        fp("tests/mod_b.sg", 200, 2000),
    ];

    let stats = module_invalidation_stats(&before, &after);
    assert_eq!(stats.total_modules, 2);
    assert_eq!(stats.implementation_only_changed_modules, 1);
    assert_eq!(stats.reused_modules, 1);
    assert!(
        stats.rebuilt_modules < stats.total_modules,
        "impl-only change should not force all modules to rebuild"
    );
}

#[test]
fn function_fingerprint_comment_only_change_keeps_hashes() {
    let before = r#"
def add(x: i64) -> i64 {
    x + 1
}
"#;
    let after = r#"
def add(x: i64) -> i64 {
    // comment-only change
    x + 1
}
"#;

    let before_fp = super::function_fingerprints_for_module("tests/main.sg", before);
    let after_fp = super::function_fingerprints_for_module("tests/main.sg", after);

    assert_eq!(before_fp.len(), 1);
    assert_eq!(after_fp.len(), 1);
    assert_eq!(before_fp[0].abi_hash, after_fp[0].abi_hash);
    assert_eq!(before_fp[0].body_hash, after_fp[0].body_hash);
}

#[test]
fn interface_fingerprint_fast_ignores_inline_function_body_changes() {
    let before = "def add(x: i64) -> i64 { x + 1 }\n";
    let after = "def add(x: i64) -> i64 { x + 2 }\n";

    let before_interface = super::interface_fingerprint_fast(before);
    let after_interface = super::interface_fingerprint_fast(after);
    let before_impl = super::implementation_fingerprint(before);
    let after_impl = super::implementation_fingerprint(after);

    assert_eq!(
        before_interface, after_interface,
        "fast interface hash should ignore inline body-only edits"
    );
    assert_ne!(
        before_impl, after_impl,
        "implementation hash should still capture body changes"
    );
}

#[test]
fn interface_fingerprint_fast_detects_inline_function_signature_changes() {
    let before = "def add(x: i64) -> i64 { x + 1 }\n";
    let after = "def add(x: i64, k: i64) -> i64 { x + k }\n";

    let before_interface = super::interface_fingerprint_fast(before);
    let after_interface = super::interface_fingerprint_fast(after);

    assert_ne!(
        before_interface, after_interface,
        "fast interface hash should track inline declaration signature changes"
    );
}

#[test]
fn fast_path_inline_body_change_classifies_as_impl_only() {
    let before = "def add(x: i64) -> i64 { x + 1 }\n";
    let after = "def add(x: i64) -> i64 { x + 2 }\n";

    let before_interface = super::interface_fingerprint_fast(before);
    let before_impl = super::implementation_fingerprint(before);
    let after_interface = super::interface_fingerprint_fast(after);
    let after_impl = super::implementation_fingerprint(after);

    let previous_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: before_interface,
            implementation_hash: before_impl,
            depends_on: vec![],
            object_path: None,
            functions: vec![],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };

    let current_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: after_interface,
            implementation_hash: after_impl,
            depends_on: vec![],
            object_path: None,
            functions: vec![],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };

    let before_modules = vec![fp("tests/main.sg", before_interface, before_impl)];
    let after_modules = vec![fp("tests/main.sg", after_interface, after_impl)];
    let impact = classify_edit_impact(
        before_interface,
        before_impl,
        after_interface,
        after_impl,
        &before_modules,
        &after_modules,
        Some(&previous_graph),
        &current_graph,
    );

    assert_eq!(impact.class, EditClass::ImplOnly);
}

#[test]
fn function_fingerprint_signature_change_updates_abi_hash() {
    let before = "def add(x: i64) -> i64 { x + 1 }\n";
    let after = "def add(x: i64, k: i64) -> i64 { x + k }\n";

    let before_fp = super::function_fingerprints_for_module("tests/main.sg", before);
    let after_fp = super::function_fingerprints_for_module("tests/main.sg", after);

    assert_eq!(before_fp.len(), 1);
    assert_eq!(after_fp.len(), 1);
    assert_ne!(before_fp[0].abi_hash, after_fp[0].abi_hash);
}

#[test]
fn function_fingerprint_body_change_is_localized_to_target_function() {
    let before = r#"
def a() -> i64 { 1 }
def b() -> i64 { 2 }
def main() -> i64 { a() + b() }
"#;
    let after = r#"
def a() -> i64 { 3 }
def b() -> i64 { 2 }
def main() -> i64 { a() + b() }
"#;

    let before_fp = super::function_fingerprints_for_module("tests/main.sg", before);
    let after_fp = super::function_fingerprints_for_module("tests/main.sg", after);
    assert_eq!(before_fp.len(), 3);
    assert_eq!(after_fp.len(), 3);

    let before_a = before_fp
        .iter()
        .find(|fp| fp.symbol.ends_with("::a"))
        .expect("before a");
    let before_b = before_fp
        .iter()
        .find(|fp| fp.symbol.ends_with("::b"))
        .expect("before b");
    let before_main = before_fp
        .iter()
        .find(|fp| fp.symbol.ends_with("::main"))
        .expect("before main");
    let after_a = after_fp
        .iter()
        .find(|fp| fp.symbol.ends_with("::a"))
        .expect("after a");
    let after_b = after_fp
        .iter()
        .find(|fp| fp.symbol.ends_with("::b"))
        .expect("after b");
    let after_main = after_fp
        .iter()
        .find(|fp| fp.symbol.ends_with("::main"))
        .expect("after main");

    assert_eq!(before_a.abi_hash, after_a.abi_hash);
    assert_ne!(before_a.body_hash, after_a.body_hash);
    assert_eq!(before_b.abi_hash, after_b.abi_hash);
    assert_eq!(before_b.body_hash, after_b.body_hash);
    assert_eq!(before_main.abi_hash, after_main.abi_hash);
    assert_eq!(before_main.body_hash, after_main.body_hash);
}

#[test]
fn generic_fingerprints_collect_generic_item_and_instance() {
    let source = r#"
def generic_add<T>(x: i64) -> i64 {
    x + 1
}

def main() -> i64 {
    generic_add(1)
}
"#;

    let (items, instances) = generic_fingerprints_for_module("tests/main.sg", source);
    assert_eq!(items.len(), 1);
    assert_eq!(instances.len(), 1);
    assert_eq!(items[0].kind, "function");
    assert!(instances[0]
        .instance_key
        .contains("tests/main.sg::generic_add"));
}

#[test]
fn generic_fingerprint_comment_only_change_keeps_hashes() {
    let before = r#"
def generic_add<T>(x: i64) -> i64 {
    x + 1
}

def main() -> i64 {
    generic_add(1)
}
"#;
    let after = r#"
// comment changed only
def generic_add<T>(x: i64) -> i64 {
    x + 1
}

def main() -> i64 {
    generic_add(1)
}
"#;

    let (before_items, before_instances) = generic_fingerprints_for_module("tests/main.sg", before);
    let (after_items, after_instances) = generic_fingerprints_for_module("tests/main.sg", after);

    assert_eq!(before_items.len(), 1);
    assert_eq!(after_items.len(), 1);
    assert_eq!(before_instances.len(), 1);
    assert_eq!(after_instances.len(), 1);
    assert_eq!(
        before_items[0].interface_hash,
        after_items[0].interface_hash
    );
    assert_eq!(before_items[0].body_hash, after_items[0].body_hash);
    assert_eq!(
        before_instances[0].instance_key,
        after_instances[0].instance_key
    );
}

#[test]
fn generic_fingerprint_body_change_is_localized_to_target_item() {
    let before = r#"
def ga<T>(marker: T, x: i64) -> i64 { x + 1 }
def gb<T>(marker: T, x: i64) -> i64 { x + 2 }
def main() -> i64 { ga(0, 1) + gb(0, 1) }
"#;
    let after = r#"
def ga<T>(marker: T, x: i64) -> i64 { x + 3 }
def gb<T>(marker: T, x: i64) -> i64 { x + 2 }
def main() -> i64 { ga(0, 1) + gb(0, 1) }
"#;

    let (before_items, before_instances) = generic_fingerprints_for_module("tests/main.sg", before);
    let (after_items, after_instances) = generic_fingerprints_for_module("tests/main.sg", after);
    assert_eq!(before_items.len(), 2);
    assert_eq!(after_items.len(), 2);
    assert_eq!(before_instances.len(), 2);
    assert_eq!(after_instances.len(), 2);

    let before_ga = before_items
        .iter()
        .find(|item| item.stable_item_id.ends_with("::ga"))
        .expect("before ga");
    let before_gb = before_items
        .iter()
        .find(|item| item.stable_item_id.ends_with("::gb"))
        .expect("before gb");
    let after_ga = after_items
        .iter()
        .find(|item| item.stable_item_id.ends_with("::ga"))
        .expect("after ga");
    let after_gb = after_items
        .iter()
        .find(|item| item.stable_item_id.ends_with("::gb"))
        .expect("after gb");

    assert_eq!(before_ga.interface_hash, after_ga.interface_hash);
    assert_ne!(before_ga.body_hash, after_ga.body_hash);
    assert_eq!(before_gb.interface_hash, after_gb.interface_hash);
    assert_eq!(before_gb.body_hash, after_gb.body_hash);

    let before_gb_instance = before_instances
        .iter()
        .find(|inst| inst.item_stable_id.ends_with("::gb"))
        .expect("before gb instance");
    let after_gb_instance = after_instances
        .iter()
        .find(|inst| inst.item_stable_id.ends_with("::gb"))
        .expect("after gb instance");
    assert_eq!(
        before_gb_instance.instance_key,
        after_gb_instance.instance_key
    );
}

fn stdlib_impl_method_instance_source() -> String {
    format!(
        "{}\n\n{}",
        load_stdlib_surface_source(),
        r#"
def main() -> bool {
    let opt: Option<bool> = Option { is_some: true, value: true };
    let ok_result: Result<bool, bool> = opt.ok_or(false);
    let res: Result<bool, i64> = Result { is_ok: true, value: true, error: 9 };
    opt.unwrap_or(false) && ok_result.ok().unwrap_or(false) && res.unwrap_or(false)
}
"#
    )
}

fn stdlib_chained_impl_method_instance_source() -> String {
    format!(
        "{}\n\n{}",
        load_stdlib_surface_source(),
        r#"
def main() -> bool {
    let ok_result: Result<bool, i64> = Result { is_ok: true, value: true, error: 9 };
    let err_result: Result<i64, bool> = Result { is_ok: false, value: 0, error: true };
    ok_result.ok().unwrap_or(false) && err_result.err().unwrap_or(false)
}
"#
    )
}

#[test]
fn generic_fingerprints_collect_stdlib_impl_method_instances_from_typed_receivers() {
    let source = stdlib_impl_method_instance_source();
    sengoo_compiler::Parser::parse(&source).expect("stdlib impl-method source should parse");
    let (_, instances) = generic_fingerprints_for_module("tests/main.sg", &source);

    let option_unwrap = instances
        .iter()
        .find(|inst| {
            inst.item_stable_id
                .ends_with("::impl::Option<T>::unwrap_or")
        })
        .expect("expected Option<T>::unwrap_or instance");
    assert_eq!(option_unwrap.canonical_type_args, vec!["bool".to_string()]);

    let option_ok_or = instances
        .iter()
        .find(|inst| inst.item_stable_id.ends_with("::impl::Option<T>::ok_or"))
        .expect("expected Option<T>::ok_or instance");
    assert_eq!(
        option_ok_or.canonical_type_args,
        vec!["bool".to_string(), "bool".to_string()]
    );

    let result_unwrap = instances
        .iter()
        .find(|inst| {
            inst.item_stable_id
                .ends_with("::impl::Result<T,E>::unwrap_or")
        })
        .expect("expected Result<T,E>::unwrap_or instance");
    assert_eq!(
        result_unwrap.canonical_type_args,
        vec!["bool".to_string(), "i64".to_string()]
    );
}

#[test]
fn generic_fingerprints_collect_chained_stdlib_impl_method_return_instances() {
    let source = stdlib_chained_impl_method_instance_source();
    let (_, instances) = generic_fingerprints_for_module("tests/main.sg", &source);

    assert!(
        instances.iter().any(|inst| {
            inst.item_stable_id.ends_with("::impl::Result<T,E>::ok")
                && inst.canonical_type_args == vec!["bool".to_string(), "i64".to_string()]
        }),
        "expected Result<bool,i64>::ok instance"
    );
    assert!(
        instances.iter().any(|inst| {
            inst.item_stable_id.ends_with("::impl::Result<T,E>::err")
                && inst.canonical_type_args == vec!["i64".to_string(), "bool".to_string()]
        }),
        "expected Result<i64,bool>::err instance"
    );
    assert!(
        instances.iter().any(|inst| {
            inst.item_stable_id
                .ends_with("::impl::Option<T>::unwrap_or")
                && inst.canonical_type_args == vec!["bool".to_string()]
        }),
        "expected chained Option<bool>::unwrap_or instance"
    );
}

#[test]
fn generic_fingerprints_bind_impl_method_generics_from_parameter_templates() {
    let source = r#"
struct Boxed<T> {
    value: T,
}

impl<T> Boxed<T> {
    def choose_second<U>(self, fixed: i64, value: U) -> U {
        value
    }
}

def main() -> bool {
    let boxed: Boxed<i64> = Boxed { value: 7 };
    boxed.choose_second(1, false)
}
"#;

    let (_, instances) = generic_fingerprints_for_module("tests/main.sg", source);
    let choose_second = instances
        .iter()
        .find(|inst| {
            inst.item_stable_id
                .ends_with("::impl::Boxed<T>::choose_second")
        })
        .expect("expected Boxed<T>::choose_second instance");
    assert_eq!(
        choose_second.canonical_type_args,
        vec!["i64".to_string(), "bool".to_string()]
    );
}

#[test]
fn generic_fingerprints_ignore_impl_method_calls_with_wrong_arity() {
    let source = r#"
struct Boxed<T> {
    value: T,
}

impl<T> Boxed<T> {
    def choose_second<U>(self, fixed: i64, value: U) -> U {
        value
    }
}

def main() -> bool {
    let boxed: Boxed<i64> = Boxed { value: 7 };
    boxed.choose_second(false);
    boxed.choose_second(1, false, false);
    false
}
"#;

    let (_, instances) = generic_fingerprints_for_module("tests/main.sg", source);
    assert!(
        !instances.iter().any(|inst| {
            inst.item_stable_id
                .ends_with("::impl::Boxed<T>::choose_second")
        }),
        "wrong-arity impl-method calls should not enter generic instance planning"
    );
}

#[test]
fn generic_instance_plan_reuses_stdlib_impl_method_instances_on_warm_cache() {
    let source = stdlib_impl_method_instance_source();
    let (items, instances) = generic_fingerprints_for_module("tests/main.sg", &source);
    let graph = generic_graph_with_items(items, instances);
    let flags = ["emit_llvm=false".to_string()];

    let (_, cold_cache) = derive_generic_instance_plan(None, &graph, 1, &flags);
    let (warm_stats, _) = derive_generic_instance_plan(Some(&cold_cache), &graph, 1, &flags);

    assert!(
        warm_stats.total_instances >= 4,
        "stdlib helper additions may create extra generic instances, but the original method instances must remain tracked"
    );
    assert_eq!(warm_stats.cache_hits, warm_stats.total_instances);
    assert_eq!(warm_stats.rebuilt_instances, 0);
    assert!(warm_stats
        .reuse_instance_keys
        .iter()
        .any(|key| key.contains("::impl::Option<T>::ok_or<bool,bool>")));
    assert!(warm_stats
        .reuse_instance_keys
        .iter()
        .any(|key| key.contains("::impl::Option<T>::unwrap_or<bool>")));
    assert!(warm_stats
        .reuse_instance_keys
        .iter()
        .any(|key| key.contains("::impl::Result<T,E>::ok<bool,bool>")));
    assert!(warm_stats
        .reuse_instance_keys
        .iter()
        .any(|key| key.contains("::impl::Result<T,E>::unwrap_or<bool,i64>")));
}

#[test]
fn generic_instance_plan_reuses_chained_stdlib_impl_method_instances_on_warm_cache() {
    let source = stdlib_chained_impl_method_instance_source();
    let (items, instances) = generic_fingerprints_for_module("tests/main.sg", &source);
    let graph = generic_graph_with_items(items, instances);
    let flags = ["emit_llvm=false".to_string()];

    let (_, cold_cache) = derive_generic_instance_plan(None, &graph, 1, &flags);
    let (warm_stats, _) = derive_generic_instance_plan(Some(&cold_cache), &graph, 1, &flags);

    assert!(
        warm_stats.total_instances >= 3,
        "stdlib helper additions may create extra generic instances, but the chained method instances must remain tracked"
    );
    assert_eq!(warm_stats.cache_hits, warm_stats.total_instances);
    assert_eq!(warm_stats.rebuilt_instances, 0);
    assert!(warm_stats
        .reuse_instance_keys
        .iter()
        .any(|key| key.contains("::impl::Result<T,E>::ok<bool,i64>")));
    assert!(warm_stats
        .reuse_instance_keys
        .iter()
        .any(|key| key.contains("::impl::Result<T,E>::err<i64,bool>")));
    assert!(warm_stats
        .reuse_instance_keys
        .iter()
        .any(|key| key.contains("::impl::Option<T>::unwrap_or<bool>")));
}

fn generic_graph_with_items(
    items: Vec<GenericItemFingerprint>,
    instances: Vec<GenericInstanceFingerprint>,
) -> BuildGraphV2 {
    BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 1,
            implementation_hash: 1,
            depends_on: vec![],
            object_path: None,
            functions: vec![],
            generic_items: items,
            generic_instances: instances,
        }],
    }
}

fn generic_item(
    id: &str,
    symbol: &str,
    interface_hash: u64,
    body_hash: u64,
    calls: Vec<&str>,
) -> GenericItemFingerprint {
    GenericItemFingerprint {
        stable_item_id: id.to_string(),
        symbol: symbol.to_string(),
        module_id: "tests/main.sg".to_string(),
        kind: "function".to_string(),
        interface_hash,
        body_hash,
        type_param_count: 1,
        calls: calls.into_iter().map(|s| s.to_string()).collect(),
    }
}

fn generic_instance(
    item_id: &str,
    symbol_key: &str,
    interface_hash: u64,
    body_hash: u64,
) -> GenericInstanceFingerprint {
    GenericInstanceFingerprint {
        item_stable_id: item_id.to_string(),
        module_id: "tests/main.sg".to_string(),
        canonical_type_args: vec!["i64".to_string()],
        instance_key: format!("{}<i64>", symbol_key),
        interface_hash,
        body_hash,
    }
}

fn generic_cache_metadata(entries: Vec<GenericInstanceCacheEntry>) -> GenericInstanceCacheMetadata {
    GenericInstanceCacheMetadata {
        schema_version: GENERIC_INSTANCE_CACHE_SCHEMA_VERSION,
        compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        target_triple: format!(
            "{}-{}-{}",
            std::env::consts::ARCH,
            std::env::consts::OS,
            std::env::consts::FAMILY
        ),
        opt_level: 1,
        feature_flags: vec!["emit_llvm=false".to_string()],
        entries,
    }
}

fn generic_cache_entry(
    key: &str,
    item_id: &str,
    last_seen_unix_ms: u64,
) -> GenericInstanceCacheEntry {
    GenericInstanceCacheEntry {
        instance_key: key.to_string(),
        item_stable_id: item_id.to_string(),
        module_id: "tests/main.sg".to_string(),
        canonical_type_args: vec!["i64".to_string()],
        interface_hash: 1,
        body_hash: 1,
        last_seen_unix_ms,
    }
}

fn normalized_generic_entries(
    cache: &GenericInstanceCacheMetadata,
) -> Vec<(String, String, String, Vec<String>, u64, u64)> {
    let mut rows = cache
        .entries
        .iter()
        .map(|entry| {
            (
                entry.instance_key.clone(),
                entry.item_stable_id.clone(),
                entry.module_id.clone(),
                entry.canonical_type_args.clone(),
                entry.interface_hash,
                entry.body_hash,
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn generic_instance_plan_body_change_invalidates_only_same_item() {
    let item_a_id = "tests/main.sg::generic_add";
    let item_b_id = "tests/main.sg::generic_keep";
    let before_graph = generic_graph_with_items(
        vec![
            generic_item(item_a_id, item_a_id, 10, 100, vec![]),
            generic_item(item_b_id, item_b_id, 20, 200, vec![]),
        ],
        vec![
            generic_instance(item_a_id, item_a_id, 10, 100),
            generic_instance(item_b_id, item_b_id, 20, 200),
        ],
    );
    let (_, cache_before) =
        derive_generic_instance_plan(None, &before_graph, 1, &["emit_llvm=false".to_string()]);

    let after_graph = generic_graph_with_items(
        vec![
            generic_item(item_a_id, item_a_id, 10, 101, vec![]),
            generic_item(item_b_id, item_b_id, 20, 200, vec![]),
        ],
        vec![
            generic_instance(item_a_id, item_a_id, 10, 101),
            generic_instance(item_b_id, item_b_id, 20, 200),
        ],
    );
    let (stats, _) = derive_generic_instance_plan(
        Some(&cache_before),
        &after_graph,
        1,
        &["emit_llvm=false".to_string()],
    );

    assert_eq!(stats.total_instances, 2);
    assert_eq!(stats.body_invalidated, 1);
    assert_eq!(stats.interface_invalidated, 0);
    assert_eq!(stats.dependency_invalidated, 0);
    assert_eq!(stats.rebuilt_instances, 1);
    assert_eq!(stats.cache_hits, 1);
    assert!(generic_instance_hit_ratio(&stats) > 0.4);
}

#[test]
fn generic_instance_plan_interface_change_invalidates_dependents() {
    let callee_id = "tests/main.sg::generic_add";
    let caller_id = "tests/main.sg::generic_caller";
    let before_graph = generic_graph_with_items(
        vec![
            generic_item(callee_id, callee_id, 10, 100, vec![]),
            generic_item(caller_id, caller_id, 20, 200, vec![callee_id]),
        ],
        vec![
            generic_instance(callee_id, callee_id, 10, 100),
            generic_instance(caller_id, caller_id, 20, 200),
        ],
    );
    let (_, cache_before) =
        derive_generic_instance_plan(None, &before_graph, 1, &["emit_llvm=false".to_string()]);

    let after_graph = generic_graph_with_items(
        vec![
            generic_item(callee_id, callee_id, 99, 100, vec![]),
            generic_item(caller_id, caller_id, 20, 200, vec![callee_id]),
        ],
        vec![
            generic_instance(callee_id, callee_id, 99, 100),
            generic_instance(caller_id, caller_id, 20, 200),
        ],
    );
    let (stats, _) = derive_generic_instance_plan(
        Some(&cache_before),
        &after_graph,
        1,
        &["emit_llvm=false".to_string()],
    );

    assert_eq!(stats.total_instances, 2);
    assert_eq!(stats.interface_invalidated, 1);
    assert_eq!(stats.dependency_invalidated, 1);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.rebuilt_instances, 2);
}

#[test]
fn generic_instance_plan_same_input_reuses_cached_entries() {
    let item_id = "tests/main.sg::generic_add";
    let graph = generic_graph_with_items(
        vec![generic_item(item_id, item_id, 10, 100, vec![])],
        vec![generic_instance(item_id, item_id, 10, 100)],
    );
    let flags = ["emit_llvm=false".to_string()];
    let (_, cold_cache) = derive_generic_instance_plan(None, &graph, 1, &flags);
    let (warm_stats, warm_cache) =
        derive_generic_instance_plan(Some(&cold_cache), &graph, 1, &flags);

    assert_eq!(warm_stats.total_instances, 1);
    assert_eq!(warm_stats.cache_hits, 1);
    assert_eq!(warm_stats.rebuilt_instances, 0);
    assert_eq!(
        normalized_generic_entries(&cold_cache),
        normalized_generic_entries(&warm_cache)
    );
}

#[test]
fn generic_instance_plan_prunes_old_history_entries() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let stale_age_ms = 30 * 24 * 60 * 60 * 1000;
    let previous_cache = generic_cache_metadata(vec![
        generic_cache_entry(
            "stale::<i64>",
            "tests/main.sg::stale",
            now.saturating_sub(stale_age_ms),
        ),
        generic_cache_entry("recent::<i64>", "tests/main.sg::recent", now),
    ]);
    let item_id = "tests/main.sg::generic_add";
    let graph = generic_graph_with_items(
        vec![generic_item(item_id, item_id, 10, 100, vec![])],
        vec![generic_instance(item_id, item_id, 10, 100)],
    );
    let (_, next_cache) = derive_generic_instance_plan(
        Some(&previous_cache),
        &graph,
        1,
        &["emit_llvm=false".to_string()],
    );
    let keys = next_cache
        .entries
        .iter()
        .map(|entry| entry.instance_key.as_str())
        .collect::<HashSet<_>>();

    assert!(keys.contains("recent::<i64>"));
    assert!(!keys.contains("stale::<i64>"));
    assert!(keys.iter().any(|key| key.contains("generic_add")));
}

#[test]
fn generic_instance_plan_retention_has_entry_budget_cap() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let history = (0..9_000usize)
        .map(|idx| {
            generic_cache_entry(
                &format!("hist_{}::<i64>", idx),
                &format!("tests/main.sg::hist_{}", idx),
                now,
            )
        })
        .collect::<Vec<_>>();
    let previous_cache = generic_cache_metadata(history);
    let graph = generic_graph_with_items(Vec::new(), Vec::new());
    let (_, next_cache) = derive_generic_instance_plan(
        Some(&previous_cache),
        &graph,
        1,
        &["emit_llvm=false".to_string()],
    );
    assert_eq!(next_cache.entries.len(), 8_192);
}

#[test]
fn generic_instance_plan_ignores_uninstantiated_item_drift() {
    let hot_item_id = "tests/main.sg::generic_hot";
    let cold_item_id = "tests/main.sg::generic_cold";

    let before_graph = generic_graph_with_items(
        vec![
            generic_item(hot_item_id, hot_item_id, 10, 100, vec![]),
            generic_item(cold_item_id, cold_item_id, 20, 200, vec![]),
        ],
        vec![generic_instance(hot_item_id, hot_item_id, 10, 100)],
    );
    let flags = vec!["emit_llvm=false".to_string()];
    let (_, warm_cache) = derive_generic_instance_plan(None, &before_graph, 1, &flags);

    let after_graph = generic_graph_with_items(
        vec![
            generic_item(hot_item_id, hot_item_id, 10, 100, vec![]),
            // cold item changed, but still has no instantiated generic instance.
            generic_item(cold_item_id, cold_item_id, 99, 201, vec![]),
        ],
        vec![generic_instance(hot_item_id, hot_item_id, 10, 100)],
    );

    let (stats, _) = derive_generic_instance_plan(Some(&warm_cache), &after_graph, 1, &flags);
    assert_eq!(stats.total_instances, 1);
    assert_eq!(stats.cache_hits, 1);
    assert_eq!(stats.rebuilt_instances, 0);
    assert_eq!(stats.interface_invalidated, 0);
    assert_eq!(stats.body_invalidated, 0);
    assert_eq!(stats.dependency_invalidated, 0);
}

#[test]
fn generic_cache_skip_codegen_when_all_impacted_generics_are_cache_hits() {
    let item_id = "tests/main.sg::generic_add";
    let graph = generic_graph_with_items(
        vec![generic_item(item_id, item_id, 10, 100, vec![])],
        vec![generic_instance(item_id, item_id, 10, 100)],
    );
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/main.sg".to_string()],
        impacted_modules: vec!["tests/main.sg".to_string()],
        changed_functions: vec![item_id.to_string()],
        impacted_functions: vec![item_id.to_string()],
    };
    let stats = GenericInstancePlanStats {
        total_instances: 1,
        cache_hits: 1,
        rebuilt_instances: 0,
        interface_invalidated: 0,
        body_invalidated: 0,
        dependency_invalidated: 0,
        new_instances: 0,
        rebuild_item_ids: Vec::new(),
        rebuild_instance_keys: Vec::new(),
        reuse_item_ids: vec![item_id.to_string()],
        reuse_instance_keys: vec![format!("{}<i64>", item_id)],
    };
    assert!(can_skip_codegen_via_generic_cache(
        Some(&impact),
        &graph,
        &stats
    ));
}

#[test]
fn generic_cache_skip_codegen_disabled_when_rebuild_exists() {
    let item_id = "tests/main.sg::generic_add";
    let graph = generic_graph_with_items(
        vec![generic_item(item_id, item_id, 10, 100, vec![])],
        vec![generic_instance(item_id, item_id, 10, 100)],
    );
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/main.sg".to_string()],
        impacted_modules: vec!["tests/main.sg".to_string()],
        changed_functions: vec![item_id.to_string()],
        impacted_functions: vec![item_id.to_string()],
    };
    let stats = GenericInstancePlanStats {
        total_instances: 1,
        cache_hits: 0,
        rebuilt_instances: 1,
        interface_invalidated: 0,
        body_invalidated: 1,
        dependency_invalidated: 0,
        new_instances: 0,
        rebuild_item_ids: vec![item_id.to_string()],
        rebuild_instance_keys: vec![format!("{}<i64>", item_id)],
        reuse_item_ids: Vec::new(),
        reuse_instance_keys: Vec::new(),
    };
    assert!(!can_skip_codegen_via_generic_cache(
        Some(&impact),
        &graph,
        &stats
    ));
}

#[test]
fn unreachable_impl_only_root_changes_can_reuse_previous_artifacts() {
    let root = "tests/main.sg".to_string();
    let mut functions = vec![
        FunctionFingerprint {
            symbol: format!("{}::main", root),
            abi_hash: 1,
            body_hash: 1,
            calls: vec![format!("{}::live", root)],
            module_imports: vec![],
        },
        FunctionFingerprint {
            symbol: format!("{}::live", root),
            abi_hash: 2,
            body_hash: 2,
            calls: vec![],
            module_imports: vec![],
        },
        FunctionFingerprint {
            symbol: format!("{}::dead_changed", root),
            abi_hash: 3,
            body_hash: 3,
            calls: vec![],
            module_imports: vec![],
        },
    ];
    for idx in 0..20_000u32 {
        functions.push(FunctionFingerprint {
            symbol: format!("{}::dead_{}", root, idx),
            abi_hash: 10 + idx as u64,
            body_hash: 20 + idx as u64,
            calls: vec![],
            module_imports: vec![],
        });
    }
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: root.clone(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: root.clone(),
            interface_hash: 1,
            implementation_hash: 2,
            depends_on: vec![],
            object_path: None,
            functions,
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec![root.clone()],
        impacted_modules: vec![root.clone()],
        changed_functions: vec![format!("{}::dead_changed", root)],
        impacted_functions: vec![format!("{}::dead_changed", root)],
    };

    assert!(can_reuse_artifacts_for_unreachable_impl_only_changes(
        Some(&impact),
        &graph,
        Some(true),
    ));
}

#[test]
fn reachable_impl_only_root_changes_must_not_reuse_previous_artifacts() {
    let root = "tests/main.sg".to_string();
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: root.clone(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: root.clone(),
            interface_hash: 1,
            implementation_hash: 2,
            depends_on: vec![],
            object_path: None,
            functions: vec![
                FunctionFingerprint {
                    symbol: format!("{}::main", root),
                    abi_hash: 1,
                    body_hash: 1,
                    calls: vec![format!("{}::live", root)],
                    module_imports: vec![],
                },
                FunctionFingerprint {
                    symbol: format!("{}::live", root),
                    abi_hash: 2,
                    body_hash: 2,
                    calls: vec![],
                    module_imports: vec![],
                },
            ],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec![root.clone()],
        impacted_modules: vec![root.clone()],
        changed_functions: vec![format!("{}::live", root)],
        impacted_functions: vec![format!("{}::live", root)],
    };

    assert!(!can_reuse_artifacts_for_unreachable_impl_only_changes(
        Some(&impact),
        &graph,
        Some(true),
    ));
    assert!(!can_reuse_artifacts_for_unreachable_impl_only_changes(
        Some(&impact),
        &graph,
        Some(false),
    ));
}

#[test]
fn function_interface_change_propagates_via_call_edges() {
    let previous_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 7,
            implementation_hash: 9,
            depends_on: vec![],
            object_path: None,
            functions: vec![
                FunctionFingerprint {
                    symbol: "tests/main.sg::add".to_string(),
                    abi_hash: 11,
                    body_hash: 101,
                    calls: vec![],
                    module_imports: vec![],
                },
                FunctionFingerprint {
                    symbol: "tests/main.sg::main".to_string(),
                    abi_hash: 12,
                    body_hash: 102,
                    calls: vec!["tests/main.sg::add".to_string()],
                    module_imports: vec![],
                },
            ],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };

    let current_graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 7,
            implementation_hash: 9,
            depends_on: vec![],
            object_path: None,
            functions: vec![
                FunctionFingerprint {
                    symbol: "tests/main.sg::add".to_string(),
                    abi_hash: 999,
                    body_hash: 101,
                    calls: vec![],
                    module_imports: vec![],
                },
                FunctionFingerprint {
                    symbol: "tests/main.sg::main".to_string(),
                    abi_hash: 12,
                    body_hash: 102,
                    calls: vec!["tests/main.sg::add".to_string()],
                    module_imports: vec![],
                },
            ],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };

    let impact = classify_edit_impact(7, 9, 7, 9, &[], &[], Some(&previous_graph), &current_graph);
    assert_eq!(impact.class, EditClass::InterfaceChange);
    assert!(impact
        .impacted_functions
        .contains(&"tests/main.sg::main".to_string()));
    assert!(impact
        .impacted_functions
        .contains(&"tests/main.sg::add".to_string()));
}

#[test]
fn impl_only_impacted_symbols_expand_to_transitive_callers() {
    let previous = vec![
        FunctionFingerprint {
            symbol: "tests/main.sg::leaf".to_string(),
            abi_hash: 11,
            body_hash: 101,
            calls: vec![],
            module_imports: vec![],
        },
        FunctionFingerprint {
            symbol: "tests/main.sg::mid".to_string(),
            abi_hash: 12,
            body_hash: 102,
            calls: vec!["tests/main.sg::leaf".to_string()],
            module_imports: vec![],
        },
        FunctionFingerprint {
            symbol: "tests/main.sg::top".to_string(),
            abi_hash: 13,
            body_hash: 103,
            calls: vec!["tests/main.sg::mid".to_string()],
            module_imports: vec![],
        },
    ];
    let current = vec![
        FunctionFingerprint {
            symbol: "tests/main.sg::leaf".to_string(),
            abi_hash: 11,
            body_hash: 999,
            calls: vec![],
            module_imports: vec![],
        },
        FunctionFingerprint {
            symbol: "tests/main.sg::mid".to_string(),
            abi_hash: 12,
            body_hash: 102,
            calls: vec!["tests/main.sg::leaf".to_string()],
            module_imports: vec![],
        },
        FunctionFingerprint {
            symbol: "tests/main.sg::top".to_string(),
            abi_hash: 13,
            body_hash: 103,
            calls: vec!["tests/main.sg::mid".to_string()],
            module_imports: vec![],
        },
    ];

    let impacted = collect_impl_only_impacted_symbols(&previous, &current);
    assert_eq!(
        impacted,
        vec![
            "tests/main.sg::leaf".to_string(),
            "tests/main.sg::mid".to_string(),
            "tests/main.sg::top".to_string(),
        ]
    );
}

#[test]
fn workset_plan_reuses_previous_artifacts_when_impl_only_does_not_touch_root() {
    let previous = BuildCacheMetadata {
        cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        source_hash: 1,
        root_interface_hash: 10,
        root_implementation_hash: 20,
        module_fingerprints: vec![],
        opt_level: 2,
        contract_checks: false,
        debug_info: false,
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(777),
        llvm_ir_path: "tests/build/main.ll".to_string(),
        output_path: "tests/build/main.exe".to_string(),
        llvm_ir_hash: 33,
        object_path: Some("tests/build/main.obj".to_string()),
        build_graph_v2: None,
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/dep.sg".to_string()],
        impacted_modules: vec!["tests/dep.sg".to_string()],
        changed_functions: vec!["tests/dep.sg::add".to_string()],
        impacted_functions: vec!["tests/dep.sg::add".to_string()],
    };

    let plan = derive_build_workset_plan(
        Some(&previous),
        Some(&impact),
        "tests/main.sg",
        false,
        2,
        false,
        false,
        "tests/build/main.exe",
        Some("tools/stdlib/runtime.c"),
    );
    assert_eq!(plan, BuildWorksetPlan::ReusePreviousArtifacts);
}

#[test]
fn workset_plan_rebuilds_root_when_impl_only_touches_root() {
    let previous = BuildCacheMetadata {
        cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        source_hash: 1,
        root_interface_hash: 10,
        root_implementation_hash: 20,
        module_fingerprints: vec![],
        opt_level: 2,
        contract_checks: false,
        debug_info: false,
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(777),
        llvm_ir_path: "tests/build/main.ll".to_string(),
        output_path: "tests/build/main.exe".to_string(),
        llvm_ir_hash: 33,
        object_path: Some("tests/build/main.obj".to_string()),
        build_graph_v2: None,
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/main.sg".to_string()],
        impacted_modules: vec!["tests/main.sg".to_string()],
        changed_functions: vec!["tests/main.sg::main".to_string()],
        impacted_functions: vec!["tests/main.sg::main".to_string()],
    };

    let plan = derive_build_workset_plan(
        Some(&previous),
        Some(&impact),
        "tests/main.sg",
        false,
        2,
        false,
        false,
        "tests/build/main.exe",
        Some("tools/stdlib/runtime.c"),
    );
    assert_eq!(plan, BuildWorksetPlan::RebuildImpactedRoot);
}

#[test]
fn codegen_workset_manifest_reuse_marks_all_modules_reusable() {
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![
            BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 1,
                implementation_hash: 10,
                depends_on: vec!["tests/dep.sg".to_string()],
                object_path: None,
                functions: vec![],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
            BuildGraphNodeV2 {
                module_path: "tests/dep.sg".to_string(),
                interface_hash: 2,
                implementation_hash: 20,
                depends_on: vec![],
                object_path: None,
                functions: vec![],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
        ],
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/dep.sg".to_string()],
        impacted_modules: vec!["tests/dep.sg".to_string()],
        changed_functions: vec!["tests/dep.sg::add".to_string()],
        impacted_functions: vec!["tests/dep.sg::add".to_string()],
    };

    let manifest = derive_codegen_workset_manifest(
        &graph,
        Some(&impact),
        BuildWorksetPlan::ReusePreviousArtifacts,
        None,
    );
    assert!(manifest.rebuild_modules.is_empty());
    assert_eq!(
        manifest.reuse_modules,
        vec!["tests/dep.sg".to_string(), "tests/main.sg".to_string()]
    );
}

#[test]
fn codegen_workset_manifest_full_rebuild_marks_all_modules_rebuild() {
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![
            BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 1,
                implementation_hash: 10,
                depends_on: vec!["tests/dep.sg".to_string()],
                object_path: None,
                functions: vec![],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
            BuildGraphNodeV2 {
                module_path: "tests/dep.sg".to_string(),
                interface_hash: 2,
                implementation_hash: 20,
                depends_on: vec![],
                object_path: None,
                functions: vec![],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
        ],
    };

    let manifest =
        derive_codegen_workset_manifest(&graph, None, BuildWorksetPlan::FullRebuild, None);
    assert_eq!(
        manifest.rebuild_modules,
        vec!["tests/dep.sg".to_string(), "tests/main.sg".to_string()]
    );
    assert!(manifest.reuse_modules.is_empty());
}

#[test]
fn codegen_workset_manifest_rebuild_root_defaults_to_root_when_impact_absent() {
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 1,
            implementation_hash: 10,
            depends_on: vec![],
            object_path: None,
            functions: vec![],
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        }],
    };

    let manifest =
        derive_codegen_workset_manifest(&graph, None, BuildWorksetPlan::RebuildImpactedRoot, None);
    assert_eq!(manifest.rebuild_modules, vec!["tests/main.sg".to_string()]);
    assert!(manifest.reuse_modules.is_empty());
}

#[test]
fn codegen_workset_manifest_rebuild_root_tracks_symbol_frontier() {
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![
            BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 1,
                implementation_hash: 10,
                depends_on: vec!["tests/dep.sg".to_string()],
                object_path: None,
                functions: vec![FunctionFingerprint {
                    symbol: "tests/main.sg::main".to_string(),
                    abi_hash: 11,
                    body_hash: 101,
                    calls: vec!["tests/dep.sg::add".to_string()],
                    module_imports: vec!["tests/dep.sg".to_string()],
                }],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
            BuildGraphNodeV2 {
                module_path: "tests/dep.sg".to_string(),
                interface_hash: 2,
                implementation_hash: 20,
                depends_on: vec![],
                object_path: None,
                functions: vec![FunctionFingerprint {
                    symbol: "tests/dep.sg::add".to_string(),
                    abi_hash: 12,
                    body_hash: 102,
                    calls: vec![],
                    module_imports: vec![],
                }],
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            },
        ],
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/dep.sg".to_string()],
        impacted_modules: vec!["tests/dep.sg".to_string()],
        changed_functions: vec!["tests/dep.sg::add".to_string()],
        impacted_functions: vec!["tests/dep.sg::add".to_string()],
    };

    let manifest = derive_codegen_workset_manifest(
        &graph,
        Some(&impact),
        BuildWorksetPlan::RebuildImpactedRoot,
        None,
    );
    assert_eq!(
        manifest.changed_symbols,
        vec!["tests/dep.sg::add".to_string()]
    );
    assert_eq!(
        manifest.impacted_symbols,
        vec!["tests/dep.sg::add".to_string()]
    );
    assert_eq!(
        manifest.rebuild_symbols,
        vec!["tests/dep.sg::add".to_string()]
    );
    assert_eq!(
        manifest.reuse_symbols,
        vec!["tests/main.sg::main".to_string()]
    );
}

#[test]
fn codegen_workset_manifest_skips_generic_symbols_when_all_generic_instances_hit() {
    let generic_symbol = "tests/main.sg::generic_add".to_string();
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 1,
            implementation_hash: 10,
            depends_on: vec![],
            object_path: None,
            functions: vec![
                FunctionFingerprint {
                    symbol: generic_symbol.clone(),
                    abi_hash: 11,
                    body_hash: 101,
                    calls: vec![],
                    module_imports: vec![],
                },
                FunctionFingerprint {
                    symbol: "tests/main.sg::main".to_string(),
                    abi_hash: 12,
                    body_hash: 102,
                    calls: vec![generic_symbol.clone()],
                    module_imports: vec![],
                },
            ],
            generic_items: vec![generic_item(
                &generic_symbol,
                &generic_symbol,
                11,
                101,
                vec![],
            )],
            generic_instances: vec![generic_instance(&generic_symbol, &generic_symbol, 11, 101)],
        }],
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/main.sg".to_string()],
        impacted_modules: vec!["tests/main.sg".to_string()],
        changed_functions: vec![generic_symbol.clone()],
        impacted_functions: vec![generic_symbol.clone()],
    };
    let generic_stats = GenericInstancePlanStats {
        total_instances: 1,
        cache_hits: 1,
        reuse_item_ids: vec![generic_symbol.clone()],
        reuse_instance_keys: vec![format!("{}<i64>", generic_symbol)],
        ..Default::default()
    };

    let manifest = derive_codegen_workset_manifest(
        &graph,
        Some(&impact),
        BuildWorksetPlan::RebuildImpactedRoot,
        Some(&generic_stats),
    );
    assert!(manifest.generic_reuse_items.contains(&generic_symbol));
    assert!(!manifest.rebuild_symbols.contains(&generic_symbol));
}

#[test]
fn codegen_workset_manifest_keeps_rebuilt_generic_symbols() {
    let generic_symbol = "tests/main.sg::generic_add".to_string();
    let graph = BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: "tests/main.sg".to_string(),
        nodes: vec![BuildGraphNodeV2 {
            module_path: "tests/main.sg".to_string(),
            interface_hash: 1,
            implementation_hash: 10,
            depends_on: vec![],
            object_path: None,
            functions: vec![FunctionFingerprint {
                symbol: generic_symbol.clone(),
                abi_hash: 11,
                body_hash: 101,
                calls: vec![],
                module_imports: vec![],
            }],
            generic_items: vec![generic_item(
                &generic_symbol,
                &generic_symbol,
                11,
                101,
                vec![],
            )],
            generic_instances: vec![generic_instance(&generic_symbol, &generic_symbol, 11, 101)],
        }],
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/main.sg".to_string()],
        impacted_modules: vec!["tests/main.sg".to_string()],
        changed_functions: vec![generic_symbol.clone()],
        impacted_functions: vec![generic_symbol.clone()],
    };
    let generic_stats = GenericInstancePlanStats {
        total_instances: 1,
        rebuilt_instances: 1,
        body_invalidated: 1,
        rebuild_item_ids: vec![generic_symbol.clone()],
        rebuild_instance_keys: vec![format!("{}<i64>", generic_symbol)],
        ..Default::default()
    };

    let manifest = derive_codegen_workset_manifest(
        &graph,
        Some(&impact),
        BuildWorksetPlan::RebuildImpactedRoot,
        Some(&generic_stats),
    );
    assert!(manifest.generic_rebuild_items.contains(&generic_symbol));
    assert!(manifest.rebuild_symbols.contains(&generic_symbol));
}

#[test]
fn frontend_session_reuses_modules_when_source_is_unchanged() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-sgc-frontend-session-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let input = root.join("main.sg");
    let source = "def main() -> i64 {\n    1\n}\n";
    fs::write(&input, source).unwrap();

    let first = collect_module_graph_snapshot(
        &input,
        source,
        None,
        None,
        FrontendProbeMode::FastNoVerify,
        FrontendJobs::Auto,
        false,
        true,
    );
    assert!(first.reused_modules.is_empty());
    assert!(!first.rebuilt_modules.is_empty());

    let second = collect_module_graph_snapshot(
        &input,
        source,
        None,
        Some(&first.frontend_session_store),
        FrontendProbeMode::VerifyChangedAndDependents,
        FrontendJobs::Auto,
        false,
        true,
    );
    assert!(second.diagnostics.is_empty());
    assert_eq!(second.rebuilt_modules.len(), 0);
    assert_eq!(
        second.reused_modules.len(),
        first.frontend_session_store.modules.len()
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn frontend_parallel_and_serial_outputs_are_equivalent() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-sgc-frontend-determinism-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let main_path = root.join("main.sg");
    let dep_a = root.join("dep_a.sg");
    let dep_b = root.join("dep_b.sg");

    fs::write(
        &main_path,
        "import dep_a;\nimport dep_b;\ndef main() -> i64 {\n    dep_a_value() + dep_b_value()\n}\n",
    )
    .unwrap();
    fs::write(&dep_a, "def dep_a_value( -> i64 { 1 }\n").unwrap();
    fs::write(&dep_b, "def dep_b_value( -> i64 { 2 }\n").unwrap();

    let source = fs::read_to_string(&main_path).unwrap();
    let serial = collect_module_graph_snapshot(
        &main_path,
        &source,
        None,
        None,
        FrontendProbeMode::VerifyAll,
        FrontendJobs::Fixed(1),
        true,
        true,
    );
    let parallel = collect_module_graph_snapshot(
        &main_path,
        &source,
        None,
        None,
        FrontendProbeMode::VerifyAll,
        FrontendJobs::Fixed(4),
        true,
        true,
    );

    assert_eq!(serial.module_fingerprints, parallel.module_fingerprints);
    assert_eq!(
        serial.module_function_fingerprints,
        parallel.module_function_fingerprints
    );
    assert_eq!(serial.diagnostics, parallel.diagnostics);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn frontend_trace_decisions_are_deterministic_for_same_input() {
    let root =
        std::env::temp_dir().join(format!("sengoo-sgc-frontend-trace-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let input = root.join("main.sg");
    let source = "def main() -> i64 {\n    1\n}\n";
    fs::write(&input, source).unwrap();

    let first = collect_module_graph_snapshot(
        &input,
        source,
        None,
        None,
        FrontendProbeMode::FastNoVerify,
        FrontendJobs::Fixed(1),
        true,
        true,
    );
    let second = collect_module_graph_snapshot(
        &input,
        source,
        None,
        None,
        FrontendProbeMode::FastNoVerify,
        FrontendJobs::Fixed(1),
        true,
        true,
    );
    assert_eq!(first.planner_trace, second.planner_trace);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn frontend_dependency_digest_mismatch_escalates_full_fallback() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-sgc-frontend-digest-fallback-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let input = root.join("main.sg");
    let source = "def main() -> i64 {\n    1\n}\n";
    fs::write(&input, source).unwrap();

    let first = collect_module_graph_snapshot(
        &input,
        source,
        None,
        None,
        FrontendProbeMode::FastNoVerify,
        FrontendJobs::Auto,
        false,
        true,
    );
    let mut stale_session = first.frontend_session_store.clone();
    stale_session.dependency_graph_digest ^= 1;

    let second = collect_module_graph_snapshot(
        &input,
        source,
        None,
        Some(&stale_session),
        FrontendProbeMode::VerifyChangedAndDependents,
        FrontendJobs::Auto,
        false,
        true,
    );
    assert!(second
        .fallback_events
        .iter()
        .any(|event| event.scope == FrontendFallbackScope::FullFrontend
            && event.reason.contains("dependency digest mismatch")));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn run_workset_plan_reuses_previous_artifacts_when_impl_only_does_not_touch_root() {
    let previous = RunCacheMetadata {
        source_hash: 1,
        root_interface_hash: 10,
        root_implementation_hash: 20,
        module_fingerprints: vec![],
        opt_level: 2,
        contract_checks: false,
        debug_info: false,
        requested_engine: RunEngine::Auto,
        resolved_engine: RunEngine::Native,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(777),
        llvm_ir_path: "tests/build/main.ll".to_string(),
        executable_path: Some("tests/build/main.exe".to_string()),
        llvm_ir_hash: 33,
        object_path: Some("tests/build/main.obj".to_string()),
        build_graph_v2: None,
    };
    let impact = EditImpact {
        class: EditClass::ImplOnly,
        changed_modules: vec!["tests/dep.sg".to_string()],
        impacted_modules: vec!["tests/dep.sg".to_string()],
        changed_functions: vec!["tests/dep.sg::add".to_string()],
        impacted_functions: vec!["tests/dep.sg::add".to_string()],
    };

    let plan = derive_run_workset_plan(
        Some(&previous),
        Some(&impact),
        "tests/main.sg",
        2,
        false,
        false,
        RunEngine::Auto,
        RunEngine::Native,
        Some("tools/stdlib/runtime.c"),
    );
    assert_eq!(plan, BuildWorksetPlan::ReusePreviousArtifacts);
}

#[test]
fn run_workset_plan_full_rebuild_when_engine_changes() {
    let previous = RunCacheMetadata {
        source_hash: 1,
        root_interface_hash: 10,
        root_implementation_hash: 20,
        module_fingerprints: vec![],
        opt_level: 2,
        contract_checks: false,
        debug_info: false,
        requested_engine: RunEngine::Auto,
        resolved_engine: RunEngine::Native,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        runtime_c_fingerprint: Some(777),
        llvm_ir_path: "tests/build/main.ll".to_string(),
        executable_path: Some("tests/build/main.exe".to_string()),
        llvm_ir_hash: 33,
        object_path: Some("tests/build/main.obj".to_string()),
        build_graph_v2: None,
    };
    let impact = EditImpact {
        class: EditClass::Noop,
        changed_modules: vec![],
        impacted_modules: vec![],
        changed_functions: vec![],
        impacted_functions: vec![],
    };

    let plan = derive_run_workset_plan(
        Some(&previous),
        Some(&impact),
        "tests/main.sg",
        2,
        false,
        false,
        RunEngine::Native,
        RunEngine::Native,
        Some("tools/stdlib/runtime.c"),
    );
    assert_eq!(plan, BuildWorksetPlan::FullRebuild);
}

#[test]
fn stdlib_surface_runtime_iterator_filter_with_executes_non_capturing_lambda() {
    let output = require_stdlib_runtime_output!(
        "iter-filter-higher-order",
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    vec.push(4);

    let is_even = |x| x % 2;

    let iter = vec.iter();
    let first_odd = iter.filter_with(is_even).unwrap_or(0);
    iter.free();
    vec.free();
    first_odd
}
"#,
    );

    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn stdlib_surface_runtime_generic_i64_instantiations_work() {
    let output = require_stdlib_runtime_output!(
        "generic-i64-instantiations",
        r#"
def main() -> i64 {
    let vec: Vec<i64> = vec_new_i64();
    vec.push(5);
    let popped: Option<i64> = vec.pop();

    let map: HashMap<i64, i64> = hashmap_new_i64_i64();
    map.insert(1, popped.unwrap_or(0));
    let got: Option<i64> = map.get(1);

    let result: Result<i64, i64> = result_ok_i64(got.unwrap_or(0));

    vec.free();
    map.free();
    result.unwrap_or(0)
}
"#,
    );

    assert_eq!(output.status.code(), Some(5));
}

#[test]
fn stdlib_surface_runtime_bool_vec_and_hashmap_mutators_work() {
    let output = require_stdlib_runtime_output!(
        "bool-vec-hashmap-mutators",
        r#"
def main() -> i64 {
    let vec = vec_new_bool();
    vec.push(true);
    vec.push(false);
    vec.insert(1, true);

    let iter = vec.iter();
    let iter_first = iter.next().unwrap_or(false);
    iter.reset();
    let flip = |value| !value;
    let iter_mapped = iter.map_with(flip).unwrap_or(true);
    iter.reset();
    let keep_true = |value| value == true;
    let iter_filtered = iter.filter_with(keep_true).unwrap_or(false);
    iter.free();

    let first = vec.get(0).unwrap_or(false);
    let second = vec.pop().unwrap_or(true);
    let had_true = vec.contains(true);
    vec.set(0, false);
    let removed = vec.remove(0).unwrap_or(true);
    let removed_tail = vec.remove(0).unwrap_or(false);
    let vec_ok = iter_first
        && !iter_mapped
        && iter_filtered
        && first
        && !second
        && had_true
        && !removed
        && removed_tail
        && vec.is_empty();

    let map = hashmap_new_bool_bool();
    map.insert(true, false);
    map.insert(false, true);
    let map_true = map.get(true).unwrap_or(true);
    let map_false = map.get(false).unwrap_or(false);
    let had_key = map.contains(true);
    let removed_key = map.remove(true);
    let missing_key = map.get(true).unwrap_or(true);
    let map_ok = !map_true && map_false && had_key && removed_key && missing_key;

    let key_map = hashmap_new_bool_i64();
    key_map.insert(true, 6);
    let value_map = hashmap_new_i64_bool();
    value_map.insert(3, true);
    let mixed_ok = key_map.get(true).unwrap_or(0) == 6 && value_map.get(3).unwrap_or(false);

    vec.free();
    map.free();
    key_map.free();
    value_map.free();

    if vec_ok && map_ok && mixed_ok {
        9
    } else {
        0
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(9));
}

#[test]
fn stdlib_surface_runtime_string_vec_wrapper_and_generic_constructors_match_and_drop_cleanly() {
    let output = require_stdlib_runtime_output!(
        "string-vec-wrapper-generic-equivalence",
        r#"
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
}

def exercise_string_vecs() -> i64 {
    let status = {
        let wrapped = vec_new_string();
        let wrapped_pushed = wrapped.push(string_from_str("alpha").unwrap_or(String { handle: 0 }));
        let wrapped_inserted = wrapped.insert(0, string_from_str("go").unwrap_or(String { handle: 0 }));
        let wrapped_set = wrapped.set(1, string_from_str("rust").unwrap_or(String { handle: 0 }));
        let wrapped_first = wrapped.get(0).unwrap_or(String { handle: 0 });
        let wrapped_second = wrapped.get(1).unwrap_or(String { handle: 0 });
        let wrapped_iter = wrapped.iter();
        let wrapped_iter_value = wrapped_iter.next().unwrap_or(String { handle: 0 });
        let wrapped_done_before = wrapped_iter.done();
        wrapped_iter.reset();
        let wrapped_taken = wrapped_iter.take(1);
        wrapped_iter.free();
        let wrapped_removed = wrapped.remove(0).unwrap_or(String { handle: 0 });
        let wrapped_remaining = wrapped.get(0).unwrap_or(String { handle: 0 });

        let generic: Vec<String> = vec_new();
        let generic_pushed = generic.push(string_from_str("alpha").unwrap_or(String { handle: 0 }));
        let generic_inserted = generic.insert(0, string_from_str("go").unwrap_or(String { handle: 0 }));
        let generic_set = generic.set(1, string_from_str("rust").unwrap_or(String { handle: 0 }));
        let generic_first = generic.get(0).unwrap_or(String { handle: 0 });
        let generic_second = generic.get(1).unwrap_or(String { handle: 0 });
        let generic_iter = generic.iter();
        let generic_iter_value = generic_iter.next().unwrap_or(String { handle: 0 });
        let generic_done_before = generic_iter.done();
        generic_iter.reset();
        let generic_taken = generic_iter.take(1);
        generic_iter.free();
        let generic_removed = generic.remove(0).unwrap_or(String { handle: 0 });
        let generic_remaining = generic.get(0).unwrap_or(String { handle: 0 });

        let wrapped_status = if !wrapped_pushed { 1 }
            else if !wrapped_inserted { 2 }
            else if !wrapped_set { 3 }
            else if wrapped_done_before { 4 }
            else if wrapped_taken.len() != 1 { 5 }
            else if wrapped_first.len() != 2 { 6 }
            else if wrapped_second.len() != 4 { 7 }
            else if wrapped_iter_value.len() != 2 { 8 }
            else if wrapped_removed.len() != 2 { 9 }
            else if wrapped_remaining.len() != 4 { 10 }
            else { 0 };
        let generic_status = if !generic_pushed { 21 }
            else if !generic_inserted { 22 }
            else if !generic_set { 23 }
            else if generic_done_before { 24 }
            else if generic_taken.len() != 1 { 25 }
            else if generic_first.len() != 2 { 26 }
            else if generic_second.len() != 4 { 27 }
            else if generic_iter_value.len() != 2 { 28 }
            else if generic_removed.len() != 2 { 29 }
            else if generic_remaining.len() != 4 { 30 }
            else { 0 };

        if wrapped_status != 0 { wrapped_status } else { generic_status }
    };
    status
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let status = exercise_string_vecs();
    let after = sengoo_string_live_handle_count();

    if status == 0 && after == before {
        42
    } else {
        status + if after == before { 40 } else {
            if after > before { 100 + (after - before) } else { 90 }
        }
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_import_runtime_text_list_copies_values_and_iterates() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "text-list",
        r#"
import std::collections;
import std::io;
import std::string;

def main() -> i64 {
    let list = text_list_new();
    let buffer = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let small = ffi_buffer_new(1).unwrap_or(Buffer { handle: 0 });

    let pushed_first = list.push(str_append("al", "pha"));
    let pushed_second = list.push("beta");
    let first = list.get_copy(0, buffer).unwrap_or(0);
    let wrote_first = io_stdout_write_raw(buffer.ptr(), first).unwrap_or(0);
    let wrote_sep_one = io_stdout_write("|").unwrap_or(0);
    let replaced = list.set(1, str_repeat("z", 2));
    let removed = list.remove_copy(0, buffer).unwrap_or(0);
    let wrote_removed = io_stdout_write_raw(buffer.ptr(), removed).unwrap_or(0);
    let wrote_sep_two = io_stdout_write("|").unwrap_or(0);
    let too_small = list.get_copy(0, small).is_err();
    let missing = list.get_copy(9, buffer).is_err();

    let iter = list.iter();
    let not_done = !iter.done();
    let iter_len = iter.next_copy(buffer).unwrap_or(0);
    let iter_done = iter.done();
    let wrote_iter = io_stdout_write_raw(buffer.ptr(), iter_len).unwrap_or(0);
    iter.reset();
    let iter_len_again = iter.next_copy(buffer).unwrap_or(0);
    let reset_ok = iter_len_again == 2;
    let final_len = list.len();
    iter.free();

    small.free();
    buffer.free();
    list.free();

    if pushed_first
        && pushed_second
        && first == 5
        && wrote_first == 5
        && wrote_sep_one == 1
        && replaced
        && removed == 5
        && wrote_removed == 5
        && wrote_sep_two == 1
        && too_small
        && missing
        && not_done
        && iter_len == 2
        && iter_done
        && wrote_iter == 2
        && reset_ok
        && final_len == 1 {
        0
    } else {
        1
    }
}
"#,
        "",
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "alpha|alpha|zz");
}

#[test]
fn stdlib_surface_runtime_string_map_compatibility_wrappers_match_generic_and_drop_cleanly() {
    let output = require_stdlib_runtime_output!(
        "string-map-wrapper-generic-equivalence",
        r#"
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let status = {
        let numbers = string_map_i64_new();
        let flags = string_map_bool_new();
        let texts = string_map_string_new();

        let inserted_beta = numbers.insert(str_append("be", "ta"), 1);
        let inserted_alpha = numbers.insert("alpha", 2);
        let replaced_beta = numbers.insert("beta", 7);
        let contains_alpha = numbers.contains("alpha");
        let beta_value = numbers.get("beta").unwrap_or(0);
        let removed_alpha = numbers.remove("alpha");
        let missing_alpha = !numbers.contains("alpha");
        let number_keys = numbers.iter_keys();
        let number_key = number_keys.next().unwrap_or(String { handle: 0 });
        let number_done_before = number_keys.done();
        number_keys.reset();
        let number_taken = number_keys.take(1);
        number_keys.free();

        let inserted_on = flags.insert(str_append("o", "n"), true);
        let inserted_off = flags.insert("off", false);
        let replaced_on = flags.insert("on", false);
        let on_value = flags.get("on").unwrap_or(true);
        let off_value = flags.get("off").unwrap_or(true);
        let flag_keys = flags.iter_keys().collect();

        let text_inserted = texts.insert("title", string_from_str("alpha").unwrap_or(String { handle: 0 }));
        let text_replaced = texts.insert("title", string_from_str("gamma").unwrap_or(String { handle: 0 }));
        let text_value = texts.get("title").unwrap_or(String { handle: 0 });
        let text_removed = texts.remove("title").unwrap_or(String { handle: 0 });
        let text_missing = !texts.contains("title");

        let generic_numbers: HashMap<String, i64> = hashmap_new();
        let generic_inserted_beta = generic_numbers.insert("beta", 1);
        let generic_inserted_alpha = generic_numbers.insert("alpha", 2);
        let generic_replaced_beta = generic_numbers.insert("beta", 7);
        let generic_beta_value = generic_numbers.get("beta").unwrap_or(0);
        let generic_contains_alpha = generic_numbers.contains("alpha");
        let generic_removed_alpha = generic_numbers.remove("alpha");
        let generic_number_keys = generic_numbers.iter_keys().collect();

        let generic_flags: HashMap<String, bool> = hashmap_new();
        let generic_flag_inserted = generic_flags.insert("on", true);
        let generic_flag_replaced = generic_flags.insert("on", false);
        let generic_flag_value = generic_flags.get("on").unwrap_or(true);
        let generic_flag_removed = generic_flags.remove("on");

        let generic_texts: HashMap<String, String> = hashmap_new();
        let generic_text_inserted = generic_texts.insert(
            "title",
            string_from_str("alpha").unwrap_or(String { handle: 0 }),
        );
        let generic_text_replaced = generic_texts.insert(
            "title",
            string_from_str("gamma").unwrap_or(String { handle: 0 }),
        );
        let generic_text_value = generic_texts.get("title").unwrap_or(String { handle: 0 });
        let generic_text_removed = generic_texts.remove("title").unwrap_or(String { handle: 0 });
        let generic_text_key_count = generic_texts.iter_keys().count();

        let string_set = hashset_new_string();
        string_set.insert("ready");
        let set_contains = string_set.contains("ready");
        let set_iter = string_set.iter().collect();
        let set_removed = string_set.remove("ready");

        let generic_set: HashSet<String> = hashset_new();
        generic_set.insert("ready");
        let generic_set_contains = generic_set.contains("ready");
        let generic_set_iter = generic_set.iter().collect();
        let generic_set_removed = generic_set.remove("ready");

        let wrapper_status = if !inserted_beta { 1 }
            else if !inserted_alpha { 2 }
            else if !replaced_beta { 3 }
            else if !contains_alpha { 4 }
            else if beta_value != 7 { 5 }
            else if !removed_alpha { 6 }
            else if !missing_alpha { 7 }
            else if !number_done_before { 8 }
            else if number_key.len() != 4 { 9 }
            else if number_taken.len() != 1 { 10 }
            else if !inserted_on { 11 }
            else if !inserted_off { 12 }
            else if !replaced_on { 13 }
            else if on_value { 14 }
            else if off_value { 15 }
            else if flag_keys.len() != 2 { 16 }
            else if !text_inserted { 17 }
            else if !text_replaced { 18 }
            else if text_value.len() != 5 { 19 }
            else if text_removed.len() != 5 { 20 }
            else if !text_missing { 21 }
            else if !set_contains { 22 }
            else if set_iter.len() != 1 { 23 }
            else if !set_removed { 24 }
            else { 0 };
        let generic_ok = generic_inserted_beta
            && generic_inserted_alpha
            && generic_replaced_beta
            && generic_contains_alpha
            && generic_beta_value == 7
            && generic_removed_alpha
            && generic_number_keys.len() == 1
            && generic_flag_inserted
            && generic_flag_replaced
            && !generic_flag_value
            && generic_flag_removed
            && generic_text_inserted
            && generic_text_replaced
            && generic_text_value.len() == 5
            && generic_text_removed.len() == 5
            && generic_text_key_count == 0
            && generic_set_contains
            && generic_set_iter.len() == 1
            && generic_set_removed;

        if wrapper_status == 0 {
            if generic_ok { 0 } else { 30 }
        } else {
            wrapper_status
        }
    };
    let after = sengoo_string_live_handle_count();

    if status == 0 && after == before {
        42
    } else {
        status + if after == before { 40 } else {
            if after > before { 100 + (after - before) } else { 90 }
        }
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn stdlib_surface_runtime_option_ok_or_and_result_projections_are_correct() {
    let output = require_stdlib_runtime_output!(
        "option-result-projections",
        r#"
def main() -> i64 {
    let ok_from_option = option_some_i64(7).ok_or(9).ok().unwrap_or(0);
    let err_from_option = option_none_i64().ok_or(9).err().unwrap_or(0);
    let ok_projection = result_ok_i64(4).ok().unwrap_or(0);
    let err_projection = result_err_i64(5).err().unwrap_or(0);
    ok_from_option + err_from_option + ok_projection + err_projection
}
"#,
    );

    assert_eq!(output.status.code(), Some(25));
}

#[test]
fn stdlib_surface_runtime_bool_option_result_constructors_work() {
    let output = require_stdlib_runtime_output!(
        "bool-option-result-constructors",
        r#"
def main() -> i64 {
    let some_flag = option_some_bool(true);
    let none_flag = option_none_bool();
    let ok_flag = result_ok_bool(true);
    let err_flag = result_err_bool(7);

    if some_flag.unwrap_or(false)
        && none_flag.is_none()
        && ok_flag.ok().unwrap_or(false) {
        err_flag.err().unwrap_or(0)
    } else {
        0
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn stdlib_surface_runtime_bool_option_result_unwrap_and_expect_work() {
    let output = require_stdlib_runtime_output!(
        "bool-option-result-unwrap-expect",
        r#"
def main() -> i64 {
    let option_value = option_some_bool(true).unwrap();
    let expected_option = option_some_bool(true).expect("option bool ok");
    let result_value = result_ok_bool(false).unwrap();
    let expected_result = result_ok_bool(true).expect("result bool ok");

    if option_value && expected_option && !result_value && expected_result {
        11
    } else {
        0
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(11));
}

#[test]
fn stdlib_surface_runtime_generic_result_projections_work() {
    let output = require_stdlib_runtime_output!(
        "generic-result-projections",
        r#"
def main() -> i64 {
    let ok_result: Result<bool, i64> = Result { is_ok: true, value: true, error: 6 };
    let err_result: Result<bool, i64> = Result { is_ok: false, value: false, error: 6 };
    let bool_err: Result<i64, bool> = Result { is_ok: false, value: 0, error: true };

    let ok_option: Option<bool> = ok_result.ok();
    let err_code_option: Option<i64> = err_result.err();
    let err_flag_option: Option<bool> = bool_err.err();

    let ok_value: bool = ok_option.unwrap_or(false);
    let err_code: i64 = err_code_option.unwrap_or(0);
    let err_flag: bool = err_flag_option.unwrap_or(false);

    if ok_value && err_flag {
        err_code + 1
    } else {
        0
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn stdlib_surface_runtime_generic_result_projections_infer_local_types() {
    let output = require_stdlib_runtime_output!(
        "generic-result-projections-infer-locals",
        r#"
def main() -> i64 {
    let ok_result: Result<bool, i64> = Result { is_ok: true, value: true, error: 6 };
    let err_result: Result<bool, i64> = Result { is_ok: false, value: false, error: 6 };
    let bool_err: Result<i64, bool> = Result { is_ok: false, value: 0, error: true };

    let ok_option = ok_result.ok();
    let err_code_option = err_result.err();
    let err_flag_option = bool_err.err();

    let ok_value = ok_option.unwrap_or(false);
    let err_code = err_code_option.unwrap_or(0);
    let err_flag = err_flag_option.unwrap_or(false);

    if ok_value && err_flag {
        err_code + 1
    } else {
        0
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn stdlib_surface_runtime_default_generic_trait_method_executes_specialized_body() {
    let output = require_stdlib_runtime_output!(
        "default-trait-generic-wrap",
        r#"
struct Wrap<T> {
    value: T,
}

trait WrapValue {
    def wrap<T>(self, value: T) -> Wrap<T> {
        Wrap { value: value }
    }
}

impl WrapValue for i64 {
}

def main() -> i64 {
    let wrapped = 1.wrap(true);
    if wrapped.value {
        42
    } else {
        0
    }
}
"#,
    );

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_surface_runtime_default_generic_trait_method_supports_multiple_instantiations() {
    let output = require_stdlib_runtime_output!(
        "default-trait-generic-multi-inst",
        r#"
struct Wrap<T> {
    value: T,
}

trait WrapValue {
    def wrap<T>(self, value: T) -> Wrap<T> {
        Wrap { value: value }
    }
}

impl WrapValue for i64 {
}

def main() -> i64 {
    let wrapped_bool = 1.wrap(true);
    let wrapped_i64 = 1.wrap(7);

    if wrapped_bool.value {
        wrapped_i64.value + 1
    } else {
        0
    }
}
"#,
    );

    assert_eq!(
        output.status.code(),
        Some(8),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdlib_surface_runtime_generic_option_ok_or_with_i64_error_works() {
    let output = require_stdlib_runtime_output!(
        "generic-option-ok-or-i64",
        r#"
def main() -> i64 {
    let some_flag: Option<bool> = Option { is_some: true, value: true };
    let none_flag: Option<bool> = Option { is_some: false, value: false };

    let ok_result = some_flag.ok_or(6);
    let err_result = none_flag.ok_or(6);

    let ok_value = ok_result.ok().unwrap_or(false);
    let err_code = err_result.err().unwrap_or(0);

    if ok_value {
        err_code + 1
    } else {
        0
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn stdlib_surface_runtime_generic_option_ok_or_with_bool_error_works() {
    let output = require_stdlib_runtime_output!(
        "generic-option-ok-or-bool",
        r#"
def main() -> i64 {
    let some_flag: Option<bool> = Option { is_some: true, value: true };
    let none_flag: Option<bool> = Option { is_some: false, value: false };

    let ok_result = some_flag.ok_or(false);
    let err_result = none_flag.ok_or(true);

    let ok_value = ok_result.ok().unwrap_or(false);
    let err_flag = err_result.err().unwrap_or(false);

    if ok_value && err_flag {
        1
    } else {
        0
    }
}
"#,
    );

    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn stdlib_surface_runtime_unwrap_returns_values_on_success() {
    let output = require_stdlib_runtime_output!(
        "option-result-unwrap-success",
        r#"
def main() -> i64 {
    let option_value = option_some_i64(7).unwrap();
    let result_value = result_ok_i64(5).unwrap();
    option_value + result_value
}
"#,
    );

    assert_eq!(output.status.code(), Some(12));
}

#[test]
fn stdlib_surface_runtime_option_unwrap_panics_on_none() {
    let output = require_stdlib_runtime_output!(
        "option-unwrap-none",
        r#"
def main() -> i64 {
    option_none_i64().unwrap()
}
"#,
    );

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Option unwrap failed"),
        "stderr should mention unwrap failure: {}",
        stderr
    );
}

#[test]
fn stdlib_surface_runtime_expect_returns_values_on_success() {
    let output = require_stdlib_runtime_output!(
        "option-result-expect-success",
        r#"
def main() -> i64 {
    let option_value = option_some_i64(7).expect("option ok");
    let result_value = result_ok_i64(5).expect("result ok");
    option_value + result_value
}
"#,
    );

    assert_eq!(output.status.code(), Some(12));
}

#[test]
fn stdlib_surface_runtime_option_expect_prints_message_and_exits() {
    let output = require_stdlib_runtime_output!(
        "option-expect-none",
        r#"
def main() -> i64 {
    option_none_i64().expect("custom expect failure")
}
"#,
    );

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("custom expect failure"),
        "stdout should include custom message: {}",
        stdout
    );
    assert!(
        stderr.contains("Option unwrap failed"),
        "stderr should include fatal message: {}",
        stderr
    );
}

#[test]
fn stdlib_surface_runtime_result_expect_prints_message_and_exits() {
    let output = require_stdlib_runtime_output!(
        "result-expect-err",
        r#"
def main() -> i64 {
    result_err_i64(9).expect("result expect failure")
}
"#,
    );

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("result expect failure"),
        "stdout should include custom message: {}",
        stdout
    );
    assert!(
        stderr.contains("Result unwrap failed"),
        "stderr should include fatal message: {}",
        stderr
    );
}
