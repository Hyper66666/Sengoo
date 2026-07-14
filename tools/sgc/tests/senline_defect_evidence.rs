use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const RECORD_FIELDS: &[&str] = &[
    "record_id",
    "senline_failure",
    "ownership",
    "minimized_regression",
    "fix",
    "target_artifacts",
    "senline_pin",
    "final_consumer_gate",
    "workaround",
];
const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";
const LINUX_TARGET: &str = "x86_64-unknown-linux-gnu";
type EvidenceMutation = (&'static str, Box<dyn Fn(&mut Value)>);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .to_path_buf()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(
        &fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn object<'a>(value: &'a Value, label: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{label} must be an object"))
}

fn schema_ref<'a>(root: &'a Value, reference: &str) -> Result<&'a Value, String> {
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| format!("external schema reference is forbidden: {reference}"))?;
    root.pointer(pointer)
        .ok_or_else(|| format!("missing schema reference {reference}"))
}

fn validate_schema_keywords(schema: &Value, path: &str) -> Result<(), String> {
    const ALLOWED: &[&str] = &[
        "$schema",
        "$id",
        "$defs",
        "$ref",
        "title",
        "type",
        "const",
        "enum",
        "pattern",
        "minLength",
        "minItems",
        "maxItems",
        "required",
        "properties",
        "additionalProperties",
        "items",
        "contains",
        "minContains",
        "maxContains",
        "allOf",
        "if",
        "then",
        "else",
    ];
    let Some(fields) = schema.as_object() else {
        return Err(format!("{path}: schema node must be an object"));
    };
    for key in fields.keys() {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(format!("{path}: unsupported schema keyword {key}"));
        }
    }
    for container in ["$defs", "properties"] {
        if let Some(children) = fields.get(container).and_then(Value::as_object) {
            for (name, child) in children {
                validate_schema_keywords(child, &format!("{path}/{container}/{name}"))?;
            }
        }
    }
    for child_key in ["items", "contains", "if", "then", "else"] {
        if let Some(child) = fields.get(child_key) {
            validate_schema_keywords(child, &format!("{path}/{child_key}"))?;
        }
    }
    if let Some(children) = fields.get("allOf").and_then(Value::as_array) {
        for (index, child) in children.iter().enumerate() {
            validate_schema_keywords(child, &format!("{path}/allOf/{index}"))?;
        }
    }
    Ok(())
}

fn matches_type(value: &Value, expected: &str) -> bool {
    match expected {
        "null" => value.is_null(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        _ => false,
    }
}

fn matches_pattern(value: &str, pattern: &str) -> bool {
    match pattern {
        "^[0-9a-f]{40}$" => {
            value.len() == 40
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }
        "^[0-9a-f]{64}$" => {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }
        "^SGDOG-[0-9]{4}-[0-9]{3}$" => {
            value.len() == 14
                && value.starts_with("SGDOG-")
                && value.as_bytes()[6..10].iter().all(u8::is_ascii_digit)
                && value.as_bytes()[10] == b'-'
                && value.as_bytes()[11..14].iter().all(u8::is_ascii_digit)
        }
        _ => false,
    }
}

fn validate_schema_node(
    root: &Value,
    schema: &Value,
    value: &Value,
    path: &str,
) -> Result<(), String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        validate_schema_node(root, schema_ref(root, reference)?, value, path)?;
    }
    if let Some(expected) = schema.get("type") {
        let valid = match expected {
            Value::String(expected) => matches_type(value, expected),
            Value::Array(expected) => expected
                .iter()
                .filter_map(Value::as_str)
                .any(|expected| matches_type(value, expected)),
            _ => false,
        };
        if !valid {
            return Err(format!("{path}: type mismatch"));
        }
    }
    if let Some(expected) = schema.get("const") {
        if value != expected {
            return Err(format!("{path}: const mismatch"));
        }
    }
    if let Some(expected) = schema.get("enum").and_then(Value::as_array) {
        if !expected.contains(value) {
            return Err(format!("{path}: value is outside enum"));
        }
    }
    if let Some(text) = value.as_str() {
        if let Some(minimum) = schema.get("minLength").and_then(Value::as_u64) {
            if text.len() < minimum as usize {
                return Err(format!("{path}: string is shorter than minLength"));
            }
        }
        if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
            if !matches_pattern(text, pattern) {
                return Err(format!("{path}: string does not match {pattern}"));
            }
        }
    }
    if let Some(fields) = value.as_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for field in required.iter().filter_map(Value::as_str) {
                if !fields.contains_key(field) {
                    return Err(format!("{path}: missing required field {field}"));
                }
            }
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                for field in fields.keys() {
                    if !properties.contains_key(field) {
                        return Err(format!("{path}: unknown field {field}"));
                    }
                }
            }
            for (field, field_schema) in properties {
                if let Some(field_value) = fields.get(field) {
                    validate_schema_node(
                        root,
                        field_schema,
                        field_value,
                        &format!("{path}/{field}"),
                    )?;
                }
            }
        }
    }
    if let Some(items) = value.as_array() {
        if let Some(minimum) = schema.get("minItems").and_then(Value::as_u64) {
            if items.len() < minimum as usize {
                return Err(format!("{path}: array is shorter than minItems"));
            }
        }
        if let Some(maximum) = schema.get("maxItems").and_then(Value::as_u64) {
            if items.len() > maximum as usize {
                return Err(format!("{path}: array is longer than maxItems"));
            }
        }
        if let Some(item_schema) = schema.get("items") {
            for (index, item) in items.iter().enumerate() {
                validate_schema_node(root, item_schema, item, &format!("{path}/{index}"))?;
            }
        }
        if let Some(contains) = schema.get("contains") {
            let count = items
                .iter()
                .filter(|item| validate_schema_node(root, contains, item, path).is_ok())
                .count();
            let minimum = schema
                .get("minContains")
                .and_then(Value::as_u64)
                .unwrap_or(1) as usize;
            let maximum = schema
                .get("maxContains")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX) as usize;
            if count < minimum || count > maximum {
                return Err(format!("{path}: contains count {count} is outside bounds"));
            }
        }
    }
    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for child in all_of {
            validate_schema_node(root, child, value, path)?;
        }
    }
    if let Some(condition) = schema.get("if") {
        if validate_schema_node(root, condition, value, path).is_ok() {
            if let Some(then_schema) = schema.get("then") {
                validate_schema_node(root, then_schema, value, path)?;
            }
        } else if let Some(else_schema) = schema.get("else") {
            validate_schema_node(root, else_schema, value, path)?;
        }
    }
    Ok(())
}

fn non_empty_string<'a>(value: &'a Value, path: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{path}: expected non-empty string"))
}

fn normalized_relative_path(value: &Value, path: &str) -> Result<PathBuf, String> {
    let text = non_empty_string(value, path)?;
    if text.contains('\\') || Path::new(text).is_absolute() {
        return Err(format!("{path}: path must be normalized and relative"));
    }
    let parsed = PathBuf::from(text);
    if parsed
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{path}: path contains a non-normal component"));
    }
    Ok(parsed)
}

fn validate_immutable_reference(value: &Value, path: &str) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    let text = non_empty_string(value, path)?;
    let bytes = text.as_bytes();
    let has_windows_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if has_windows_drive || text.starts_with('/') || text.starts_with("//") {
        return Err(format!(
            "{path}: absolute or checkout-local reference is forbidden"
        ));
    }
    let reference = normalized_relative_path(value, path)?;
    for component in reference.components() {
        let Component::Normal(component) = component else {
            return Err(format!("{path}: non-normal reference component"));
        };
        let component = component.to_string_lossy().to_ascii_lowercase();
        if matches!(
            component.as_str(),
            ".git" | ".worktrees" | "target" | "latest" | "current" | "head" | "main" | "master"
        ) {
            return Err(format!("{path}: mutable reference component {component}"));
        }
    }
    Ok(())
}

fn validate_cross_field_bindings(ledger: &Value) -> Result<(), String> {
    let records = ledger["records"]
        .as_array()
        .ok_or_else(|| "records must be an array".to_owned())?;
    let mut record_ids = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        let path = format!("/records/{index}");
        let record_id = non_empty_string(&record["record_id"], &format!("{path}/record_id"))?;
        if !record_ids.insert(record_id) {
            return Err(format!("{path}: duplicate record_id {record_id}"));
        }

        let failure = &record["senline_failure"];
        if failure["failure_id"] != record["record_id"] {
            return Err(format!(
                "{path}: pending/rehearsal failure_id must link record_id"
            ));
        }
        let expected_discovery = match failure["evidence_kind"].as_str() {
            Some("consumer-failure") => "consumer-discovered",
            Some("known-baseline-rehearsal") => "known-baseline",
            Some("injected-rehearsal") => "injected-rehearsal",
            _ => return Err(format!("{path}: unknown evidence kind")),
        };
        if record["ownership"]["discovery_status"] != expected_discovery {
            return Err(format!(
                "{path}: evidence kind and discovery status disagree"
            ));
        }

        let fixing_commit = record["fix"]["fixing_commit"].as_str();
        let artifacts = record["target_artifacts"]
            .as_array()
            .ok_or_else(|| format!("{path}: target_artifacts must be an array"))?;
        let targets = artifacts
            .iter()
            .filter_map(|artifact| artifact["target"].as_str())
            .collect::<BTreeSet<_>>();
        if targets != BTreeSet::from([WINDOWS_TARGET, LINUX_TARGET]) {
            return Err(format!(
                "{path}: target artifacts must bind Windows and Linux once"
            ));
        }
        for artifact in artifacts {
            for field in ["provenance", "archive", "manifest"] {
                validate_immutable_reference(
                    &artifact[field],
                    &format!("{path}/target_artifacts/{field}"),
                )?;
            }
            if artifact["status"] == "verified" {
                let source_revision = non_empty_string(
                    &artifact["source_revision"],
                    &format!("{path}/target_artifacts/source_revision"),
                )?;
                if fixing_commit != Some(source_revision) {
                    return Err(format!(
                        "{path}: artifact source_revision must equal fixing_commit"
                    ));
                }
            }
        }

        let pin = &record["senline_pin"];
        if pin["status"] == "verified" {
            let pinned = non_empty_string(
                &pin["pinned_sengoo_revision"],
                &format!("{path}/senline_pin/pinned_sengoo_revision"),
            )?;
            if fixing_commit != Some(pinned) {
                return Err(format!(
                    "{path}: pinned Sengoo revision must equal fixing_commit"
                ));
            }
            let manifests = pin["target_manifests"]
                .as_array()
                .ok_or_else(|| format!("{path}: pin target manifests must be an array"))?;
            for artifact in artifacts {
                let target = artifact["target"].as_str().unwrap_or_default();
                let artifact_hash = artifact["manifest_sha256"].as_str();
                let pin_hash = manifests
                    .iter()
                    .find(|manifest| manifest["target"] == target)
                    .and_then(|manifest| manifest["manifest_sha256"].as_str());
                if artifact_hash.is_none() || pin_hash != artifact_hash {
                    return Err(format!(
                        "{path}: pin manifest hash must equal artifact hash"
                    ));
                }
            }
        }
        if record["final_consumer_gate"]["status"] == "green"
            && record["senline_pin"]["status"] != "verified"
        {
            return Err(format!(
                "{path}: green consumer gate requires a verified pin"
            ));
        }
        let workaround = &record["workaround"];
        if workaround["active"] == true {
            if workaround["linked_defect"] != record["record_id"] {
                return Err(format!("{path}: workaround must link its owning defect"));
            }
            validate_immutable_reference(
                &workaround["removal_test"],
                &format!("{path}/workaround/removal_test"),
            )?;
            if record["final_consumer_gate"]["status"] == "green" {
                return Err(format!(
                    "{path}: an active workaround cannot count as green"
                ));
            }
        }
    }
    Ok(())
}

fn validate_evidence(schema: &Value, ledger: &Value) -> Result<(), String> {
    validate_schema_keywords(schema, "")?;
    validate_schema_node(schema, schema, ledger, "")?;
    validate_cross_field_bindings(ledger)
}

fn evidence_paths() -> (PathBuf, PathBuf) {
    let docs = repo_root().join("docs");
    (
        docs.join("senline-dogfood-evidence.schema.json"),
        docs.join("senline-dogfood-evidence.v1.json"),
    )
}

fn fill_synthetic_verified_chain(record: &mut Value) {
    let fixing_revision = "a".repeat(40);
    record["minimized_regression"]["red_status"] = Value::String("preserved".to_owned());
    record["minimized_regression"]["red_commit"] = Value::String("9".repeat(40));
    record["fix"]["fixing_commit"] = Value::String(fixing_revision.clone());
    let artifacts = record["target_artifacts"]
        .as_array_mut()
        .expect("target artifacts");
    for (index, artifact) in artifacts.iter_mut().enumerate() {
        let platform = if index == 0 { "windows" } else { "linux" };
        artifact["status"] = Value::String("verified".to_owned());
        artifact["source_revision"] = Value::String(fixing_revision.clone());
        artifact["build_manifest_id"] = Value::String("1".repeat(64));
        artifact["provenance"] = Value::String(format!(
            "artifacts/SGDOG-2026-001/{platform}/provenance.json"
        ));
        artifact["archive"] =
            Value::String(format!("artifacts/SGDOG-2026-001/{platform}/toolchain.zip"));
        artifact["manifest"] =
            Value::String(format!("artifacts/SGDOG-2026-001/{platform}/manifest.json"));
        artifact["archive_sha256"] = Value::String("2".repeat(64));
        artifact["manifest_sha256"] = Value::String(if index == 0 { "3" } else { "4" }.repeat(64));
    }
    record["senline_pin"] = serde_json::json!({
        "status": "verified",
        "senline_pin_revision": "8".repeat(40),
        "pinned_sengoo_revision": fixing_revision,
        "target_manifests": [
            { "target": WINDOWS_TARGET, "manifest_sha256": "3".repeat(64) },
            { "target": LINUX_TARGET, "manifest_sha256": "4".repeat(64) }
        ]
    });
    record["final_consumer_gate"] = serde_json::json!({
        "status": "green",
        "command": "cargo test --locked --workspace",
        "evidence": "evidence/SGDOG-2026-001/consumer-green.json"
    });
}

#[test]
fn durable_senline_defect_evidence_validates_against_the_versioned_schema() {
    let (schema_path, ledger_path) = evidence_paths();
    let schema = read_json(&schema_path);
    let ledger = read_json(&ledger_path);
    validate_evidence(&schema, &ledger).unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["$id"],
        "https://sengoo.dev/schema/senline-dogfood-evidence-v1.json"
    );
    assert_eq!(ledger["schema_version"], 1);
    assert_eq!(ledger["change"], "senline-service-dogfood");
    assert_eq!(
        ledger["linked_senline_change"],
        "adopt-sengoo-backend-slice"
    );

    let first = &ledger["records"][0];
    let actual_fields = object(first, "first record")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_fields, RECORD_FIELDS.iter().copied().collect());
    assert_eq!(first["record_id"], "SGDOG-2026-001");
    assert_eq!(
        first["senline_failure"]["evidence_kind"],
        "known-baseline-rehearsal"
    );
    assert_eq!(first["ownership"]["authority"], "sengoo-owned");
    assert_eq!(
        first["ownership"]["component_classification"],
        "sengoo-standard-library"
    );
    assert_eq!(first["ownership"]["discovery_status"], "known-baseline");
    assert_eq!(first["fix"]["fixing_commit"], Value::Null);
    assert_eq!(first["senline_pin"]["status"], "pending");
    assert_eq!(first["final_consumer_gate"]["status"], "pending");

    for (index, record) in ledger["records"]
        .as_array()
        .expect("records array")
        .iter()
        .enumerate()
    {
        let failure = &record["senline_failure"];
        let record_id = record["record_id"].as_str().expect("record id");
        let consumer_path = normalized_relative_path(
            &failure["consumer_record"],
            &format!("records[{index}].consumer_record"),
        )
        .expect("normalized consumer record path");
        let consumer_bytes = fs::read(repo_root().join(consumer_path))
            .unwrap_or_else(|error| panic!("read linked consumer record: {error}"));
        assert_eq!(
            format!("{:x}", Sha256::digest(&consumer_bytes)),
            failure["consumer_record_sha256"],
            "records[{index}] consumer record hash changed"
        );
        assert!(
            String::from_utf8_lossy(&consumer_bytes).contains(record_id),
            "records[{index}] consumer record must contain its evidence ID"
        );

        let senline_fixture =
            normalized_relative_path(&failure["fixture"], &format!("records[{index}].fixture"))
                .expect("normalized Senline fixture path");
        let mirror_fixture = normalized_relative_path(
            &failure["fixture_mirror"],
            &format!("records[{index}].fixture_mirror"),
        )
        .expect("normalized Sengoo fixture mirror path");
        let senline_prefix = Path::new("fixtures/sengoo-worker/v1");
        let mirror_prefix = Path::new("examples/realworld/senline-domain-worker/fixtures/v1");
        assert_eq!(
            senline_fixture
                .strip_prefix(senline_prefix)
                .expect("Senline fixture must use the frozen v1 root"),
            mirror_fixture
                .strip_prefix(mirror_prefix)
                .expect("Sengoo fixture must use the mirrored frozen v1 root"),
            "records[{index}] fixture paths must identify the same relative file"
        );
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(
                    fs::read(repo_root().join(mirror_fixture))
                        .expect("read exact mirrored fixture path")
                )
            ),
            failure["fixture_sha256"],
            "records[{index}] fixture hash changed"
        );
    }
}

#[test]
fn evidence_schema_rejects_every_empty_or_workaround_only_green_state() {
    let (schema_path, ledger_path) = evidence_paths();
    let schema = read_json(&schema_path);
    let ledger = read_json(&ledger_path);
    let mut valid_green = ledger.clone();
    fill_synthetic_verified_chain(&mut valid_green["records"][0]);
    validate_evidence(&schema, &valid_green)
        .expect("a complete synthetic immutable chain should validate");
    let mut mutations: Vec<EvidenceMutation> = Vec::new();
    mutations.push((
        "empty records",
        Box::new(|value| value["records"] = Value::Array(vec![])),
    ));
    mutations.push((
        "missing target",
        Box::new(|value| {
            value["records"][0]["target_artifacts"]
                .as_array_mut()
                .expect("target artifacts")
                .pop();
        }),
    ));
    mutations.push((
        "duplicate target",
        Box::new(|value| {
            value["records"][0]["target_artifacts"][1]["target"] =
                Value::String(WINDOWS_TARGET.to_owned());
        }),
    ));
    mutations.push((
        "verified artifact without hashes",
        Box::new(|value| {
            value["records"][0]["target_artifacts"][0]["status"] =
                Value::String("verified".to_owned());
        }),
    ));
    mutations.push((
        "preserved red without commit",
        Box::new(|value| {
            value["records"][0]["minimized_regression"]["red_status"] =
                Value::String("preserved".to_owned());
        }),
    ));
    mutations.push((
        "verified pin without revisions",
        Box::new(|value| {
            value["records"][0]["senline_pin"]["status"] = Value::String("verified".to_owned());
        }),
    ));
    mutations.push((
        "verified pin over pending red and artifacts",
        Box::new(|value| {
            let revision = "a".repeat(40);
            value["records"][0]["fix"]["fixing_commit"] = Value::String(revision.clone());
            for (index, artifact) in value["records"][0]["target_artifacts"]
                .as_array_mut()
                .expect("target artifacts")
                .iter_mut()
                .enumerate()
            {
                artifact["source_revision"] = Value::String(revision.clone());
                artifact["build_manifest_id"] = Value::String("b".repeat(64));
                artifact["provenance"] = Value::String("attestation.json".to_owned());
                artifact["archive"] = Value::String(format!("target-{index}.tar.gz"));
                artifact["manifest"] = Value::String(format!("manifest-{index}.json"));
                artifact["archive_sha256"] = Value::String("c".repeat(64));
                artifact["manifest_sha256"] =
                    Value::String(if index == 0 { "d" } else { "e" }.repeat(64));
            }
            value["records"][0]["senline_pin"] = serde_json::json!({
                "status": "verified",
                "senline_pin_revision": "f".repeat(40),
                "pinned_sengoo_revision": revision,
                "target_manifests": [
                    { "target": WINDOWS_TARGET, "manifest_sha256": "d".repeat(64) },
                    { "target": LINUX_TARGET, "manifest_sha256": "e".repeat(64) }
                ]
            });
        }),
    ));
    mutations.push((
        "green gate without evidence",
        Box::new(|value| {
            value["records"][0]["final_consumer_gate"]["status"] =
                Value::String("green".to_owned());
        }),
    ));
    mutations.push((
        "active workaround without removal contract",
        Box::new(|value| {
            value["records"][0]["workaround"]["active"] = Value::Bool(true);
        }),
    ));
    mutations.push((
        "artifact path reaches mutable Sengoo checkout",
        Box::new(|value| {
            value["records"][0]["target_artifacts"][0]["archive"] =
                Value::String("D:/Sengoo/target/latest/toolchain.zip".to_owned());
        }),
    ));
    mutations.push((
        "artifact provenance escapes through a floating path",
        Box::new(|value| {
            value["records"][0]["target_artifacts"][0]["provenance"] =
                Value::String("../target/latest/provenance.json".to_owned());
        }),
    ));
    mutations.push((
        "active workaround links a different defect",
        Box::new(|value| {
            value["records"][0]["workaround"] = serde_json::json!({
                "active": true,
                "owner": "Senline backend team",
                "linked_defect": "SGDOG-2099-999",
                "expiry_condition": "Remove after the pinned fixing artifact is verified",
                "removal_test": "tests/workaround_removal.rs::removes_gap_workaround"
            });
        }),
    ));
    mutations.push((
        "active workaround uses an absolute removal test",
        Box::new(|value| {
            value["records"][0]["workaround"] = serde_json::json!({
                "active": true,
                "owner": "Senline backend team",
                "linked_defect": "SGDOG-2026-001",
                "expiry_condition": "Remove after the pinned fixing artifact is verified",
                "removal_test": "D:/senline/tests/workaround_removal.rs"
            });
        }),
    ));
    mutations.push((
        "partial pin omits one target manifest",
        Box::new(|value| {
            fill_synthetic_verified_chain(&mut value["records"][0]);
            value["records"][0]["senline_pin"]["target_manifests"]
                .as_array_mut()
                .expect("target manifests")
                .pop();
        }),
    ));
    mutations.push((
        "active workaround tries to claim a complete chain as green",
        Box::new(|value| {
            fill_synthetic_verified_chain(&mut value["records"][0]);
            value["records"][0]["workaround"] = serde_json::json!({
                "active": true,
                "owner": "Senline backend team",
                "linked_defect": "SGDOG-2026-001",
                "expiry_condition": "Remove after the pinned fixing artifact is verified",
                "removal_test": "tests/workaround_removal.rs::removes_gap_workaround"
            });
        }),
    ));
    mutations.push((
        "inactive workaround hides mutable registry fields",
        Box::new(|value| {
            value["records"][0]["workaround"]["owner"] =
                Value::String("untracked owner".to_owned());
        }),
    ));
    mutations.push((
        "rehearsal mislabeled as consumer discovered",
        Box::new(|value| {
            value["records"][0]["ownership"]["discovery_status"] =
                Value::String("consumer-discovered".to_owned());
        }),
    ));

    for (label, mutate) in mutations {
        let mut invalid = ledger.clone();
        mutate(&mut invalid);
        assert!(
            validate_evidence(&schema, &invalid).is_err(),
            "{label} must not validate"
        );
    }

    let mut unsupported_schema = schema.clone();
    unsupported_schema["$defs"]["record"]["not"] = serde_json::json!({});
    let error = validate_evidence(&unsupported_schema, &ledger)
        .expect_err("unknown schema keywords must fail closed");
    assert!(error.contains("unsupported schema keyword not"));
}

#[test]
fn every_recorded_red_command_selects_exactly_one_regression() {
    let (_, ledger_path) = evidence_paths();
    let ledger = read_json(&ledger_path);
    for (index, record) in ledger["records"]
        .as_array()
        .expect("records array")
        .iter()
        .enumerate()
    {
        let regression = &record["minimized_regression"];
        let selector = &regression["selector"];
        let package = selector["package"].as_str().expect("selector package");
        let target_kind = selector["target_kind"]
            .as_str()
            .expect("selector target kind");
        let target_name = selector["target_name"]
            .as_str()
            .expect("selector target name");
        let test_name = selector["test_name"].as_str().expect("selector test name");
        let target_flag = format!("--{target_kind}");
        let expected_command = format!(
            "cargo test -p {package} {target_flag} {target_name} {test_name} -- --exact --nocapture"
        );
        assert_eq!(
            regression["red_command"], expected_command,
            "records[{index}] RED command must match its structured selector"
        );
        assert!(
            regression["test"]
                .as_str()
                .is_some_and(|path| path.ends_with(test_name.trim_start_matches("tests::"))),
            "records[{index}] test path must name its exact selector"
        );

        let output = Command::new(env!("CARGO"))
            .args([
                "test",
                "-p",
                package,
                target_flag.as_str(),
                target_name,
                test_name,
                "--",
                "--exact",
                "--list",
            ])
            .current_dir(repo_root())
            .output()
            .unwrap_or_else(|error| panic!("list records[{index}] RED regression: {error}"));
        assert!(
            output.status.success(),
            "records[{index}] cargo test --list failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let listed = String::from_utf8_lossy(&output.stdout);
        let selected = listed
            .lines()
            .filter(|line| line.ends_with(": test"))
            .collect::<Vec<_>>();
        assert_eq!(
            selected,
            [format!("{test_name}: test")],
            "records[{index}] RED command must select exactly one regression"
        );
    }
}
