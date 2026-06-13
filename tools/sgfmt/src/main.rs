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
    format_source(&content, options)
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

    println!("{}", formatted);
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
}
