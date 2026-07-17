use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
#[cfg(test)]
use std::sync::{Barrier, OnceLock};

use sengoo_compiler::ast::{Decl, DeclKind, VariantField};
use sengoo_compiler::lexer::{Lexer, Token, TokenKind};
use sengoo_compiler::Parser as SgParser;
use tower_lsp::lsp_types::{
    GotoDefinitionResponse, Location, Position, SymbolInformation, TextDocumentContentChangeEvent,
    Url,
};

use crate::dependency_sources::{
    dependency_aliases_from_lockfiles, dependency_roots_for_workspace_roots,
};
use crate::protocol::SymbolOrigin;
use crate::signatures::{
    collect_function_signatures, qualify_function_signatures, FunctionSignatureInfo,
};
use crate::stdlib::{
    stdlib_definitions_for_content, stdlib_signatures_for_content, stdlib_symbols_for_content,
};
use crate::symbols::{
    collect_ast_symbols, completion_kind_to_symbol_kind, extract_identifier_at,
    find_declaration_in_text, find_definition_in_text, AstSymbol,
};
use crate::text_editing::{apply_content_changes, span_to_range};

#[derive(Debug, Clone, Default)]
pub(crate) struct IndexCancellation(Arc<AtomicBool>);

impl IndexCancellation {
    #[allow(dead_code)]
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn check(&self) -> io::Result<()> {
        if self.0.load(Ordering::Acquire) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "workspace indexing cancelled",
            ))
        } else {
            Ok(())
        }
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct IndexMetrics {
    pub(crate) recursive_scans: u64,
    pub(crate) disk_reads: u64,
    pub(crate) parsed_documents: u64,
    pub(crate) published_snapshots: u64,
    pub(crate) core_queries: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImportFactKind {
    Simple,
    Alias { alias: String },
    Selective { names: Vec<String> },
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportFact {
    pub(crate) path: String,
    pub(crate) kind: ImportFactKind,
    pub(crate) byte_start: u32,
    pub(crate) byte_end: u32,
    pub(crate) range: tower_lsp::lsp_types::Range,
    pub(crate) canonical_identity: String,
    pub(crate) fact_id: String,
    pub(crate) source_hash: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IndexedEnumVariant {
    Unit { name: String },
    Tuple { name: String, arity: usize },
    Struct { name: String, fields: Vec<String> },
}

impl IndexedEnumVariant {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Unit { name } | Self::Tuple { name, .. } | Self::Struct { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexedEnum {
    pub(crate) name: String,
    pub(crate) variants: Vec<IndexedEnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexedScope {
    pub(crate) container: Option<String>,
    pub(crate) symbol_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct IndexedCompletionCandidate {
    pub(crate) symbol: AstSymbol,
    pub(crate) definition_uri: Url,
    pub(crate) origin: SymbolOrigin,
    pub(crate) symbol_id: String,
    pub(crate) semantic_detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexFailure {
    #[allow(dead_code)]
    pub(crate) message: String,
    #[allow(dead_code)]
    pub(crate) generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexedRoot {
    pub(crate) path: PathBuf,
    pub(crate) origin: SymbolOrigin,
    pub(crate) package_name: String,
    pub(crate) import_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleIdentity {
    pub(crate) package_name: String,
    pub(crate) relative_segments: Vec<String>,
    pub(crate) import_path: String,
    pub(crate) origin: SymbolOrigin,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct EntryEpoch {
    disk: u64,
    overlay: u64,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TestPublicationKind {
    Overlay,
    Save,
    Refresh,
}

#[cfg(test)]
#[derive(Debug)]
struct TestPublicationGate {
    reached: Barrier,
    release: Barrier,
}

#[cfg(test)]
type PublicationGateMap = HashMap<(String, TestPublicationKind), Arc<TestPublicationGate>>;

#[cfg(test)]
fn publication_gates() -> &'static Mutex<PublicationGateMap> {
    static GATES: OnceLock<Mutex<PublicationGateMap>> = OnceLock::new();
    GATES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn install_publication_gate(uri: &Url, kind: TestPublicationKind) -> Arc<TestPublicationGate> {
    let gate = Arc::new(TestPublicationGate {
        reached: Barrier::new(2),
        release: Barrier::new(2),
    });
    publication_gates()
        .lock()
        .expect("publication gate lock poisoned")
        .insert((uri.to_string(), kind), Arc::clone(&gate));
    gate
}

#[cfg(test)]
fn wait_at_publication_gate(uri: &Url, kind: TestPublicationKind) {
    let gate = publication_gates()
        .lock()
        .expect("publication gate lock poisoned")
        .remove(&(uri.to_string(), kind));
    if let Some(gate) = gate {
        gate.reached.wait();
        gate.release.wait();
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IndexedDocument {
    pub(crate) uri: Url,
    pub(crate) content: Arc<str>,
    pub(crate) revision: Option<i32>,
    pub(crate) parse_valid: bool,
    pub(crate) generation: u64,
    pub(crate) origin: SymbolOrigin,
    pub(crate) symbols: Vec<AstSymbol>,
    pub(crate) members: Vec<AstSymbol>,
    pub(crate) signatures: Vec<FunctionSignatureInfo>,
    pub(crate) imports: Vec<ImportFact>,
    pub(crate) enums: Vec<IndexedEnum>,
    pub(crate) documentation: HashMap<String, String>,
    pub(crate) scopes: Vec<IndexedScope>,
    pub(crate) stdlib_symbols: Vec<AstSymbol>,
    pub(crate) stdlib_signatures: Vec<FunctionSignatureInfo>,
    pub(crate) stdlib_definitions: HashMap<String, Location>,
}

impl IndexedDocument {
    fn parse(
        uri: Url,
        text: String,
        revision: Option<i32>,
        generation: u64,
        origin: SymbolOrigin,
        last_good: Option<&IndexedDocument>,
    ) -> Self {
        let content: Arc<str> = Arc::from(text);
        let parsed = sgfmt::format_source(&content, &sgfmt::FormatOptions::default()).is_ok();
        let (
            symbols,
            members,
            signatures,
            imports,
            enums,
            documentation,
            scopes,
            stdlib_symbols,
            stdlib_signatures,
            stdlib_definitions,
        ) = match (parsed, last_good) {
            (true, _) | (false, None) => {
                let symbols = collect_ast_symbols(&content);
                let members = symbols
                    .iter()
                    .filter(|symbol| symbol.detail.contains("method"))
                    .cloned()
                    .collect();
                let mut signatures = collect_function_signatures(&content);
                signatures.sort_by_key(|signature| {
                    (signature.range.start.line, signature.range.start.character)
                });
                let imports = collect_import_facts(&content);
                let enums = collect_indexed_enums(&content);
                let documentation = collect_documentation(&content, &symbols);
                let scopes = collect_scopes(&uri, &symbols, &signatures);
                let stdlib_symbols = stdlib_symbols_for_content(&content);
                let stdlib_signatures = stdlib_signatures_for_content(&content);
                let stdlib_definitions = stdlib_definitions_for_content(&content);
                (
                    symbols,
                    members,
                    signatures,
                    imports,
                    enums,
                    documentation,
                    scopes,
                    stdlib_symbols,
                    stdlib_signatures,
                    stdlib_definitions,
                )
            }
            (false, Some(previous)) => (
                previous.symbols.clone(),
                previous.members.clone(),
                previous.signatures.clone(),
                previous.imports.clone(),
                previous.enums.clone(),
                previous.documentation.clone(),
                previous.scopes.clone(),
                previous.stdlib_symbols.clone(),
                previous.stdlib_signatures.clone(),
                previous.stdlib_definitions.clone(),
            ),
        };
        Self {
            uri,
            content,
            revision,
            parse_valid: parsed,
            generation,
            origin,
            symbols,
            members,
            signatures,
            imports,
            enums,
            documentation,
            scopes,
            stdlib_symbols,
            stdlib_signatures,
            stdlib_definitions,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceSnapshot {
    pub(crate) generation: u64,
    pub(crate) documents: HashMap<Url, Arc<IndexedDocument>>,
    disk_documents: HashMap<Url, Arc<IndexedDocument>>,
    failures: HashMap<Url, IndexFailure>,
    entry_epochs: HashMap<Url, EntryEpoch>,
    roots: Vec<IndexedRoot>,
    pub(crate) stdlib_metadata_revision: u32,
}

impl Default for WorkspaceSnapshot {
    fn default() -> Self {
        Self {
            generation: 0,
            documents: HashMap::new(),
            disk_documents: HashMap::new(),
            failures: HashMap::new(),
            entry_epochs: HashMap::new(),
            roots: Vec::new(),
            stdlib_metadata_revision: 1,
        }
    }
}

#[derive(Debug, Default)]
struct MetricCounters {
    recursive_scans: AtomicU64,
    disk_reads: AtomicU64,
    parsed_documents: AtomicU64,
    published_snapshots: AtomicU64,
    core_queries: AtomicU64,
}

#[derive(Debug, Default)]
pub(crate) struct WorkspaceIndex {
    snapshot: RwLock<Arc<WorkspaceSnapshot>>,
    metrics: MetricCounters,
    next_build_generation: AtomicU64,
    active_build: Mutex<Option<(u64, IndexCancellation)>>,
}

#[derive(Debug)]
struct IndexBuildGuard {
    generation: u64,
    cancellation: IndexCancellation,
    completed: bool,
}

#[derive(Debug)]
pub(crate) struct IndexOperationGuard {
    cancellation: IndexCancellation,
    completed: bool,
}

impl IndexOperationGuard {
    pub(crate) fn new() -> Self {
        Self {
            cancellation: IndexCancellation::default(),
            completed: false,
        }
    }

    pub(crate) fn token(&self) -> IndexCancellation {
        self.cancellation.clone()
    }

    pub(crate) fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for IndexOperationGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.cancellation.cancel();
        }
    }
}

impl IndexBuildGuard {
    fn token(&self) -> IndexCancellation {
        self.cancellation.clone()
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for IndexBuildGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.cancellation.cancel();
        }
    }
}

impl WorkspaceIndex {
    pub(crate) fn build(roots: &[PathBuf], cancellation: IndexCancellation) -> io::Result<Self> {
        cancellation.check()?;
        let index = Self::default();
        index
            .metrics
            .recursive_scans
            .fetch_add(1, Ordering::Relaxed);

        let dependency_roots = dependency_roots_for_workspace_roots(roots);
        let dependency_aliases = dependency_aliases_for_workspace_roots(roots);
        let mut search_roots = roots
            .iter()
            .cloned()
            .map(|root| (root, SymbolOrigin::Workspace))
            .collect::<Vec<_>>();
        for root in dependency_roots {
            if !search_roots.iter().any(|(known, _)| known == &root) {
                search_roots.push((root, SymbolOrigin::Dependency));
            }
        }
        search_roots.sort_by(|(left, _), (right, _)| left.cmp(right));
        let mut indexed_roots = search_roots
            .iter()
            .map(|(path, origin)| {
                let canonical = canonical_path_for_matching(path);
                let package_name = package_name_for_root(path);
                let import_prefix = dependency_aliases
                    .get(&canonical)
                    .cloned()
                    .unwrap_or_else(|| package_name.clone());
                IndexedRoot {
                    path: canonical,
                    origin: *origin,
                    package_name,
                    import_prefix,
                }
            })
            .collect::<Vec<_>>();
        indexed_roots.sort_by(|left, right| {
            right
                .path
                .components()
                .count()
                .cmp(&left.path.components().count())
                .then_with(|| left.path.cmp(&right.path))
        });
        indexed_roots.dedup_by(|left, right| left.path == right.path);

        let mut files = Vec::new();
        for (root, origin) in search_roots {
            collect_sengoo_files(&root, origin, &cancellation, &mut files)?;
        }
        files.sort_by(|(left, _), (right, _)| left.cmp(right));
        files.dedup_by(|(left, _), (right, _)| left == right);

        let mut documents = HashMap::new();
        let mut failures = HashMap::new();
        for (path, origin) in files {
            cancellation.check()?;
            index.metrics.disk_reads.fetch_add(1, Ordering::Relaxed);
            let uri = match Url::from_file_path(&path) {
                Ok(uri) => uri,
                Err(()) => continue,
            };
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) => {
                    tracing::warn!(path = %path.display(), error = %error, "failed to read Sengoo file during initial indexing");
                    failures.insert(
                        uri,
                        IndexFailure {
                            message: error.to_string(),
                            generation: 1,
                        },
                    );
                    continue;
                }
            };
            cancellation.check()?;
            index
                .metrics
                .parsed_documents
                .fetch_add(1, Ordering::Relaxed);
            let document = Arc::new(IndexedDocument::parse(
                uri.clone(),
                text,
                None,
                1,
                origin,
                None,
            ));
            documents.insert(uri, document);
        }
        let entry_epochs = documents
            .keys()
            .chain(failures.keys())
            .cloned()
            .map(|uri| {
                (
                    uri,
                    EntryEpoch {
                        disk: 1,
                        overlay: 0,
                    },
                )
            })
            .collect();
        let snapshot = WorkspaceSnapshot {
            generation: 1,
            disk_documents: documents.clone(),
            documents,
            failures,
            entry_epochs,
            roots: indexed_roots,
            stdlib_metadata_revision: 1,
        };
        *index
            .snapshot
            .write()
            .expect("workspace index lock poisoned") = Arc::new(snapshot);
        index
            .metrics
            .published_snapshots
            .fetch_add(1, Ordering::Release);
        Ok(index)
    }

    pub(crate) fn snapshot(&self) -> Arc<WorkspaceSnapshot> {
        self.snapshot
            .read()
            .expect("workspace index lock poisoned")
            .clone()
    }

    fn entry_epoch(snapshot: &WorkspaceSnapshot, uri: &Url) -> EntryEpoch {
        snapshot.entry_epochs.get(uri).copied().unwrap_or_default()
    }

    fn origin_for_uri(snapshot: &WorkspaceSnapshot, uri: &Url) -> SymbolOrigin {
        let Ok(path) = uri.to_file_path() else {
            return SymbolOrigin::Workspace;
        };
        let path = canonical_path_for_matching(&path);
        snapshot
            .roots
            .iter()
            .find(|root| path.starts_with(&root.path))
            .map_or(SymbolOrigin::Workspace, |root| root.origin)
    }

    fn begin_build(&self) -> IndexBuildGuard {
        let generation = self.next_build_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let cancellation = IndexCancellation::default();
        let mut active = self
            .active_build
            .lock()
            .expect("active build lock poisoned");
        if let Some((_, previous)) = active.replace((generation, cancellation.clone())) {
            previous.cancel();
        }
        IndexBuildGuard {
            generation,
            cancellation,
            completed: false,
        }
    }

    fn publish_rebuilt(&self, generation: u64, rebuilt: WorkspaceIndex) -> bool {
        let mut active = self
            .active_build
            .lock()
            .expect("active build lock poisoned");
        let Some((active_generation, cancellation)) = active.as_ref() else {
            return false;
        };
        if *active_generation != generation || cancellation.is_cancelled() {
            return false;
        }

        let mut rebuilt_snapshot = (*rebuilt.snapshot()).clone();
        rebuilt_snapshot.generation = self.snapshot().generation + 1;
        *self
            .snapshot
            .write()
            .expect("workspace index lock poisoned") = Arc::new(rebuilt_snapshot);
        self.metrics.recursive_scans.fetch_add(
            rebuilt.metrics.recursive_scans.load(Ordering::Acquire),
            Ordering::Relaxed,
        );
        self.metrics.disk_reads.fetch_add(
            rebuilt.metrics.disk_reads.load(Ordering::Acquire),
            Ordering::Relaxed,
        );
        self.metrics.parsed_documents.fetch_add(
            rebuilt.metrics.parsed_documents.load(Ordering::Acquire),
            Ordering::Relaxed,
        );
        self.metrics
            .published_snapshots
            .fetch_add(1, Ordering::Release);
        *active = None;
        true
    }

    pub(crate) fn document(&self, uri: &Url) -> Option<Arc<IndexedDocument>> {
        self.snapshot().documents.get(uri).cloned()
    }

    pub(crate) fn enum_candidates(&self) -> Vec<(Url, IndexedEnum)> {
        let snapshot = self.snapshot();
        let mut candidates = snapshot
            .documents
            .iter()
            .flat_map(|(uri, document)| {
                document
                    .enums
                    .iter()
                    .cloned()
                    .map(|item| (uri.clone(), item))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.0
                .as_str()
                .cmp(right.0.as_str())
                .then_with(|| left.1.name.cmp(&right.1.name))
        });
        candidates
    }

    #[cfg(test)]
    pub(crate) fn failure(&self, uri: &Url) -> Option<IndexFailure> {
        self.snapshot().failures.get(uri).cloned()
    }

    pub(crate) fn documents(&self) -> HashMap<Url, String> {
        let snapshot = self.snapshot();
        snapshot
            .documents
            .iter()
            .map(|(uri, document)| {
                debug_assert_eq!(uri, &document.uri);
                debug_assert!(document.generation <= snapshot.generation);
                (uri.clone(), document.content.to_string())
            })
            .collect()
    }

    pub(crate) fn module_paths(&self, current_uri: &Url) -> Vec<String> {
        let snapshot = self.snapshot();
        let mut modules = snapshot
            .documents
            .keys()
            .filter(|uri| *uri != current_uri)
            .filter_map(|uri| module_identity_from_snapshot(&snapshot, uri))
            .map(|identity| identity.import_path)
            .collect::<Vec<_>>();
        modules.sort();
        modules.dedup();
        modules
    }

    pub(crate) fn module_identity(&self, uri: &Url) -> Option<ModuleIdentity> {
        module_identity_from_snapshot(&self.snapshot(), uri)
    }

    fn count_query(&self) {
        self.metrics.core_queries.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn document_symbols(&self, uri: &Url) -> Vec<AstSymbol> {
        self.count_query();
        self.document(uri)
            .map(|document| document.symbols.clone())
            .unwrap_or_default()
    }

    pub(crate) fn symbol_documentation(&self, uri: &Url, symbol: &str) -> Option<String> {
        self.document(uri)?.documentation.get(symbol).cloned()
    }

    pub(crate) fn completion_candidates(
        &self,
        current_uri: &Url,
    ) -> Vec<IndexedCompletionCandidate> {
        self.count_query();
        let snapshot = self.snapshot();
        let mut documents = snapshot.documents.iter().collect::<Vec<_>>();
        documents.sort_by(|(left, _), (right, _)| {
            match (*left == current_uri, *right == current_uri) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => left.as_str().cmp(right.as_str()),
            }
        });
        let mut candidates = Vec::new();
        let mut identity_counts = HashMap::<String, usize>::new();
        for (uri, document) in documents {
            let origin = if uri == current_uri {
                SymbolOrigin::CurrentDocument
            } else {
                document.origin
            };
            for symbol in &document.symbols {
                let semantic_detail = document
                    .signatures
                    .iter()
                    .find(|signature| {
                        signature.name == symbol.name && signature.range == symbol.range
                    })
                    .map(|signature| signature.label.as_str())
                    .unwrap_or(symbol.detail.as_str());
                let base_id = stable_symbol_id(uri, symbol, semantic_detail);
                let ordinal = identity_counts.entry(base_id.clone()).or_default();
                let symbol_id = if *ordinal == 0 {
                    base_id
                } else {
                    format!("{base_id}~{ordinal}")
                };
                *ordinal += 1;
                candidates.push(IndexedCompletionCandidate {
                    symbol: symbol.clone(),
                    definition_uri: uri.clone(),
                    origin,
                    symbol_id,
                    semantic_detail: semantic_detail.to_string(),
                });
            }
        }
        if let Some(document) = snapshot.documents.get(current_uri) {
            let stdlib_uri = Url::parse("sengoo-stdlib:/indexed-symbols.sg")
                .expect("static stdlib URI must parse");
            for symbol in &document.stdlib_symbols {
                candidates.push(IndexedCompletionCandidate {
                    symbol: symbol.clone(),
                    definition_uri: stdlib_uri.clone(),
                    origin: SymbolOrigin::StandardLibrary,
                    symbol_id: stable_symbol_id(&stdlib_uri, symbol, &symbol.detail),
                    semantic_detail: symbol.detail.clone(),
                });
            }
        }
        candidates
    }

    pub(crate) fn signature_candidates(&self, current_uri: &Url) -> Vec<FunctionSignatureInfo> {
        self.count_query();
        let snapshot = self.snapshot();
        let mut documents = snapshot.documents.iter().collect::<Vec<_>>();
        documents.sort_by(|(left, _), (right, _)| {
            match (*left == current_uri, *right == current_uri) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => left.as_str().cmp(right.as_str()),
            }
        });
        let mut signatures = Vec::new();
        for (uri, document) in documents {
            let mut document_signatures = document.signatures.clone();
            if let Some(identity) = module_identity_from_snapshot(&snapshot, uri) {
                qualify_function_signatures(&mut document_signatures, &identity.import_path);
            }
            signatures.extend(document_signatures);
        }
        if let Some(document) = snapshot.documents.get(current_uri) {
            signatures.extend(document.stdlib_signatures.clone());
        }
        signatures
    }

    pub(crate) fn visible_unqualified_signature_candidates(
        &self,
        current_uri: &Url,
    ) -> Vec<FunctionSignatureInfo> {
        let Some(current) = self.document(current_uri) else {
            return Vec::new();
        };
        let Some(current_module) = self
            .module_identity(current_uri)
            .map(|module| module.import_path)
        else {
            return Vec::new();
        };
        self.signature_candidates(current_uri)
            .into_iter()
            .filter(|signature| {
                let Some(module) = signature.module_path.as_deref() else {
                    return false;
                };
                if module == current_module {
                    return true;
                }
                current.imports.iter().any(|import| {
                    if import.path != module {
                        return false;
                    }
                    match &import.kind {
                        ImportFactKind::Simple | ImportFactKind::Wildcard => true,
                        ImportFactKind::Selective { names } => {
                            names.iter().any(|name| name == &signature.name)
                        }
                        ImportFactKind::Alias { .. } => false,
                    }
                })
            })
            .collect()
    }

    pub(crate) fn canonical_signature_qualifier(
        &self,
        current_uri: &Url,
        qualifier: &str,
    ) -> Option<String> {
        let signatures = self.signature_candidates(current_uri);
        self.canonical_signature_qualifier_from(current_uri, qualifier, &signatures)
    }

    pub(crate) fn canonical_signature_qualifier_from(
        &self,
        current_uri: &Url,
        qualifier: &str,
        signatures: &[FunctionSignatureInfo],
    ) -> Option<String> {
        let qualifier = qualifier.trim_matches(':');
        if qualifier.is_empty() {
            return None;
        }
        let snapshot = self.snapshot();
        let current = snapshot.documents.get(current_uri)?;
        let (head, tail) = qualifier
            .split_once("::")
            .map_or((qualifier, None), |(head, tail)| (head, Some(tail)));
        if let Some(path) = current
            .imports
            .iter()
            .find_map(|import| match &import.kind {
                ImportFactKind::Alias { alias } if alias == head => Some(import.path.as_str()),
                _ => None,
            })
        {
            return Some(match tail {
                Some(tail) => format!("{path}::{tail}"),
                None => path.to_string(),
            });
        }
        let known_root =
            snapshot.roots.iter().any(|root| root.import_prefix == head) || head == "std";
        if known_root {
            return Some(qualifier.to_string());
        }
        let current_module = module_identity_from_snapshot(&snapshot, current_uri)?.import_path;
        if tail.is_some() {
            let relative = format!("{current_module}::{qualifier}");
            return signature_target_exists(signatures, &relative).then_some(relative);
        }

        let mut candidates = BTreeSet::new();
        let current_candidate = format!("{current_module}::{qualifier}");
        if signature_target_exists(signatures, &current_candidate) {
            candidates.insert(current_candidate);
        }
        for import in &current.imports {
            let exposes = match &import.kind {
                ImportFactKind::Simple | ImportFactKind::Wildcard => true,
                ImportFactKind::Selective { names } => names.iter().any(|name| name == qualifier),
                ImportFactKind::Alias { .. } => false,
            };
            if !exposes {
                continue;
            }
            let imported_candidate = format!("{}::{qualifier}", import.path);
            if signature_target_exists(signatures, &imported_candidate) {
                candidates.insert(imported_candidate);
            }
        }
        (candidates.len() == 1).then(|| candidates.pop_first().unwrap())
    }

    pub(crate) fn symbol_detail(&self, current_uri: &Url, name: &str) -> Option<AstSymbol> {
        self.count_query();
        let snapshot = self.snapshot();
        if let Some(current) = snapshot.documents.get(current_uri) {
            if let Some(symbol) = current.symbols.iter().find(|symbol| symbol.name == name) {
                return Some(symbol.clone());
            }
            if let Some(symbol) = current
                .stdlib_symbols
                .iter()
                .find(|symbol| symbol.name == name)
            {
                return Some(symbol.clone());
            }
        }
        let mut documents = snapshot.documents.iter().collect::<Vec<_>>();
        documents.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
        documents.into_iter().find_map(|(uri, document)| {
            (uri != current_uri)
                .then(|| {
                    document
                        .symbols
                        .iter()
                        .find(|symbol| symbol.name == name)
                        .cloned()
                })
                .flatten()
        })
    }

    pub(crate) fn stdlib_definition(&self, current_uri: &Url, name: &str) -> Option<Location> {
        self.count_query();
        self.document(current_uri)
            .and_then(|document| document.stdlib_definitions.get(name).cloned())
    }

    pub(crate) fn workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        self.count_query();
        let query = query.trim().to_ascii_lowercase();
        let snapshot = self.snapshot();
        let mut documents = snapshot.documents.iter().collect::<Vec<_>>();
        documents.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
        documents
            .into_iter()
            .flat_map(|(uri, document)| {
                let query = query.clone();
                document.symbols.iter().filter_map(move |symbol| {
                    if !query.is_empty() && !symbol.name.to_ascii_lowercase().contains(&query) {
                        return None;
                    }
                    #[allow(deprecated)]
                    Some(SymbolInformation {
                        name: symbol.name.clone(),
                        kind: completion_kind_to_symbol_kind(symbol.kind),
                        tags: None,
                        deprecated: None,
                        location: Location::new(uri.clone(), symbol.range),
                        container_name: symbol
                            .container
                            .clone()
                            .or_else(|| Some(symbol.detail.clone())),
                    })
                })
            })
            .collect()
    }

    pub(crate) fn goto_definition(
        &self,
        current_uri: &Url,
        position: Position,
    ) -> Option<GotoDefinitionResponse> {
        self.count_query();
        let snapshot = self.snapshot();
        let current = snapshot.documents.get(current_uri)?;
        let symbol = extract_identifier_at(&current.content, position)?;
        if let Some(found) = current.symbols.iter().find(|item| item.name == symbol.name) {
            return Some(GotoDefinitionResponse::Scalar(Location::new(
                current_uri.clone(),
                found.range,
            )));
        }
        if let Some(range) = find_declaration_in_text(&current.content, &symbol.name) {
            return Some(GotoDefinitionResponse::Scalar(Location::new(
                current_uri.clone(),
                range,
            )));
        }
        let mut documents = snapshot.documents.iter().collect::<Vec<_>>();
        documents.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
        for (uri, document) in documents {
            if uri == current_uri {
                continue;
            }
            if let Some(found) = document
                .symbols
                .iter()
                .find(|item| item.name == symbol.name)
            {
                return Some(GotoDefinitionResponse::Scalar(Location::new(
                    uri.clone(),
                    found.range,
                )));
            }
            if let Some(range) = find_declaration_in_text(&document.content, &symbol.name)
                .or_else(|| find_definition_in_text(&document.content, &symbol.name))
            {
                return Some(GotoDefinitionResponse::Scalar(Location::new(
                    uri.clone(),
                    range,
                )));
            }
        }
        find_definition_in_text(&current.content, &symbol.name)
            .map(|range| GotoDefinitionResponse::Scalar(Location::new(current_uri.clone(), range)))
    }

    pub(crate) fn metrics(&self) -> IndexMetrics {
        IndexMetrics {
            recursive_scans: self.metrics.recursive_scans.load(Ordering::Acquire),
            disk_reads: self.metrics.disk_reads.load(Ordering::Acquire),
            parsed_documents: self.metrics.parsed_documents.load(Ordering::Acquire),
            published_snapshots: self.metrics.published_snapshots.load(Ordering::Acquire),
            core_queries: self.metrics.core_queries.load(Ordering::Acquire),
        }
    }

    pub(crate) fn open(
        &self,
        uri: Url,
        revision: i32,
        text: String,
        cancellation: &IndexCancellation,
    ) -> bool {
        self.publish_overlay(uri, revision, text, cancellation)
    }

    pub(crate) fn change(
        &self,
        uri: &Url,
        revision: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
        cancellation: &IndexCancellation,
    ) -> bool {
        let Some(current) = self.document(uri) else {
            return false;
        };
        if current.revision.is_some_and(|known| revision <= known) || cancellation.check().is_err()
        {
            return false;
        }
        let mut text = current.content.to_string();
        apply_content_changes(&mut text, changes);
        self.publish_overlay(uri.clone(), revision, text, cancellation)
    }

    fn publish_overlay(
        &self,
        uri: Url,
        revision: i32,
        text: String,
        cancellation: &IndexCancellation,
    ) -> bool {
        if cancellation.check().is_err() {
            return false;
        }
        let captured = self.snapshot();
        let captured_generation = captured.generation;
        let captured_overlay_epoch = Self::entry_epoch(&captured, &uri).overlay;
        let previous = captured.documents.get(&uri).cloned();
        if previous
            .as_ref()
            .and_then(|document| document.revision)
            .is_some_and(|known| revision <= known)
        {
            return false;
        }
        let generation = captured_generation + 1;
        self.metrics
            .parsed_documents
            .fetch_add(1, Ordering::Relaxed);
        let parsed = Arc::new(IndexedDocument::parse(
            uri.clone(),
            text,
            Some(revision),
            generation,
            previous.as_ref().map_or_else(
                || Self::origin_for_uri(&captured, &uri),
                |document| document.origin,
            ),
            previous.as_deref(),
        ));
        if cancellation.check().is_err() {
            return false;
        }
        #[cfg(test)]
        wait_at_publication_gate(&uri, TestPublicationKind::Overlay);

        let mut guard = self
            .snapshot
            .write()
            .expect("workspace index lock poisoned");
        if cancellation.check().is_err()
            || Self::entry_epoch(&guard, &uri).overlay != captured_overlay_epoch
        {
            return false;
        }
        if guard
            .documents
            .get(&uri)
            .and_then(|doc| doc.revision)
            .is_some_and(|known| revision <= known)
        {
            return false;
        }
        let mut next = (**guard).clone();
        next.generation = guard.generation + 1;
        let mut epoch = Self::entry_epoch(&guard, &uri);
        epoch.overlay += 1;
        next.entry_epochs.insert(uri.clone(), epoch);
        let mut published = (*parsed).clone();
        published.generation = next.generation;
        next.documents.insert(uri, Arc::new(published));
        *guard = Arc::new(next);
        self.metrics
            .published_snapshots
            .fetch_add(1, Ordering::Release);
        true
    }

    pub(crate) fn save(
        &self,
        uri: &Url,
        text: Option<String>,
        cancellation: &IndexCancellation,
    ) -> bool {
        if cancellation.check().is_err() {
            return false;
        }
        let snapshot = self.snapshot();
        let captured_generation = snapshot.generation;
        let captured_disk_epoch = Self::entry_epoch(&snapshot, uri).disk;
        if !snapshot.documents.contains_key(uri) {
            return false;
        }
        let text = match text {
            Some(text) => text,
            None => {
                let Ok(path) = uri.to_file_path() else {
                    return false;
                };
                self.metrics.disk_reads.fetch_add(1, Ordering::Relaxed);
                match fs::read_to_string(path) {
                    Ok(text) => text,
                    Err(error) => {
                        return self.record_failure(
                            uri,
                            error.to_string(),
                            captured_disk_epoch,
                            cancellation,
                        );
                    }
                }
            }
        };
        if cancellation.check().is_err() {
            return false;
        }
        let generation = captured_generation + 1;
        let prior_disk = snapshot.disk_documents.get(uri).map(Arc::as_ref);
        self.metrics
            .parsed_documents
            .fetch_add(1, Ordering::Relaxed);
        let disk = Arc::new(IndexedDocument::parse(
            uri.clone(),
            text,
            None,
            generation,
            prior_disk.map_or_else(
                || Self::origin_for_uri(&snapshot, uri),
                |document| document.origin,
            ),
            prior_disk,
        ));
        if cancellation.check().is_err() {
            return false;
        }
        #[cfg(test)]
        wait_at_publication_gate(uri, TestPublicationKind::Save);
        let mut guard = self
            .snapshot
            .write()
            .expect("workspace index lock poisoned");
        if cancellation.check().is_err()
            || Self::entry_epoch(&guard, uri).disk != captured_disk_epoch
        {
            return false;
        }
        let mut next = (**guard).clone();
        next.generation = guard.generation + 1;
        let mut epoch = Self::entry_epoch(&guard, uri);
        epoch.disk += 1;
        next.entry_epochs.insert(uri.clone(), epoch);
        let mut published_disk = (*disk).clone();
        published_disk.generation = next.generation;
        let published_disk = Arc::new(published_disk);
        next.disk_documents
            .insert(uri.clone(), published_disk.clone());
        next.failures.remove(uri);
        if next
            .documents
            .get(uri)
            .is_none_or(|document| document.revision.is_none())
        {
            next.documents.insert(uri.clone(), published_disk);
        }
        *guard = Arc::new(next);
        self.metrics
            .published_snapshots
            .fetch_add(1, Ordering::Release);
        true
    }

    pub(crate) fn close(&self, uri: &Url) -> bool {
        let mut guard = self
            .snapshot
            .write()
            .expect("workspace index lock poisoned");
        if !guard.documents.contains_key(uri) {
            return false;
        }
        let mut next = (**guard).clone();
        next.generation += 1;
        let mut epoch = Self::entry_epoch(&guard, uri);
        epoch.overlay += 1;
        next.entry_epochs.insert(uri.clone(), epoch);
        if let Some(disk) = next.disk_documents.get(uri).cloned() {
            next.documents.insert(uri.clone(), disk);
        } else {
            next.documents.remove(uri);
        }
        *guard = Arc::new(next);
        self.metrics
            .published_snapshots
            .fetch_add(1, Ordering::Release);
        true
    }

    pub(crate) fn refresh_file(&self, uri: &Url, cancellation: &IndexCancellation) -> bool {
        if cancellation.check().is_err() {
            return false;
        }
        let snapshot = self.snapshot();
        let captured_generation = snapshot.generation;
        let captured_disk_epoch = Self::entry_epoch(&snapshot, uri).disk;
        let Ok(path) = uri.to_file_path() else {
            return false;
        };
        self.metrics.disk_reads.fetch_add(1, Ordering::Relaxed);
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                return self.record_failure(
                    uri,
                    error.to_string(),
                    captured_disk_epoch,
                    cancellation,
                );
            }
        };
        if cancellation.check().is_err() {
            return false;
        }
        #[cfg(test)]
        wait_at_publication_gate(uri, TestPublicationKind::Refresh);
        let generation = captured_generation + 1;
        let origin = snapshot
            .disk_documents
            .get(uri)
            .map_or_else(|| Self::origin_for_uri(&snapshot, uri), |doc| doc.origin);
        let previous = snapshot.disk_documents.get(uri).map(Arc::as_ref);
        self.metrics
            .parsed_documents
            .fetch_add(1, Ordering::Relaxed);
        let parsed = Arc::new(IndexedDocument::parse(
            uri.clone(),
            text,
            None,
            generation,
            origin,
            previous,
        ));
        if cancellation.check().is_err() {
            return false;
        }
        let mut guard = self
            .snapshot
            .write()
            .expect("workspace index lock poisoned");
        if cancellation.check().is_err()
            || Self::entry_epoch(&guard, uri).disk != captured_disk_epoch
        {
            return false;
        }
        let mut next = (**guard).clone();
        next.generation = guard.generation + 1;
        let mut epoch = Self::entry_epoch(&guard, uri);
        epoch.disk += 1;
        next.entry_epochs.insert(uri.clone(), epoch);
        let mut published = (*parsed).clone();
        published.generation = next.generation;
        let published = Arc::new(published);
        next.disk_documents.insert(uri.clone(), published.clone());
        next.failures.remove(uri);
        if next
            .documents
            .get(uri)
            .is_none_or(|doc| doc.revision.is_none())
        {
            next.documents.insert(uri.clone(), published);
        }
        *guard = Arc::new(next);
        self.metrics
            .published_snapshots
            .fetch_add(1, Ordering::Release);
        true
    }

    pub(crate) fn remove_file(&self, uri: &Url) -> bool {
        let mut guard = self
            .snapshot
            .write()
            .expect("workspace index lock poisoned");
        if !guard.disk_documents.contains_key(uri) && !guard.failures.contains_key(uri) {
            return false;
        }
        let mut next = (**guard).clone();
        next.generation += 1;
        let mut epoch = Self::entry_epoch(&guard, uri);
        epoch.disk += 1;
        next.entry_epochs.insert(uri.clone(), epoch);
        next.disk_documents.remove(uri);
        next.failures.remove(uri);
        if next
            .documents
            .get(uri)
            .is_none_or(|doc| doc.revision.is_none())
        {
            next.documents.remove(uri);
        }
        *guard = Arc::new(next);
        self.metrics
            .published_snapshots
            .fetch_add(1, Ordering::Release);
        true
    }

    fn record_failure(
        &self,
        uri: &Url,
        message: String,
        captured_disk_epoch: u64,
        cancellation: &IndexCancellation,
    ) -> bool {
        if cancellation.check().is_err() {
            return false;
        }
        tracing::warn!(uri = %uri, error = %message, "failed to refresh indexed Sengoo file; retaining last-good entry");
        let mut guard = self
            .snapshot
            .write()
            .expect("workspace index lock poisoned");
        if cancellation.check().is_err()
            || Self::entry_epoch(&guard, uri).disk != captured_disk_epoch
        {
            return false;
        }
        let mut next = (**guard).clone();
        next.generation += 1;
        let mut epoch = Self::entry_epoch(&guard, uri);
        epoch.disk += 1;
        next.entry_epochs.insert(uri.clone(), epoch);
        next.failures.insert(
            uri.clone(),
            IndexFailure {
                message,
                generation: next.generation,
            },
        );
        *guard = Arc::new(next);
        self.metrics
            .published_snapshots
            .fetch_add(1, Ordering::Release);
        false
    }
}

pub(crate) async fn rebuild_workspace_index(
    index: Arc<WorkspaceIndex>,
    roots: Vec<PathBuf>,
) -> io::Result<bool> {
    let mut guard = index.begin_build();
    let cancellation = guard.token();
    let build_token = cancellation.clone();
    let rebuilt = tokio::task::spawn_blocking(move || WorkspaceIndex::build(&roots, build_token))
        .await
        .map_err(|error| io::Error::other(format!("workspace index task failed: {error}")))??;
    cancellation.check()?;
    let published = index.publish_rebuilt(guard.generation, rebuilt);
    guard.complete();
    Ok(published)
}

pub(crate) async fn run_index_operation<F>(operation: F) -> bool
where
    F: FnOnce(IndexCancellation) -> bool + Send + 'static,
{
    let mut guard = IndexOperationGuard::new();
    let cancellation = guard.token();
    let result = tokio::task::spawn_blocking(move || operation(cancellation))
        .await
        .unwrap_or(false);
    guard.complete();
    result
}

fn should_skip_workspace_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                ".git" | ".hg" | ".svn" | ".sgpm" | ".cache" | "node_modules" | "target"
            )
        })
}

fn canonical_path_for_matching(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    let mut existing = path;
    let mut missing = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name() else {
            return path.to_path_buf();
        };
        missing.push(name.to_os_string());
        let Some(parent) = existing.parent() else {
            return path.to_path_buf();
        };
        existing = parent;
    }
    let mut canonical = fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    canonical
}

fn normalize_import_segment(value: &str) -> String {
    value.replace('-', "_")
}

fn package_name_for_root(root: &Path) -> String {
    let manifest = fs::read_to_string(root.join("Sengoo.toml")).unwrap_or_default();
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(value) = line
                .strip_prefix("name")
                .and_then(|rest| rest.trim_start().strip_prefix('='))
            {
                if let Some(name) = parse_manifest_string(value) {
                    return normalize_import_segment(name);
                }
            }
        }
    }
    root.file_name()
        .and_then(|name| name.to_str())
        .map(normalize_import_segment)
        .unwrap_or_else(|| "workspace".into())
}

fn parse_manifest_string(value: &str) -> Option<&str> {
    let value = value.trim_start();
    let quote = value.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

fn dependency_aliases_for_workspace_roots(roots: &[PathBuf]) -> HashMap<PathBuf, String> {
    let mut aliases = dependency_aliases_from_lockfiles(roots)
        .into_iter()
        .map(|(root, alias)| {
            (
                canonical_path_for_matching(&root),
                normalize_import_segment(&alias),
            )
        })
        .collect::<HashMap<_, _>>();
    for root in roots {
        let manifest = fs::read_to_string(root.join("Sengoo.toml")).unwrap_or_default();
        let mut in_dependencies = false;
        for line in manifest.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_dependencies = line == "[dependencies]";
                continue;
            }
            if !in_dependencies || line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((alias, spec)) = line.split_once('=') else {
                continue;
            };
            let Some(path_marker) = spec.find("path") else {
                continue;
            };
            let Some(value) = spec[path_marker + 4..]
                .trim_start()
                .strip_prefix('=')
                .and_then(parse_manifest_string)
            else {
                continue;
            };
            aliases
                .entry(canonical_path_for_matching(&root.join(value)))
                .or_insert_with(|| normalize_import_segment(alias.trim()));
        }
    }
    aliases
}

fn module_identity_from_snapshot(
    snapshot: &WorkspaceSnapshot,
    uri: &Url,
) -> Option<ModuleIdentity> {
    let path = canonical_path_for_matching(&uri.to_file_path().ok()?);
    let root = snapshot
        .roots
        .iter()
        .find(|root| path.starts_with(&root.path))?;
    let relative = path.strip_prefix(root.path.join("src")).ok()?;
    let mut segments = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let file = segments.pop()?;
    let stem = Path::new(&file).file_stem()?.to_str()?;
    if stem != "lib" {
        segments.push(stem.to_string());
    }
    let relative_segments = segments
        .into_iter()
        .map(|segment| normalize_import_segment(&segment))
        .collect::<Vec<_>>();
    let import_path = std::iter::once(root.import_prefix.clone())
        .chain(relative_segments.iter().cloned())
        .collect::<Vec<_>>()
        .join("::");
    Some(ModuleIdentity {
        package_name: root.package_name.clone(),
        relative_segments,
        import_path,
        origin: root.origin,
    })
}

fn signature_target_exists(signatures: &[FunctionSignatureInfo], target: &str) -> bool {
    signatures.iter().any(|signature| {
        signature.module_path.as_deref() == Some(target)
            || signature.qualified_owner.as_deref() == Some(target)
    })
}

fn collect_sengoo_files(
    path: &Path,
    origin: SymbolOrigin,
    cancellation: &IndexCancellation,
    out: &mut Vec<(PathBuf, SymbolOrigin)>,
) -> io::Result<()> {
    cancellation.check()?;
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "sg") {
            out.push((path.to_path_buf(), origin));
        }
        return Ok(());
    }
    if should_skip_workspace_dir(path) {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(path) else {
        return Ok(());
    };
    let mut paths = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect::<Vec<_>>();
    paths.sort();
    for child in paths {
        collect_sengoo_files(&child, origin, cancellation, out)?;
    }
    Ok(())
}

pub(crate) fn collect_import_facts(content: &str) -> Vec<ImportFact> {
    fn stable_hash(value: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn visit(content: &str, tokens: &[Token], decls: &[Decl], out: &mut Vec<ImportFact>) {
        for decl in decls {
            match &decl.kind {
                DeclKind::Import(import) => {
                    let path = import
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.name.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    let kind = if let Some(alias) = &import.alias {
                        ImportFactKind::Alias {
                            alias: alias.name.clone(),
                        }
                    } else {
                        match &import.kind {
                            sengoo_compiler::ast::ImportKind::Simple => ImportFactKind::Simple,
                            sengoo_compiler::ast::ImportKind::Wildcard => ImportFactKind::Wildcard,
                            sengoo_compiler::ast::ImportKind::Selective(names) => {
                                ImportFactKind::Selective {
                                    names: names.iter().map(|name| name.name.clone()).collect(),
                                }
                            }
                        }
                    };
                    let byte_start = decl.span.lo;
                    let Some(byte_end) = tokens
                        .iter()
                        .find(|token| {
                            token.span.lo >= byte_start && token.kind == TokenKind::Semicolon
                        })
                        .map(|token| token.span.hi.min(content.len() as u32))
                    else {
                        continue;
                    };
                    let Some(source) = content.get(byte_start as usize..byte_end as usize) else {
                        continue;
                    };
                    let form = match &kind {
                        ImportFactKind::Simple => "simple".to_string(),
                        ImportFactKind::Alias { alias } => format!("alias:{alias}"),
                        ImportFactKind::Selective { names } => {
                            format!("selective:{}", names.join(","))
                        }
                        ImportFactKind::Wildcard => "wildcard".to_string(),
                    };
                    let canonical_identity = format!("{form}:{path}");
                    let source_hash = stable_hash(source);
                    let fact_id =
                        format!("{canonical_identity}@{byte_start}:{byte_end}:{source_hash:016x}");
                    out.push(ImportFact {
                        path,
                        kind,
                        byte_start,
                        byte_end,
                        range: span_to_range(content, byte_start, byte_end),
                        canonical_identity,
                        fact_id,
                        source_hash,
                    });
                }
                DeclKind::Module(module) => visit(content, tokens, &module.items, out),
                _ => {}
            }
        }
    }

    let Ok(program) = SgParser::parse(content) else {
        return Vec::new();
    };
    let tokens = Lexer::tokenize(content);
    let mut facts = Vec::new();
    visit(content, &tokens, &program.decls, &mut facts);
    facts.sort_by_key(|fact| (fact.byte_start, fact.byte_end));
    facts
}

fn collect_indexed_enums(content: &str) -> Vec<IndexedEnum> {
    fn visit(decls: &[Decl], out: &mut Vec<IndexedEnum>) {
        for decl in decls {
            match &decl.kind {
                DeclKind::Enum(item) => {
                    let variants = item
                        .variants
                        .iter()
                        .map(|variant| {
                            let name = variant.name.name.clone();
                            if variant.fields.is_empty() {
                                IndexedEnumVariant::Unit { name }
                            } else if variant
                                .fields
                                .iter()
                                .all(|field| matches!(field, VariantField::Unnamed(_)))
                            {
                                IndexedEnumVariant::Tuple {
                                    name,
                                    arity: variant.fields.len(),
                                }
                            } else {
                                IndexedEnumVariant::Struct {
                                    name,
                                    fields: variant
                                        .fields
                                        .iter()
                                        .filter_map(|field| match field {
                                            VariantField::Named(name, _) => Some(name.name.clone()),
                                            VariantField::Unnamed(_) => None,
                                        })
                                        .collect(),
                                }
                            }
                        })
                        .collect();
                    out.push(IndexedEnum {
                        name: item.name.name.clone(),
                        variants,
                    });
                }
                DeclKind::Module(module) => visit(&module.items, out),
                _ => {}
            }
        }
    }

    let Ok(program) = SgParser::parse(content) else {
        return Vec::new();
    };
    let mut enums = Vec::new();
    visit(&program.decls, &mut enums);
    enums
}

fn collect_documentation(content: &str, symbols: &[AstSymbol]) -> HashMap<String, String> {
    let mut docs = HashMap::new();
    let lines = content.lines().collect::<Vec<_>>();
    for symbol in symbols {
        let mut line = symbol.range.start.line as usize;
        let mut parts = Vec::new();
        while line > 0 {
            let previous = lines[line - 1].trim_start();
            let Some(doc) = previous.strip_prefix("///") else {
                break;
            };
            parts.push(doc.trim().to_string());
            line -= 1;
        }
        parts.reverse();
        if !parts.is_empty() {
            docs.insert(symbol.name.clone(), parts.join("\n"));
        }
    }
    docs
}

fn completion_kind_wire(kind: tower_lsp::lsp_types::CompletionItemKind) -> &'static str {
    use tower_lsp::lsp_types::CompletionItemKind;
    match kind {
        CompletionItemKind::METHOD => "method",
        CompletionItemKind::FUNCTION => "function",
        CompletionItemKind::CONSTRUCTOR => "constructor",
        CompletionItemKind::FIELD => "field",
        CompletionItemKind::VARIABLE => "variable",
        CompletionItemKind::CLASS => "class",
        CompletionItemKind::INTERFACE => "interface",
        CompletionItemKind::MODULE => "module",
        CompletionItemKind::PROPERTY => "property",
        CompletionItemKind::CONSTANT => "constant",
        CompletionItemKind::STRUCT => "struct",
        CompletionItemKind::ENUM => "enum",
        CompletionItemKind::ENUM_MEMBER => "enumMember",
        CompletionItemKind::TYPE_PARAMETER => "typeParameter",
        _ => "symbol",
    }
}

fn normalized_identity_fragment(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn stable_symbol_id(uri: &Url, symbol: &AstSymbol, semantic_detail: &str) -> String {
    format!(
        "{}#{}::{}::{}::{}",
        uri,
        symbol.container.as_deref().unwrap_or("<document>"),
        completion_kind_wire(symbol.kind),
        symbol.name,
        normalized_identity_fragment(semantic_detail),
    )
}

fn collect_scopes(
    uri: &Url,
    symbols: &[AstSymbol],
    signatures: &[FunctionSignatureInfo],
) -> Vec<IndexedScope> {
    let mut scopes = BTreeMap::<Option<String>, Vec<String>>::new();
    for symbol in symbols {
        scopes
            .entry(symbol.container.clone())
            .or_default()
            .push(stable_symbol_id(
                uri,
                symbol,
                signatures
                    .iter()
                    .find(|signature| {
                        signature.name == symbol.name && signature.range == symbol.range
                    })
                    .map(|signature| signature.label.as_str())
                    .unwrap_or(symbol.detail.as_str()),
            ));
    }
    scopes
        .into_iter()
        .map(|(container, mut symbol_ids)| {
            symbol_ids.sort();
            IndexedScope {
                container,
                symbol_ids,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signatures::{active_call_site, select_signature_help};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};

    #[test]
    fn enum_index_preserves_unit_tuple_and_struct_variant_shapes() {
        let enums = collect_indexed_enums(
            "enum Message { Quit, Move(i64, i64), Write { text: String } }\n",
        );

        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "Message");
        assert_eq!(
            enums[0].variants,
            vec![
                IndexedEnumVariant::Unit {
                    name: "Quit".into()
                },
                IndexedEnumVariant::Tuple {
                    name: "Move".into(),
                    arity: 2,
                },
                IndexedEnumVariant::Struct {
                    name: "Write".into(),
                    fields: vec!["text".into()],
                },
            ]
        );
    }

    #[test]
    fn parser_backed_import_facts_have_exact_unique_ranges_and_hashes() {
        let source = "// keep 😀\nimport demo::a as a; import demo::b { beta, gamma };\n";
        let facts = collect_import_facts(source);

        assert_eq!(facts.len(), 2);
        assert_eq!(
            &source[facts[0].byte_start as usize..facts[0].byte_end as usize],
            "import demo::a as a;"
        );
        assert_eq!(
            &source[facts[1].byte_start as usize..facts[1].byte_end as usize],
            "import demo::b { beta, gamma };"
        );
        assert_eq!(facts[1].range.start.line, 1);
        assert_eq!(
            facts[1].range.end.character,
            source.lines().nth(1).unwrap().encode_utf16().count() as u32
        );
        assert_ne!(facts[0].fact_id, facts[1].fact_id);
        assert_ne!(facts[0].source_hash, facts[1].source_hash);
    }

    fn temp_workspace() -> std::path::PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sglsp-index-{id}"));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.sg"), "def disk_symbol() -> i64 { 0 }\n").unwrap();
        fs::write(
            root.join("src/other.sg"),
            "def other_symbol() -> i64 { 0 }\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn initial_snapshot_scans_once_and_open_overlay_updates_one_document() {
        let root = temp_workspace();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let baseline = index.metrics();
        assert_eq!(baseline.recursive_scans, 1);
        assert_eq!(baseline.parsed_documents, 2);

        assert!(index.open(
            uri.clone(),
            1,
            "def open_symbol() -> i64 { 1 }\n".into(),
            &IndexCancellation::default()
        ));
        let after_open = index.metrics();
        assert_eq!(after_open.recursive_scans, baseline.recursive_scans);
        assert_eq!(after_open.parsed_documents, baseline.parsed_documents + 1);
        assert_eq!(index.document(&uri).unwrap().revision, Some(1));
        assert!(index
            .document(&uri)
            .unwrap()
            .symbols
            .iter()
            .any(|s| s.name == "open_symbol"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stale_incremental_change_cannot_replace_newer_overlay() {
        let root = temp_workspace();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        assert!(index.open(
            uri.clone(),
            7,
            "def newest() -> i64 { 7 }\n".into(),
            &IndexCancellation::default()
        ));
        let stale = vec![TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 4), Position::new(0, 10))),
            range_length: None,
            text: "stale".into(),
        }];
        assert!(!index.change(&uri, 6, stale, &IndexCancellation::default()));
        assert_eq!(index.document(&uri).unwrap().revision, Some(7));
        assert!(index.document(&uri).unwrap().content.contains("newest"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parser_backed_import_fixture_records_all_four_forms() {
        let facts = collect_import_facts(include_str!("../tests/fixtures/import_forms.sg"));
        assert_eq!(facts.len(), 4);
        assert!(matches!(facts[0].kind, ImportFactKind::Simple));
        assert!(matches!(facts[1].kind, ImportFactKind::Alias { ref alias } if alias == "coll"));
        assert!(
            matches!(facts[2].kind, ImportFactKind::Selective { ref names } if names == &["alpha", "beta"])
        );
        assert!(matches!(facts[3].kind, ImportFactKind::Wildcard));
    }

    #[test]
    fn broken_overlay_retains_last_good_semantic_entry() {
        let root = temp_workspace();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        assert!(index.open(
            uri.clone(),
            1,
            "def newest() -> i64 { 1 }\n".into(),
            &IndexCancellation::default()
        ));
        assert!(index.change(
            &uri,
            2,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "def newest( {".into()
            }],
            &IndexCancellation::default(),
        ));
        let document = index.document(&uri).unwrap();
        assert_eq!(document.revision, Some(2));
        assert_eq!(&*document.content, "def newest( {");
        assert!(document
            .symbols
            .iter()
            .any(|symbol| symbol.name == "newest"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cancelled_initial_build_stops_before_scanning() {
        let root = temp_workspace();
        let cancellation = IndexCancellation::default();
        cancellation.cancel();
        let error = WorkspaceIndex::build(std::slice::from_ref(&root), cancellation).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_and_close_refresh_only_the_affected_disk_entry() {
        let root = temp_workspace();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let baseline = index.metrics();
        assert!(index.open(
            uri.clone(),
            4,
            "def saved_symbol() -> i64 { 4 }\n".into(),
            &IndexCancellation::default()
        ));
        assert!(index.save(
            &uri,
            Some("def saved_symbol() -> i64 { 4 }\n".into()),
            &IndexCancellation::default(),
        ));
        assert!(index.close(&uri));
        let restored = index.document(&uri).unwrap();
        assert_eq!(restored.revision, None);
        assert!(restored
            .symbols
            .iter()
            .any(|symbol| symbol.name == "saved_symbol"));
        assert_eq!(index.metrics().recursive_scans, baseline.recursive_scans);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watched_file_create_change_delete_never_rescans_workspace() {
        let root = temp_workspace();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let baseline = index.metrics();
        let path = root.join("src/watched.sg");
        let uri = Url::from_file_path(&path).unwrap();
        fs::write(&path, "def created() -> i64 { 1 }\n").unwrap();
        assert!(index.refresh_file(&uri, &IndexCancellation::default()));
        assert!(index
            .document(&uri)
            .unwrap()
            .symbols
            .iter()
            .any(|symbol| symbol.name == "created"));
        fs::write(&path, "def changed() -> i64 { 2 }\n").unwrap();
        assert!(index.refresh_file(&uri, &IndexCancellation::default()));
        assert!(index
            .document(&uri)
            .unwrap()
            .symbols
            .iter()
            .any(|symbol| symbol.name == "changed"));
        fs::remove_file(&path).unwrap();
        assert!(!index.refresh_file(&uri, &IndexCancellation::default()));
        assert!(index.document(&uri).is_some());
        assert!(index.remove_file(&uri));
        assert!(index.document(&uri).is_none());
        assert_eq!(index.metrics().recursive_scans, baseline.recursive_scans);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn initial_snapshot_includes_direct_dependency_with_origin() {
        let root = temp_workspace();
        let app = root.join("app");
        let dep = root.join("dep");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(dep.join("src")).unwrap();
        fs::write(app.join("src/main.sg"), "def app_symbol() -> i64 { 0 }\n").unwrap();
        fs::write(dep.join("src/lib.sg"), "def dep_symbol() -> i64 { 0 }\n").unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            r#"version = 1
root = "app"

[[package]]
name = "dep"
version = "0.1.0"
source = "path+../dep"
manifest = "../dep/Sengoo.toml"
"#,
        )
        .unwrap();
        let index = WorkspaceIndex::build(&[app], IndexCancellation::default()).unwrap();
        let dep_uri = Url::from_file_path(dep.join("src/lib.sg")).unwrap();
        assert_eq!(
            index.document(&dep_uri).unwrap().origin,
            SymbolOrigin::Dependency
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn module_identity_uses_manifest_package_and_src_relative_path() {
        let root = temp_workspace();
        let app = root.join("app");
        let dep = root.join("sggame");
        fs::create_dir_all(app.join("src/nested")).unwrap();
        fs::create_dir_all(dep.join("src/nested")).unwrap();
        fs::write(
            app.join("Sengoo.toml"),
            "[package]\nname = 'demo-app' # app package\n\n[dependencies]\ngame_alias = { package = \"sggame\", path = \"../sggame\" } # exposed alias\n",
        )
        .unwrap();
        fs::write(dep.join("Sengoo.toml"), "[package]\nname = 'sggame'\n").unwrap();
        fs::write(app.join("src/lib.sg"), "def app() -> i64 { 0 }\n").unwrap();
        fs::write(
            app.join("src/nested/snake_logic.sg"),
            "def project_snake() -> i64 { 0 }\n",
        )
        .unwrap();
        fs::write(
            dep.join("src/nested/snake_logic.sg"),
            "def dependency_snake() -> i64 { 0 }\n",
        )
        .unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            r#"version = 1
root = "demo-app"
[[package]]
name = "sggame"
version = "0.1.0"
source = "path+../sggame"
manifest = "../sggame/Sengoo.toml"
"#,
        )
        .unwrap();
        let index = WorkspaceIndex::build(std::slice::from_ref(&app), IndexCancellation::default())
            .unwrap();
        let lib = Url::from_file_path(app.join("src/lib.sg")).unwrap();
        let project = Url::from_file_path(app.join("src/nested/snake_logic.sg")).unwrap();
        let dependency = Url::from_file_path(dep.join("src/nested/snake_logic.sg")).unwrap();
        assert_eq!(index.module_identity(&lib).unwrap().import_path, "demo_app");
        assert_eq!(
            index.module_identity(&project).unwrap().import_path,
            "demo_app::nested::snake_logic"
        );
        assert_eq!(
            index.module_identity(&dependency).unwrap().import_path,
            "game_alias::nested::snake_logic"
        );
        assert_ne!(
            index.module_identity(&project),
            index.module_identity(&dependency)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dependency_alias_qualifies_signature_namespaces_and_type_owners() {
        let root = temp_workspace();
        let app = root.join("app");
        let dep = root.join("game");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(dep.join("src/net")).unwrap();
        fs::write(
            app.join("Sengoo.toml"),
            "[package]\nname = \"app\"\n[dependencies]\ngame_alias = { package = \"game\", path = \"../game\" }\n",
        )
        .unwrap();
        fs::write(dep.join("Sengoo.toml"), "[package]\nname = \"game\"\n").unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            "version = 1\nroot = \"app\"\n[[package]]\nname = \"game\"\nversion = \"0.1.0\"\nsource = \"path+../game\"\nmanifest = \"../game/Sengoo.toml\"\n",
        )
        .unwrap();
        fs::write(
            dep.join("src/net/client.sg"),
            "def connect(port: i64) -> unit {}\nstruct Client {}\nimpl Client { def send(self, value: i64) -> unit {} }\n",
        )
        .unwrap();
        let main = app.join("src/main.sg");
        fs::write(
            &main,
            "import game_alias::net::client as client_api;\ndef main() -> unit {}\n",
        )
        .unwrap();
        let uri = Url::from_file_path(&main).unwrap();
        let index = WorkspaceIndex::build(&[app], IndexCancellation::default()).unwrap();
        let signatures = index.signature_candidates(&uri);

        let namespace_source = "game_alias::net::client::connect(";
        let namespace = active_call_site(namespace_source, namespace_source.len()).unwrap();
        let qualifier = index
            .canonical_signature_qualifier(&uri, namespace.qualifier.as_deref().unwrap())
            .unwrap();
        let selection = select_signature_help(&namespace, &signatures, Some(&qualifier)).unwrap();
        assert_eq!(selection.signatures.len(), 1);
        assert_eq!(
            selection.signatures[0].module_path.as_deref(),
            Some("game_alias::net::client")
        );

        let alias_source = "client_api::connect(";
        let alias_call = active_call_site(alias_source, alias_source.len()).unwrap();
        let alias_qualifier = index
            .canonical_signature_qualifier(&uri, alias_call.qualifier.as_deref().unwrap())
            .unwrap();
        assert_eq!(alias_qualifier, "game_alias::net::client");
        assert!(select_signature_help(&alias_call, &signatures, Some(&alias_qualifier)).is_some());

        let method_source = "client.send(";
        let method = active_call_site(method_source, method_source.len()).unwrap();
        let owner = index
            .canonical_signature_qualifier(&uri, "game_alias::net::client::Client")
            .unwrap();
        let selection = select_signature_help(&method, &signatures, Some(&owner)).unwrap();
        assert_eq!(
            selection.signatures[0].qualified_owner.as_deref(),
            Some("game_alias::net::client::Client")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn imported_receiver_pipeline_resolves_stdlib_selective_and_alias_types() {
        let root = temp_workspace();
        let app = root.join("app");
        let dep = root.join("dep");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(dep.join("src")).unwrap();
        fs::write(
            app.join("Sengoo.toml"),
            "[package]\nname = \"app\"\n[dependencies]\ndep_alias = { path = \"../dep\" }\n",
        )
        .unwrap();
        fs::write(dep.join("Sengoo.toml"), "[package]\nname = \"dep\"\n").unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            "version = 1\nroot = \"app\"\n[[package]]\nname = \"dep\"\nversion = \"0.1.0\"\nsource = \"path+../dep\"\nmanifest = \"../dep/Sengoo.toml\"\n",
        )
        .unwrap();
        fs::write(
            dep.join("src/client.sg"),
            "struct Client {}\nimpl Client { def ping(self, value: i64) -> unit {} }\n",
        )
        .unwrap();
        let main = app.join("src/main.sg");
        fs::write(
            &main,
            concat!(
                "import std::net;\n",
                "import dep_alias::client { Client };\n",
                "import dep_alias::client as client_api;\n",
                "def main(http: HttpClient, selected: Client, aliased: client_api::Client) -> unit {}\n",
            ),
        )
        .unwrap();
        let uri = Url::from_file_path(&main).unwrap();
        let index = WorkspaceIndex::build(&[app], IndexCancellation::default()).unwrap();
        let indexed_std_method = index
            .document(&uri)
            .unwrap()
            .stdlib_signatures
            .iter()
            .find(|signature| signature.qualified_owner.as_deref() == Some("std::net::HttpClient"))
            .cloned()
            .expect("stdlib signatures retain their source module before candidate merging");
        assert_eq!(indexed_std_method.module_path.as_deref(), Some("std::net"));
        let signatures = index.signature_candidates(&uri);

        let std_owner = index
            .canonical_signature_qualifier(&uri, "HttpClient")
            .unwrap();
        assert_eq!(std_owner, "std::net::HttpClient");
        let std_call = active_call_site("http.status(", 12).unwrap();
        let std_selection =
            select_signature_help(&std_call, &signatures, Some(&std_owner)).unwrap();
        assert_eq!(
            std_selection.signatures[0].module_path.as_deref(),
            Some("std::net")
        );
        assert_eq!(
            std_selection.signatures[0].qualified_owner.as_deref(),
            Some("std::net::HttpClient")
        );

        let selected_owner = index.canonical_signature_qualifier(&uri, "Client").unwrap();
        assert_eq!(selected_owner, "dep_alias::client::Client");
        let ping_call = active_call_site("selected.ping(", 14).unwrap();
        assert!(select_signature_help(&ping_call, &signatures, Some(&selected_owner)).is_some());

        let alias_owner = index
            .canonical_signature_qualifier(&uri, "client_api::Client")
            .unwrap();
        assert_eq!(alias_owner, "dep_alias::client::Client");
        assert!(select_signature_help(&ping_call, &signatures, Some(&alias_owner)).is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wildcard_receiver_resolution_rejects_multiple_exporting_modules() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"app\"\n").unwrap();
        fs::write(
            root.join("src/a.sg"),
            concat!(
                "struct Client {}\nimpl Client { def first(self) -> unit {} }\n",
                "struct Solo {}\nimpl Solo { def only(self) -> unit {} }\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("src/b.sg"),
            "struct Client {}\nimpl Client { def second(self) -> unit {} }\n",
        )
        .unwrap();
        let main = root.join("src/main.sg");
        fs::write(
            &main,
            concat!(
                "import app::a * from;\n",
                "import app::b * from;\n",
                "struct Client {}\nimpl Client { def local(self) -> unit {} }\n",
                "def main(client: Client) -> unit {}\n",
            ),
        )
        .unwrap();
        let uri = Url::from_file_path(&main).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        assert_eq!(index.canonical_signature_qualifier(&uri, "Client"), None);
        assert_eq!(
            index.canonical_signature_qualifier(&uri, "Solo").as_deref(),
            Some("app::a::Solo")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unqualified_signatures_include_only_current_and_import_exposed_functions() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"app\"\n").unwrap();
        fs::write(root.join("src/a.sg"), "def shared(value: i64) -> unit {}\n").unwrap();
        fs::write(
            root.join("src/b.sg"),
            "def shared(value: &str) -> unit {}\n",
        )
        .unwrap();
        let main = root.join("src/main.sg");
        fs::write(&main, "import app::a;\ndef local() -> unit {}\n").unwrap();
        let uri = Url::from_file_path(&main).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let visible = index.visible_unqualified_signature_candidates(&uri);
        let shared = visible
            .iter()
            .filter(|signature| signature.name == "shared")
            .collect::<Vec<_>>();
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].module_path.as_deref(), Some("app::a"));
        assert!(visible.iter().any(|signature| signature.name == "local"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn signature_builder_uses_overlay_utf16_and_scope_aware_shadowing() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"app\"\n").unwrap();
        let path = root.join("src/main.sg");
        fs::write(&path, "def placeholder() -> unit {}\n").unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let source = concat!(
            "struct First {}\nimpl First { def ping(self, value: i64) -> unit {} }\n",
            "struct Second {}\nimpl Second { def ping(self, text: &str) -> unit {} }\n",
            "def main(client: First) -> unit { { let client: Second; let emoji = \"😀\"; client.ping(\"x\" } }\n",
        );
        assert!(index.open(
            uri.clone(),
            6,
            source.replace("client.ping(\"x\"", "client.ping(\"x\")"),
            &IndexCancellation::default(),
        ));
        assert!(index.open(uri.clone(), 7, source.into(), &IndexCancellation::default()));
        let prefix = source.split("client.ping").next().unwrap().to_string() + "client.ping(\"x\"";
        let line = prefix.lines().count() as u32 - 1;
        let character = prefix.lines().last().unwrap().encode_utf16().count() as u32;
        let help = crate::signatures::signature_help_for_request(
            &index,
            &uri,
            source,
            Position::new(line, character),
        )
        .unwrap();
        assert_eq!(help.signatures.len(), 1);
        assert!(help.signatures[0].label.contains("text: &str"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn signature_builder_resolves_self_and_field_call_chains_without_comment_bindings() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"app\"\n").unwrap();
        let path = root.join("src/main.sg");
        let template = concat!(
            "struct Child {}\nimpl Child { def ping(self, value: i64) -> unit {} }\n",
            "struct Holder { child: Child }\ndef make() -> Holder { Holder { child: Child {} } }\n",
            "impl Holder { def own(self, value: &str) -> unit {} def run(self) -> unit { self.own(\"x\" } }\n",
            "def main(holder: Holder) -> unit { // let holder: Child; \"let holder: Child\"\n make().child.ping(1 }\n",
        );
        fs::write(
            &path,
            template
                .replace("self.own(\"x\"", "self.own(\"x\")")
                .replace("make().child.ping(1", "make().child.ping(1)"),
        )
        .unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();

        assert!(index.open(
            uri.clone(),
            2,
            template.into(),
            &IndexCancellation::default()
        ));
        for (needle, expected) in [
            ("self.own(\"x\"", "value: &str"),
            ("make().child.ping(1", "value: i64"),
        ] {
            let offset = template.find(needle).unwrap() + needle.len();
            let prefix = &template[..offset];
            let position = Position::new(
                prefix.lines().count() as u32 - 1,
                prefix.lines().last().unwrap().encode_utf16().count() as u32,
            );
            let help =
                crate::signatures::signature_help_for_request(&index, &uri, template, position)
                    .unwrap_or_else(|| panic!("scope-aware chain should resolve: {needle}"));
            assert!(help.signatures[0].label.contains(expected));
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn warm_signature_help_p95_stays_below_eighty_ms_on_large_workspace() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"app\"\n").unwrap();
        for index in 0..100 {
            fs::write(
                root.join(format!("src/module_{index}.sg")),
                format!("def helper_{index}(value: i64) -> i64 {{ value }}\n"),
            )
            .unwrap();
        }
        let source = "def target(value: i64) -> unit {}\ndef main() -> unit { target(1) }\n";
        let path = root.join("src/main.sg");
        fs::write(&path, source).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let prefix = source.find("target(1").unwrap() + "target(1".len();
        let before = &source[..prefix];
        let position = Position::new(
            before.lines().count() as u32 - 1,
            before.lines().last().unwrap().encode_utf16().count() as u32,
        );
        let mut samples = Vec::new();
        for _ in 0..30 {
            let started = std::time::Instant::now();
            assert!(
                crate::signatures::signature_help_for_request(&index, &uri, source, position)
                    .is_some()
            );
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
        assert!(
            p95 < std::time::Duration::from_millis(80),
            "signature p95 was {p95:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn nested_modules_extend_canonical_signature_namespace_without_tail_guessing() {
        let root = temp_workspace();
        fs::create_dir_all(root.join("src/outer")).unwrap();
        let uri = Url::from_file_path(root.join("src/outer/inner.sg")).unwrap();
        fs::write(
            root.join("Sengoo.toml"),
            "[package]\nname = \"sengoo-project\"\n",
        )
        .unwrap();
        fs::write(
            root.join("src/outer/inner.sg"),
            "def choose(value: i64) -> unit {}\n",
        )
        .unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        assert_eq!(
            index.module_identity(&uri).unwrap().import_path,
            "sengoo_project::outer::inner"
        );
        let signatures = index.signature_candidates(&uri);
        assert_eq!(
            signatures
                .iter()
                .find(|signature| signature.name == "choose")
                .unwrap()
                .module_path
                .as_deref(),
            Some("sengoo_project::outer::inner")
        );
        let exact_source = "sengoo_project::outer::inner::choose(";
        let exact = active_call_site(exact_source, exact_source.len()).unwrap();
        assert!(select_signature_help(&exact, &signatures, exact.qualifier.as_deref(),).is_some());
        let ambiguous_source = "inner::choose(";
        let ambiguous_tail = active_call_site(ambiguous_source, ambiguous_source.len()).unwrap();
        assert!(select_signature_help(
            &ambiguous_tail,
            &signatures,
            ambiguous_tail.qualifier.as_deref(),
        )
        .is_none());
        assert_eq!(
            signatures
                .iter()
                .find(|signature| signature.name == "choose")
                .unwrap()
                .module_path
                .as_deref(),
            Some("sengoo_project::outer::inner")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn one_hundred_warm_core_queries_do_not_walk_read_or_parse() {
        let root = temp_workspace();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let baseline = index.metrics();
        for _ in 0..100 {
            let _ = index.completion_candidates(&uri);
            let _ = index.signature_candidates(&uri);
            let _ = index.symbol_detail(&uri, "disk_symbol");
            let _ = index.goto_definition(&uri, Position::new(0, 5));
            let _ = index.workspace_symbols("symbol");
        }
        let after = index.metrics();
        assert_eq!(after.recursive_scans, baseline.recursive_scans);
        assert_eq!(after.disk_reads, baseline.disk_reads);
        assert_eq!(after.parsed_documents, baseline.parsed_documents);
        assert_eq!(after.core_queries, baseline.core_queries + 500);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn completion_identity_uses_definition_uri_container_and_true_origin() {
        let root = temp_workspace();
        fs::write(root.join("src/main.sg"), "def collision() -> i64 { 0 }\n").unwrap();
        fs::write(root.join("src/other.sg"), "def collision() -> i64 { 1 }\n").unwrap();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let other_uri = Url::from_file_path(root.join("src/other.sg")).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let collisions = index
            .completion_candidates(&uri)
            .into_iter()
            .filter(|candidate| candidate.symbol.name == "collision")
            .collect::<Vec<_>>();
        assert_eq!(collisions.len(), 2);
        let current = collisions
            .iter()
            .find(|candidate| candidate.definition_uri == uri)
            .unwrap();
        let other = collisions
            .iter()
            .find(|candidate| candidate.definition_uri == other_uri)
            .unwrap();
        assert_eq!(current.origin, SymbolOrigin::CurrentDocument);
        assert_eq!(other.origin, SymbolOrigin::Workspace);
        assert_ne!(current.symbol_id, other.symbol_id);
        assert!(current.symbol_id.starts_with(uri.as_str()));
        assert!(other.symbol_id.starts_with(other_uri.as_str()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_contains_scope_container_and_real_stdlib_metadata() {
        let root = temp_workspace();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        fs::write(
            root.join("src/main.sg"),
            "import std::io;\n\ndef local() -> i64 { 0 }\n",
        )
        .unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let document = index.document(&uri).unwrap();
        assert!(document
            .scopes
            .iter()
            .any(|scope| scope.container.as_deref() == Some("<document>")));
        assert!(!document.stdlib_symbols.is_empty());
        assert!(!document.stdlib_signatures.is_empty());
        assert!(document.stdlib_definitions.contains_key("io_stdin_read"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_failure_retains_last_good_until_explicit_delete_and_recovers() {
        let root = temp_workspace();
        let path = root.join("src/main.sg");
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(!index.refresh_file(&uri, &IndexCancellation::default()));
        assert!(index
            .document(&uri)
            .unwrap()
            .content
            .contains("disk_symbol"));
        assert!(index.failure(&uri).is_some());

        fs::remove_file(&path).unwrap();
        assert!(!index.refresh_file(&uri, &IndexCancellation::default()));
        assert!(index.document(&uri).is_some());
        fs::write(&path, "def recovered() -> i64 { 0 }\n").unwrap();
        assert!(index.refresh_file(&uri, &IndexCancellation::default()));
        assert!(index.failure(&uri).is_none());
        assert!(index
            .document(&uri)
            .unwrap()
            .symbols
            .iter()
            .any(|symbol| symbol.name == "recovered"));

        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(!index.refresh_file(&uri, &IndexCancellation::default()));
        assert!(index
            .document(&uri)
            .unwrap()
            .symbols
            .iter()
            .any(|symbol| symbol.name == "recovered"));
        assert!(index.failure(&uri).is_some());
        fs::remove_dir(&path).unwrap();

        assert!(index.remove_file(&uri));
        assert!(index.document(&uri).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn dropping_real_async_build_adapter_cancels_and_never_swaps_snapshot() {
        let root = temp_workspace();
        let index = Arc::new(
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap(),
        );
        let initial_generation = index.snapshot().generation;
        let large = root.join("large");
        fs::create_dir_all(&large).unwrap();
        for number in 0..1_000 {
            fs::write(
                large.join(format!("item-{number}.sg")),
                format!("def item_{number}() -> i64 {{ {number} }}\n"),
            )
            .unwrap();
        }
        let task = tokio::spawn(rebuild_workspace_index(Arc::clone(&index), vec![large]));
        tokio::task::yield_now().await;
        task.abort();
        let _ = task.await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(index.snapshot().generation, initial_generation);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn newer_async_build_generation_cancels_older_and_wins_publication() {
        let root = temp_workspace();
        let index = Arc::new(
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap(),
        );
        let large = root.join("large");
        let newest = root.join("newest");
        fs::create_dir_all(&large).unwrap();
        fs::create_dir_all(&newest).unwrap();
        for number in 0..1_000 {
            fs::write(
                large.join(format!("old-{number}.sg")),
                format!("def old_{number}() -> i64 {{ {number} }}\n"),
            )
            .unwrap();
        }
        fs::write(
            newest.join("winner.sg"),
            "def newest_winner() -> i64 { 1 }\n",
        )
        .unwrap();
        let older = tokio::spawn(rebuild_workspace_index(Arc::clone(&index), vec![large]));
        tokio::task::yield_now().await;
        assert!(rebuild_workspace_index(Arc::clone(&index), vec![newest])
            .await
            .unwrap());
        let _ = older.await;
        let snapshot = index.snapshot();
        let names = snapshot
            .documents
            .values()
            .flat_map(|document| document.symbols.iter().map(|symbol| symbol.name.as_str()))
            .collect::<Vec<_>>();
        assert!(names.contains(&"newest_winner"));
        assert!(!names.iter().any(|name| name.starts_with("old_")));
        let _ = fs::remove_dir_all(root);
    }

    fn large_source(prefix: &str, count: usize) -> String {
        (0..count)
            .map(|number| format!("def {prefix}_{number}() -> i64 {{ {number} }}\n"))
            .collect()
    }

    async fn wait_for_parse_start(index: &WorkspaceIndex, baseline: u64) {
        for _ in 0..2_000 {
            if index.metrics().parsed_documents > baseline {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        panic!("blocking parser did not start before timeout");
    }

    #[tokio::test]
    async fn dropped_document_adapters_never_swap_after_parse() {
        let root = temp_workspace();
        let path = root.join("src/main.sg");
        let uri = Url::from_file_path(&path).unwrap();
        let index = Arc::new(
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap(),
        );
        let baseline = index.snapshot().generation;
        let baseline_parses = index.metrics().parsed_documents;

        let open_index = Arc::clone(&index);
        let open_uri = uri.clone();
        let open_task = tokio::spawn(run_index_operation(move |cancellation| {
            open_index.open(
                open_uri,
                1,
                large_source("cancelled_open", 500),
                &cancellation,
            )
        }));
        wait_for_parse_start(&index, baseline_parses).await;
        open_task.abort();
        let _ = open_task.await;
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(index.snapshot().generation, baseline);
        assert_eq!(index.document(&uri).unwrap().revision, None);

        assert!(index.open(
            uri.clone(),
            1,
            "def overlay() -> i64 { 1 }\n".into(),
            &IndexCancellation::default()
        ));
        let save_baseline = index.snapshot().generation;
        let save_baseline_parses = index.metrics().parsed_documents;
        let save_index = Arc::clone(&index);
        let save_uri = uri.clone();
        let save_task = tokio::spawn(run_index_operation(move |cancellation| {
            save_index.save(
                &save_uri,
                Some(large_source("cancelled_save", 500)),
                &cancellation,
            )
        }));
        wait_for_parse_start(&index, save_baseline_parses).await;
        save_task.abort();
        let _ = save_task.await;
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(index.snapshot().generation, save_baseline);
        assert!(index.document(&uri).unwrap().content.contains("overlay"));

        fs::write(&path, large_source("cancelled_refresh", 500)).unwrap();
        let refresh_baseline = index.snapshot().generation;
        let refresh_baseline_parses = index.metrics().parsed_documents;
        let refresh_index = Arc::clone(&index);
        let refresh_uri = uri.clone();
        let refresh_task = tokio::spawn(run_index_operation(move |cancellation| {
            refresh_index.refresh_file(&refresh_uri, &cancellation)
        }));
        wait_for_parse_start(&index, refresh_baseline_parses).await;
        refresh_task.abort();
        let _ = refresh_task.await;
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(index.snapshot().generation, refresh_baseline);
        assert!(index.document(&uri).unwrap().content.contains("overlay"));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn newer_overlay_generation_wins_against_old_change_save_and_refresh() {
        let root = temp_workspace();
        let path = root.join("src/main.sg");
        let uri = Url::from_file_path(&path).unwrap();
        let index = Arc::new(
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap(),
        );
        assert!(index.open(
            uri.clone(),
            1,
            "def first() -> i64 { 1 }\n".into(),
            &IndexCancellation::default()
        ));

        let old_change_index = Arc::clone(&index);
        let old_change_uri = uri.clone();
        let old_change_parses = index.metrics().parsed_documents;
        let old_change = tokio::spawn(run_index_operation(move |cancellation| {
            old_change_index.change(
                &old_change_uri,
                2,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: large_source("old_change", 500),
                }],
                &cancellation,
            )
        }));
        wait_for_parse_start(&index, old_change_parses).await;
        let newest_index = Arc::clone(&index);
        let newest_uri = uri.clone();
        assert!(
            run_index_operation(move |cancellation| {
                newest_index.change(
                    &newest_uri,
                    3,
                    vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: "def newest() -> i64 { 3 }\n".into(),
                    }],
                    &cancellation,
                )
            })
            .await
        );
        let _ = old_change.await;
        assert_eq!(index.document(&uri).unwrap().revision, Some(3));
        assert!(index.document(&uri).unwrap().content.contains("newest"));

        let old_save_index = Arc::clone(&index);
        let old_save_uri = uri.clone();
        let old_save_parses = index.metrics().parsed_documents;
        let old_save = tokio::spawn(run_index_operation(move |cancellation| {
            old_save_index.save(
                &old_save_uri,
                Some(large_source("old_save", 500)),
                &cancellation,
            )
        }));
        wait_for_parse_start(&index, old_save_parses).await;
        assert!(index.open(
            uri.clone(),
            4,
            "def newest_four() -> i64 { 4 }\n".into(),
            &IndexCancellation::default()
        ));
        let generation_after_newest = index.snapshot().generation;
        assert!(old_save.await.unwrap());
        assert_eq!(index.snapshot().generation, generation_after_newest + 1);
        assert_eq!(index.document(&uri).unwrap().revision, Some(4));
        assert!(index
            .document(&uri)
            .unwrap()
            .content
            .contains("newest_four"));
        assert!(index.snapshot().disk_documents[&uri]
            .content
            .contains("old_save"));

        fs::write(&path, large_source("old_refresh", 500)).unwrap();
        let old_refresh_index = Arc::clone(&index);
        let old_refresh_uri = uri.clone();
        let old_refresh_parses = index.metrics().parsed_documents;
        let old_refresh = tokio::spawn(run_index_operation(move |cancellation| {
            old_refresh_index.refresh_file(&old_refresh_uri, &cancellation)
        }));
        wait_for_parse_start(&index, old_refresh_parses).await;
        assert!(index.open(
            uri.clone(),
            5,
            "def newest_five() -> i64 { 5 }\n".into(),
            &IndexCancellation::default()
        ));
        let generation_after_five = index.snapshot().generation;
        assert!(old_refresh.await.unwrap());
        assert_eq!(index.snapshot().generation, generation_after_five + 1);
        assert_eq!(index.document(&uri).unwrap().revision, Some(5));
        assert!(index
            .document(&uri)
            .unwrap()
            .content
            .contains("newest_five"));
        assert!(index.snapshot().disk_documents[&uri]
            .content
            .contains("old_refresh"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_delete_clears_failure_only_entries() {
        let root = temp_workspace();
        let path = root.join("src/invalid.sg");
        let uri = Url::from_file_path(&path).unwrap();
        fs::write(&path, [0xff, 0xfe]).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        assert!(index.document(&uri).is_none());
        assert!(index.failure(&uri).is_some());
        assert!(index.remove_file(&uri));
        assert!(index.failure(&uri).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn concurrent_different_uri_open_change_save_refresh_all_merge() {
        let root = temp_workspace();
        let first_uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let second_uri = Url::from_file_path(root.join("src/other.sg")).unwrap();
        let index = Arc::new(
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap(),
        );

        let open_first_index = Arc::clone(&index);
        let open_first_uri = first_uri.clone();
        let open_first = tokio::spawn(run_index_operation(move |cancellation| {
            open_first_index.open(
                open_first_uri,
                1,
                large_source("open_first", 200),
                &cancellation,
            )
        }));
        let open_second_index = Arc::clone(&index);
        let open_second_uri = second_uri.clone();
        let open_second = tokio::spawn(run_index_operation(move |cancellation| {
            open_second_index.open(
                open_second_uri,
                1,
                large_source("open_second", 200),
                &cancellation,
            )
        }));
        assert!(open_first.await.unwrap());
        assert!(open_second.await.unwrap());
        assert_eq!(index.document(&first_uri).unwrap().revision, Some(1));
        assert_eq!(index.document(&second_uri).unwrap().revision, Some(1));

        let change_first_index = Arc::clone(&index);
        let change_first_uri = first_uri.clone();
        let change_first = tokio::spawn(run_index_operation(move |cancellation| {
            change_first_index.change(
                &change_first_uri,
                2,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: large_source("change_first", 200),
                }],
                &cancellation,
            )
        }));
        let change_second_index = Arc::clone(&index);
        let change_second_uri = second_uri.clone();
        let change_second = tokio::spawn(run_index_operation(move |cancellation| {
            change_second_index.change(
                &change_second_uri,
                2,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: large_source("change_second", 200),
                }],
                &cancellation,
            )
        }));
        assert!(change_first.await.unwrap());
        assert!(change_second.await.unwrap());
        assert!(index
            .document(&first_uri)
            .unwrap()
            .content
            .contains("change_first"));
        assert!(index
            .document(&second_uri)
            .unwrap()
            .content
            .contains("change_second"));

        let save_first_index = Arc::clone(&index);
        let save_first_uri = first_uri.clone();
        let save_first = tokio::spawn(run_index_operation(move |cancellation| {
            save_first_index.save(
                &save_first_uri,
                Some(large_source("save_first", 200)),
                &cancellation,
            )
        }));
        let save_second_index = Arc::clone(&index);
        let save_second_uri = second_uri.clone();
        let save_second = tokio::spawn(run_index_operation(move |cancellation| {
            save_second_index.save(
                &save_second_uri,
                Some(large_source("save_second", 200)),
                &cancellation,
            )
        }));
        assert!(save_first.await.unwrap());
        assert!(save_second.await.unwrap());

        fs::write(root.join("src/main.sg"), large_source("refresh_first", 200)).unwrap();
        fs::write(
            root.join("src/other.sg"),
            large_source("refresh_second", 200),
        )
        .unwrap();
        let refresh_first_index = Arc::clone(&index);
        let refresh_first_uri = first_uri.clone();
        let refresh_first = tokio::spawn(run_index_operation(move |cancellation| {
            refresh_first_index.refresh_file(&refresh_first_uri, &cancellation)
        }));
        let refresh_second_index = Arc::clone(&index);
        let refresh_second_uri = second_uri.clone();
        let refresh_second = tokio::spawn(run_index_operation(move |cancellation| {
            refresh_second_index.refresh_file(&refresh_second_uri, &cancellation)
        }));
        assert!(refresh_first.await.unwrap());
        assert!(refresh_second.await.unwrap());
        assert!(index.close(&first_uri));
        assert!(index.close(&second_uri));
        assert!(index
            .document(&first_uri)
            .unwrap()
            .content
            .contains("refresh_first"));
        assert!(index
            .document(&second_uri)
            .unwrap()
            .content
            .contains("refresh_second"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn new_dependency_files_use_longest_canonical_root_origin() {
        let root = temp_workspace();
        let app = root.join("app");
        let dep = root.join("dep");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(dep.join("src")).unwrap();
        fs::write(app.join("src/main.sg"), "def app() -> i64 { 0 }\n").unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            r#"version = 1
root = "app"
[[package]]
name = "dep"
version = "0.1.0"
source = "path+../dep"
manifest = "../dep/Sengoo.toml"
"#,
        )
        .unwrap();
        let index = WorkspaceIndex::build(&[app], IndexCancellation::default()).unwrap();
        let watched_path = dep.join("src/watched.sg");
        fs::write(&watched_path, "def watched() -> i64 { 0 }\n").unwrap();
        let watched_uri = Url::from_file_path(&watched_path).unwrap();
        assert!(index.refresh_file(&watched_uri, &IndexCancellation::default()));
        assert_eq!(
            index.document(&watched_uri).unwrap().origin,
            SymbolOrigin::Dependency
        );
        let open_uri = Url::from_file_path(dep.join("src/unsaved.sg")).unwrap();
        assert!(index.open(
            open_uri.clone(),
            1,
            "def unsaved() -> i64 { 0 }\n".into(),
            &IndexCancellation::default()
        ));
        assert_eq!(
            index.document(&open_uri).unwrap().origin,
            SymbolOrigin::Dependency
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stable_symbol_identity_ignores_position_and_uses_explicit_semantics() {
        let root = temp_workspace();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        fs::write(
            root.join("src/main.sg"),
            "def stable(value: i64) -> i64 { value }\n",
        )
        .unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let before = index
            .completion_candidates(&uri)
            .into_iter()
            .find(|candidate| candidate.symbol.name == "stable")
            .unwrap()
            .symbol_id;
        assert!(!before.contains("CompletionItemKind"));
        assert!(index.open(
            uri.clone(),
            1,
            "\n\n\ndef stable(value: i64) -> i64 { value }\n".into(),
            &IndexCancellation::default()
        ));
        let after = index
            .completion_candidates(&uri)
            .into_iter()
            .find(|candidate| candidate.symbol.name == "stable")
            .unwrap()
            .symbol_id;
        assert_eq!(before, after);
        assert!(after.contains("function"));
        assert!(after.contains("def stable(value: i64) -> i64"));
        let _ = fs::remove_dir_all(root);
    }

    async fn wait_gate_reached(gate: Arc<TestPublicationGate>) {
        tokio::task::spawn_blocking(move || gate.reached.wait())
            .await
            .unwrap();
    }

    async fn release_gate(gate: Arc<TestPublicationGate>) {
        tokio::task::spawn_blocking(move || gate.release.wait())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn disk_and_overlay_publications_merge_in_both_orders_with_deterministic_barriers() {
        let root = temp_workspace();
        let path = root.join("src/main.sg");
        let uri = Url::from_file_path(&path).unwrap();
        let index = Arc::new(
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap(),
        );
        assert!(index.open(
            uri.clone(),
            2,
            "def overlay_two() -> i64 { 2 }\n".into(),
            &IndexCancellation::default()
        ));

        let change_gate = install_publication_gate(&uri, TestPublicationKind::Overlay);
        let change_index = Arc::clone(&index);
        let change_uri = uri.clone();
        let change = tokio::spawn(run_index_operation(move |cancellation| {
            change_index.change(
                &change_uri,
                3,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "def overlay_three() -> i64 { 3 }\n".into(),
                }],
                &cancellation,
            )
        }));
        wait_gate_reached(Arc::clone(&change_gate)).await;
        assert!(index.save(
            &uri,
            Some("def disk_saved() -> i64 { 30 }\n".into()),
            &IndexCancellation::default()
        ));
        release_gate(change_gate).await;
        assert!(change.await.unwrap());
        assert_eq!(index.document(&uri).unwrap().revision, Some(3));
        assert!(index
            .document(&uri)
            .unwrap()
            .content
            .contains("overlay_three"));
        assert!(index.snapshot().disk_documents[&uri]
            .content
            .contains("disk_saved"));

        let refresh_before_change_gate =
            install_publication_gate(&uri, TestPublicationKind::Overlay);
        let refresh_before_change_index = Arc::clone(&index);
        let refresh_before_change_uri = uri.clone();
        let change_after_refresh = tokio::spawn(run_index_operation(move |cancellation| {
            refresh_before_change_index.change(
                &refresh_before_change_uri,
                4,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "def overlay_four() -> i64 { 4 }\n".into(),
                }],
                &cancellation,
            )
        }));
        wait_gate_reached(Arc::clone(&refresh_before_change_gate)).await;
        fs::write(&path, "def disk_early_refresh() -> i64 { 35 }\n").unwrap();
        assert!(index.refresh_file(&uri, &IndexCancellation::default()));
        release_gate(refresh_before_change_gate).await;
        assert!(change_after_refresh.await.unwrap());
        assert_eq!(index.document(&uri).unwrap().revision, Some(4));
        assert!(index
            .document(&uri)
            .unwrap()
            .content
            .contains("overlay_four"));
        assert!(index.snapshot().disk_documents[&uri]
            .content
            .contains("disk_early_refresh"));

        let refresh_gate = install_publication_gate(&uri, TestPublicationKind::Refresh);
        fs::write(&path, "def disk_refreshed() -> i64 { 40 }\n").unwrap();
        let refresh_index = Arc::clone(&index);
        let refresh_uri = uri.clone();
        let refresh = tokio::spawn(run_index_operation(move |cancellation| {
            refresh_index.refresh_file(&refresh_uri, &cancellation)
        }));
        wait_gate_reached(Arc::clone(&refresh_gate)).await;
        assert!(index.change(
            &uri,
            5,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "def overlay_five() -> i64 { 5 }\n".into(),
            }],
            &IndexCancellation::default()
        ));
        release_gate(refresh_gate).await;
        assert!(refresh.await.unwrap());
        assert_eq!(index.document(&uri).unwrap().revision, Some(5));
        assert!(index
            .document(&uri)
            .unwrap()
            .content
            .contains("overlay_five"));
        assert!(index.snapshot().disk_documents[&uri]
            .content
            .contains("disk_refreshed"));

        let save_gate = install_publication_gate(&uri, TestPublicationKind::Save);
        let save_index = Arc::clone(&index);
        let save_uri = uri.clone();
        let save = tokio::spawn(run_index_operation(move |cancellation| {
            save_index.save(
                &save_uri,
                Some("def disk_late_save() -> i64 { 50 }\n".into()),
                &cancellation,
            )
        }));
        wait_gate_reached(Arc::clone(&save_gate)).await;
        assert!(index.change(
            &uri,
            6,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "def overlay_six() -> i64 { 6 }\n".into(),
            }],
            &IndexCancellation::default()
        ));
        release_gate(save_gate).await;
        assert!(save.await.unwrap());
        assert_eq!(index.document(&uri).unwrap().revision, Some(6));
        assert!(index
            .document(&uri)
            .unwrap()
            .content
            .contains("overlay_six"));
        assert!(index.snapshot().disk_documents[&uri]
            .content
            .contains("disk_late_save"));

        assert!(index.close(&uri));
        assert_eq!(index.document(&uri).unwrap().revision, None);
        assert!(index
            .document(&uri)
            .unwrap()
            .content
            .contains("disk_late_save"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn close_and_delete_advance_only_their_owned_epochs() {
        let root = temp_workspace();
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let initial = WorkspaceIndex::entry_epoch(&index.snapshot(), &uri);
        assert!(index.open(
            uri.clone(),
            1,
            "def overlay() -> i64 { 1 }\n".into(),
            &IndexCancellation::default(),
        ));
        let opened = WorkspaceIndex::entry_epoch(&index.snapshot(), &uri);
        assert_eq!(opened.disk, initial.disk);
        assert_eq!(opened.overlay, initial.overlay + 1);

        assert!(index.save(
            &uri,
            Some("def disk_saved() -> i64 { 2 }\n".into()),
            &IndexCancellation::default(),
        ));
        let saved = WorkspaceIndex::entry_epoch(&index.snapshot(), &uri);
        assert_eq!(saved.disk, opened.disk + 1);
        assert_eq!(saved.overlay, opened.overlay);
        assert!(index.document(&uri).unwrap().content.contains("overlay"));

        assert!(index.remove_file(&uri));
        let deleted = WorkspaceIndex::entry_epoch(&index.snapshot(), &uri);
        assert_eq!(deleted.disk, saved.disk + 1);
        assert_eq!(deleted.overlay, saved.overlay);
        assert!(index.document(&uri).is_some());

        assert!(index.close(&uri));
        let closed = WorkspaceIndex::entry_epoch(&index.snapshot(), &uri);
        assert_eq!(closed.disk, deleted.disk);
        assert_eq!(closed.overlay, deleted.overlay + 1);
        assert!(index.document(&uri).is_none());
        let _ = fs::remove_dir_all(root);
    }
}
