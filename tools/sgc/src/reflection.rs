use sengoo_compiler::{Decl, DeclKind, Import, ImportKind, Path as AstPath};
use std::path::Path;

use crate::{canonical_or_lossy, ModuleGraphSnapshot, ReflectionCliOptions, ReflectionMode};
#[cfg(test)]
use sengoo_compiler::Parser;

pub(crate) fn reflection_options_from_cli(
    mode: ReflectionMode,
    modules: &[String],
    symbols: &[String],
) -> ReflectionCliOptions {
    let mut normalized_modules = modules
        .iter()
        .map(|module| canonical_or_lossy(Path::new(module)))
        .collect::<Vec<_>>();
    normalized_modules.sort();
    normalized_modules.dedup();

    let mut normalized_symbols = symbols
        .iter()
        .map(|symbol| normalize_reflection_symbol_selector(symbol))
        .filter(|symbol| !symbol.is_empty())
        .collect::<Vec<_>>();
    normalized_symbols.sort();
    normalized_symbols.dedup();

    ReflectionCliOptions {
        mode,
        enabled: matches!(mode, ReflectionMode::On),
        modules: normalized_modules,
        symbols: normalized_symbols,
    }
}

fn normalize_reflection_symbol_selector(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(index) = trimmed.find(".sg::") {
        let module_end = index + 3;
        let suffix_start = index + 5;
        if module_end <= trimmed.len() && suffix_start <= trimmed.len() {
            let module = canonical_or_lossy(Path::new(&trimmed[..module_end]));
            let suffix = &trimmed[suffix_start..];
            if !suffix.trim().is_empty() {
                return format!("{}::{}", module, suffix);
            }
        }
    }

    trimmed.to_string()
}

fn import_path_segments_lower(path: &AstPath) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.name.trim().to_ascii_lowercase())
        .collect::<Vec<_>>()
}

fn import_decl_requests_reflection(import_decl: &Import) -> bool {
    let segments = import_path_segments_lower(&import_decl.path);
    if segments.is_empty() {
        return false;
    }

    if segments.len() == 1 && segments[0] == "reflect" {
        return true;
    }

    if segments.len() >= 2
        && (segments[0] == "std" || segments[0] == "sengoo")
        && segments[1] == "reflect"
    {
        return true;
    }

    if segments.len() == 1
        && (segments[0] == "std" || segments[0] == "sengoo")
        && matches!(&import_decl.kind, ImportKind::Selective(names) if names
            .iter()
            .any(|name| name.name.eq_ignore_ascii_case("reflect")))
    {
        return true;
    }

    false
}

pub(crate) fn decl_requests_reflection(decl: &Decl) -> bool {
    match &decl.kind {
        DeclKind::Import(import_decl) => import_decl_requests_reflection(import_decl),
        DeclKind::Module(module_decl) => module_decl.items.iter().any(decl_requests_reflection),
        _ => false,
    }
}

#[cfg(test)]
pub(crate) fn source_requests_reflection(source: &str) -> bool {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return false,
    };
    program.decls.iter().any(decl_requests_reflection)
}

pub(crate) fn resolve_reflection_options_for_snapshot(
    mut reflection: ReflectionCliOptions,
    snapshot: &ModuleGraphSnapshot,
) -> ReflectionCliOptions {
    reflection.enabled = match reflection.mode {
        ReflectionMode::On => true,
        ReflectionMode::Off => false,
        ReflectionMode::Auto => {
            if !reflection.modules.is_empty() || !reflection.symbols.is_empty() {
                true
            } else {
                !snapshot.reflection_import_modules.is_empty()
            }
        }
    };
    reflection
}

pub(crate) fn reflection_mode_note(
    reflection: &ReflectionCliOptions,
    snapshot: &ModuleGraphSnapshot,
) -> String {
    match reflection.mode {
        ReflectionMode::On => "reflection: forced on (--reflect=on)".to_string(),
        ReflectionMode::Off => "reflection: forced off (--reflect=off)".to_string(),
        ReflectionMode::Auto => {
            if !reflection.enabled {
                return "reflection: auto disabled (no reflect import detected)".to_string();
            }
            if !reflection.modules.is_empty() || !reflection.symbols.is_empty() {
                return "reflection: auto enabled by explicit selector filters".to_string();
            }
            if snapshot.reflection_import_modules.len() == 1 {
                return format!(
                    "reflection: auto enabled by import in {}",
                    snapshot.reflection_import_modules[0]
                );
            }
            format!(
                "reflection: auto enabled by imports in {} module(s)",
                snapshot.reflection_import_modules.len()
            )
        }
    }
}
