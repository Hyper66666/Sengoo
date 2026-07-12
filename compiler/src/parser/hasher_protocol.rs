//! Hash protocol support: `impl Hash for T` may define
//! `def hash_into(&self, h: &mut Hasher)` instead of `hash`.
//!
//! After parsing, this pass synthesizes `hash(&self) -> i64` so existing
//! generic `T: Hash` code can keep calling `value.hash()`.

use crate::ast::{DeclKind, Function, Program};
use crate::error::CompileError;
use crate::Result;

use super::Parser;

const SYNTH_HASH_SNIPPET: &str = r#"
impl Hash for __HasherProtocolTarget {
    def hash(&self) -> i64 {
        let mut __hasher = hasher_new();
        self.hash_into(&mut __hasher);
        __hasher.finish()
    }
}
"#;

pub(super) fn synthesize_hash_from_hash_into(program: &mut Program) -> Result<()> {
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
        if trait_name != "Hash" {
            continue;
        }

        let has_hash_into = impl_decl
            .items
            .iter()
            .any(|item| item.name.name == "hash_into" && item.self_param.is_some());
        let has_hash = impl_decl.items.iter().any(|item| item.name.name == "hash");
        if !has_hash_into || has_hash {
            continue;
        }

        if template.is_none() {
            template = Some(parse_synth_hash()?);
        }
        impl_decl
            .items
            .push(template.clone().expect("template just initialized"));
    }

    Ok(())
}

fn parse_synth_hash() -> Result<Function> {
    let mut parser = Parser::new(SYNTH_HASH_SNIPPET);
    let program = parser.parse_program()?;
    for decl in program.decls {
        if let DeclKind::Impl(impl_decl) = decl.kind {
            if let Some(func) = impl_decl.items.into_iter().next() {
                return Ok(func);
            }
        }
    }
    Err(CompileError::HirLower(
        "internal error: hasher protocol hash template failed to parse".to_string(),
    ))
}
