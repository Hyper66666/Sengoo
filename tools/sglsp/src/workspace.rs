use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{
    GotoDefinitionResponse, InitializeParams, Location, Position, SymbolInformation, TextEdit, Url,
    WorkspaceEdit,
};

use super::symbols::{
    collect_ast_symbols, completion_kind_to_symbol_kind, extract_identifier_at,
    find_declaration_in_text, find_definition_in_text, find_symbol_occurrences,
    valid_identifier_name,
};

pub(crate) fn workspace_symbols_for_documents(
    query: &str,
    documents: &HashMap<Url, String>,
) -> Vec<SymbolInformation> {
    let query = query.trim().to_ascii_lowercase();
    let mut items = Vec::new();
    let mut sorted_docs = documents.iter().collect::<Vec<_>>();
    sorted_docs.sort_by(|(left_uri, _), (right_uri, _)| left_uri.as_str().cmp(right_uri.as_str()));

    for (uri, content) in sorted_docs {
        for symbol in collect_ast_symbols(content) {
            if !query.is_empty() && !symbol.name.to_ascii_lowercase().contains(&query) {
                continue;
            }
            #[allow(deprecated)]
            items.push(SymbolInformation {
                name: symbol.name,
                kind: completion_kind_to_symbol_kind(symbol.kind),
                tags: None,
                deprecated: None,
                location: Location::new(uri.clone(), symbol.range),
                container_name: Some(symbol.detail),
            });
        }
    }

    items
}

fn should_skip_workspace_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git" | ".hg" | ".svn" | ".sgpm" | ".cache" | "node_modules" | "target"
    )
}

fn collect_sengoo_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "sg") {
            out.push(path.to_path_buf());
        }
        return;
    }

    if should_skip_workspace_dir(path) {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut paths = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        collect_sengoo_files(&path, out);
    }
}

pub(crate) fn workspace_documents_for_roots_and_open_documents(
    roots: &[PathBuf],
    open_documents: &HashMap<Url, String>,
) -> HashMap<Url, String> {
    let mut documents = HashMap::new();
    let mut files = Vec::new();
    for root in roots {
        collect_sengoo_files(root, &mut files);
    }
    files.sort();

    for file in files {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        if let Ok(uri) = Url::from_file_path(&file) {
            documents.insert(uri, content);
        }
    }

    for (uri, content) in open_documents {
        documents.insert(uri.clone(), content.clone());
    }

    documents
}

#[cfg(test)]
pub(crate) fn workspace_symbols_for_roots_and_documents(
    query: &str,
    roots: &[PathBuf],
    open_documents: &HashMap<Url, String>,
) -> Vec<SymbolInformation> {
    let documents = workspace_documents_for_roots_and_open_documents(roots, open_documents);
    workspace_symbols_for_documents(query, &documents)
}

pub(crate) fn workspace_roots_from_initialize(params: &InitializeParams) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(folders) = &params.workspace_folders {
        for folder in folders {
            if let Ok(path) = folder.uri.to_file_path() {
                roots.push(path);
            }
        }
    }

    if roots.is_empty() {
        if let Some(root_uri) = &params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                roots.push(path);
            }
        }
    }

    roots.sort();
    roots.dedup();
    roots
}

pub(crate) fn goto_definition_in_documents(
    uri: &Url,
    position: Position,
    documents: &HashMap<Url, String>,
) -> Option<GotoDefinitionResponse> {
    let current_content = documents.get(uri)?;
    let symbol = extract_identifier_at(current_content, position)?;
    let local_symbols = collect_ast_symbols(current_content);
    if let Some(found) = local_symbols.iter().find(|item| item.name == symbol.name) {
        return Some(GotoDefinitionResponse::Scalar(Location::new(
            uri.clone(),
            found.range,
        )));
    }

    if let Some(range) = find_declaration_in_text(current_content, &symbol.name) {
        return Some(GotoDefinitionResponse::Scalar(Location::new(
            uri.clone(),
            range,
        )));
    }

    let mut sorted_docs = documents.iter().collect::<Vec<_>>();
    sorted_docs.sort_by(|(left_uri, _), (right_uri, _)| left_uri.as_str().cmp(right_uri.as_str()));

    for (doc_uri, doc_content) in sorted_docs {
        if doc_uri == uri {
            continue;
        }
        let symbols = collect_ast_symbols(doc_content);
        if let Some(found) = symbols.iter().find(|item| item.name == symbol.name) {
            return Some(GotoDefinitionResponse::Scalar(Location::new(
                doc_uri.clone(),
                found.range,
            )));
        }
        if let Some(range) = find_declaration_in_text(doc_content, &symbol.name) {
            return Some(GotoDefinitionResponse::Scalar(Location::new(
                doc_uri.clone(),
                range,
            )));
        }
    }

    if let Some(range) = find_definition_in_text(current_content, &symbol.name) {
        return Some(GotoDefinitionResponse::Scalar(Location::new(
            uri.clone(),
            range,
        )));
    }

    for (doc_uri, doc_content) in documents {
        if doc_uri == uri {
            continue;
        }
        if let Some(range) = find_definition_in_text(doc_content, &symbol.name) {
            return Some(GotoDefinitionResponse::Scalar(Location::new(
                doc_uri.clone(),
                range,
            )));
        }
    }

    None
}

pub(crate) fn references_in_documents(
    uri: &Url,
    position: Position,
    include_declaration: bool,
    documents: &HashMap<Url, String>,
) -> Option<Vec<Location>> {
    let current_content = documents.get(uri)?;
    let symbol = extract_identifier_at(current_content, position)?;

    let mut locations = Vec::new();
    let mut sorted_docs = documents.iter().collect::<Vec<_>>();
    sorted_docs.sort_by(|(left_uri, _), (right_uri, _)| left_uri.as_str().cmp(right_uri.as_str()));
    for (doc_uri, doc_content) in sorted_docs {
        for range in find_symbol_occurrences(doc_content, &symbol.name) {
            locations.push(Location::new(doc_uri.clone(), range));
        }
    }

    if !include_declaration {
        locations.retain(|loc| loc.range != symbol.range || loc.uri != *uri);
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

pub(crate) fn rename_in_documents(
    uri: &Url,
    position: Position,
    new_name: &str,
    documents: &HashMap<Url, String>,
) -> Option<WorkspaceEdit> {
    if !valid_identifier_name(new_name) {
        return None;
    }

    let current_content = documents.get(uri)?;
    let symbol = extract_identifier_at(current_content, position)?;

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    let mut sorted_docs = documents.iter().collect::<Vec<_>>();
    sorted_docs.sort_by(|(left_uri, _), (right_uri, _)| left_uri.as_str().cmp(right_uri.as_str()));
    for (doc_uri, doc_content) in sorted_docs {
        let edits = find_symbol_occurrences(doc_content, &symbol.name)
            .into_iter()
            .map(|range| TextEdit {
                range,
                new_text: new_name.to_string(),
            })
            .collect::<Vec<_>>();
        if !edits.is_empty() {
            changes.insert(doc_uri.clone(), edits);
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
}
