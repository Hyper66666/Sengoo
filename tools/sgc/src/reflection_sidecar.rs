use miette::{IntoDiagnostic, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    function_signatures_for_module, BuildGraphV2, ReflectionCliOptions, ReflectionMetadata,
    ReflectionModuleMetadata, ReflectionSymbolMetadata, REFLECTION_SCHEMA_VERSION,
};

pub(crate) fn reflection_sidecar_path_for_artifact(artifact_path: &Path) -> PathBuf {
    PathBuf::from(format!(
        "{}.sgreflect.json",
        artifact_path.to_string_lossy()
    ))
}

fn llvm_defined_function_names(llvm_ir: &str) -> HashSet<String> {
    let mut symbols = HashSet::new();
    for line in llvm_ir.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("define ") {
            continue;
        }
        let Some(at_index) = trimmed.find('@') else {
            continue;
        };
        let after_at = &trimmed[at_index + 1..];
        let Some(paren_index) = after_at.find('(') else {
            continue;
        };
        let mut symbol = after_at[..paren_index].trim().to_string();
        if let Some(unquoted) = symbol
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
        {
            symbol = unquoted.to_string();
        }
        if !symbol.is_empty() {
            symbols.insert(symbol);
        }
    }
    symbols
}

fn read_llvm_defined_function_names(path: &Path) -> Result<HashSet<String>> {
    let llvm_ir = fs::read_to_string(path).into_diagnostic().map_err(|e| {
        miette::miette!(
            "failed to read LLVM IR for reflection metadata {}: {}",
            path.to_string_lossy(),
            e
        )
    })?;
    Ok(llvm_defined_function_names(&llvm_ir))
}

pub(crate) fn validate_reflection_metadata(metadata: &ReflectionMetadata) -> Result<()> {
    if metadata.schema_version != REFLECTION_SCHEMA_VERSION {
        return Err(miette::miette!(
            "reflection metadata schema mismatch: expected {} got {}",
            REFLECTION_SCHEMA_VERSION,
            metadata.schema_version
        ));
    }
    if metadata.compiler_version.trim().is_empty() {
        return Err(miette::miette!(
            "reflection metadata missing compiler_version"
        ));
    }
    if metadata.compatible_compiler_versions.is_empty() {
        return Err(miette::miette!(
            "reflection metadata missing compatible_compiler_versions"
        ));
    }
    if metadata
        .compatible_compiler_versions
        .iter()
        .any(|version| version.trim().is_empty())
    {
        return Err(miette::miette!(
            "reflection metadata contains empty compatible compiler version"
        ));
    }
    if metadata.root_module.trim().is_empty() {
        return Err(miette::miette!("reflection metadata missing root_module"));
    }

    let mut module_ids = HashSet::<String>::new();
    for module in &metadata.modules {
        if module.module_id.trim().is_empty() {
            return Err(miette::miette!(
                "reflection metadata contains empty module id"
            ));
        }
        if !module_ids.insert(module.module_id.clone()) {
            return Err(miette::miette!(
                "reflection metadata contains duplicate module id: {}",
                module.module_id
            ));
        }

        let mut symbol_ids = HashSet::<String>::new();
        for symbol in &module.symbols {
            if symbol.symbol.trim().is_empty() {
                return Err(miette::miette!(
                    "reflection metadata contains empty symbol in module {}",
                    module.module_id
                ));
            }
            if symbol.signature.trim().is_empty() {
                return Err(miette::miette!(
                    "reflection metadata contains empty signature for symbol {}",
                    symbol.symbol
                ));
            }
            if let Some(native_symbol) = &symbol.native_symbol {
                if native_symbol.trim().is_empty() {
                    return Err(miette::miette!(
                        "reflection metadata contains empty native symbol for {}",
                        symbol.symbol
                    ));
                }
            }
            if !symbol
                .symbol
                .starts_with(&(module.module_id.clone() + "::"))
            {
                return Err(miette::miette!(
                    "reflection symbol {} does not belong to module {}",
                    symbol.symbol,
                    module.module_id
                ));
            }
            if !symbol_ids.insert(symbol.symbol.clone()) {
                return Err(miette::miette!(
                    "reflection metadata contains duplicate symbol {} in module {}",
                    symbol.symbol,
                    module.module_id
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn build_reflection_metadata(
    graph_v2: &BuildGraphV2,
    reflection: &ReflectionCliOptions,
    llvm_defined_symbols: Option<&HashSet<String>>,
) -> Result<Option<ReflectionMetadata>> {
    if !reflection.enabled {
        return Ok(None);
    }

    let available_modules = graph_v2
        .nodes
        .iter()
        .map(|node| node.module_path.clone())
        .collect::<HashSet<_>>();
    let mut selected_modules = if !reflection.modules.is_empty() {
        reflection.modules.clone()
    } else if !reflection.symbols.is_empty() {
        available_modules.iter().cloned().collect::<Vec<_>>()
    } else {
        vec![graph_v2.root_module.clone()]
    };
    selected_modules.sort();
    selected_modules.dedup();

    for module in &selected_modules {
        if !available_modules.contains(module) {
            return Err(miette::miette!(
                "reflection module not found in build graph: {}",
                module
            ));
        }
    }

    let mut selected_full_symbols = HashSet::<String>::new();
    let mut selected_short_symbols = HashSet::<String>::new();
    for selector in &reflection.symbols {
        if selector.contains("::") {
            selected_full_symbols.insert(selector.clone());
        } else {
            selected_short_symbols.insert(selector.clone());
        }
    }
    let filter_by_symbol = !selected_full_symbols.is_empty() || !selected_short_symbols.is_empty();
    let mut unresolved_full_symbols = selected_full_symbols.clone();
    let mut unresolved_short_symbols = selected_short_symbols.clone();

    let mut modules = Vec::new();
    for module in selected_modules {
        let source = fs::read_to_string(&module).into_diagnostic().map_err(|e| {
            miette::miette!(
                "failed to read module for reflection metadata {}: {}",
                module,
                e
            )
        })?;
        let mut signatures = function_signatures_for_module(&module, &source)
            .into_iter()
            .map(|entry| ReflectionSymbolMetadata {
                symbol: entry.symbol,
                signature: entry.signature,
                native_symbol: None,
            })
            .collect::<Vec<_>>();

        if filter_by_symbol {
            signatures.retain(|entry| {
                let mut matched = false;
                if selected_full_symbols.contains(&entry.symbol) {
                    matched = true;
                }
                let short = entry.symbol.rsplit("::").next().unwrap_or_default();
                if selected_short_symbols.contains(short) {
                    matched = true;
                }
                matched
            });
        }
        signatures.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        signatures.dedup_by(|a, b| a.symbol == b.symbol);

        if let Some(llvm_defined_symbols) = llvm_defined_symbols {
            let mut short_counts = HashMap::<String, usize>::new();
            for entry in &signatures {
                let short = entry
                    .symbol
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .to_string();
                *short_counts.entry(short).or_insert(0) += 1;
            }

            let mut filtered = Vec::new();
            for mut entry in signatures {
                let short = entry
                    .symbol
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let explicitly_selected = selected_full_symbols.contains(&entry.symbol)
                    || selected_short_symbols.contains(&short);

                if short_counts.get(&short).copied().unwrap_or_default() > 1 {
                    if explicitly_selected {
                        return Err(miette::miette!(
                            "reflection symbol {} has ambiguous native binding name {}",
                            entry.symbol,
                            short
                        ));
                    }
                    continue;
                }

                if llvm_defined_symbols.contains(&short) {
                    entry.native_symbol = Some(short.clone());
                    unresolved_full_symbols.remove(&entry.symbol);
                    unresolved_short_symbols.remove(&short);
                    filtered.push(entry);
                } else if explicitly_selected {
                    return Err(miette::miette!(
                        "reflection symbol {} is not emitted in LLVM IR (native symbol: {})",
                        entry.symbol,
                        short
                    ));
                }
            }
            signatures = filtered;
        } else {
            for entry in &signatures {
                unresolved_full_symbols.remove(&entry.symbol);
                let short = entry.symbol.rsplit("::").next().unwrap_or_default();
                unresolved_short_symbols.remove(short);
            }
        }

        if !filter_by_symbol || !signatures.is_empty() {
            modules.push(ReflectionModuleMetadata {
                module_id: module,
                symbols: signatures,
            });
        }
    }

    if !unresolved_full_symbols.is_empty() || !unresolved_short_symbols.is_empty() {
        let mut unresolved = unresolved_full_symbols
            .into_iter()
            .chain(unresolved_short_symbols)
            .collect::<Vec<_>>();
        unresolved.sort();
        return Err(miette::miette!(
            "reflection symbol(s) not found in selected modules: {}",
            unresolved.join(", ")
        ));
    }

    modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));

    let metadata = ReflectionMetadata {
        schema_version: REFLECTION_SCHEMA_VERSION,
        compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        compatible_compiler_versions: vec![env!("CARGO_PKG_VERSION").to_string()],
        root_module: graph_v2.root_module.clone(),
        modules,
    };
    validate_reflection_metadata(&metadata)?;
    Ok(Some(metadata))
}

pub(crate) fn maybe_emit_reflection_sidecar(
    artifact_path: &Path,
    graph_v2: &BuildGraphV2,
    reflection: &ReflectionCliOptions,
    llvm_ir_path: Option<&Path>,
) -> Result<()> {
    let sidecar_path = reflection_sidecar_path_for_artifact(artifact_path);
    if !reflection.enabled {
        if sidecar_path.exists() {
            fs::remove_file(&sidecar_path).into_diagnostic().map_err(|e| {
                miette::miette!(
                    "failed to remove stale reflection metadata {}: {}",
                    sidecar_path.to_string_lossy(),
                    e
                )
            })?;
        }
        return Ok(());
    }

    let llvm_defined_symbols = if let Some(llvm_ir_path) = llvm_ir_path {
        Some(read_llvm_defined_function_names(llvm_ir_path)?)
    } else {
        None
    };

    let Some(metadata) = build_reflection_metadata(graph_v2, reflection, llvm_defined_symbols.as_ref())? else {
        return Ok(());
    };
    let bytes = serde_json::to_vec_pretty(&metadata)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to serialize reflection metadata sidecar: {}", e))?;
    fs::write(&sidecar_path, bytes).into_diagnostic().map_err(|e| {
        miette::miette!(
            "failed to write reflection metadata sidecar {}: {}",
            sidecar_path.to_string_lossy(),
            e
        )
    })?;
    println!("Reflection metadata: {}", sidecar_path.to_string_lossy());
    Ok(())
}
