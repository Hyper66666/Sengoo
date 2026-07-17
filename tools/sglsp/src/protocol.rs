use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tower_lsp::lsp_types::Url;

pub(crate) const COMPLETION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SymbolOrigin {
    CurrentDocument,
    Workspace,
    Dependency,
    StandardLibrary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CompletionCategory {
    LocalVariable,
    Parameter,
    Field,
    ImportedSymbol,
    ProjectSymbol,
    StandardLibrary,
    Keyword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ResolveKind {
    None,
    Documentation,
    AutoImport,
    DocumentationAndAutoImport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SengooCompletionDataV1 {
    pub(crate) schema_version: u32,
    pub(crate) symbol_id: String,
    pub(crate) origin: SymbolOrigin,
    pub(crate) category: CompletionCategory,
    pub(crate) document_uri: Url,
    pub(crate) document_revision: i32,
    pub(crate) resolve_kind: ResolveKind,
    #[serde(flatten)]
    pub(crate) extensions: Map<String, Value>,
}

pub(crate) fn completion_experimental_capability() -> Value {
    serde_json::json!({
        "sengoo": { "completionSchemaVersion": COMPLETION_SCHEMA_VERSION }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tower_lsp::lsp_types::Url;

    #[test]
    fn schema_v1_round_trips_unknown_fields_additively() {
        let value = json!({
            "schemaVersion": 1,
            "symbolId": "workspace:file:///demo.sg#Thing",
            "origin": "workspace",
            "category": "projectSymbol",
            "documentUri": "file:///demo.sg",
            "documentRevision": 17,
            "resolveKind": "documentation",
            "futureField": { "enabled": true }
        });

        let data: SengooCompletionDataV1 = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(data.document_uri, Url::parse("file:///demo.sg").unwrap());
        assert_eq!(data.document_revision, 17);
        assert_eq!(serde_json::to_value(data).unwrap(), value);
    }

    #[test]
    fn experimental_capability_advertises_schema_v1() {
        assert_eq!(
            completion_experimental_capability(),
            json!({"sengoo": {"completionSchemaVersion": 1}})
        );
    }

    #[test]
    fn schema_v1_rejects_missing_or_non_integer_revision_identity() {
        let missing_uri = json!({
            "schemaVersion": 1, "symbolId": "x", "origin": "workspace",
            "category": "projectSymbol", "documentRevision": 1, "resolveKind": "none"
        });
        assert!(serde_json::from_value::<SengooCompletionDataV1>(missing_uri).is_err());

        let fractional_revision = json!({
            "schemaVersion": 1, "symbolId": "x", "origin": "workspace",
            "category": "projectSymbol", "documentUri": "file:///demo.sg",
            "documentRevision": 1.5, "resolveKind": "none"
        });
        assert!(serde_json::from_value::<SengooCompletionDataV1>(fractional_revision).is_err());
    }

    #[test]
    fn current_document_origin_has_stable_wire_spelling() {
        assert_eq!(
            serde_json::to_value(SymbolOrigin::CurrentDocument).unwrap(),
            json!("currentDocument")
        );
    }
}
