use super::{
    bench_root_dir, build_cache_key, build_graph_v2_for_source, build_metadata_matches,
    build_reflection_metadata, cache_key, cache_mismatch_reasons,
    can_reuse_artifacts_for_unreachable_impl_only_changes, can_skip_codegen_via_generic_cache,
    can_use_incremental_link_with_metadata, can_use_incremental_link_with_run_metadata,
    classify_edit_impact, cmd_build, collect_bench_cases, collect_impl_only_impacted_symbols,
    collect_module_graph_snapshot, compile_ir_to_object, compile_native_binary, compile_source,
    compile_source_with_phase_timings, daemon_request_build, derive_build_workset_plan,
    derive_cached_native_recovery_plan, derive_codegen_workset_manifest,
    derive_generic_instance_plan, derive_run_workset_plan, dispatch_build_via_daemon,
    edit_class_label, ensure_runtime_object, find_clang, find_runtime_c, format_edit_impact_lines,
    generic_fingerprints_for_module, generic_instance_hit_ratio, handle_daemon_client,
    link_native_binary_from_objects, maybe_emit_reflection_sidecar, metadata_matches,
    module_dependency_levels, module_fingerprints_for_source, module_invalidation_stats,
    parse_frontend_jobs_arg, parse_linker_mode, reflection_options_from_cli,
    reflection_sidecar_path_for_artifact, resolve_bench_suite_path, resolve_daemon_addr,
    resolve_engine, select_reflection_i64_zero_arity_symbol, send_daemon_request,
    signature_is_zero_arity_i64, validate_reflection_metadata, BuildCacheMetadata,
    BuildGraphNodeV2, BuildGraphV2, BuildWorksetPlan, CachedNativeRecoveryPlan, ContractChecksMode,
    DaemonDispatchOutcome, EditClass, EditImpact, FrontendFallbackScope, FrontendJobs,
    FrontendMemoryMode, FrontendModuleCacheEntryV4, FrontendProbeMode, FrontendSchedulerTelemetry,
    FrontendSessionStoreV4, FunctionFingerprint, GenericInstanceCacheEntry,
    GenericInstanceCacheMetadata, GenericInstanceFingerprint, GenericInstancePlanStats,
    GenericItemFingerprint, LinkerMode, ModuleFingerprint, ModuleGraphSnapshot, ReflectionMetadata,
    ReflectionMode, RunCacheMetadata, RunEngine, BUILD_GRAPH_SCHEMA_VERSION,
    DAEMON_PROTOCOL_VERSION, DEFAULT_DAEMON_ADDR, DEFAULT_SYMBOL_FINGERPRINT_MAX_SOURCE_BYTES,
    FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES, GENERIC_INSTANCE_CACHE_SCHEMA_VERSION,
    LOW_MEMORY_HINT_AVAILABLE_BYTES,
};
use crate::cli::Cli;
use clap::Parser as _;
use serde_json::Value;
use sengoo_compiler::compile_to_ir as compile_compiler_ir;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
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
        requested_engine: RunEngine::Auto,
        resolved_engine: RunEngine::Native,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
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

fn snapshot_with_root_hashes(
    input_path: &Path,
    interface_hash: u64,
    body_hash: u64,
) -> ModuleGraphSnapshot {
    let root_module = super::canonical_or_lossy(input_path);
    ModuleGraphSnapshot {
        module_fingerprints: Vec::new(),
        module_function_fingerprints: BTreeMap::new(),
        module_generic_items: BTreeMap::new(),
        module_generic_instances: BTreeMap::new(),
        dependency_edges: BTreeMap::new(),
        reflection_import_modules: Vec::new(),
        diagnostics: Vec::new(),
        planner_trace: Vec::new(),
        fallback_events: Vec::new(),
        frontend_scheduler: FrontendSchedulerTelemetry::default(),
        frontend_session_store: FrontendSessionStoreV4 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            scheduler_schema_version: super::FRONTEND_SCHEDULER_SCHEMA_VERSION,
            dependency_graph_digest: 0,
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            root_module: root_module.clone(),
            modules: vec![FrontendModuleCacheEntryV4 {
                module_id: root_module,
                source_hash: body_hash,
                parse_hash: body_hash,
                interface_hash,
                body_hash,
                hir_hash: body_hash,
                dependency_digest: 0,
                scheduler_schema_version: super::FRONTEND_SCHEDULER_SCHEMA_VERSION,
                depends_on: Vec::new(),
                symbols: Vec::new(),
                generic_items: Vec::new(),
                generic_instances: Vec::new(),
            }],
        },
        reused_modules: Vec::new(),
        rebuilt_modules: Vec::new(),
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
        RunEngine::Auto,
        RunEngine::Native,
        Some("tools/stdlib/runtime.c".to_string()),
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
        RunEngine::Auto,
        RunEngine::Lli,
        Some("tools/stdlib/runtime.c".to_string()),
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
        RunEngine::Auto,
        RunEngine::Native,
        Some("tools/stdlib/runtime.c".to_string()),
    );
    assert!(metadata_matches(&metadata, &key));
}

#[test]
fn cache_miss_when_module_dependency_changes() {
    let metadata = metadata_for_test();
    let key = cache_key(
        123,
        vec![fp("tests/mod_a.sg", 11, 99)],
        1,
        false,
        RunEngine::Auto,
        RunEngine::Native,
        Some("tools/stdlib/runtime.c".to_string()),
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
        RunEngine::Auto,
        RunEngine::Native,
        Some("tools/stdlib/runtime.c".to_string()),
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
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);
    let runtime_c = fs::read_to_string(workspace_root.join("tools/stdlib/runtime.c")).unwrap();

    for symbol in [
        "sengoo_vec_new_i64",
        "sengoo_vec_free_i64",
        "sengoo_vec_len_i64",
        "sengoo_vec_clear_i64_status",
        "sengoo_vec_push_i64",
        "sengoo_vec_get_i64",
        "sengoo_vec_set_i64",
        "sengoo_vec_pop_i64",
        "sengoo_vec_get_or_default_i64",
        "sengoo_vec_contains_i64",
        "sengoo_vec_remove_i64",
        "sengoo_vec_remove_or_default_i64",
        "sengoo_vec_pop_or_default_i64",
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
        assert!(runtime_c.contains(symbol), "runtime stdlib missing symbol: {symbol}");
    }
}

#[test]
fn stdlib_runtime_exports_iterator_and_option_result_adapters() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);
    let runtime_c = fs::read_to_string(workspace_root.join("tools/stdlib/runtime.c")).unwrap();

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
        assert!(runtime_c.contains(symbol), "runtime stdlib missing symbol: {symbol}");
    }
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
        assert!(ps.contains(needle), "ps1 missing updated acceptance command: {needle}");
        assert!(sh.contains(needle), "sh missing updated acceptance command: {needle}");
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
        assert!(ps.contains(capability), "ps1 missing capability: {capability}");
        assert!(sh.contains(capability), "sh missing capability: {capability}");
    }
}
#[test]
fn advanced_pipeline_memory_buckets_cover_100k_and_1000k() {
    let root = bench_root_dir();
    let script = fs::read_to_string(root.join("advanced_pipeline_bench.py")).unwrap();
    assert!(script.contains("MEMORY_LOC_BUCKETS = [10000, 100000, 1000000]"));
}

#[test]
fn advanced_kpi_gate_requires_100k_and_1000k_memory_buckets() {
    let root = bench_root_dir();
    let gate = fs::read_to_string(root.join("scripts/advanced-kpi-gate.py")).unwrap();
    assert!(gate.contains("DEFAULT_REQUIRED_MEMORY_LOCS = (\"10000\", \"100000\", \"1000000\")"));
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
        Cli::try_parse_from([
            "sgc",
            "build",
            "tests/demo.sg",
            "--output",
            "dist/app",
        ])
        .is_ok()
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
async fn doc_command_generates_rustdoc_like_layout() {
    let root =
        std::env::temp_dir().join(format!("sengoo-sgc-doc-gen-{}", std::process::id()));
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
fn run_supported_engine_flags_parse() {
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--engine", "auto",]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--engine", "native",]).is_ok());
    assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--engine", "lli",]).is_ok());
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
fn location_from_compile_error_extracts_invalid_pattern_span() {
    let src = "def main() -> i64 {\n    let = 1;\n}\n";
    let error = super::compile_to_ir(src).expect_err("source should fail parsing");
    let location =
        super::location_from_compile_error(src, &error).expect("parse errors should include location");

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
fn frontend_memory_mode_auto_defaults_to_legacy_for_small_and_large_sources() {
    assert_eq!(
        super::resolve_frontend_memory_mode(64),
        FrontendMemoryMode::Legacy
    );
    assert_eq!(
        super::resolve_frontend_memory_mode(FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES * 8),
        FrontendMemoryMode::Legacy
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
fn root_hashes_reuse_snapshot_without_force_rebuild() {
    let input = Path::new("tests/main.sg");
    let snapshot = snapshot_with_root_hashes(input, 111, 222);
    let (interface_hash, impl_hash) = super::resolve_root_hashes_for_request(
        input,
        "invalid source {}",
        &snapshot,
        false,
        None,
        None,
    );
    assert_eq!(interface_hash, 111);
    assert_eq!(impl_hash, 222);
}

#[test]
fn force_rebuild_root_hashes_can_reuse_previous_interface_when_impl_unchanged() {
    let input = Path::new("tests/main.sg");
    let snapshot = snapshot_with_root_hashes(input, 111, 222);
    let (interface_hash, impl_hash) = super::resolve_root_hashes_for_request(
        input,
        "def main() -> i64 { 1 }",
        &snapshot,
        true,
        Some(222),
        Some(333),
    );
    assert_eq!(impl_hash, 222);
    assert_eq!(interface_hash, 111);
}

#[test]
fn force_rebuild_root_hashes_fallback_to_previous_interface_when_snapshot_missing() {
    let input = Path::new("tests/main.sg");
    let mut snapshot = snapshot_with_root_hashes(input, 111, 222);
    snapshot.frontend_session_store.modules.clear();
    let source = "def main() -> i64 { 1 }";
    let source_impl_hash = super::implementation_fingerprint(source);
    let (interface_hash, impl_hash) = super::resolve_root_hashes_for_request(
        input,
        source,
        &snapshot,
        true,
        Some(source_impl_hash),
        Some(333),
    );
    assert_eq!(impl_hash, source_impl_hash);
    assert_eq!(interface_hash, 333);
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
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        handle_daemon_client(stream).await.unwrap();
    });

    let input = bench_root_dir().join("tests").join("simple_array.sg");
    let request = daemon_request_build(
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
    );
    let response = send_daemon_request(&addr.to_string(), &request)
        .await
        .unwrap();
    assert!(response.ok, "{}", response.message);

    server.await.unwrap();
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
        Some("tools/stdlib/runtime.c".to_string()),
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
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
        llvm_ir_path: "tests/build/a.ll".to_string(),
        output_path: "tests/build/a.exe".to_string(),
        llvm_ir_hash: 777,
        object_path: Some("tests/build/a.obj".to_string()),
        build_graph_v2: None,
    };
    assert!(!build_metadata_matches(&metadata, &key));
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
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
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
        requested_engine: RunEngine::Native,
        resolved_engine: RunEngine::Native,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
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

    let exe_path = temp_artifact(
        "async-native-main",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async native executable should run");
    assert_eq!(output.status.code(), Some(43));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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

    let llvm_ir = compile_source(source, 1).expect("timeout-ready source should compile to LLVM IR");
    let ll_path = temp_artifact("async-timeout-ready", "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact("async-timeout-ready", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("timeout-ready native executable should run");
    assert_eq!(output.status.code(), Some(9));

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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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

    let exe_path = temp_artifact("async-spawn-task-status", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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

    let exe_path = temp_artifact("async-cancel-task-status", if cfg!(windows) { "exe" } else { "" });
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("cancel_task executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
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

    let llvm_ir = compile_source(source, 1).expect("spawn polling source should compile to LLVM IR");
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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("f64 select executable should run");
    assert_eq!(output.status.code(), Some(1));

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

    let exe_path = temp_artifact(
        "async-live-local",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async native executable should run");
    assert_eq!(output.status.code(), Some(40));

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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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
    let x = 0;
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

    let exe_path = temp_artifact(
        "async-loop-body",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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
    let ticks = 0;
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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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

    let exe_path = temp_artifact(
        "async-match-arms",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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

    let exe_path = temp_artifact(
        "async-bool-local",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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

    let exe_path = temp_artifact(
        "async-ref-local",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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

    let exe_path = temp_artifact(
        "async-f64-local",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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
    let exe_path = temp_artifact(
        "async-f32-local",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&custom_runtime_c_str), 1).unwrap();

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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&custom_runtime_c_str), 1).unwrap();

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
    let exe_path = temp_artifact(
        "async-frame-guard",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&custom_runtime_c_str), 1).unwrap();

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
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&custom_runtime_c_str), 1).unwrap();

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

    let exe_path = temp_artifact(
        "async-struct-local",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

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

    let exe_path = temp_artifact(
        "async-array-local",
        if cfg!(windows) { "exe" } else { "" },
    );
    compile_native_binary(&clang, &ll_path, &exe_path, Some(&runtime_c), 1).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("async array executable should run");
    assert_eq!(output.status.code(), Some(42));

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
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
    compile_native_binary(&clang, &ll_path, &full_exe, runtime_c.as_deref(), 2).unwrap();
    compile_ir_to_object(&clang, &ll_path, &obj_path, 2).unwrap();

    let mut object_paths = vec![obj_path.clone()];
    if let Some(runtime_c) = runtime_c.as_deref() {
        object_paths.push(ensure_runtime_object(&clang, runtime_c, 2).unwrap());
    }
    link_native_binary_from_objects(&clang, &object_paths, &inc_exe).unwrap();

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
        let probe_obj = temp_artifact(
            "stdlib-runtime-c-probe",
            if cfg!(windows) { "obj" } else { "o" },
        );
        let mut command = Command::new(clang);
        command.arg("-Wno-override-module").arg("-O2");
        #[cfg(windows)]
        {
            command.arg("--target=x86_64-pc-windows-msvc");
        }
        let status = command
            .arg("-c")
            .arg(runtime_c)
            .arg("-o")
            .arg(&probe_obj)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = fs::remove_file(&probe_obj);
        status.map(|status| status.success()).unwrap_or(false)
    })
}

fn load_stdlib_surface_source() -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(manifest_dir);
    fs::read_to_string(workspace_root.join("tools/stdlib/collections.sg"))
        .expect("stdlib surface should exist")
}

fn compile_and_run_stdlib_program(tag: &str, source: &str) -> Option<std::process::Output> {
    let Some(clang) = find_clang() else {
        return None;
    };

    let combined = format!("{}\n\n{}", load_stdlib_surface_source(), source);
    let llvm_ir = compile_compiler_ir(&combined).expect("stdlib source should compile");
    let ll_path = temp_artifact(&format!("stdlib-runtime-{}", tag), "ll");
    fs::write(&ll_path, llvm_ir).unwrap();

    let exe_path = temp_artifact(&format!("stdlib-runtime-{}", tag), if cfg!(windows) { "exe" } else { "" });
    let obj_path = temp_artifact(&format!("stdlib-runtime-{}", tag), if cfg!(windows) { "obj" } else { "o" });
    compile_ir_to_object(&clang, &ll_path, &obj_path, 2).unwrap();

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
    let runtime_obj = temp_artifact(&format!("stdlib-runtime-c-{}", tag), if cfg!(windows) { "obj" } else { "o" });
    compile_ir_to_object(&clang, &runtime_c, &runtime_obj, 2).unwrap();

    let object_paths = vec![obj_path.clone(), runtime_obj.clone()];
    link_native_binary_from_objects(&clang, &object_paths, &exe_path).unwrap();

    let output = Command::new(&exe_path).output().expect("stdlib binary should run");

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&runtime_obj);
    let _ = fs::remove_file(&exe_path);
    Some(output)
}

macro_rules! require_stdlib_runtime_output {
    ($tag:expr, $source:expr $(,)?) => {{
        let Some(output) = compile_and_run_stdlib_program($tag, $source) else {
            return;
        };
        output
    }};
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
    let output = require_stdlib_runtime_output!("boundary", 
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
    let item = iter.next();
    let total = 0;
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
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
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
        emit_llvm: false,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
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
        requested_engine: RunEngine::Auto,
        resolved_engine: RunEngine::Native,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
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
        requested_engine: RunEngine::Auto,
        resolved_engine: RunEngine::Native,
        runtime_c: Some("tools/stdlib/runtime.c".to_string()),
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
    assert!(stderr.contains("Option unwrap failed"), "stderr should mention unwrap failure: {}", stderr);
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
    assert!(stdout.contains("custom expect failure"), "stdout should include custom message: {}", stdout);
    assert!(stderr.contains("Option unwrap failed"), "stderr should include fatal message: {}", stderr);
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
    assert!(stdout.contains("result expect failure"), "stdout should include custom message: {}", stdout);
    assert!(stderr.contains("Result unwrap failed"), "stderr should include fatal message: {}", stderr);
}


