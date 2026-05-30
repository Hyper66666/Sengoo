use clap::Parser;
use miette::{Context, IntoDiagnostic, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "sgfmt")]
#[command(about = "Sengoo source formatter", long_about = None)]
pub(super) struct Args {
    #[arg(value_name = "FILE")]
    pub(super) file: PathBuf,
    #[arg(short, long)]
    pub(super) write: bool,
    #[arg(long)]
    pub(super) check: bool,
    #[arg(short = 'C', long = "config")]
    pub(super) config_path: Option<PathBuf>,
    #[arg(short, long)]
    pub(super) max_width: Option<usize>,
    #[arg(short, long)]
    pub(super) indent_width: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FormatOptions {
    pub(super) max_width: usize,
    pub(super) indent_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            max_width: 100,
            indent_width: 4,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct FormatConfig {
    max_width: Option<usize>,
    indent_width: Option<usize>,
}

fn find_config_path(source_file: &Path, explicit: Option<&PathBuf>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        if path.exists() {
            return Ok(Some(path.clone()));
        }
        miette::bail!("config file not found: {}", path.display());
    }

    let mut cursor = source_file
        .parent()
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok());
    while let Some(dir) = cursor {
        let candidate = dir.join("sgfmt.toml");
        if candidate.exists() {
            return Ok(Some(candidate));
        }
        cursor = dir.parent().map(Path::to_path_buf);
    }
    Ok(None)
}

fn load_format_config(path: &Path) -> Result<FormatConfig> {
    let raw = fs::read_to_string(path)
        .into_diagnostic()
        .with_context(|| format!("failed to read config {}", path.display()))?;
    toml::from_str(&raw)
        .into_diagnostic()
        .with_context(|| format!("failed to parse config {}", path.display()))
}

pub(super) fn resolve_options(args: &Args) -> Result<FormatOptions> {
    let mut options = FormatOptions::default();

    if let Some(path) = find_config_path(&args.file, args.config_path.as_ref())? {
        let config = load_format_config(&path)?;
        if let Some(max_width) = config.max_width {
            options.max_width = max_width;
        }
        if let Some(indent_width) = config.indent_width {
            options.indent_width = indent_width;
        }
    }

    if let Some(max_width) = args.max_width {
        options.max_width = max_width;
    }
    if let Some(indent_width) = args.indent_width {
        options.indent_width = indent_width;
    }

    if options.max_width == 0 {
        miette::bail!("max_width must be greater than 0");
    }
    if options.indent_width == 0 {
        miette::bail!("indent_width must be greater than 0");
    }

    Ok(options)
}
