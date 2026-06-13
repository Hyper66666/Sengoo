//! 声明解析。

use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;

use super::Parser;

mod data_declarations;
mod ffi;
mod object_declarations;
mod simple;

impl<'source> Parser<'source> {
    fn check_any(&mut self, kinds: &[TokenKind]) -> bool {
        kinds.iter().any(|kind| self.check(kind.clone()))
    }

    pub(super) fn parse_trait_bounds(&mut self) -> Result<Vec<TraitBound>> {
        let mut bounds = Vec::new();
        loop {
            let path = self.parse_path()?;
            bounds.push(TraitBound::new(path));
            if self.consume(TokenKind::Plus).is_none() {
                break;
            }
        }
        Ok(bounds)
    }

    fn merge_where_bounds(
        &self,
        type_params: &mut [TypeParam],
        param_name: &Ident,
        bounds: Vec<TraitBound>,
    ) -> Result<()> {
        if let Some(param) = type_params
            .iter_mut()
            .find(|param| param.name.name == param_name.name)
        {
            param.bounds.extend(bounds);
            return Ok(());
        }

        Err(CompileError::ParseError(ParseError::InvalidPattern(
            format!(
                "invalid where clause: unknown type parameter `{}`",
                param_name.name
            ),
        )))
    }

    pub(super) fn parse_optional_where_clause(
        &mut self,
        type_params: &mut [TypeParam],
        terminators: &[TokenKind],
    ) -> Result<()> {
        if self.consume(TokenKind::WhereKw).is_none() {
            return Ok(());
        }

        while !self.is_eof() && !self.check_any(terminators) {
            let param_name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let bounds = self.parse_trait_bounds()?;
            self.merge_where_bounds(type_params, &param_name, bounds)?;

            if self.consume(TokenKind::Comma).is_none() {
                break;
            }
        }

        if !self.check_any(terminators) {
            return Err(CompileError::ParseError(ParseError::InvalidPattern(
                "invalid where clause: expected declaration body".to_string(),
            )));
        }

        Ok(())
    }

    fn parse_contract_clause_expr(
        &mut self,
        keyword_span: crate::lexer::Span,
        clause_name: &str,
    ) -> Result<Expr> {
        let clause_start = keyword_span.hi as usize;
        let mut clause_end_index = self.pos;
        loop {
            self.fill_to(clause_end_index.saturating_sub(self.pos));
            let Some(token) = self.tokens.get(clause_end_index) else {
                break;
            };
            if matches!(
                token.kind,
                TokenKind::RequiresKw | TokenKind::EnsuresKw | TokenKind::LBrace
            ) {
                break;
            }
            clause_end_index += 1;
        }

        let clause_end = self
            .tokens
            .get(clause_end_index)
            .map(|token| token.span.lo as usize)
            .unwrap_or_else(|| self.source.len());

        let snippet = self.source[clause_start..clause_end]
            .trim()
            .trim_end_matches(';')
            .trim();
        if snippet.is_empty() {
            return Err(CompileError::ParseError(ParseError::InvalidPattern(
                format!("{} clause requires an expression", clause_name),
            )));
        }

        let mut clause_parser = Parser::new(snippet);
        let expr = clause_parser.parse_expr()?;
        if !clause_parser.is_eof() {
            return Err(CompileError::ParseError(ParseError::InvalidPattern(
                format!("invalid expression in {} clause", clause_name),
            )));
        }

        self.pos = clause_end_index;
        Ok(expr)
    }

    pub(super) fn parse_optional_contract_clauses(
        &mut self,
    ) -> Result<(Option<Expr>, Option<Expr>)> {
        let mut precondition = None;
        let mut postcondition = None;

        loop {
            if let Some(keyword) = self.consume(TokenKind::RequiresKw) {
                if precondition.is_some() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "duplicate requires clause".to_string(),
                    )));
                }
                precondition = Some(self.parse_contract_clause_expr(keyword.span, "requires")?);
                continue;
            }

            if let Some(keyword) = self.consume(TokenKind::EnsuresKw) {
                if postcondition.is_some() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "duplicate ensures clause".to_string(),
                    )));
                }
                postcondition = Some(self.parse_contract_clause_expr(keyword.span, "ensures")?);
                continue;
            }

            break;
        }

        Ok((precondition, postcondition))
    }

    /// 解析顶层声明。
    pub(super) fn parse_decl(&mut self) -> Result<Decl> {
        let lo = self.current_span().lo;
        let attrs = self.parse_outer_attributes()?;
        let vis = self.parse_visibility()?;
        let leading_unsafe = self.consume(TokenKind::UnsafeKw).is_some();
        let leading_async = self.consume(TokenKind::AsyncKw).is_some();

        let kind = match self.current().map(|t| &t.kind) {
            Some(TokenKind::ExternKw) => {
                self.advance();
                self.parse_extern_decl(vis, attrs.clone(), leading_unsafe)?
            }

            // 兼容 Python 风格的 `def`。
            Some(TokenKind::DefKw) => {
                if leading_unsafe {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "`unsafe` is only supported with `extern` declarations".to_string(),
                    )));
                }
                if attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "attributes are only supported on extern declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_function_decl(vis, leading_async)?
            }

            _ if leading_async => {
                return Err(CompileError::ParseError(ParseError::InvalidPattern(
                    "`async` is only supported on function declarations".to_string(),
                )));
            }

            // 解析 `struct`。
            Some(TokenKind::StructKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "unsafe is not supported on struct declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_struct_decl(vis)?
            }

            // 解析 `enum`。
            Some(TokenKind::EnumKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "unsafe is not supported on enum declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_enum_decl(vis)?
            }

            // 解析 `class`。
            Some(TokenKind::ClassKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "unsafe is not supported on class declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_class_decl(vis)?
            }

            // 解析 `trait`。
            Some(TokenKind::TraitKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "unsafe is not supported on trait declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_trait_decl(vis)?
            }

            // 解析 `impl` 声明。
            Some(TokenKind::ImplKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "unsafe is not supported on impl declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_impl_decl(vis)?
            }

            // 解析类型别名。
            Some(TokenKind::TypeKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "attributes/unsafe are not supported on type alias declarations"
                            .to_string(),
                    )));
                }
                self.advance();
                self.parse_type_alias_decl(vis)?
            }

            // 解析 `const`。
            Some(TokenKind::ConstKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "unsafe is not supported on const declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_const_decl(vis)?
            }

            // 解析 `static`。
            Some(TokenKind::StaticKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "attributes/unsafe are not supported on static declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_static_decl(vis)?
            }

            // 解析 `import`。
            Some(TokenKind::ImportKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "attributes/unsafe are not supported on import declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_import_decl()?
            }

            _ => {
                return Err(CompileError::ParseError(ParseError::expected_declaration()));
            }
        };

        Ok(Decl::new(kind, self.span_at(lo)))
    }

    /// 解析可见性修饰符。
    pub(super) fn parse_visibility(&mut self) -> Result<Visibility> {
        if self.consume(TokenKind::PubKw).is_some() {
            Ok(Visibility::Public)
        } else {
            self.consume(TokenKind::PrivKw);
            Ok(Visibility::Private)
        }
    }

    /// 解析函数声明。
    fn parse_function_decl(&mut self, vis: Visibility, is_async: bool) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 解析泛型参数。
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LParen)?;

        let mut params = Vec::new();
        let mut self_param = None;

        // 先处理可能出现的 self 参数。
        if self.check_self_param() {
            self_param = Some(self.parse_self_param()?);
            if self.consume(TokenKind::Comma).is_some() {
                // 允许 `self` 后面跟逗号继续解析普通参数。
            }
        }

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

        self.parse_optional_where_clause(&mut type_params, &[TokenKind::LBrace])?;
        let (precondition, postcondition) = self.parse_optional_contract_clauses()?;

        let body = self.parse_block()?;

        Ok(DeclKind::Function(Function {
            vis,
            name,
            type_params,
            params,
            self_param,
            return_type,
            precondition: precondition.map(Box::new),
            postcondition: postcondition.map(Box::new),
            body,
            is_async,
            abi: None,
            is_unsafe: false,
            no_mangle: false,
            export_name: None,
            span: self.current_span(),
        }))
    }

    /// 检查当前位置是否是 self 参数。
    pub(super) fn check_self_param(&mut self) -> bool {
        if let Some(token) = self.current() {
            match &token.kind {
                TokenKind::SelfLowerKw | TokenKind::MutKw => {
                    // 处理 `mut self`。
                    if let TokenKind::MutKw = &token.kind {
                        self.check_peek(TokenKind::SelfLowerKw)
                    } else {
                        true
                    }
                }
                TokenKind::BitAnd => {
                    // 处理 `&self` 或 `&mut self`。
                    if let Some(next) = self.peek(1) {
                        matches!(next.kind, TokenKind::SelfLowerKw | TokenKind::MutKw)
                    } else {
                        false
                    }
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// 解析 self 参数。
    pub(super) fn parse_self_param(&mut self) -> Result<SelfParam> {
        if self.consume(TokenKind::BitAnd).is_some() {
            // 处理 `&self` 或 `&mut self`。
            let is_mut = self.consume(TokenKind::MutKw).is_some();
            self.expect(TokenKind::SelfLowerKw)?;
            Ok(if is_mut {
                SelfParam::BorrowedMut
            } else {
                SelfParam::Borrowed
            })
        } else {
            // 处理 `self` 与 `mut self`。
            let is_mut = self.consume(TokenKind::MutKw).is_some();
            self.expect(TokenKind::SelfLowerKw)?;
            Ok(if is_mut {
                SelfParam::OwnedMut
            } else {
                SelfParam::Owned
            })
        }
    }

    /// 解析普通参数。
    pub(super) fn parse_param(&mut self) -> Result<Param> {
        let lo = self.current_span().lo;
        let is_mut = self.consume(TokenKind::MutKw).is_some();
        let name = self.expect_ident()?;

        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type()?;

        Ok(Param {
            name,
            ty,
            is_mut,
            span: self.span_at(lo),
        })
    }

    /// 解析泛型参数列表。
    pub(super) fn parse_type_params(&mut self) -> Result<Vec<TypeParam>> {
        let mut params = Vec::new();

        while !self.is_eof() {
            if self.check(TokenKind::Gt) || self.check(TokenKind::Shr) {
                break;
            }

            let name = self.expect_ident()?;
            let mut param = TypeParam::new(name);
            if self.consume(TokenKind::Colon).is_some() {
                let bounds = self.parse_trait_bounds()?;
                param = param.with_bounds(bounds);
            }

            if self.consume(TokenKind::Assign).is_some() || self.consume(TokenKind::Eq).is_some() {
                let default_ty = self.parse_type()?;
                param = param.with_default(default_ty);
            }

            params.push(param);

            self.consume(TokenKind::Comma);
        }

        Ok(params)
    }
}
