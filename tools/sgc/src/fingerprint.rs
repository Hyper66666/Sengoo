use miette::{IntoDiagnostic, Result};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufReader as StdBufReader, Read};
use std::path::Path;

use crate::ast_interface_signature;

pub(crate) fn source_fingerprint(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn normalize_source_for_hash(source: &str) -> String {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn implementation_fingerprint_from_normalized(normalized: &str) -> u64 {
    source_fingerprint(normalized)
}

pub(crate) fn file_fingerprint(path: &Path) -> Result<u64> {
    let file = fs::File::open(path).into_diagnostic().map_err(|e| {
        miette::miette!(
            "failed to read file for fingerprint {}: {}",
            path.display(),
            e
        )
    })?;
    let mut reader = StdBufReader::new(file);
    let mut hasher = DefaultHasher::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer).into_diagnostic().map_err(|e| {
            miette::miette!(
                "failed to stream file for fingerprint {}: {}",
                path.display(),
                e
            )
        })?;
        if read == 0 {
            break;
        }
        buffer[..read].hash(&mut hasher);
    }
    Ok(hasher.finish())
}

pub(crate) fn implementation_fingerprint(source: &str) -> u64 {
    let normalized = normalize_source_for_hash(source);
    implementation_fingerprint_from_normalized(&normalized)
}

pub(crate) fn interface_fingerprint(source: &str) -> u64 {
    if let Some(interface_repr) = ast_interface_signature(source) {
        return source_fingerprint(&interface_repr);
    }

    interface_fingerprint_fast(source)
}

pub(crate) fn interface_fingerprint_fast(source: &str) -> u64 {
    let normalized = normalize_source_for_hash(source);
    interface_fingerprint_fast_from_normalized(&normalized)
}

pub(crate) fn interface_fingerprint_fast_from_normalized(normalized: &str) -> u64 {
    fn line_starts_with_any(line: &str, prefixes: &[&str]) -> bool {
        prefixes.iter().any(|prefix| line.starts_with(prefix))
    }

    fn strip_inline_block_signature(line: &str) -> &str {
        line.split_once('{')
            .map(|(head, _)| head.trim_end())
            .unwrap_or(line)
    }

    let import_prefixes = ["import ", "pub import "];
    let function_prefixes = ["def ", "pub def ", "async def ", "pub async def "];
    let impl_prefixes = ["impl ", "pub impl "];
    let type_prefixes = [
        "struct ",
        "pub struct ",
        "enum ",
        "pub enum ",
        "trait ",
        "pub trait ",
        "class ",
        "pub class ",
        "type ",
        "pub type ",
    ];

    let mut fallback_repr = String::new();
    for line in normalized.lines() {
        let trimmed = line.trim_start();

        if line_starts_with_any(trimmed, &import_prefixes) {
            fallback_repr.push_str(trimmed);
            fallback_repr.push('\n');
            continue;
        }

        if line_starts_with_any(trimmed, &function_prefixes) {
            // Keep declaration signature only so body-only edits do not look like interface drift.
            fallback_repr.push_str(strip_inline_block_signature(trimmed));
            fallback_repr.push('\n');
            continue;
        }

        if line_starts_with_any(trimmed, &impl_prefixes) {
            // Impl block body is implementation detail for coarse fallback hashing.
            fallback_repr.push_str(strip_inline_block_signature(trimmed));
            fallback_repr.push('\n');
            continue;
        }

        if line_starts_with_any(trimmed, &type_prefixes) {
            fallback_repr.push_str(trimmed);
            fallback_repr.push('\n');
        }
    }
    source_fingerprint(&fallback_repr)
}

fn resolve_root_interface_hash(
    source: &str,
    root_implementation_hash: u64,
    previous_root_implementation_hash: Option<u64>,
    previous_root_interface_hash: Option<u64>,
) -> u64 {
    if previous_root_implementation_hash == Some(root_implementation_hash) {
        if let Some(previous_interface_hash) = previous_root_interface_hash {
            return previous_interface_hash;
        }
    }
    interface_fingerprint(source)
}

pub(crate) fn resolve_root_hashes_for_request(
    source: &str,
    previous_root_implementation_hash: Option<u64>,
    previous_root_interface_hash: Option<u64>,
) -> (u64, u64) {
    let root_implementation_hash = implementation_fingerprint(source);
    let root_interface_hash = resolve_root_interface_hash(
        source,
        root_implementation_hash,
        previous_root_implementation_hash,
        previous_root_interface_hash,
    );

    (root_interface_hash, root_implementation_hash)
}
