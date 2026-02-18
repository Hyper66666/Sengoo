#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RunEngine {
    Auto,
    Native,
    Lli,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ReflectionMode {
    Auto,
    On,
    Off,
}

impl Default for ReflectionMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FrontendMemoryMode {
    Auto,
    Stream,
    Legacy,
}

impl Default for FrontendMemoryMode {
    fn default() -> Self {
        Self::Auto
    }
}

const BUILD_GRAPH_SCHEMA_VERSION: u32 = 4;
const DAEMON_PROTOCOL_VERSION: u32 = 1;
const REFLECTION_SCHEMA_VERSION: u32 = 1;
const FRONTEND_SCHEDULER_SCHEMA_VERSION: u32 = 1;
const FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES: usize = 256 * 1024;
const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:48765";
const DAEMON_CONNECT_TIMEOUT: Duration = Duration::from_millis(1200);
const LINKER_UNKNOWN: i8 = -1;
const LINKER_UNAVAILABLE: i8 = 0;
const LINKER_AVAILABLE: i8 = 1;

static LLD_AVAILABILITY: AtomicI8 = AtomicI8::new(LINKER_UNKNOWN);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ReflectionCliOptions {
    mode: ReflectionMode,
    enabled: bool,
    modules: Vec<String>,
    symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionSymbolMetadata {
    symbol: String,
    signature: String,
    #[serde(default)]
    native_symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionModuleMetadata {
    module_id: String,
    #[serde(default)]
    symbols: Vec<ReflectionSymbolMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionMetadata {
    #[serde(default = "default_reflection_schema_version")]
    schema_version: u32,
    compiler_version: String,
    #[serde(default)]
    compatible_compiler_versions: Vec<String>,
    root_module: String,
    #[serde(default)]
    modules: Vec<ReflectionModuleMetadata>,
}

fn default_reflection_schema_version() -> u32 {
    REFLECTION_SCHEMA_VERSION
}

fn default_frontend_scheduler_schema_version() -> u32 {
    FRONTEND_SCHEDULER_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModuleFingerprint {
    path: String,
    #[serde(default)]
    interface_hash: u64,
    /// Implementation hash (normalized source, comments stripped).
    hash: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct ModuleInvalidationStats {
    #[allow(dead_code)]
    total_modules: u32,
    reused_modules: u32,
    rebuilt_modules: u32,
    interface_changed_modules: u32,
    implementation_only_changed_modules: u32,
}

#[derive(Debug, Clone)]
struct ModuleSourceInfo {
    source: Arc<str>,
    depends_on: Vec<String>,
    requests_reflection: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontendProbeMode {
    FastNoVerify,
    VerifyChangedAndDependents,
    VerifyAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontendJobs {
    Auto,
    Fixed(usize),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FrontendSchedulerTelemetry {
    #[serde(default)]
    requested_jobs: String,
    #[serde(default)]
    selected_jobs: u32,
    #[serde(default)]
    serial_mode: bool,
    #[serde(default)]
    parse_interface_task_count: u32,
    #[serde(default)]
    body_hir_task_count: u32,
    #[serde(default)]
    queue_wait_avg_ms: f64,
    #[serde(default)]
    queue_wait_max_ms: f64,
    #[serde(default)]
    worker_utilization_pct: f64,
}

impl Default for FrontendSchedulerTelemetry {
    fn default() -> Self {
        Self {
            requested_jobs: "auto".to_string(),
            selected_jobs: 1,
            serial_mode: true,
            parse_interface_task_count: 0,
            body_hir_task_count: 0,
            queue_wait_avg_ms: 0.0,
            queue_wait_max_ms: 0.0,
            worker_utilization_pct: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FrontendFallbackScope {
    Symbol,
    Module,
    FullFrontend,
}

fn frontend_fallback_scope_label(scope: FrontendFallbackScope) -> &'static str {
    match scope {
        FrontendFallbackScope::Symbol => "symbol",
        FrontendFallbackScope::Module => "module",
        FrontendFallbackScope::FullFrontend => "full_frontend",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FrontendFallbackEvent {
    stage: String,
    scope: FrontendFallbackScope,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FrontendModuleCacheEntryV4 {
    module_id: String,
    source_hash: u64,
    parse_hash: u64,
    interface_hash: u64,
    body_hash: u64,
    hir_hash: u64,
    #[serde(default)]
    dependency_digest: u64,
    #[serde(default = "default_frontend_scheduler_schema_version")]
    scheduler_schema_version: u32,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    symbols: Vec<FunctionFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FrontendSessionStoreV4 {
    #[serde(default = "default_build_graph_schema_version")]
    schema_version: u32,
    #[serde(default = "default_frontend_scheduler_schema_version")]
    scheduler_schema_version: u32,
    #[serde(default)]
    dependency_graph_digest: u64,
    compiler_version: String,
    root_module: String,
    #[serde(default)]
    modules: Vec<FrontendModuleCacheEntryV4>,
}

#[derive(Debug, Clone)]
struct ModuleGraphSnapshot {
    module_fingerprints: Vec<ModuleFingerprint>,
    module_function_fingerprints: BTreeMap<String, Vec<FunctionFingerprint>>,
    dependency_edges: BTreeMap<String, Vec<String>>,
    reflection_import_modules: Vec<String>,
    diagnostics: Vec<String>,
    planner_trace: Vec<String>,
    fallback_events: Vec<FrontendFallbackEvent>,
    frontend_scheduler: FrontendSchedulerTelemetry,
    frontend_session_store: FrontendSessionStoreV4,
    reused_modules: Vec<String>,
    rebuilt_modules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunCacheMetadata {
    source_hash: u64,
    #[serde(default)]
    root_interface_hash: u64,
    #[serde(default)]
    root_implementation_hash: u64,
    #[serde(default)]
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    runtime_c: Option<String>,
    llvm_ir_path: String,
    executable_path: Option<String>,
    #[serde(default)]
    llvm_ir_hash: u64,
    #[serde(default)]
    object_path: Option<String>,
    #[serde(default)]
    build_graph_v2: Option<BuildGraphV2>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunCacheKey {
    source_hash: u64,
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    runtime_c: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuildCacheMetadata {
    #[serde(default = "default_build_cache_schema_version")]
    cache_schema_version: u32,
    source_hash: u64,
    #[serde(default)]
    root_interface_hash: u64,
    #[serde(default)]
    root_implementation_hash: u64,
    #[serde(default)]
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    emit_llvm: bool,
    runtime_c: Option<String>,
    llvm_ir_path: String,
    output_path: String,
    #[serde(default)]
    llvm_ir_hash: u64,
    #[serde(default)]
    object_path: Option<String>,
    #[serde(default)]
    build_graph_v2: Option<BuildGraphV2>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildCacheKey {
    source_hash: u64,
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    emit_llvm: bool,
    runtime_c: Option<String>,
    output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FunctionFingerprint {
    symbol: String,
    #[serde(default)]
    abi_hash: u64,
    #[serde(default)]
    body_hash: u64,
    #[serde(default)]
    calls: Vec<String>,
    #[serde(default)]
    module_imports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionSignatureInfo {
    symbol: String,
    signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BuildGraphNodeV2 {
    module_path: String,
    #[serde(default)]
    interface_hash: u64,
    implementation_hash: u64,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    object_path: Option<String>,
    #[serde(default)]
    functions: Vec<FunctionFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BuildGraphV2 {
    #[serde(default = "default_build_graph_schema_version")]
    schema_version: u32,
    root_module: String,
    #[serde(default)]
    nodes: Vec<BuildGraphNodeV2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EditClass {
    Noop,
    ImplOnly,
    InterfaceChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IncrementalLinkMode {
    Auto,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkerMode {
    Auto,
    Lld,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedNativeRecoveryPlan {
    RelinkFromObject,
    RebuildObjectFromCachedIr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BuildWorksetPlan {
    ReusePreviousArtifacts,
    RebuildImpactedRoot,
    FullRebuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleChangeKind {
    ImplOnly,
    Interface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionChangeKind {
    ImplOnly,
    Interface,
}

#[derive(Debug, Clone, Copy, Default)]
struct FrontendSchedulerPhaseStats {
    task_count: u32,
    queue_wait_total_ms: f64,
    queue_wait_max_ms: f64,
    worker_busy_ms: f64,
    wall_ms: f64,
    worker_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditImpact {
    class: EditClass,
    changed_modules: Vec<String>,
    impacted_modules: Vec<String>,
    changed_functions: Vec<String>,
    impacted_functions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CodegenWorksetManifest {
    #[serde(default = "default_build_graph_schema_version")]
    schema_version: u32,
    root_module: String,
    plan: BuildWorksetPlan,
    #[serde(default)]
    edit_class: Option<EditClass>,
    #[serde(default)]
    changed_modules: Vec<String>,
    #[serde(default)]
    impacted_modules: Vec<String>,
    #[serde(default)]
    changed_symbols: Vec<String>,
    #[serde(default)]
    impacted_symbols: Vec<String>,
    #[serde(default)]
    rebuild_modules: Vec<String>,
    #[serde(default)]
    reuse_modules: Vec<String>,
    #[serde(default)]
    rebuild_symbols: Vec<String>,
    #[serde(default)]
    reuse_symbols: Vec<String>,
}

