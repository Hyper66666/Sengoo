//! Formatter 协议支持：`impl Display/Debug for T` 可以定义
//! `def fmt(&self, f: &mut Formatter)` 代替 `to_string`。
//! 本 pass 在解析后为仅提供 `fmt` 的 impl 合成一个
//! `to_string(&self) -> String`，其主体通过 `Formatter` 缓冲驱动 `fmt`。

use crate::ast::{DeclKind, Function, Program};
use crate::error::CompileError;
use crate::Result;

use super::Parser;

const SYNTH_TO_STRING_SNIPPET: &str = r#"
impl Display for __FormatterProtocolTarget {
    def to_string(&self) -> String {
        let mut __formatter = formatter_new();
        self.fmt(&mut __formatter);
        __formatter.finish()
    }
}
"#;

pub(super) fn synthesize_formatter_to_string(program: &mut Program) -> Result<()> {
    let mut template: Option<Function> = None;

    for decl in &mut program.decls {
        let DeclKind::Impl(impl_decl) = &mut decl.kind else {
            continue;
        };
        let Some(trait_name) = impl_decl
            .trait_path
            .as_ref()
            .and_then(|path| path.as_simple())
            .map(|ident| ident.name.as_str())
        else {
            continue;
        };
        if trait_name != "Display" && trait_name != "Debug" {
            continue;
        }
        let has_fmt = impl_decl
            .items
            .iter()
            .any(|item| item.name.name == "fmt" && item.self_param.is_some());
        let has_to_string = impl_decl
            .items
            .iter()
            .any(|item| item.name.name == "to_string");
        if !has_fmt || has_to_string {
            continue;
        }

        if template.is_none() {
            template = Some(parse_synth_to_string()?);
        }
        impl_decl
            .items
            .push(template.clone().expect("template just initialized"));
    }

    Ok(())
}

fn parse_synth_to_string() -> Result<Function> {
    let mut parser = Parser::new(SYNTH_TO_STRING_SNIPPET);
    let program = parser.parse_program()?;
    for decl in program.decls {
        if let DeclKind::Impl(impl_decl) = decl.kind {
            if let Some(func) = impl_decl.items.into_iter().next() {
                return Ok(func);
            }
        }
    }
    Err(CompileError::HirLower(
        "internal error: formatter protocol to_string template failed to parse".to_string(),
    ))
}
