//! sglsp - Sengoo language server.

use miette::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

mod diagnostics;
mod formatting;
mod semantic;
mod signatures;
mod stdlib;
mod symbols;
mod text_editing;
mod workspace;
use diagnostics::{build_diagnostics, compiler_diagnostics_from_sgc_json, quick_fix_actions};
use formatting::{full_document_range, normalized_format};
#[cfg(test)]
use semantic::SemanticKind;
use semantic::{semantic_legend, semantic_tokens_for};
use signatures::{active_call_site, collect_function_signatures, FunctionSignatureInfo};
use stdlib::{stdlib_symbol_detail_for_content, stdlib_symbols_for_content};
#[cfg(test)]
use symbols::find_symbol_occurrences;
use symbols::{
    collect_ast_symbols, completion_kind_to_symbol_kind, extract_identifier_at,
    valid_identifier_name,
};
use text_editing::{apply_content_changes, folding_ranges_for, position_to_byte_index};
use workspace::{
    completion_symbols_for_documents, find_symbol_detail_in_documents,
    goto_definition_in_documents, references_in_documents, rename_in_documents,
    workspace_documents_for_roots_and_open_documents, workspace_roots_from_initialize,
    workspace_symbols_for_documents,
};

#[tokio::main]
async fn main() -> Result<()> {
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
    documents: RwLock<HashMap<Url, String>>,
    workspace_roots: RwLock<Vec<PathBuf>>,
    config: ServerConfig,
}

impl SengooLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
            workspace_roots: RwLock::new(Vec::new()),
            config: ServerConfig::default(),
        }
    }

    async fn document_text(&self, uri: &Url) -> Option<String> {
        self.documents.read().await.get(uri).cloned()
    }

    async fn publish_diagnostics(&self, uri: &Url) {
        let content = self.document_text(uri).await.unwrap_or_default();
        let mut diagnostics = compiler_diagnostics_from_sgc_json(uri, &content);
        let mut style = build_diagnostics(&content, self.config.max_problems);
        diagnostics.append(&mut style);
        diagnostics.truncate(self.config.max_problems);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn all_documents(&self) -> HashMap<Url, String> {
        self.documents.read().await.clone()
    }

    async fn workspace_documents(&self) -> HashMap<Url, String> {
        let roots = self.workspace_roots.read().await.clone();
        let open_documents = self.all_documents().await;
        workspace_documents_for_roots_and_open_documents(&roots, &open_documents)
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SengooLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        *self.workspace_roots.write().await = workspace_roots_from_initialize(&params);
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
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
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
        self.documents.write().await.insert(uri.clone(), content);
        self.publish_diagnostics(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let content_changes = params.content_changes;
        let mut documents = self.documents.write().await;
        if let Some(current) = documents.get_mut(&uri) {
            apply_content_changes(current, content_changes);
        } else if let Some(last) = content_changes.last() {
            documents.insert(uri.clone(), last.text.clone());
        }
        drop(documents);
        self.publish_diagnostics(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let content = self.document_text(&uri).await.unwrap_or_default();
        let documents = self.workspace_documents().await;
        let ast_symbols = completion_symbols_for_documents(&uri, &documents);

        let mut items = vec![
            CompletionItem::new_simple("fn".to_string(), "Define a function".to_string()),
            CompletionItem::new_simple("struct".to_string(), "Define a struct".to_string()),
            CompletionItem::new_simple("let".to_string(), "Declare a local variable".to_string()),
            CompletionItem::new_simple("const".to_string(), "Declare a constant".to_string()),
            CompletionItem::new_simple("match".to_string(), "Pattern matching".to_string()),
        ];

        let mut seen = std::collections::HashSet::new();
        for symbol in ast_symbols
            .into_iter()
            .chain(stdlib_symbols_for_content(&content))
        {
            if seen.insert(symbol.name.clone()) {
                items.push(CompletionItem {
                    label: symbol.name,
                    kind: Some(symbol.kind),
                    detail: Some(symbol.detail),
                    ..Default::default()
                });
            }
        }

        for line in content.lines() {
            for token in line.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                if token.len() < 2 || !valid_identifier_name(token) {
                    continue;
                }
                if seen.insert(token.to_string()) {
                    items.push(CompletionItem {
                        label: token.to_string(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.workspace_documents().await;
        Ok(goto_definition_in_documents(&uri, position, &documents))
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

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.workspace_documents().await;
        let Some(content) = documents.get(&uri).cloned() else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(&content, position) else {
            return Ok(None);
        };
        if let Some(item) = find_symbol_detail_in_documents(&uri, &symbol.name, &documents)
            .or_else(|| stdlib_symbol_detail_for_content(&content, &symbol.name))
        {
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

        let Some(offset) = position_to_byte_index(&content, position) else {
            return Ok(None);
        };
        let Some((call_name, active_param)) = active_call_site(&content, offset) else {
            return Ok(None);
        };

        let mut signatures = collect_function_signatures(&content)
            .into_iter()
            .filter(|sig| sig.name == call_name)
            .collect::<Vec<_>>();

        if call_name == "print" && signatures.is_empty() {
            signatures.push(FunctionSignatureInfo {
                name: "print".to_string(),
                label: "def print(value: Any) -> unit".to_string(),
                params: vec!["value: Any".to_string()],
                range: full_document_range(&content),
            });
        }

        if signatures.is_empty() {
            return Ok(None);
        }

        signatures.sort_by_key(|sig| (sig.range.start.line, sig.range.start.character));

        let signature_items = signatures
            .iter()
            .map(|sig| SignatureInformation {
                label: sig.label.clone(),
                documentation: None,
                parameters: Some(
                    sig.params
                        .iter()
                        .map(|param| ParameterInformation {
                            label: ParameterLabel::Simple(param.clone()),
                            documentation: None,
                        })
                        .collect(),
                ),
                active_parameter: None,
            })
            .collect::<Vec<_>>();

        let first_param_count = signatures.first().map(|sig| sig.params.len()).unwrap_or(0);
        let clamped_active_param = if first_param_count == 0 {
            0
        } else {
            active_param.min((first_param_count.saturating_sub(1)) as u32)
        };

        Ok(Some(SignatureHelp {
            signatures: signature_items,
            active_signature: Some(0),
            active_parameter: Some(clamped_active_param),
        }))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        let symbols = collect_ast_symbols(&content);
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
        let documents = self.workspace_documents().await;
        let symbols = workspace_symbols_for_documents(&params.query, &documents);
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

        let actions = quick_fix_actions(uri, &content, params.context.diagnostics);

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
        let formatted = normalized_format(&content);
        if formatted == content {
            return Ok(Some(Vec::new()));
        }

        Ok(Some(vec![TextEdit {
            range: full_document_range(&content),
            new_text: formatted,
        }]))
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let (name, active_param) = active_call_site(src, cursor).expect("call site should exist");
        assert_eq!(name, "foo");
        assert_eq!(active_param, 2);
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
