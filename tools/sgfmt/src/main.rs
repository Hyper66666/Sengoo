//! sgfmt - Sengoo 代码格式化工具
//!
//! 用法:
//!   sgfmt main.sg           # 格式化文件并打印到标准输出
//!   sgfmt --write main.sg    # 格式化文件并原地修改
//!   sgfmt --check main.sg    # 检查文件是否已格式化

use clap::Parser;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

use sengoo_compiler::lexer::Lexer;
use sengoo_compiler::parser::Parser;
use sengoo_compiler::Config;

/// Sengoo 代码格式化工具
#[derive(Parser, Debug)]
#[command(name = "sgfmt")]
#[command(about = "Sengoo 代码格式化工具", long_about = None)]
struct Args {
    /// 输入文件
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// 原地修改文件
    #[arg(short, long)]
    write: bool,

    /// 检查文件是否已格式化
    #[arg(long)]
    check: bool,

    /// 格式化输出宽度
    #[arg(short, long, default_value_t = 100)]
    max_width: usize,

    /// 缩进空格数
    #[arg(short, long, default_value_t = 4)]
    indent_width: usize,
}

/// 格式化选项
#[derive(Debug, Clone)]
struct FormatOptions {
    pub max_width: usize,
    pub indent_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            max_width: 100,
            indent_width: 4,
        }
    }
}

/// 格式化器
struct Formatter {
    options: FormatOptions,
    input: String,
    tokens: Vec<sengoo_compiler::lexer::Token>,
    lines: Vec<String>,
}

impl Formatter {
    fn new(input: String, options: FormatOptions) -> Self {
        let lexer = Lexer::new(&input, sengoo_compiler::lexer::LexerConfig::default());
        // TODO: 实际分词
        Self {
            options,
            input,
            tokens: Vec::new(),
            lines: input.lines().map(|s| s.to_string()).collect(),
        }
    }

    /// 格式化代码
    fn format(&mut self) -> String {
        let mut result = Vec::new();
        let mut indent_level = 0;

        for line in &self.lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // 计算缩进
            indent_level = self.calculate_indent_level(trimmed, indent_level);

            // 处理特殊行
            let formatted = if trimmed.starts_with('}') || trimmed.starts_with(']') {
                self.format_line(trimmed, indent_level.saturating_sub(1))
            } else if trimmed.starts_with('{') || trimmed.starts_with('[') {
                self.format_line(trimmed, indent_level)
            } else {
                self.format_line(trimmed, indent_level)
            };

            result.push(formatted);
        }

        result.join("\n")
    }

    fn calculate_indent_level(&self, line: &str, current: usize) -> usize {
        // 计算大括号/方括号的嵌套层级
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

        // 简单的格式化规则
        let formatted = line
            // 操作符周围添加空格
            .replace("=", " = ")
            .replace("+", " + ")
            .replace("-", " - ")
            .replace("*", " * ")
            .replace("/", " / ")
            .replace("==", " == ")
            .replace("!=", " != ")
            .replace("<=", " <= ")
            .replace(">=", " >= ")
            .replace("<", " < ")
            .replace(">", " > ")
            // 逗号后添加空格
            .replace(",", ", ")
            // 清理多余空格
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        // 处理特殊情况
        if formatted.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            // 语句或声明开始
            if formatted.contains('{') {
                format!("{}{{", formatted)
            } else if formatted.contains('=')
                && !formatted.contains("==")
                && !formatted.contains("!=")
            {
                // 变量声明
                format!("{};", formatted)
            } else {
                formatted
            }
        } else if formatted.starts_with('}') {
            format!("}};", formatted)
        } else {
            formatted
        };

        // 添加缩进
        if !formatted.is_empty() {
            format!("{}{}", indent, formatted)
        } else {
            String::new()
        }
    }
}

fn format_file(path: &PathBuf, options: &FormatOptions) -> Result<String> {
    let content = fs::read_to_string(path).into_diagnostic()?;

    // 解析代码
    let config = Config::default();
    let mut parser = Parser::new(&content, config.clone());
    parser.parse_file().into_diagnostic()?;

    // 格式化
    let mut formatter = Formatter::new(content, options.clone());
    let formatted = formatter.format();

    Ok(formatted)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let options = FormatOptions {
        max_width: args.max_width,
        indent_width: args.indent_width,
    };

    let formatted = format_file(&args.file, &options)?;

    if args.check {
        // 检查模式
        let original = fs::read_to_string(&args.file).into_diagnostic()?;
        if original.trim() != formatted.trim() {
            miette::bail!("文件 {} 需要格式化", args.file.display());
        }
    } else if args.write {
        // 写入模式
        fs::write(&args.file, formatted).into_diagnostic()?;
        eprintln!("格式化完成: {}", args.file.display());
    } else {
        // 输出模式
        println!("{}", formatted);
    }

    Ok(())
}
