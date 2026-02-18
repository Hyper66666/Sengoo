//! sgfmt - Sengoo source formatter.

use clap::Parser;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

use sengoo_compiler::Parser as SgParser;

#[derive(Parser, Debug)]
#[command(name = "sgfmt")]
#[command(about = "Sengoo source formatter", long_about = None)]
struct Args {
    #[arg(value_name = "FILE")]
    file: PathBuf,

    #[arg(short, long)]
    write: bool,

    #[arg(long)]
    check: bool,

    #[arg(short, long, default_value_t = 100)]
    max_width: usize,

    #[arg(short, long, default_value_t = 4)]
    indent_width: usize,
}

#[derive(Debug, Clone)]
struct FormatOptions {
    max_width: usize,
    indent_width: usize,
}

struct Formatter {
    options: FormatOptions,
    lines: Vec<String>,
}

impl Formatter {
    fn new(input: String, options: FormatOptions) -> Self {
        Self {
            options,
            lines: input.lines().map(|s| s.to_string()).collect(),
        }
    }

    fn format(&self) -> String {
        let mut result = Vec::new();
        let mut indent_level = 0usize;

        for line in &self.lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                result.push(String::new());
                continue;
            }

            let starts_closing = trimmed.starts_with('}') || trimmed.starts_with(']');
            let effective_indent = if starts_closing {
                indent_level.saturating_sub(1)
            } else {
                indent_level
            };

            result.push(self.format_line(trimmed, effective_indent));
            indent_level = Self::next_indent_level(trimmed, indent_level);
        }

        result.join("\n")
    }

    fn next_indent_level(line: &str, current: usize) -> usize {
        let mut level = current;
        for ch in line.chars() {
            match ch {
                '{' | '[' => level += 1,
                '}' | ']' => level = level.saturating_sub(1),
                _ => {}
            }
        }
        level
    }

    fn format_line(&self, line: &str, indent_level: usize) -> String {
        let indent = " ".repeat(self.options.indent_width * indent_level);
        let mut normalized = Self::normalize_spacing(line);

        if normalized.ends_with('}') {
            // Keep block-closing lines as-is.
        } else if normalized.contains('=')
            && !normalized.contains("==")
            && !normalized.contains("!=")
            && !normalized.ends_with(';')
            && !normalized.ends_with('{')
        {
            normalized.push(';');
        }

        if normalized.len() > self.options.max_width {
            // Keep simple behavior for now: no wrapping, just respect option usage.
        }

        format!("{}{}", indent, normalized)
    }

    fn normalize_spacing(line: &str) -> String {
        line
            .replace("==", " __EQ__ ")
            .replace("!=", " __NE__ ")
            .replace("<=", " __LE__ ")
            .replace(">=", " __GE__ ")
            .replace("=", " = ")
            .replace("+", " + ")
            .replace("-", " - ")
            .replace("*", " * ")
            .replace("/", " / ")
            .replace("<", " < ")
            .replace(">", " > ")
            .replace(",", ", ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace("__EQ__", "==")
            .replace("__NE__", "!=")
            .replace("__LE__", "<=")
            .replace("__GE__", ">=")
    }
}

fn format_file(path: &PathBuf, options: &FormatOptions) -> Result<String> {
    let content = fs::read_to_string(path).into_diagnostic()?;

    // Parse validation with current compiler API.
    SgParser::parse(&content).into_diagnostic()?;

    let formatter = Formatter::new(content, options.clone());
    Ok(formatter.format())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let options = FormatOptions {
        max_width: args.max_width,
        indent_width: args.indent_width,
    };

    let formatted = format_file(&args.file, &options)?;

    if args.check {
        let original = fs::read_to_string(&args.file).into_diagnostic()?;
        if original.trim() != formatted.trim() {
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