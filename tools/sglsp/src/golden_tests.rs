use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tower_lsp::lsp_types::{Position, TextDocumentContentChangeEvent, Url};

use crate::completion::completion_items_for_request;
use crate::workspace_index::{IndexCancellation, WorkspaceIndex};

fn normalize_uri(value: &str, main_uri: &Url, dependency_uri: &Url) -> String {
    value
        .replace(main_uri.as_str(), "$MAIN")
        .replace(dependency_uri.as_str(), "$DEPENDENCY")
}

fn selected_completion_json(
    index: &WorkspaceIndex,
    uri: &Url,
    position: Position,
    dependency_uri: &Url,
) -> Vec<Value> {
    completion_items_for_request(index, uri, position)
        .into_iter()
        .filter(|item| {
            matches!(
                item.label.as_str(),
                "local_symbol" | "dependency_symbol" | "io_i64_result"
            )
        })
        .map(|item| {
            let detail = item
                .detail
                .as_deref()
                .map(|detail| normalize_uri(detail, uri, dependency_uri));
            let data = item.data.map(|mut data| {
                if let Some(symbol_id) = data.get_mut("symbolId") {
                    if let Some(value) = symbol_id.as_str() {
                        *symbol_id = Value::String(normalize_uri(value, uri, dependency_uri));
                    }
                }
                if let Some(document_uri) = data.get_mut("documentUri") {
                    *document_uri = Value::String("$MAIN".to_string());
                }
                data
            });
            json!({ "label": item.label, "detail": detail, "data": data })
        })
        .collect()
}

#[test]
fn production_index_and_completion_match_checked_in_golden() {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("sglsp-production-golden-{id}"));
    let app = root.join("app");
    let dependency = root.join("dependency");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(dependency.join("src")).unwrap();
    let source = include_str!("../tests/fixtures/protocol_baseline.sg");
    let main_path = app.join("src/main.sg");
    let dependency_path = dependency.join("src/lib.sg");
    fs::write(&main_path, source).unwrap();
    fs::write(
        &dependency_path,
        "def dependency_symbol(value: i64) -> i64 { value }\n",
    )
    .unwrap();
    fs::write(
        app.join("Sengoo.lock"),
        r#"version = 1
root = "app"
[[package]]
name = "dependency"
version = "0.1.0"
source = "path+../dependency"
manifest = "../dependency/Sengoo.toml"
"#,
    )
    .unwrap();

    let main_uri = Url::from_file_path(&main_path).unwrap();
    let dependency_uri = Url::from_file_path(&dependency_path).unwrap();
    let index = WorkspaceIndex::build(&[app], IndexCancellation::default()).unwrap();
    assert!(index.open(
        main_uri.clone(),
        7,
        source.to_string(),
        &IndexCancellation::default(),
    ));
    let unicode_line = source.lines().nth(3).unwrap();
    let cursor_byte = unicode_line.find("loc }").unwrap() + 3;
    let utf16_character = unicode_line[..cursor_byte].encode_utf16().count() as u32;
    let normal = selected_completion_json(
        &index,
        &main_uri,
        Position::new(3, utf16_character),
        &dependency_uri,
    );
    let signatures = index
        .signature_candidates(&main_uri)
        .into_iter()
        .filter(|signature| {
            signature.name == "local_symbol" || signature.name == "dependency_symbol"
        })
        .map(|signature| signature.label)
        .collect::<Vec<_>>();

    assert!(index.change(
        &main_uri,
        8,
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "def broken( {".to_string(),
        }],
        &IndexCancellation::default(),
    ));
    let broken = selected_completion_json(&index, &main_uri, Position::new(0, 13), &dependency_uri);
    let actual = json!({
        "utf16Character": utf16_character,
        "normal": normal,
        "brokenLastGood": broken,
        "signatures": signatures,
    });
    let expected: Value =
        serde_json::from_str(include_str!("../tests/golden/protocol_baseline.json")).unwrap();
    assert_eq!(actual, expected);
    let _ = fs::remove_dir_all(root);
}
