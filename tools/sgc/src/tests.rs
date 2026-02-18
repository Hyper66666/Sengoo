    use super::{
        bench_root_dir, build_cache_key, build_graph_v2_for_source, build_metadata_matches,
        build_reflection_metadata, cache_key, cache_mismatch_reasons,
        can_use_incremental_link_with_metadata, can_use_incremental_link_with_run_metadata,
        classify_edit_impact, cmd_build, collect_bench_cases, collect_impl_only_impacted_symbols,
        collect_module_graph_snapshot, compile_ir_to_object, compile_native_binary, compile_source,
        compile_source_with_phase_timings, daemon_request_build, derive_build_workset_plan,
        derive_cached_native_recovery_plan, derive_codegen_workset_manifest,
        derive_run_workset_plan, dispatch_build_via_daemon, edit_class_label,
        ensure_runtime_object, find_clang, find_runtime_c, handle_daemon_client,
        link_native_binary_from_objects, maybe_emit_reflection_sidecar, metadata_matches,
        module_dependency_levels, module_fingerprints_for_source, module_invalidation_stats,
        parse_frontend_jobs_arg, parse_linker_mode, reflection_options_from_cli,
        reflection_sidecar_path_for_artifact, resolve_bench_suite_path, resolve_daemon_addr,
        resolve_engine, select_reflection_i64_zero_arity_symbol, send_daemon_request,
        signature_is_zero_arity_i64, validate_reflection_metadata, BuildCacheMetadata,
        BuildGraphNodeV2, BuildGraphV2, BuildWorksetPlan, CachedNativeRecoveryPlan,
        DaemonDispatchOutcome, EditClass, EditImpact, FrontendFallbackScope, FrontendJobs,
        FrontendMemoryMode, FrontendModuleCacheEntryV4, FrontendProbeMode,
        FrontendSchedulerTelemetry, FrontendSessionStoreV4, FunctionFingerprint, LinkerMode,
        ModuleFingerprint, ModuleGraphSnapshot, ReflectionMetadata, ReflectionMode,
        RunCacheMetadata, RunEngine,
        BUILD_GRAPH_SCHEMA_VERSION, DAEMON_PROTOCOL_VERSION, DEFAULT_DAEMON_ADDR,
    };
    use crate::cli::Cli;
    use clap::Parser as _;
    use std::collections::{BTreeMap, HashSet};
    use std::fs;
    use std::path::Path;
    use std::process::Command;
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
            }],
        }
    }

    fn snapshot_with_root_hashes(input_path: &Path, interface_hash: u64, body_hash: u64) -> ModuleGraphSnapshot {
        let root_module = super::canonical_or_lossy(input_path);
        ModuleGraphSnapshot {
            module_fingerprints: Vec::new(),
            module_function_fingerprints: BTreeMap::new(),
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
    fn low_memory_flag_parses_for_build_and_run() {
        assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--low-memory"]).is_ok());
        assert!(Cli::try_parse_from(["sgc", "run", "tests/demo.sg", "--low-memory"]).is_ok());
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
            Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--frontend-jobs", "auto",])
                .is_ok()
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
        let root =
            std::env::temp_dir().join(format!("sengoo-sgc-reflect-auto-{}", std::process::id()));
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
                signature: "pub|reflect_probe|async=false|self=-|tp=[]|params=[]|ret=i64"
                    .to_string(),
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
        let module_path =
            temp_sg_module("meta-unknown", "def add(a: i64, b: i64) -> i64 { a + b }\n");
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
            }],
        };
        let metadata = BuildCacheMetadata {
            cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            source_hash: 1,
            root_interface_hash: 1,
            root_implementation_hash: 1,
            module_fingerprints: vec![],
            opt_level: 2,
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
            }],
        };
        let metadata = RunCacheMetadata {
            source_hash: 1,
            root_interface_hash: 1,
            root_implementation_hash: 1,
            module_fingerprints: vec![],
            opt_level: 2,
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

    #[test]
    fn runtime_suite_name_prefers_bench_directory() {
        let suite_path = resolve_bench_suite_path("runtime", "runtime").unwrap();
        assert!(suite_path.ends_with(Path::new("bench").join("suites").join("runtime")));
        let cases = collect_bench_cases(&suite_path).unwrap();
        assert!(!cases.is_empty());
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
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 1,
                    implementation_hash: 11,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![],
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
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 1,
                    implementation_hash: 11,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![],
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
            }],
        };

        let impact =
            classify_edit_impact(7, 9, 7, 9, &[], &[], Some(&previous_graph), &current_graph);
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
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 2,
                    implementation_hash: 20,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![],
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
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 2,
                    implementation_hash: 20,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![],
                },
            ],
        };

        let manifest = derive_codegen_workset_manifest(&graph, None, BuildWorksetPlan::FullRebuild);
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
            }],
        };

        let manifest =
            derive_codegen_workset_manifest(&graph, None, BuildWorksetPlan::RebuildImpactedRoot);
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
            RunEngine::Native,
            RunEngine::Native,
            Some("tools/stdlib/runtime.c"),
        );
        assert_eq!(plan, BuildWorksetPlan::FullRebuild);
    }

