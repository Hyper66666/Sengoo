use miette::{IntoDiagnostic, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    canonical_or_lossy, module_invalidation_stats, BuildCacheKey, BuildCacheMetadata,
    BuildGraphV2, BuildWorksetPlan, CodegenWorksetManifest, EditClass, EditImpact,
    ModuleFingerprint, RunCacheKey, RunCacheMetadata, RunEngine, BUILD_GRAPH_SCHEMA_VERSION,
};

pub(crate) fn can_use_incremental_link_with_metadata(
    previous: &BuildCacheMetadata,
    llvm_ir_hash: u64,
    object_path: &Path,
    output_path: &str,
    runtime_c: Option<&str>,
    opt_level: u8,
    graph_v2: &BuildGraphV2,
) -> std::result::Result<(), String> {
    if previous.cache_schema_version != BUILD_GRAPH_SCHEMA_VERSION {
        return Err("cache schema version changed".to_string());
    }
    if previous.emit_llvm {
        return Err("previous artifact is LLVM-only".to_string());
    }
    if previous.opt_level != opt_level {
        return Err("optimization level changed".to_string());
    }
    if previous.output_path != output_path {
        return Err("output path changed".to_string());
    }
    if previous.runtime_c.as_deref() != runtime_c {
        return Err("runtime linkage input changed".to_string());
    }
    if previous.llvm_ir_hash != llvm_ir_hash {
        return Err("LLVM IR changed".to_string());
    }
    let Some(prev_object) = previous.object_path.as_deref() else {
        return Err("previous object path missing".to_string());
    };
    if !Path::new(prev_object).exists() {
        return Err("previous object artifact missing".to_string());
    }
    if canonical_or_lossy(Path::new(prev_object)) != canonical_or_lossy(object_path) {
        return Err("object path changed".to_string());
    }
    if previous.build_graph_v2.as_ref() != Some(graph_v2) {
        return Err("build graph changed".to_string());
    }
    Ok(())
}

pub(crate) fn can_use_incremental_link_with_run_metadata(
    previous: &RunCacheMetadata,
    llvm_ir_hash: u64,
    object_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    graph_v2: &BuildGraphV2,
) -> std::result::Result<(), String> {
    if previous.opt_level != opt_level {
        return Err("optimization level changed".to_string());
    }
    if previous.requested_engine != requested_engine || previous.resolved_engine != resolved_engine
    {
        return Err("engine selection changed".to_string());
    }
    if previous.runtime_c.as_deref() != runtime_c {
        return Err("runtime linkage input changed".to_string());
    }
    if previous.llvm_ir_hash != llvm_ir_hash {
        return Err("LLVM IR changed".to_string());
    }
    let Some(prev_object) = previous.object_path.as_deref() else {
        return Err("previous object path missing".to_string());
    };
    if !Path::new(prev_object).exists() {
        return Err("previous object artifact missing".to_string());
    }
    if canonical_or_lossy(Path::new(prev_object)) != canonical_or_lossy(object_path) {
        return Err("object path changed".to_string());
    }
    if previous.build_graph_v2.as_ref() != Some(graph_v2) {
        return Err("build graph changed".to_string());
    }
    Ok(())
}

pub(crate) fn resolve_engine(
    requested: RunEngine,
    has_clang: bool,
    has_lli: bool,
) -> Result<RunEngine> {
    match requested {
        RunEngine::Auto => {
            if has_clang {
                Ok(RunEngine::Native)
            } else if has_lli {
                Ok(RunEngine::Lli)
            } else {
                Err(miette::miette!(
                    "unable to run: neither clang (native) nor lli (JIT) was found"
                ))
            }
        }
        RunEngine::Native => {
            if has_clang {
                Ok(RunEngine::Native)
            } else {
                Err(miette::miette!("compile failed"))
            }
        }
        RunEngine::Lli => {
            if has_lli {
                Ok(RunEngine::Lli)
            } else {
                Err(miette::miette!("compile failed"))
            }
        }
    }
}

pub(crate) fn cache_key(
    source_hash: u64,
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    runtime_c: Option<String>,
) -> RunCacheKey {
    RunCacheKey {
        source_hash,
        module_fingerprints,
        opt_level,
        requested_engine,
        resolved_engine,
        runtime_c,
    }
}

pub(crate) fn build_cache_key(
    source_hash: u64,
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    emit_llvm: bool,
    runtime_c: Option<String>,
    output_path: String,
) -> BuildCacheKey {
    BuildCacheKey {
        source_hash,
        module_fingerprints,
        opt_level,
        emit_llvm,
        runtime_c,
        output_path,
    }
}

pub(crate) fn metadata_matches(metadata: &RunCacheMetadata, key: &RunCacheKey) -> bool {
    metadata.source_hash == key.source_hash
        && metadata.module_fingerprints == key.module_fingerprints
        && metadata.opt_level == key.opt_level
        && metadata.requested_engine == key.requested_engine
        && metadata.resolved_engine == key.resolved_engine
        && metadata.runtime_c == key.runtime_c
}

pub(crate) fn build_metadata_matches(
    metadata: &BuildCacheMetadata,
    key: &BuildCacheKey,
) -> bool {
    metadata.cache_schema_version == BUILD_GRAPH_SCHEMA_VERSION
        && metadata.source_hash == key.source_hash
        && metadata.module_fingerprints == key.module_fingerprints
        && metadata.opt_level == key.opt_level
        && metadata.emit_llvm == key.emit_llvm
        && metadata.runtime_c == key.runtime_c
        && metadata.output_path == key.output_path
}

pub(crate) fn build_cache_mismatch_reasons(
    metadata: &BuildCacheMetadata,
    key: &BuildCacheKey,
) -> Vec<String> {
    let mut reasons = Vec::new();

    if metadata.cache_schema_version != BUILD_GRAPH_SCHEMA_VERSION {
        reasons.push(format!(
            "cache schema version changed ({} -> {})",
            metadata.cache_schema_version, BUILD_GRAPH_SCHEMA_VERSION
        ));
    }
    if metadata.source_hash != key.source_hash {
        reasons.push("source changed".to_string());
    }
    if metadata.module_fingerprints != key.module_fingerprints {
        let stats =
            module_invalidation_stats(&metadata.module_fingerprints, &key.module_fingerprints);
        if stats.interface_changed_modules > 0 {
            reasons.push(format!(
                "module interfaces changed ({} module(s))",
                stats.interface_changed_modules
            ));
        }
        if stats.implementation_only_changed_modules > 0 {
            reasons.push(format!(
                "module implementations changed ({} module(s))",
                stats.implementation_only_changed_modules
            ));
        }
    }
    if metadata.opt_level != key.opt_level {
        reasons.push(format!(
            "optimization level changed ({} -> {})",
            metadata.opt_level, key.opt_level
        ));
    }
    if metadata.emit_llvm != key.emit_llvm {
        reasons.push(format!(
            "emit mode changed (emit_llvm {} -> {})",
            metadata.emit_llvm, key.emit_llvm
        ));
    }
    if metadata.runtime_c != key.runtime_c {
        reasons.push("runtime path changed".to_string());
    }
    if metadata.output_path != key.output_path {
        reasons.push("output path changed".to_string());
    }

    if reasons.is_empty() {
        reasons.push("build cache metadata mismatch".to_string());
    }
    reasons
}

pub(crate) fn derive_build_workset_plan(
    previous: Option<&BuildCacheMetadata>,
    impact: Option<&EditImpact>,
    root_module: &str,
    emit_llvm: bool,
    opt_level: u8,
    output_path: &str,
    runtime_c: Option<&str>,
) -> BuildWorksetPlan {
    let Some(previous) = previous else {
        return BuildWorksetPlan::FullRebuild;
    };
    if previous.cache_schema_version != BUILD_GRAPH_SCHEMA_VERSION {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.emit_llvm != emit_llvm {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.opt_level != opt_level {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.output_path != output_path {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.runtime_c.as_deref() != runtime_c {
        return BuildWorksetPlan::FullRebuild;
    }

    derive_workset_plan_from_impact(impact, root_module)
}

pub(crate) fn derive_run_workset_plan(
    previous: Option<&RunCacheMetadata>,
    impact: Option<&EditImpact>,
    root_module: &str,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    runtime_c: Option<&str>,
) -> BuildWorksetPlan {
    let Some(previous) = previous else {
        return BuildWorksetPlan::FullRebuild;
    };
    if previous.opt_level != opt_level {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.requested_engine != requested_engine || previous.resolved_engine != resolved_engine
    {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.runtime_c.as_deref() != runtime_c {
        return BuildWorksetPlan::FullRebuild;
    }

    derive_workset_plan_from_impact(impact, root_module)
}

fn derive_workset_plan_from_impact(
    impact: Option<&EditImpact>,
    root_module: &str,
) -> BuildWorksetPlan {
    let Some(impact) = impact else {
        return BuildWorksetPlan::FullRebuild;
    };
    match impact.class {
        EditClass::Noop => BuildWorksetPlan::ReusePreviousArtifacts,
        EditClass::InterfaceChange => BuildWorksetPlan::FullRebuild,
        EditClass::ImplOnly => {
            let touches_root = impact
                .changed_modules
                .iter()
                .chain(impact.impacted_modules.iter())
                .any(|module| module == root_module);
            if touches_root {
                BuildWorksetPlan::RebuildImpactedRoot
            } else {
                BuildWorksetPlan::ReusePreviousArtifacts
            }
        }
    }
}

pub(crate) fn derive_codegen_workset_manifest(
    graph: &BuildGraphV2,
    impact: Option<&EditImpact>,
    plan: BuildWorksetPlan,
) -> CodegenWorksetManifest {
    let mut all_modules = graph
        .nodes
        .iter()
        .map(|node| node.module_path.clone())
        .collect::<Vec<_>>();
    all_modules.push(graph.root_module.clone());
    all_modules.sort();
    all_modules.dedup();

    let mut changed_modules = impact
        .map(|edit| edit.changed_modules.clone())
        .unwrap_or_default();
    changed_modules.sort();
    changed_modules.dedup();

    let mut impacted_modules = impact
        .map(|edit| edit.impacted_modules.clone())
        .unwrap_or_default();
    impacted_modules.sort();
    impacted_modules.dedup();

    let mut changed_symbols = impact
        .map(|edit| edit.changed_functions.clone())
        .unwrap_or_default();
    changed_symbols.sort();
    changed_symbols.dedup();

    let mut impacted_symbols = impact
        .map(|edit| edit.impacted_functions.clone())
        .unwrap_or_default();
    impacted_symbols.sort();
    impacted_symbols.dedup();

    let mut all_symbols = graph
        .nodes
        .iter()
        .flat_map(|node| {
            node.functions
                .iter()
                .map(|function| function.symbol.clone())
        })
        .collect::<Vec<_>>();
    all_symbols.sort();
    all_symbols.dedup();

    let mut rebuild_modules = match plan {
        BuildWorksetPlan::ReusePreviousArtifacts => Vec::new(),
        BuildWorksetPlan::RebuildImpactedRoot => {
            if impacted_modules.is_empty() {
                vec![graph.root_module.clone()]
            } else {
                impacted_modules.clone()
            }
        }
        BuildWorksetPlan::FullRebuild => all_modules.clone(),
    };
    rebuild_modules.sort();
    rebuild_modules.dedup();

    let rebuild_set = rebuild_modules.iter().cloned().collect::<HashSet<_>>();
    let reuse_modules = all_modules
        .iter()
        .filter(|module| !rebuild_set.contains(*module))
        .cloned()
        .collect::<Vec<_>>();

    let mut rebuild_symbols = match plan {
        BuildWorksetPlan::ReusePreviousArtifacts => Vec::new(),
        BuildWorksetPlan::RebuildImpactedRoot => {
            if impacted_symbols.is_empty() {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.module_path == graph.root_module)
                    .map(|node| {
                        node.functions
                            .iter()
                            .map(|function| function.symbol.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            } else {
                impacted_symbols.clone()
            }
        }
        BuildWorksetPlan::FullRebuild => all_symbols.clone(),
    };
    rebuild_symbols.sort();
    rebuild_symbols.dedup();

    let rebuild_symbol_set = rebuild_symbols.iter().cloned().collect::<HashSet<_>>();
    let reuse_symbols = all_symbols
        .iter()
        .filter(|symbol| !rebuild_symbol_set.contains(*symbol))
        .cloned()
        .collect::<Vec<_>>();

    CodegenWorksetManifest {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: graph.root_module.clone(),
        plan,
        edit_class: impact.map(|edit| edit.class),
        changed_modules,
        impacted_modules,
        changed_symbols,
        impacted_symbols,
        rebuild_modules,
        reuse_modules,
        rebuild_symbols,
        reuse_symbols,
    }
}

pub(crate) fn codegen_workset_manifest_path(
    build_dir: &Path,
    stem: &str,
    command_kind: &str,
) -> PathBuf {
    build_dir
        .join("workset")
        .join(format!("{}.{}.workset.json", stem, command_kind))
}

pub(crate) fn save_codegen_workset_manifest(
    path: &Path,
    manifest: &CodegenWorksetManifest,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|e| miette::miette!("failed to serialize workset manifest: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write workset manifest: {}", e))
}

pub(crate) fn cache_mismatch_reasons(metadata: &RunCacheMetadata, key: &RunCacheKey) -> Vec<String> {
    let mut reasons = Vec::new();

    if metadata.source_hash != key.source_hash {
        reasons.push("source changed".to_string());
    }
    if metadata.module_fingerprints != key.module_fingerprints {
        let stats =
            module_invalidation_stats(&metadata.module_fingerprints, &key.module_fingerprints);
        if stats.interface_changed_modules > 0 {
            reasons.push(format!(
                "module interfaces changed ({} module(s))",
                stats.interface_changed_modules
            ));
        }
        if stats.implementation_only_changed_modules > 0 {
            reasons.push(format!(
                "module implementations changed ({} module(s))",
                stats.implementation_only_changed_modules
            ));
        }
    }
    if metadata.opt_level != key.opt_level {
        reasons.push(format!(
            "optimization level changed ({} -> {})",
            metadata.opt_level, key.opt_level
        ));
    }
    if metadata.requested_engine != key.requested_engine {
        reasons.push(format!(
            "requested engine changed ({:?} -> {:?})",
            metadata.requested_engine, key.requested_engine
        ));
    }
    if metadata.resolved_engine != key.resolved_engine {
        reasons.push(format!(
            "resolved engine changed ({:?} -> {:?})",
            metadata.resolved_engine, key.resolved_engine
        ));
    }
    if metadata.runtime_c != key.runtime_c {
        reasons.push("runtime path changed".to_string());
    }

    if reasons.is_empty() {
        reasons.push("cache metadata mismatch".to_string());
    }

    reasons
}
