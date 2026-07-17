//! sglsp - Sengoo language server.

use miette::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

mod completion;
mod completion_context;
mod dependency_sources;
mod diagnostics;
mod formatting;
#[cfg(test)]
mod golden_tests;
mod protocol;
mod semantic;
mod signatures;
mod stdlib;
mod symbols;
mod text_editing;
mod workspace;
mod workspace_index;
use completion::{completion_items_for_request, resolve_completion_item};
use diagnostics::{build_diagnostics, quick_fix_actions, semantic_diagnostics_for_document};
use formatting::{full_document_range, normalized_format, range_format_edit};
use protocol::completion_experimental_capability;
#[cfg(test)]
use semantic::SemanticKind;
use semantic::{semantic_legend, semantic_tokens_for};
#[cfg(test)]
use signatures::active_call_site;
#[cfg(test)]
use signatures::collect_function_signatures;
use signatures::signature_help_for_request;
#[cfg(test)]
use stdlib::stdlib_definition_for_content;
#[cfg(test)]
use symbols::{collect_ast_symbols, find_declaration_in_text};
use symbols::{
    completion_kind_to_symbol_kind, extract_identifier_at, find_symbol_occurrences,
    valid_identifier_name,
};
#[cfg(test)]
use text_editing::apply_content_changes;
use text_editing::folding_ranges_for;
#[cfg(test)]
use workspace::workspace_documents_for_roots_and_open_documents;
#[cfg(test)]
use workspace::{
    completion_symbols_for_documents, find_symbol_detail_in_documents,
    goto_definition_in_documents, workspace_symbols_for_documents,
};
use workspace::{references_in_documents, rename_in_documents, workspace_roots_from_initialize};
use workspace_index::{rebuild_workspace_index, run_index_operation, WorkspaceIndex};

const SGLSP_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("SENGOO_BUILD_HASH"),
    ")"
);

fn smart_completion_options() -> CompletionOptions {
    CompletionOptions {
        resolve_provider: Some(true),
        trigger_characters: Some(vec![".".into(), ":".into(), "#".into()]),
        ..Default::default()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("sglsp {SGLSP_VERSION}");
        return Ok(());
    }

    tracing_subscriber::fmt().init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(SengooLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}

#[derive(Debug, Clone)]
struct ServerConfig {
    max_problems: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { max_problems: 128 }
    }
}

#[derive(Debug)]
struct SengooLanguageServer {
    client: Client,
    index: Arc<WorkspaceIndex>,
    config: ServerConfig,
}

impl SengooLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            index: Arc::new(WorkspaceIndex::default()),
            config: ServerConfig::default(),
        }
    }

    async fn document_text(&self, uri: &Url) -> Option<String> {
        self.index
            .document(uri)
            .map(|document| document.content.to_string())
    }

    async fn publish_diagnostics(&self, uri: &Url) {
        let content = self.document_text(uri).await.unwrap_or_default();
        let mut diagnostics = semantic_diagnostics_for_document(&self.index, uri, &content);
        let mut style = build_diagnostics(&content, self.config.max_problems);
        diagnostics.append(&mut style);
        diagnostics.truncate(self.config.max_problems);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn workspace_documents(&self) -> HashMap<Url, String> {
        self.index.documents()
    }

    async fn record_index_result(&self, operation: &str, uri: &Url, published: bool) {
        if !published {
            self.client
                .log_message(
                    MessageType::LOG,
                    format!(
                        "ignored stale or cancelled workspace-index {operation} result for {uri}"
                    ),
                )
                .await;
        }
    }
}

fn stdlib_definition_for_cursor_fallback(
    uri: &Url,
    symbol_range: Range,
    definition: &Option<GotoDefinitionResponse>,
    stdlib_definition: Option<Location>,
) -> Option<GotoDefinitionResponse> {
    let definition_points_at_cursor = matches!(
        definition,
        Some(GotoDefinitionResponse::Scalar(location))
            if location.uri == *uri && location.range == symbol_range
    );
    if definition.is_some() && !definition_points_at_cursor {
        return None;
    }

    stdlib_definition.map(GotoDefinitionResponse::Scalar)
}

fn document_highlights_for_content(
    content: &str,
    position: Position,
) -> Option<Vec<DocumentHighlight>> {
    let symbol = extract_identifier_at(content, position)?;
    let highlights = find_symbol_occurrences(content, &symbol.name)
        .into_iter()
        .map(|range| DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::TEXT),
        })
        .collect::<Vec<_>>();

    (!highlights.is_empty()).then_some(highlights)
}

fn prepare_rename_for_content(content: &str, position: Position) -> Option<PrepareRenameResponse> {
    let symbol = extract_identifier_at(content, position)?;
    if !valid_identifier_name(&symbol.name) {
        return None;
    }

    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: symbol.range,
        placeholder: symbol.name,
    })
}

#[tower_lsp::async_trait]
impl LanguageServer for SengooLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let roots = workspace_roots_from_initialize(&params);
        if let Err(error) = rebuild_workspace_index(Arc::clone(&self.index), roots).await {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("workspace index initialization failed: {error}"),
                )
                .await;
        }
        let snapshot = self.index.snapshot();
        let metrics = self.index.metrics();
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "indexed {} Sengoo documents (generation {}, scans {}, parses {}, stdlib metadata v{})",
                    snapshot.documents.len(),
                    snapshot.generation,
                    metrics.recursive_scans,
                    metrics.parsed_documents,
                    snapshot.stdlib_metadata_revision,
                ),
            )
            .await;
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(smart_completion_options()),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: Some(vec![",".to_string()]),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                            legend: semantic_legend(),
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                experimental: Some(completion_experimental_capability()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Sengoo LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let index = Arc::clone(&self.index);
        let operation_uri = uri.clone();
        let published = run_index_operation(move |cancellation| {
            index.open(
                operation_uri,
                params.text_document.version,
                content,
                &cancellation,
            )
        })
        .await;
        self.record_index_result("open", &uri, published).await;
        self.publish_diagnostics(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let index = Arc::clone(&self.index);
        let operation_uri = uri.clone();
        let published = run_index_operation(move |cancellation| {
            index.change(
                &operation_uri,
                params.text_document.version,
                params.content_changes,
                &cancellation,
            )
        })
        .await;
        self.record_index_result("change", &uri, published).await;
        self.publish_diagnostics(&uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let index = Arc::clone(&self.index);
        let operation_uri = uri.clone();
        let published = run_index_operation(move |cancellation| {
            index.save(&operation_uri, params.text, &cancellation)
        })
        .await;
        self.record_index_result("save", &uri, published).await;
        self.publish_diagnostics(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.index.close(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for change in params.changes {
            match change.typ {
                FileChangeType::DELETED => {
                    let published = self.index.remove_file(&change.uri);
                    self.record_index_result("delete", &change.uri, published)
                        .await;
                }
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    let index = Arc::clone(&self.index);
                    let uri = change.uri;
                    let operation_uri = uri.clone();
                    let published = run_index_operation(move |cancellation| {
                        index.refresh_file(&operation_uri, &cancellation)
                    })
                    .await;
                    self.record_index_result("refresh", &uri, published).await;
                }
                _ => {}
            }
        }
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        Ok(Some(CompletionResponse::Array(
            completion_items_for_request(&self.index, &uri, position),
        )))
    }

    async fn completion_resolve(&self, item: CompletionItem) -> LspResult<CompletionItem> {
        Ok(resolve_completion_item(&self.index, item))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let content = self.document_text(&uri).await;
        let symbol = content
            .as_deref()
            .and_then(|content| extract_identifier_at(content, position));
        let definition = self.index.goto_definition(&uri, position);

        if let Some(symbol) = symbol.as_ref() {
            if let Some(stdlib_definition) = stdlib_definition_for_cursor_fallback(
                &uri,
                symbol.range,
                &definition,
                self.index.stdlib_definition(&uri, &symbol.name),
            ) {
                return Ok(Some(stdlib_definition));
            }
        }

        Ok(definition)
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let documents = self.workspace_documents().await;
        Ok(references_in_documents(
            &uri,
            position,
            params.context.include_declaration,
            &documents,
        ))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> LspResult<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        Ok(document_highlights_for_content(&content, position))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(&content, position) else {
            return Ok(None);
        };
        if let Some(item) = self.index.symbol_detail(&uri, &symbol.name) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("`{}` ({})", item.name, item.detail),
                }),
                range: Some(symbol.range),
            }));
        }

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("`{}`", symbol.name),
            }),
            range: Some(symbol.range),
        }))
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> LspResult<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        Ok(signature_help_for_request(
            &self.index,
            &uri,
            &content,
            position,
        ))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(_content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        let symbols = self.index.document_symbols(&uri);
        if symbols.is_empty() {
            return Ok(None);
        }

        #[allow(deprecated)]
        let response = symbols
            .into_iter()
            .map(|symbol| SymbolInformation {
                name: symbol.name,
                kind: completion_kind_to_symbol_kind(symbol.kind),
                tags: None,
                deprecated: None,
                location: Location::new(uri.clone(), symbol.range),
                container_name: Some(symbol.detail),
            })
            .collect::<Vec<_>>();

        Ok(Some(DocumentSymbolResponse::Flat(response)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<Vec<SymbolInformation>>> {
        let symbols = self.index.workspace_symbols(&params.query);
        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(symbols))
        }
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> LspResult<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        Ok(Some(folding_ranges_for(&content)))
    }
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let Some(content) = self.document_text(&params.text_document.uri).await else {
            return Ok(None);
        };

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: semantic_tokens_for(&content),
        })))
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        let actions = quick_fix_actions(&self.index, uri, &content, params.context.diagnostics);

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        let formatted = normalized_format(&content);
        if formatted == content {
            return Ok(Some(Vec::new()));
        }

        Ok(Some(vec![TextEdit {
            range: full_document_range(&content),
            new_text: formatted,
        }]))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        Ok(Some(
            range_format_edit(&content, params.range)
                .into_iter()
                .collect(),
        ))
    }

    async fn rename(&self, params: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let documents = self.workspace_documents().await;
        Ok(rename_in_documents(
            &uri,
            position,
            &params.new_name,
            &documents,
        ))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> LspResult<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        Ok(prepare_rename_for_content(&content, position))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_capability_advertises_hash_trigger_and_resolve() {
        let options = smart_completion_options();
        assert_eq!(options.resolve_provider, Some(true));
        assert!(options
            .trigger_characters
            .as_ref()
            .is_some_and(|triggers| triggers.iter().any(|trigger| trigger == "#")));
    }
    use crate::workspace::workspace_symbols_for_roots_and_documents;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn extract_identifier_handles_cursor_inside_word() {
        let text = "let answer = 42;";
        let symbol = extract_identifier_at(
            text,
            Position {
                line: 0,
                character: 5,
            },
        )
        .expect("identifier should exist");

        assert_eq!(symbol.name, "answer");
        assert_eq!(symbol.range.start.character, 4);
        assert_eq!(symbol.range.end.character, 10);
    }

    #[test]
    fn references_are_word_boundary_aware() {
        let text = "foo foobar foo_1\nfoo";
        let ranges = find_symbol_occurrences(text, "foo");
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn references_ignore_comments_and_string_literals() {
        let text = "foo // foo\nlet label = \"foo \\\" foo\";\nfoo";
        let ranges = find_symbol_occurrences(text, "foo");

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start.line, 0);
        assert_eq!(ranges[1].start.line, 2);
    }

    #[test]
    fn document_highlights_use_code_occurrences() {
        let text = "foo // foo\nlet label = \"foo\";\nfoo";
        let highlights = document_highlights_for_content(
            text,
            Position {
                line: 0,
                character: 1,
            },
        )
        .expect("document highlights should be available");

        assert_eq!(highlights.len(), 2);
        assert!(highlights
            .iter()
            .all(|highlight| highlight.kind == Some(DocumentHighlightKind::TEXT)));
        assert_eq!(highlights[0].range.start.line, 0);
        assert_eq!(highlights[1].range.start.line, 2);
    }

    #[test]
    fn declaration_lookup_ignores_comments_and_string_literals() {
        let text = "// def hidden() -> i64 { 0 }\nlet label = \"def hidden()\";\ndef visible() -> i64 { 0 }";

        assert!(find_declaration_in_text(text, "hidden").is_none());
        assert_eq!(
            find_declaration_in_text(text, "visible")
                .expect("visible declaration should be found")
                .start
                .line,
            2
        );
    }

    #[test]
    fn prepare_rename_returns_current_identifier_range() {
        let text = "def main() -> i64 {\n    let answer = 42;\n    answer\n}";
        let response = prepare_rename_for_content(
            text,
            Position {
                line: 1,
                character: 8,
            },
        )
        .expect("identifier should be renameable");

        match response {
            PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } => {
                assert_eq!(placeholder, "answer");
                assert_eq!(range.start.line, 1);
                assert_eq!(range.start.character, 8);
                assert_eq!(range.end.character, 14);
            }
            other => panic!("expected range with placeholder, got {other:?}"),
        }
    }

    #[test]
    fn semantic_tokens_encode_fn_name_as_function() {
        let text = "fn solve(x: i32) {\n    return x\n}";
        let tokens = semantic_tokens_for(text);
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticKind::Function as u32));
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticKind::Type as u32));
    }

    #[test]
    fn incremental_change_applies_range_patch() {
        let mut content = "def main() -> i64 {\n    1\n}\n".to_string();
        apply_content_changes(
            &mut content,
            vec![TextDocumentContentChangeEvent {
                range: Some(Range {
                    start: Position {
                        line: 1,
                        character: 4,
                    },
                    end: Position {
                        line: 1,
                        character: 5,
                    },
                }),
                range_length: None,
                text: "2".to_string(),
            }],
        );
        assert!(content.contains("    2"));
    }

    #[test]
    fn ast_symbol_collection_reads_top_level_decls() {
        let src = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 { 0 }
"#;
        let symbols = collect_ast_symbols(src);
        assert!(symbols.iter().any(|s| s.name == "Point"));
        assert!(symbols.iter().any(|s| s.name == "main"));
    }

    #[test]
    fn workspace_symbols_search_open_documents_case_insensitively() {
        let mut documents = HashMap::new();
        let points_uri = Url::parse("file:///workspace/points.sg").unwrap();
        let tasks_uri = Url::parse("file:///workspace/tasks.sg").unwrap();
        documents.insert(
            points_uri.clone(),
            "struct Point { x: i64, y: i64 }\n".to_string(),
        );
        documents.insert(tasks_uri, "def solve() -> i64 { 0 }\n".to_string());

        let symbols = workspace_symbols_for_documents("po", &documents);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Point");
        assert_eq!(symbols[0].location.uri, points_uri);
        assert_eq!(symbols[0].kind, SymbolKind::STRUCT);
    }

    #[test]
    fn completion_symbols_include_workspace_documents_current_first() {
        let current_uri = Url::parse("file:///workspace/main.sg").unwrap();
        let shared_uri = Url::parse("file:///workspace/shared.sg").unwrap();
        let mut documents = HashMap::new();
        documents.insert(
            current_uri.clone(),
            "struct LocalThing { value: i64 }\n".to_string(),
        );
        documents.insert(
            shared_uri,
            "struct SharedThing { value: i64 }\ndef helper() -> i64 { 1 }\n".to_string(),
        );

        let symbols = completion_symbols_for_documents(&current_uri, &documents);
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["LocalThing", "SharedThing", "helper"]);
    }

    #[test]
    fn hover_detail_searches_workspace_documents() {
        let current_uri = Url::parse("file:///workspace/main.sg").unwrap();
        let shared_uri = Url::parse("file:///workspace/shared.sg").unwrap();
        let mut documents = HashMap::new();
        documents.insert(
            current_uri.clone(),
            "def main() -> i64 {\n    SharedThing\n}\n".to_string(),
        );
        documents.insert(
            shared_uri,
            "struct SharedThing { value: i64 }\n".to_string(),
        );

        let symbol = find_symbol_detail_in_documents(&current_uri, "SharedThing", &documents)
            .expect("workspace symbol should be found");

        assert_eq!(symbol.name, "SharedThing");
        assert_eq!(symbol.detail, "struct");
    }

    #[test]
    fn signature_symbols_include_workspace_documents_current_first() {
        let current_uri = Url::parse("file:///workspace/main.sg").unwrap();
        let shared_uri = Url::parse("file:///workspace/shared.sg").unwrap();
        let mut documents = HashMap::new();
        documents.insert(
            current_uri.clone(),
            "def local_call(value: i64) -> i64 { value }\n".to_string(),
        );
        documents.insert(
            shared_uri,
            "def shared_call(flag: bool) -> bool { flag }\n".to_string(),
        );

        let signatures = workspace::function_signatures_for_documents(&current_uri, &documents);
        let labels = signatures
            .iter()
            .map(|signature| signature.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec![
                "def local_call(value: i64) -> i64",
                "def shared_call(flag: bool) -> bool"
            ]
        );
    }

    #[test]
    fn workspace_symbols_search_disk_roots_and_open_document_overlays() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_workspace_symbols_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let nested = root.join("src");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("feature.sg"),
            "struct DiskThing { value: i64 }\n",
        )
        .unwrap();
        fs::write(
            root.join("notes.txt"),
            "struct IgnoredThing { value: i64 }\n",
        )
        .unwrap();

        let open_path = root.join("open.sg");
        fs::write(&open_path, "struct DiskVersion { value: i64 }\n").unwrap();
        let open_uri = Url::from_file_path(&open_path).unwrap();
        let mut documents = HashMap::new();
        documents.insert(
            open_uri.clone(),
            "struct OverlayThing { value: i64 }\n".to_string(),
        );

        let symbols = workspace_symbols_for_roots_and_documents(
            "thing",
            std::slice::from_ref(&root),
            &documents,
        );
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["OverlayThing", "DiskThing"]);
        assert_eq!(symbols[0].location.uri, open_uri);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_symbols_skip_generated_and_cache_directories() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_workspace_skip_dirs_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        fs::write(
            root.join("src/app.sg"),
            "struct SourceThing { value: i64 }\n",
        )
        .unwrap();
        fs::write(
            root.join("target/debug/generated.sg"),
            "struct GeneratedThing { value: i64 }\n",
        )
        .unwrap();
        fs::write(
            root.join(".git/hooks/hook.sg"),
            "struct GitThing { value: i64 }\n",
        )
        .unwrap();

        let symbols = workspace_symbols_for_roots_and_documents(
            "thing",
            std::slice::from_ref(&root),
            &HashMap::new(),
        );
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["SourceThing"]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn goto_definition_searches_disk_workspace_documents() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_goto_definition_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let defs_path = root.join("defs.sg");
        let main_path = root.join("main.sg");
        fs::write(&defs_path, "struct SharedThing { value: i64 }\n").unwrap();
        fs::write(&main_path, "def main() -> i64 {\n    SharedThing\n}\n").unwrap();

        let defs_uri = Url::from_file_path(&defs_path).unwrap();
        let main_uri = Url::from_file_path(&main_path).unwrap();
        let documents = workspace_documents_for_roots_and_open_documents(
            std::slice::from_ref(&root),
            &HashMap::new(),
        );

        let definition = goto_definition_in_documents(
            &main_uri,
            Position {
                line: 1,
                character: 7,
            },
            &documents,
        )
        .expect("definition should resolve across disk workspace files");

        match definition {
            GotoDefinitionResponse::Scalar(location) => {
                assert_eq!(location.uri, defs_uri);
                assert_eq!(location.range.start.line, 0);
            }
            other => panic!("expected scalar definition, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn goto_definition_reaches_path_dependency_module() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_goto_dep_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let app = root.join("app");
        let dep = root.join("dep");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(dep.join("src")).unwrap();
        fs::write(dep.join("src/lib.sg"), "def dep_answer() -> i64 { 7 }\n").unwrap();
        fs::write(
            app.join("src/main.sg"),
            "import dep;\ndef main() -> i64 {\n    dep_answer()\n}\n",
        )
        .unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            r#"version = 1
root = "app"

[[package]]
name = "dep"
version = "0.1.0"
source = "path+../dep"
manifest = "../dep/Sengoo.toml"

[[package]]
name = "app"
version = "0.1.0"
source = "path+."
manifest = "Sengoo.toml"
"#,
        )
        .unwrap();

        let dep_lib = dep.join("src/lib.sg");
        let dep_uri = Url::from_file_path(&dep_lib).unwrap();
        let main_uri = Url::from_file_path(app.join("src/main.sg")).unwrap();
        let documents = workspace_documents_for_roots_and_open_documents(
            std::slice::from_ref(&app),
            &HashMap::new(),
        );

        let definition = goto_definition_in_documents(
            &main_uri,
            Position {
                line: 2,
                character: 4,
            },
            &documents,
        )
        .expect("definition should resolve into path dependency sources");

        match definition {
            GotoDefinitionResponse::Scalar(location) => {
                assert_eq!(location.uri, dep_uri);
            }
            other => panic!("expected scalar definition, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn completion_symbols_include_graphics_path_dependency_modules() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_graphics_deps_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let app = root.join("app");
        let sggame = root.join("sggame");
        let sggui = root.join("sggui");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(sggame.join("src")).unwrap();
        fs::create_dir_all(sggui.join("src")).unwrap();
        fs::write(
            app.join("src/main.sg"),
            "import sggame;\nimport sggui;\ndef main() -> i64 {\n    0\n}\n",
        )
        .unwrap();
        fs::write(
            sggame.join("src/lib.sg"),
            "def sggame_ready_symbol() -> i64 { 1 }\n",
        )
        .unwrap();
        fs::write(
            sggui.join("src/lib.sg"),
            "def sggui_ready_symbol() -> i64 { 2 }\n",
        )
        .unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            r#"version = 1
root = "app"

[[package]]
name = "sggame"
version = "0.1.0"
source = "path+../sggame"
manifest = "../sggame/Sengoo.toml"

[[package]]
name = "sggui"
version = "0.1.0"
source = "path+../sggui"
manifest = "../sggui/Sengoo.toml"

[[package]]
name = "app"
version = "0.1.0"
source = "path+."
manifest = "Sengoo.toml"
"#,
        )
        .unwrap();

        let main_uri = Url::from_file_path(app.join("src/main.sg")).unwrap();
        let documents = workspace_documents_for_roots_and_open_documents(
            std::slice::from_ref(&app),
            &HashMap::new(),
        );
        let symbols = completion_symbols_for_documents(&main_uri, &documents);
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(
            names.contains(&"sggame_ready_symbol"),
            "expected sggame completion symbol, got {names:?}"
        );
        assert!(
            names.contains(&"sggui_ready_symbol"),
            "expected sggui completion symbol, got {names:?}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn goto_definition_replaces_cursor_fallback_with_stdlib_definition() {
        let uri = Url::parse("file:///workspace/main.sg").unwrap();
        let content =
            "import std::option;\ndef main() -> i64 {\n    option_some_i64(1).unwrap()\n}\n";
        let symbol = extract_identifier_at(
            content,
            Position {
                line: 2,
                character: 25,
            },
        )
        .expect("unwrap symbol should be found");
        let cursor_fallback = Some(GotoDefinitionResponse::Scalar(Location::new(
            uri.clone(),
            symbol.range,
        )));

        let definition = stdlib_definition_for_cursor_fallback(
            &uri,
            symbol.range,
            &cursor_fallback,
            stdlib_definition_for_content(content, &symbol.name),
        )
        .expect("stdlib definition should replace cursor fallback");

        match definition {
            GotoDefinitionResponse::Scalar(location) => {
                assert_eq!(location.uri.scheme(), "sengoo-stdlib");
                assert!(location.uri.as_str().ends_with("/option.sg"));
            }
            other => panic!("expected scalar stdlib definition, got {other:?}"),
        }
    }

    #[test]
    fn references_search_disk_workspace_documents() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_references_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let defs_path = root.join("defs.sg");
        let main_path = root.join("main.sg");
        fs::write(&defs_path, "struct SharedThing { value: i64 }\n").unwrap();
        fs::write(
            &main_path,
            "def main() -> i64 {\n    SharedThing\n    SharedThing\n}\n",
        )
        .unwrap();

        let defs_uri = Url::from_file_path(&defs_path).unwrap();
        let main_uri = Url::from_file_path(&main_path).unwrap();
        let documents = workspace_documents_for_roots_and_open_documents(
            std::slice::from_ref(&root),
            &HashMap::new(),
        );

        let references = references_in_documents(
            &main_uri,
            Position {
                line: 1,
                character: 7,
            },
            true,
            &documents,
        )
        .expect("references should resolve across disk workspace files");

        assert_eq!(references.len(), 3);
        assert!(references.iter().any(|location| location.uri == defs_uri));
        assert_eq!(
            references
                .iter()
                .filter(|location| location.uri == main_uri)
                .count(),
            2
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rename_builds_workspace_edit_for_disk_workspace_documents() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_rename_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let defs_path = root.join("defs.sg");
        let main_path = root.join("main.sg");
        fs::write(&defs_path, "struct SharedThing { value: i64 }\n").unwrap();
        fs::write(
            &main_path,
            "def main() -> i64 {\n    SharedThing\n    SharedThing\n}\n",
        )
        .unwrap();

        let defs_uri = Url::from_file_path(&defs_path).unwrap();
        let main_uri = Url::from_file_path(&main_path).unwrap();
        let documents = workspace_documents_for_roots_and_open_documents(
            std::slice::from_ref(&root),
            &HashMap::new(),
        );

        let edit = rename_in_documents(
            &main_uri,
            Position {
                line: 1,
                character: 7,
            },
            "RenamedThing",
            &documents,
        )
        .expect("rename should produce workspace edits across disk files");
        let changes = edit.changes.expect("rename should use document changes");

        assert_eq!(changes.get(&defs_uri).map(Vec::len), Some(1));
        assert_eq!(changes.get(&main_uri).map(Vec::len), Some(2));
        assert!(changes
            .values()
            .flatten()
            .all(|text_edit| text_edit.new_text == "RenamedThing"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn active_call_site_counts_nested_arguments() {
        let src = "def main() -> i64 {\n    foo(1, bar(2, 3), 4)\n}\n";
        let cursor = src.find("4)").expect("third arg should exist");
        let call = active_call_site(src, cursor).expect("call site should exist");
        assert_eq!(call.callee, "foo");
        assert_eq!(call.argument_index, 2);
    }
    #[test]
    fn folding_ranges_include_regions_and_comment_blocks() {
        let src =
            "// one\n// two\ndef main() -> i64 {\n    if true {\n        1\n    }\n    0\n}\n";
        let ranges = folding_ranges_for(src);

        assert!(ranges
            .iter()
            .any(|range| range.kind == Some(FoldingRangeKind::Comment)
                && range.start_line == 0
                && range.end_line == 1));
        assert!(ranges
            .iter()
            .any(|range| range.kind == Some(FoldingRangeKind::Region)
                && range.start_line == 2
                && range.end_line == 7));
        assert!(ranges
            .iter()
            .any(|range| range.kind == Some(FoldingRangeKind::Region)
                && range.start_line == 3
                && range.end_line == 5));
    }

    #[test]
    fn collect_function_signatures_reads_function_labels() {
        let src = r#"
def add(a: i64, b: i64) -> i64 {
    a + b
}
"#;

        let signatures = collect_function_signatures(src);
        assert!(signatures.iter().any(|sig| sig.name == "add"));
        assert!(signatures.iter().any(|sig| sig.label.contains("def add(")));
    }
}
