//! Read-only differential oracle. Never part of an SG compiler/self-build.
use sengoo_compiler::{Lexer, Parser, TokenKind, TypeChecker};
use std::io::{self, BufRead};

fn tokens(source: &str) -> serde_json::Value {
    let values: Vec<_> = Lexer::tokenize(source)
        .into_iter()
        .map(|token| {
            let kind = match &token.kind {
                TokenKind::Int(_) => "integer".to_owned(),
                TokenKind::Float(_) => "float".to_owned(),
                TokenKind::Ident => "identifier".to_owned(),
                TokenKind::InvalidNumber(_) => "invalid-number".to_owned(),
                TokenKind::String(_) => "string".to_owned(),
                TokenKind::RawString(_) => "raw-string".to_owned(),
                TokenKind::Bytes(_) => "bytes".to_owned(),
                TokenKind::Char(_) => "char".to_owned(),
                TokenKind::FString(_) => "fstring".to_owned(),
                other => format!("{other:?}"),
            };
            serde_json::json!({"kind":kind,"start":token.span.lo,"end":token.span.hi,
                "text": &source[token.span.lo as usize..token.span.hi as usize]})
        })
        .collect();
    serde_json::json!({"tokens":values})
}

fn serve() {
    for line in io::stdin().lock().lines() {
        let response = match line {
            Ok(line) => match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(request) => match request.get("source").and_then(|value| value.as_str()) {
                    Some(source) => {
                        let mut result = tokens(source);
                        if request.get("parse").and_then(|v| v.as_bool()).unwrap_or(false) {
                            result["parse"] = match Parser::parse(source) {
                                Ok(program) => serde_json::json!({"ok":true,"declarations":program.decls.len()}),
                                Err(error) => serde_json::json!({"ok":false,"error":error.to_string()}),
                            };
                        }
                        if request.get("format").and_then(|v| v.as_bool()).unwrap_or(false) {
                            result["format"] = match sgfmt::format_source(source, &sgfmt::FormatOptions::default()) {
                                Ok(text) => serde_json::json!({"ok":true,"text":text}),
                                Err(error) => serde_json::json!({"ok":false,"error":error.to_string()}),
                            };
                        }
                        if request.get("typecheck").and_then(|v| v.as_bool()).unwrap_or(false) {
                            result["typecheck"] = match Parser::parse(source) {
                                Ok(program) => match TypeChecker::new().check_program(&program) {
                                    Ok(()) => serde_json::json!({"ok":true}),
                                    Err(error) => serde_json::json!({"ok":false,"error":error.to_string()}),
                                },
                                Err(error) => serde_json::json!({"ok":false,"error":error.to_string()}),
                            };
                        }
                        result
                    },
                    None => serde_json::json!({"error":"source must be a string"}),
                },
                Err(error) => serde_json::json!({"error":error.to_string()}),
            },
            Err(error) => serde_json::json!({"error":error.to_string()}),
        };
        println!("{response}");
    }
}

fn main() {
    // Match sgc's 16-MiB Windows main stack instead of the default 1-MiB
    // executable stack when comparing recursive compiler phases.
    std::thread::Builder::new()
        .name("reference-oracle".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(serve)
        .expect("start reference oracle")
        .join()
        .expect("reference oracle panicked");
}
