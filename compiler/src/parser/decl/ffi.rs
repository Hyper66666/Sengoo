use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;

use super::super::Parser;

#[derive(Debug, Clone, Default)]
pub(super) struct ParsedFfiAttrs {
    no_mangle: bool,
    export_name: Option<String>,
    link_name: Option<String>,
}

impl ParsedFfiAttrs {
    fn has_export_attrs(&self) -> bool {
        self.no_mangle || self.export_name.is_some()
    }

    pub(super) fn has_any(&self) -> bool {
        self.has_export_attrs() || self.link_name.is_some()
    }
}

impl<'source> Parser<'source> {
    fn consume_attribute_assign(&mut self) -> bool {
        self.consume(TokenKind::Assign).is_some() || self.consume(TokenKind::Eq).is_some()
    }

    fn expect_string_literal(&mut self, context: &str) -> Result<String> {
        match self.current().cloned() {
            Some(token) => match token.kind {
                TokenKind::String(Some(value)) => {
                    self.advance();
                    Ok(value)
                }
                _ => Err(CompileError::ParseError(ParseError::InvalidPattern(
                    format!("{} requires a string literal", context),
                ))),
            },
            None => Err(CompileError::ParseError(ParseError::UnexpectedEof)),
        }
    }

    pub(super) fn parse_outer_attributes(&mut self) -> Result<ParsedFfiAttrs> {
        let mut attrs = ParsedFfiAttrs::default();

        while self.consume(TokenKind::Hash).is_some() {
            self.expect(TokenKind::LBracket)?;
            let attr_name = self.expect_ident()?;

            match attr_name.name.as_str() {
                "no_mangle" => {}
                "export_name" => {
                    if !self.consume_attribute_assign() {
                        return Err(CompileError::ParseError(ParseError::InvalidPattern(
                            "export_name attribute requires `=`".to_string(),
                        )));
                    }
                    attrs.export_name = Some(self.expect_string_literal("export_name attribute")?);
                }
                "link" => {
                    self.expect(TokenKind::LParen)?;
                    let key = self.expect_ident()?;
                    if key.name != "name" {
                        return Err(CompileError::ParseError(ParseError::InvalidPattern(
                            "link attribute only supports `name = \"...\"`".to_string(),
                        )));
                    }
                    if !self.consume_attribute_assign() {
                        return Err(CompileError::ParseError(ParseError::InvalidPattern(
                            "link attribute requires `=`".to_string(),
                        )));
                    }
                    attrs.link_name = Some(self.expect_string_literal("link attribute")?);
                    self.expect(TokenKind::RParen)?;
                }
                _ => {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        format!("unsupported attribute `{}`", attr_name.name),
                    )));
                }
            }

            if attr_name.name == "no_mangle" {
                attrs.no_mangle = true;
            }

            self.expect(TokenKind::RBracket)?;
        }

        Ok(attrs)
    }

    pub(super) fn parse_extern_decl(
        &mut self,
        vis: Visibility,
        attrs: ParsedFfiAttrs,
        leading_unsafe: bool,
    ) -> Result<DeclKind> {
        let abi = self.expect_string_literal("extern ABI")?;

        match self.current().map(|t| &t.kind) {
            Some(TokenKind::LBrace) => self.parse_extern_block_decl(abi, attrs, leading_unsafe),
            Some(TokenKind::FnKw) | Some(TokenKind::UnsafeKw) => {
                self.parse_extern_fn_decl(vis, abi, attrs, leading_unsafe)
            }
            _ => Err(CompileError::ParseError(ParseError::InvalidPattern(
                "expected `fn` or `{` after `extern \"...\"`".to_string(),
            ))),
        }
    }

    fn parse_extern_block_decl(
        &mut self,
        abi: String,
        attrs: ParsedFfiAttrs,
        leading_unsafe: bool,
    ) -> Result<DeclKind> {
        if leading_unsafe {
            return Err(CompileError::ParseError(ParseError::InvalidPattern(
                "`unsafe extern` block is not supported; mark extern items unsafe instead"
                    .to_string(),
            )));
        }
        if attrs.has_export_attrs() {
            return Err(CompileError::ParseError(ParseError::InvalidPattern(
                "no_mangle/export_name only apply to extern function definitions".to_string(),
            )));
        }

        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        let mut link_name = attrs.link_name.clone();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            let item_attrs = self.parse_outer_attributes()?;
            let item_vis = self.parse_visibility()?;
            let item_is_unsafe = self.consume(TokenKind::UnsafeKw).is_some();

            if let Some(item_link_name) = item_attrs.link_name.clone() {
                if link_name.is_none() {
                    link_name = Some(item_link_name);
                }
            }

            if item_attrs.has_export_attrs() {
                return Err(CompileError::ParseError(ParseError::InvalidPattern(
                    "no_mangle/export_name cannot be used inside extern blocks".to_string(),
                )));
            }

            if self.consume(TokenKind::FnKw).is_some() {
                let name = self.expect_ident()?;
                self.expect(TokenKind::LParen)?;
                let mut params = Vec::new();
                while !self.is_eof() {
                    if self.consume(TokenKind::RParen).is_some() {
                        break;
                    }
                    params.push(self.parse_param()?);
                    self.consume(TokenKind::Comma);
                }
                let return_type = if self.consume(TokenKind::Arrow).is_some() {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                self.expect(TokenKind::Semicolon)?;
                items.push(ExternItem::Function(ExternFunction {
                    vis: item_vis,
                    name,
                    params,
                    return_type,
                    is_unsafe: item_is_unsafe,
                    span: self.current_span(),
                }));
                continue;
            }

            if self.consume(TokenKind::StaticKw).is_some() {
                let is_mut = self.consume(TokenKind::MutKw).is_some();
                let name = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                self.expect(TokenKind::Semicolon)?;
                items.push(ExternItem::Static(ExternStatic {
                    vis: item_vis,
                    is_mut,
                    name,
                    ty,
                    span: self.current_span(),
                }));
                continue;
            }

            return Err(CompileError::ParseError(ParseError::InvalidPattern(
                "expected `fn` or `static` in extern block".to_string(),
            )));
        }

        Ok(DeclKind::ExternBlock(ExternBlock {
            abi,
            link_name,
            items,
            span: self.current_span(),
        }))
    }

    fn parse_extern_fn_decl(
        &mut self,
        vis: Visibility,
        abi: String,
        attrs: ParsedFfiAttrs,
        leading_unsafe: bool,
    ) -> Result<DeclKind> {
        if attrs.link_name.is_some() {
            return Err(CompileError::ParseError(ParseError::InvalidPattern(
                "link(name = ...) only applies to extern blocks".to_string(),
            )));
        }

        let mut is_unsafe = leading_unsafe;
        if self.consume(TokenKind::UnsafeKw).is_some() {
            is_unsafe = true;
        }
        self.expect(TokenKind::FnKw)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LParen)?;

        let mut params = Vec::new();
        while !self.is_eof() {
            if self.consume(TokenKind::RParen).is_some() {
                break;
            }
            params.push(self.parse_param()?);
            self.consume(TokenKind::Comma);
        }

        let return_type = if self.consume(TokenKind::Arrow).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;

        Ok(DeclKind::Function(Function {
            vis,
            name,
            type_params: Vec::new(),
            params,
            self_param: None,
            return_type,
            precondition: None,
            postcondition: None,
            body,
            is_async: false,
            abi: Some(abi),
            is_unsafe,
            no_mangle: attrs.no_mangle,
            export_name: attrs.export_name,
            span: self.current_span(),
        }))
    }
}
