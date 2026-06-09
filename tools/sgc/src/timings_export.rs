use miette::{IntoDiagnostic, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const TIMINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
struct TimingsPhasesMsV1 {
    parse: f64,
    typeck: f64,
    hir_lower: f64,
    mir_lower: f64,
    mir_opt: f64,
    codegen: f64,
}

#[derive(Debug, Serialize)]
struct TimingsJsonV1 {
    schema_version: u32,
    phases_ms: TimingsPhasesMsV1,
}

fn phase_ms(phases: &BTreeMap<String, f64>, key: &str) -> f64 {
    phases.get(key).copied().unwrap_or(0.0)
}

pub(crate) fn write_timings_json_v1(path: &Path, phases: &BTreeMap<String, f64>) -> Result<()> {
    let payload = TimingsJsonV1 {
        schema_version: TIMINGS_SCHEMA_VERSION,
        phases_ms: TimingsPhasesMsV1 {
            parse: phase_ms(phases, "parse"),
            typeck: phase_ms(phases, "typeck"),
            hir_lower: phase_ms(phases, "hir_lower"),
            mir_lower: phase_ms(phases, "mir_lower"),
            mir_opt: phase_ms(phases, "mir_opt"),
            codegen: phase_ms(phases, "codegen"),
        },
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).into_diagnostic()?;
        }
    }
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| miette::miette!("failed to encode timings json: {}", err))?;
    fs::write(path, json)
        .into_diagnostic()
        .map_err(|err| miette::miette!("failed to write {}: {}", path.display(), err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const EXPORT_PHASES: [&str; 6] = [
        "parse",
        "typeck",
        "hir_lower",
        "mir_lower",
        "mir_opt",
        "codegen",
    ];

    #[test]
    fn timings_json_export_writes_schema_v1() {
        let mut phases = BTreeMap::new();
        for key in EXPORT_PHASES {
            phases.insert(key.to_string(), 1.25);
        }
        let path = std::env::temp_dir().join(format!(
            "sgc_timings_{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write_timings_json_v1(&path, &phases).expect("timings json should write");
        let raw = fs::read_to_string(&path).expect("timings json should be readable");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("valid json");
        assert_eq!(
            value.get("schema_version").and_then(|v| v.as_u64()),
            Some(1)
        );
        let phases_ms = value
            .get("phases_ms")
            .and_then(|v| v.as_object())
            .expect("phases_ms object");
        for key in EXPORT_PHASES {
            assert!(
                phases_ms.contains_key(key),
                "missing phase key {key} in {raw}"
            );
        }
        let _ = fs::remove_file(path);
    }
}
