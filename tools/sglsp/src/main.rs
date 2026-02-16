//! sglsp - Sengoo language server.

use miette::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

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

#[derive(Debug, Clone, Copy)]
enum SemanticKind {
    Keyword = 0,
    Function = 1,
    Struct = 2,
    Type = 3,
    Variable = 4,
    Number = 5,
    String = 6,
    Comment = 7,
}

#[derive(Debug, Clone, Copy)]
struct RawSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    kind: SemanticKind,
}

#[derive(Debug, Clone)]
struct SymbolAt {
    name: String,
    range: Range,
}

fn semantic_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::STRUCT,
            SemanticTokenType::TYPE,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::NUMBER,
            SemanticTokenType::STRING,
            SemanticTokenType::COMMENT,
        ],
        token_modifiers: vec![],
    }
}

fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn char_to_byte_index(s: &str, character: u32) -> usize {
    let target = character as usize;
    if target == 0 {
        return 0;
    }

    let mut seen = 0usize;
    for (idx, _) in s.char_indices() {
        if seen == target {
            return idx;
        }
        seen += 1;
    }
    s.len()
}

fn byte_to_char_index(s: &str, byte_idx: usize) -> u32 {
    s[..byte_idx].chars().count() as u32
}

fn line_char_len(line: &str) -> u32 {
    line.chars().count() as u32
}

fn extract_identifier_at(content: &str, position: Position) -> Option<SymbolAt> {
    let line = content.lines().nth(position.line as usize)?;
    if line.is_empty() {
        return None;
    }

    let mut cursor = char_to_byte_index(line, position.character);
    if cursor >= line.len() {
        cursor = line.len().saturating_sub(1);
    }

    let bytes = line.as_bytes();
    if !is_identifier_byte(bytes[cursor]) {
        if cursor == 0 || !is_identifier_byte(bytes[cursor - 1]) {
            return None;
        }
        cursor -= 1;
    }

    let mut start = cursor;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = cursor;
    while end < bytes.len() && is_identifier_byte(bytes[end]) {
        end += 1;
    }

    let start_char = byte_to_char_index(line, start);
    let end_char = byte_to_char_index(line, end);

    Some(SymbolAt {
        name: line[start..end].to_string(),
        range: Range {
            start: Position {
                line: position.line,
                character: start_char,
            },
            end: Position {
                line: position.line,
                character: end_char,
            },
        },
    })
}

fn find_symbol_occurrences(content: &str, symbol: &str) -> Vec<Range> {
    if symbol.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let symbol_bytes = symbol.as_bytes();

    for (line_idx, line) in content.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut i = 0usize;

        while i + symbol_bytes.len() <= bytes.len() {
            let matched = &bytes[i..i + symbol_bytes.len()] == symbol_bytes;
            let left_ok = i == 0 || !is_identifier_byte(bytes[i - 1]);
            let right_bound = i + symbol_bytes.len();
            let right_ok = right_bound == bytes.len() || !is_identifier_byte(bytes[right_bound]);

            if matched && left_ok && right_ok {
                ranges.push(Range {
                    start: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, i),
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, right_bound),
                    },
                });
                i += symbol_bytes.len();
            } else {
                i += 1;
            }
        }
    }

    ranges
}

fn find_definition_in_text(content: &str, symbol: &str) -> Option<Range> {
    let declaration_keywords = ["fn", "struct", "let", "const", "type", "enum"];

    for keyword in declaration_keywords {
        let pattern = format!("{keyword} {symbol}");
        for (line_idx, line) in content.lines().enumerate() {
            if let Some(pos) = line.find(&pattern) {
                let symbol_start = pos + keyword.len() + 1;
                let symbol_end = symbol_start + symbol.len();
                let bytes = line.as_bytes();

                let left_ok = symbol_start == 0 || !is_identifier_byte(bytes[symbol_start - 1]);
                let right_ok = symbol_end == bytes.len() || !is_identifier_byte(bytes[symbol_end]);
                if left_ok && right_ok {
                    return Some(Range {
                        start: Position {
                            line: line_idx as u32,
                            character: byte_to_char_index(line, symbol_start),
                        },
                        end: Position {
                            line: line_idx as u32,
                            character: byte_to_char_index(line, symbol_end),
                        },
                    });
                }
            }
        }
    }

    find_symbol_occurrences(content, symbol).into_iter().next()
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "fn" | "struct"
            | "enum"
            | "type"
            | "let"
            | "const"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "match"
            | "return"
            | "break"
            | "continue"
            | "import"
            | "use"
            | "pub"
            | "true"
            | "false"
    )
}

fn is_builtin_type(word: &str) -> bool {
    matches!(
        word,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
            | "bool"
            | "str"
            | "char"
            | "unit"
    )
}

fn semantic_tokens_for(content: &str) -> Vec<SemanticToken> {
    let mut raw = Vec::<RawSemanticToken>::new();

    for (line_idx, line) in content.lines().enumerate() {
        let bytes = line.as_bytes();
        let comment_start = line.find("//");
        let scan_end = comment_start.unwrap_or(line.len());
        let mut i = 0usize;
        let mut pending_decl: Option<&str> = None;

        while i < scan_end {
            let b = bytes[i];
            if b.is_ascii_whitespace() {
                i += 1;
                continue;
            }

            if b == b'"' {
                let start = i;
                i += 1;
                while i < scan_end {
                    if bytes[i] == b'\\' && i + 1 < scan_end {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }

                raw.push(RawSemanticToken {
                    line: line_idx as u32,
                    start: byte_to_char_index(line, start),
                    length: byte_to_char_index(line, i) - byte_to_char_index(line, start),
                    kind: SemanticKind::String,
                });
                continue;
            }

            if b.is_ascii_digit() {
                let start = i;
                while i < scan_end && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                raw.push(RawSemanticToken {
                    line: line_idx as u32,
                    start: byte_to_char_index(line, start),
                    length: byte_to_char_index(line, i) - byte_to_char_index(line, start),
                    kind: SemanticKind::Number,
                });
                pending_decl = None;
                continue;
            }

            if b.is_ascii_alphabetic() || b == b'_' {
                let start = i;
                while i < scan_end && is_identifier_byte(bytes[i]) {
                    i += 1;
                }
                let word = &line[start..i];
                let kind = if is_keyword(word) {
                    pending_decl = Some(word);
                    SemanticKind::Keyword
                } else if matches!(pending_decl, Some("fn")) {
                    pending_decl = None;
                    SemanticKind::Function
                } else if matches!(pending_decl, Some("struct" | "enum")) {
                    pending_decl = None;
                    SemanticKind::Struct
                } else if matches!(pending_decl, Some("let" | "const")) {
                    pending_decl = None;
                    SemanticKind::Variable
                } else if is_builtin_type(word)
                    || word.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                {
                    pending_decl = None;
                    SemanticKind::Type
                } else {
                    pending_decl = None;
                    SemanticKind::Variable
                };

                raw.push(RawSemanticToken {
                    line: line_idx as u32,
                    start: byte_to_char_index(line, start),
                    length: byte_to_char_index(line, i) - byte_to_char_index(line, start),
                    kind,
                });
                continue;
            }

            pending_decl = None;
            i += 1;
        }

        if let Some(start) = comment_start {
            raw.push(RawSemanticToken {
                line: line_idx as u32,
                start: byte_to_char_index(line, start),
                length: line_char_len(line) - byte_to_char_index(line, start),
                kind: SemanticKind::Comment,
            });
        }
    }

    raw.sort_by_key(|t| (t.line, t.start));

    let mut result = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for token in raw {
        let delta_line = token.line - prev_line;
        let delta_start = if delta_line == 0 {
            token.start - prev_start
        } else {
            token.start
        };

        result.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.length,
            token_type: token.kind as u32,
            token_modifiers_bitset: 0,
        });

        prev_line = token.line;
        prev_start = token.start;
    }

    result
}

fn build_diagnostics(content: &str, max_problems: usize) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        if diagnostics.len() >= max_problems {
            break;
        }

        if line.contains("TODO") || line.contains("FIXME") {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: 0,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line_char_len(line),
                    },
                },
                severity: Some(DiagnosticSeverity::HINT),
                code: Some(NumberOrString::String("todo-item".to_string())),
                source: Some("sglsp".to_string()),
                message: "TODO/FIXME marker found".to_string(),
                ..Default::default()
            });
        }

        if diagnostics.len() >= max_problems {
            break;
        }

        if let Some(tab_pos) = line.find('\t') {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, tab_pos),
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, tab_pos + 1),
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("tab-indentation".to_string())),
                source: Some("sglsp".to_string()),
                message: "Tab indentation detected (prefer spaces)".to_string(),
                ..Default::default()
            });
        }

        if diagnostics.len() >= max_problems {
            break;
        }

        let trimmed = line.trim_end_matches([' ', '\t']);
        if trimmed.len() < line.len() {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: line_char_len(trimmed),
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line_char_len(line),
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("trailing-whitespace".to_string())),
                source: Some("sglsp".to_string()),
                message: "Trailing whitespace".to_string(),
                ..Default::default()
            });
        }
    }

    diagnostics
}

fn full_document_range(content: &str) -> Range {
    let mut last_line_idx = 0u32;
    let mut last_line = "";

    for (idx, line) in content.lines().enumerate() {
        last_line_idx = idx as u32;
        last_line = line;
    }

    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: last_line_idx,
            character: line_char_len(last_line),
        },
    }
}

fn normalized_format(content: &str) -> String {
    let mut out = String::new();
    for (idx, line) in content.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end_matches([' ', '\t']));
    }
    out
}

fn valid_identifier_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn quick_fix_action(
    uri: Url,
    edit: TextEdit,
    diagnostic: Diagnostic,
    title: &str,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic]),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri, vec![edit])])),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

#[derive(Debug)]
struct SengooLanguageServer {
    client: Client,
    documents: RwLock<HashMap<Url, String>>,
    config: ServerConfig,
}

impl SengooLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
            config: ServerConfig::default(),
        }
    }

    async fn document_text(&self, uri: &Url) -> Option<String> {
        self.documents.read().await.get(uri).cloned()
    }

    async fn publish_diagnostics(&self, uri: &Url) {
        let content = self.document_text(uri).await.unwrap_or_default();
        let diagnostics = build_diagnostics(&content, self.config.max_problems);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn all_documents(&self) -> HashMap<Url, String> {
        self.documents.read().await.clone()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SengooLanguageServer {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
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
        if let Some(change) = params.content_changes.into_iter().last() {
            self.documents
                .write()
                .await
                .insert(uri.clone(), change.text);
            self.publish_diagnostics(&uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let content = self.document_text(&uri).await.unwrap_or_default();

        let mut items = vec![
            CompletionItem::new_simple("fn".to_string(), "Define a function".to_string()),
            CompletionItem::new_simple("struct".to_string(), "Define a struct".to_string()),
            CompletionItem::new_simple("let".to_string(), "Declare a local variable".to_string()),
            CompletionItem::new_simple("const".to_string(), "Declare a constant".to_string()),
            CompletionItem::new_simple("match".to_string(), "Pattern matching".to_string()),
        ];

        let mut seen = std::collections::HashSet::new();
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
        let documents = self.all_documents().await;
        let Some(current_content) = documents.get(&uri) else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(current_content, position) else {
            return Ok(None);
        };

        if let Some(range) = find_definition_in_text(current_content, &symbol.name) {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                uri, range,
            ))));
        }

        for (doc_uri, doc_content) in documents {
            if let Some(range) = find_definition_in_text(&doc_content, &symbol.name) {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                    doc_uri, range,
                ))));
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let documents = self.all_documents().await;
        let Some(current_content) = documents.get(&uri) else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(current_content, position) else {
            return Ok(None);
        };

        let mut locations = Vec::new();
        for (doc_uri, doc_content) in documents {
            for range in find_symbol_occurrences(&doc_content, &symbol.name) {
                locations.push(Location::new(doc_uri.clone(), range));
            }
        }

        if !params.context.include_declaration {
            locations.retain(|loc| loc.range != symbol.range || loc.uri != uri);
        }

        Ok(Some(locations))
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

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("`{}`", symbol.name),
            }),
            range: Some(symbol.range),
        }))
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

        let mut actions = Vec::new();
        for diagnostic in params.context.diagnostics {
            let Some(code) = diagnostic.code.as_ref() else {
                continue;
            };

            let Some(code) = (match code {
                NumberOrString::String(s) => Some(s.as_str()),
                NumberOrString::Number(_) => None,
            }) else {
                continue;
            };

            match code {
                "todo-item" => {
                    let line_idx = diagnostic.range.start.line as usize;
                    if let Some(line) = content.lines().nth(line_idx) {
                        let fixed = line.replace("TODO", "").replace("FIXME", "");
                        if fixed != line {
                            actions.push(quick_fix_action(
                                uri.clone(),
                                TextEdit {
                                    range: Range {
                                        start: Position {
                                            line: diagnostic.range.start.line,
                                            character: 0,
                                        },
                                        end: Position {
                                            line: diagnostic.range.start.line,
                                            character: line_char_len(line),
                                        },
                                    },
                                    new_text: fixed,
                                },
                                diagnostic.clone(),
                                "Remove TODO/FIXME marker",
                            ));
                        }
                    }
                }
                "tab-indentation" => {
                    let line_idx = diagnostic.range.start.line as usize;
                    if let Some(line) = content.lines().nth(line_idx) {
                        let fixed = line.replace('\t', "    ");
                        if fixed != line {
                            actions.push(quick_fix_action(
                                uri.clone(),
                                TextEdit {
                                    range: Range {
                                        start: Position {
                                            line: diagnostic.range.start.line,
                                            character: 0,
                                        },
                                        end: Position {
                                            line: diagnostic.range.start.line,
                                            character: line_char_len(line),
                                        },
                                    },
                                    new_text: fixed,
                                },
                                diagnostic.clone(),
                                "Convert tabs to spaces",
                            ));
                        }
                    }
                }
                "trailing-whitespace" => {
                    actions.push(quick_fix_action(
                        uri.clone(),
                        TextEdit {
                            range: diagnostic.range,
                            new_text: String::new(),
                        },
                        diagnostic.clone(),
                        "Remove trailing whitespace",
                    ));
                }
                _ => {}
            }
        }

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
        if !valid_identifier_name(&params.new_name) {
            return Ok(None);
        }

        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let documents = self.all_documents().await;
        let Some(current_content) = documents.get(&uri) else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(current_content, position) else {
            return Ok(None);
        };

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        for (doc_uri, doc_content) in documents {
            let edits: Vec<TextEdit> = find_symbol_occurrences(&doc_content, &symbol.name)
                .into_iter()
                .map(|range| TextEdit {
                    range,
                    new_text: params.new_name.clone(),
                })
                .collect();
            if !edits.is_empty() {
                changes.insert(doc_uri, edits);
            }
        }

        if changes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(WorkspaceEdit {
                changes: Some(changes),
                ..Default::default()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn diagnostics_cover_three_quick_fix_kinds() {
        let text = "\tlet x = 1; // TODO   ";
        let diagnostics = build_diagnostics(text, 16);
        let mut codes = diagnostics
            .into_iter()
            .filter_map(|d| match d.code {
                Some(NumberOrString::String(code)) => Some(code),
                _ => None,
            })
            .collect::<Vec<_>>();
        codes.sort();
        assert_eq!(
            codes,
            vec![
                "tab-indentation".to_string(),
                "todo-item".to_string(),
                "trailing-whitespace".to_string()
            ]
        );
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
}
