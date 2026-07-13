//! sgfmt - Sengoo source formatter.

use clap::Parser;
use miette::{IntoDiagnostic, Result};
use sgfmt::{format_source, FormatOptions};
use std::fs;
use std::path::Path;

mod config;
use config::{resolve_options, Args};

pub(crate) const SGFMT_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("SENGOO_BUILD_HASH"),
    ")"
);

fn format_file(path: &Path, options: &FormatOptions) -> Result<String> {
    let content = fs::read_to_string(path).into_diagnostic()?;
    let (source, test_attributes) = mask_test_attributes(&content);
    let mut formatted =
        restore_test_attributes(format_source(&source, options)?, &test_attributes)?;
    formatted.push('\n');
    Ok(formatted)
}

fn mask_test_attributes(source: &str) -> (String, Vec<(String, Vec<String>)>) {
    let mut masked = Vec::new();
    let mut pending = Vec::new();
    let mut attributes = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed == "#[test]" || (trimmed.starts_with("#[case(") && trimmed.ends_with(")]")) {
            pending.push(trimmed.to_string());
            masked.push(String::new());
            continue;
        }

        if !pending.is_empty() {
            let declaration = trimmed
                .strip_prefix("def ")
                .or_else(|| trimmed.strip_prefix("async def "));
            if let Some(name) = declaration.and_then(|value| value.split('(').next()) {
                attributes.push((name.trim().to_string(), std::mem::take(&mut pending)));
            }
        }
        masked.push(line.to_string());
    }

    (masked.join("\n"), attributes)
}

fn restore_test_attributes(
    formatted: String,
    attributes: &[(String, Vec<String>)],
) -> Result<String> {
    let mut lines = formatted.lines().map(str::to_string).collect::<Vec<_>>();
    for (name, function_attributes) in attributes {
        let plain = format!("def {name}(");
        let asynchronous = format!("async def {name}(");
        let index = lines
            .iter()
            .position(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with(&plain) || trimmed.starts_with(&asynchronous)
            })
            .ok_or_else(|| miette::miette!("formatted test function `{name}` was not found"))?;
        lines.splice(index..index, function_attributes.iter().cloned());
    }
    Ok(lines.join("\n"))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let options = resolve_options(&args)?;
    let formatted = format_file(&args.file, &options)?;

    if args.check {
        let original = fs::read_to_string(&args.file).into_diagnostic()?;
        if original != formatted {
            miette::bail!("{} is not formatted", args.file.display());
        }
        return Ok(());
    }

    if args.write {
        fs::write(&args.file, &formatted).into_diagnostic()?;
        eprintln!("formatted {}", args.file.display());
        return Ok(());
    }

    print!("{}", formatted);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("sgfmt_test_{}_{}", name, stamp))
    }

    #[test]
    fn resolve_options_reads_config_and_cli_override() {
        let dir = temp_path("config");
        fs::create_dir_all(&dir).expect("create temp dir");
        let source = dir.join("main.sg");
        fs::write(&source, "def main() -> i64 { 0 }").expect("write source");
        fs::write(dir.join("sgfmt.toml"), "max_width = 88\nindent_width = 2\n")
            .expect("write config");

        let args = Args {
            file: source,
            write: false,
            check: false,
            config_path: None,
            max_width: Some(120),
            indent_width: None,
        };

        let resolved = resolve_options(&args).expect("resolve options");
        assert_eq!(resolved.max_width, 120);
        assert_eq!(resolved.indent_width, 2);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn format_file_uses_one_canonical_trailing_newline() {
        let path = temp_path("trailing_newline");
        fs::write(&path, "def main() -> i64 { 0 }\n").expect("write source");

        let formatted = format_file(&path, &FormatOptions::default()).expect("format source");

        assert!(formatted.ends_with('\n'));
        assert!(!formatted.ends_with("\n\n"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn format_file_preserves_parameterized_test_attributes() {
        let path = temp_path("case_attribute");
        fs::write(
            &path,
            "#[case(\"small\", 1)]\ndef sample(value: i64) -> i64 { value }\n",
        )
        .expect("write source");

        let formatted = format_file(&path, &FormatOptions::default()).expect("format source");

        assert!(formatted.starts_with("#[case(\"small\", 1)]\ndef sample(value: i64)"));
        let _ = fs::remove_file(path);
    }
}
