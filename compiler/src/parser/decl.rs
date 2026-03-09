//! 声明解析。

use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;

use super::Parser;

#[derive(Debug, Clone, Default)]
struct ParsedFfiAttrs {
    no_mangle: bool,
    export_name: Option<String>,
    link_name: Option<String>,
}

impl ParsedFfiAttrs {
    fn has_export_attrs(&self) -> bool {
        self.no_mangle || self.export_name.is_some()
    }

    fn has_any(&self) -> bool {
        self.has_export_attrs() || self.link_name.is_some()
    }
}

impl<'source> Parser<'source> {
    fn check_any(&self, kinds: &[TokenKind]) -> bool {
        kinds.iter().any(|kind| self.check(kind.clone()))
    }

    fn parse_trait_bounds(&mut self) -> Result<Vec<TraitBound>> {
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

    fn parse_optional_where_clause(
        &mut self,
        type_params: &mut Vec<TypeParam>,
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
        while let Some(token) = self.tokens.get(clause_end_index) {
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

    fn parse_optional_contract_clauses(&mut self) -> Result<(Option<Expr>, Option<Expr>)> {
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

    fn parse_outer_attributes(&mut self) -> Result<ParsedFfiAttrs> {
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
                        "attributes/unsafe are not supported on struct declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_struct_decl(vis)?
            }

            // 解析 `enum`。
            Some(TokenKind::EnumKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "attributes/unsafe are not supported on enum declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_enum_decl(vis)?
            }

            // 解析 `class`。
            Some(TokenKind::ClassKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "attributes/unsafe are not supported on class declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_class_decl(vis)?
            }

            // 解析 `trait`。
            Some(TokenKind::TraitKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "attributes/unsafe are not supported on trait declarations".to_string(),
                    )));
                }
                self.advance();
                self.parse_trait_decl(vis)?
            }

            // 解析 `impl` 声明。
            Some(TokenKind::ImplKw) => {
                if leading_unsafe || attrs.has_any() {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "attributes/unsafe are not supported on impl declarations".to_string(),
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
                        "attributes/unsafe are not supported on const declarations".to_string(),
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
    fn parse_visibility(&mut self) -> Result<Visibility> {
        if self.consume(TokenKind::PubKw).is_some() {
            Ok(Visibility::Public)
        } else {
            self.consume(TokenKind::PrivKw);
            Ok(Visibility::Private)
        }
    }

    fn parse_extern_decl(
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
    fn check_self_param(&self) -> bool {
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
    fn parse_self_param(&mut self) -> Result<SelfParam> {
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
    fn parse_param(&mut self) -> Result<Param> {
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
    fn parse_type_params(&mut self) -> Result<Vec<TypeParam>> {
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

    /// 解析结构体声明。
    fn parse_struct_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 解析泛型参数。
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        self.parse_optional_where_clause(
            &mut type_params,
            &[TokenKind::LBrace, TokenKind::LParen, TokenKind::Semicolon],
        )?;

        let mut fields = Vec::new();

        if self.consume(TokenKind::LBrace).is_some() {
            // 具名字段结构体。
            while !self.is_eof() {
                if self.consume(TokenKind::RBrace).is_some() {
                    break;
                }

                let field_vis = self.parse_visibility()?;
                let name = self.expect_ident()?;

                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;

                fields.push(StructField {
                    vis: field_vis,
                    name: Some(name),
                    ty,
                    span: self.current_span(),
                });

                self.consume(TokenKind::Comma);
            }
        } else if self.consume(TokenKind::LParen).is_some() {
            // 元组结构体。
            while !self.is_eof() {
                if self.consume(TokenKind::RParen).is_some() {
                    break;
                }

                let ty = self.parse_type()?;
                fields.push(StructField {
                    vis: Visibility::Private,
                    name: None,
                    ty,
                    span: self.current_span(),
                });

                self.consume(TokenKind::Comma);
            }

            self.consume(TokenKind::Semicolon);
        } else {
            // 单元结构体。
            self.expect(TokenKind::Semicolon)?;
        }

        Ok(DeclKind::Struct(Struct {
            vis,
            name,
            type_params,
            fields,
            span: self.current_span(),
        }))
    }

    /// 解析枚举声明。
    fn parse_enum_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 解析泛型参数。
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        self.parse_optional_where_clause(&mut type_params, &[TokenKind::LBrace])?;

        self.expect(TokenKind::LBrace)?;

        let mut variants = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            let name = self.expect_ident()?;

            let (fields, discriminant) = if self.consume(TokenKind::LBrace).is_some() {
                // 结构体变体，如 `Point { x: T }`。
                let mut struct_fields = Vec::new();
                while !self.is_eof() {
                    if self.consume(TokenKind::RBrace).is_some() {
                        break;
                    }

                    let field_name = self.expect_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let ty = self.parse_type()?;
                    struct_fields.push(StructField {
                        vis: Visibility::Private,
                        name: Some(field_name),
                        ty,
                        span: self.current_span(),
                    });

                    self.consume(TokenKind::Comma);
                }
                (
                    struct_fields
                        .into_iter()
                        .map(|f| match f.name {
                            Some(name) => VariantField::Named(name, f.ty),
                            None => VariantField::Unnamed(f.ty),
                        })
                        .collect(),
                    None,
                )
            } else if self.consume(TokenKind::LParen).is_some() {
                // 元组变体，如 `Some(T)`。
                let mut types = Vec::new();
                while !self.is_eof() {
                    if self.consume(TokenKind::RParen).is_some() {
                        break;
                    }

                    types.push(self.parse_type()?);
                    self.consume(TokenKind::Comma);
                }
                (types.into_iter().map(VariantField::Unnamed).collect(), None)
            } else if self.consume(TokenKind::Eq).is_some() {
                // 显式判别值变体，如 `A = 1`。
                let value = self.parse_expr()?;
                self.consume(TokenKind::Comma);
                (Vec::new(), Some(Box::new(value)))
            } else {
                // 无载荷变体，如 `A`。
                self.consume(TokenKind::Comma);
                (Vec::new(), None)
            };

            variants.push(EnumVariant {
                name,
                fields,
                discriminant,
                span: self.current_span(),
            });
        }

        Ok(DeclKind::Enum(Enum {
            vis,
            name,
            type_params,
            variants,
            span: self.current_span(),
        }))
    }

    /// 解析类声明。
    fn parse_class_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 解析泛型参数。
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // 解析继承列表。
        let extends = if self.consume(TokenKind::Colon).is_some() {
            let parent = self.parse_path()?;

            if self.consume(TokenKind::Colon).is_some() {
                return Err(CompileError::ParseError(
                    ParseError::invalid_class_header_form(),
                ));
            }

            if self.check(TokenKind::Comma) {
                return Err(CompileError::ParseError(
                    ParseError::class_header_trait_list_not_supported(),
                ));
            }

            Some(parent)
        } else {
            None
        };

        self.parse_optional_where_clause(&mut type_params, &[TokenKind::LBrace])?;

        // V1 中 trait 适配通过 `impl Trait for Type` 表达。
        let implements = Vec::new();

        self.expect(TokenKind::LBrace)?;

        let mut members = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // 解析类成员的可见性与 async 修饰。
            let member_vis = self.parse_visibility()?;
            let member_async = self.consume(TokenKind::AsyncKw).is_some();

            if self.consume(TokenKind::DefKw).is_some() {
                // 兼容 Python 风格的 `def`。
                let name = self.expect_ident()?;
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

                if self.check_self_param() {
                    self_param = Some(self.parse_self_param()?);
                    self.consume(TokenKind::Comma);
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

                members.push(ClassMember::Method(Function {
                    vis: member_vis,
                    name,
                    type_params,
                    params,
                    self_param,
                    return_type,
                    precondition: precondition.map(Box::new),
                    postcondition: postcondition.map(Box::new),
                    body,
                    is_async: member_async,
                    abi: None,
                    is_unsafe: false,
                    no_mangle: false,
                    export_name: None,
                    span: self.current_span(),
                }));
            } else {
                // 解析字段成员。
                let name = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                self.consume(TokenKind::Semicolon);
                if member_async {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "`async` is only supported on class methods".to_string(),
                    )));
                }
                members.push(ClassMember::Field(StructField {
                    vis: member_vis,
                    name: Some(name),
                    ty,
                    span: self.current_span(),
                }));
            }
        }

        Ok(DeclKind::Class(Class {
            vis,
            name,
            type_params,
            extends,
            implements,
            members,
            span: self.current_span(),
        }))
    }

    /// 解析 trait 声明。
    fn parse_trait_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 解析泛型参数。
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // 解析 trait 继承约束。
        let mut bounds = Vec::new();
        if self.consume(TokenKind::Colon).is_some() {
            loop {
                if !matches!(self.current().map(|t| &t.kind), Some(TokenKind::LBrace)) {
                    let path = self.parse_path()?;
                    bounds.push(crate::ast::TraitBound::new(path));
                } else {
                    break;
                }
                self.consume(TokenKind::Comma);
            }
        }

        self.parse_optional_where_clause(&mut type_params, &[TokenKind::LBrace])?;

        self.expect(TokenKind::LBrace)?;

        let mut items = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            let trait_fn_async = self.consume(TokenKind::AsyncKw).is_some();
            // trait 方法也兼容 Python 风格的 `def`。
            if self.consume(TokenKind::DefKw).is_some() {
                let name = self.expect_ident()?;
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

                if self.check_self_param() {
                    self_param = Some(self.parse_self_param()?);
                    self.consume(TokenKind::Comma);
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

                items.push(TraitItem::Function(Function {
                    vis: Visibility::Private,
                    name,
                    type_params,
                    params,
                    self_param,
                    return_type,
                    precondition: precondition.map(Box::new),
                    postcondition: postcondition.map(Box::new),
                    body,
                    is_async: trait_fn_async,
                    abi: None,
                    is_unsafe: false,
                    no_mangle: false,
                    export_name: None,
                    span: self.current_span(),
                }));
            } else if self.consume(TokenKind::ConstKw).is_some() {
                if trait_fn_async {
                    return Err(CompileError::ParseError(ParseError::InvalidPattern(
                        "`async` is only supported on trait methods".to_string(),
                    )));
                }
                let name = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                self.expect(TokenKind::Eq)?;
                let value = self.parse_expr()?;
                self.consume(TokenKind::Semicolon);
                items.push(TraitItem::Const(Const {
                    vis: Visibility::Private,
                    name,
                    ty,
                    value: Box::new(value),
                    span: self.current_span(),
                }));
            } else {
                return Err(CompileError::ParseError(ParseError::expected_trait_item()));
            }
        }

        Ok(DeclKind::Trait(Trait {
            vis,
            name,
            type_params,
            bounds,
            items,
            span: self.current_span(),
        }))
    }

    /// 解析 impl 声明。
    fn parse_impl_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        // 解析泛型参数。
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // 先解析第一个类型。对 `impl Type { ... }` 来说，它就是目标类型。
        // 对 `impl Trait for Type { ... }` 来说，这里实际上是 trait 路径，
        // 真正的目标类型在 `for` 之后。
        let first_type = self.parse_type()?;

        let (target_type, trait_path) = if self.consume(TokenKind::ForKw).is_some() {
            // `impl Trait for Type` 中，`first_type` 实际上是 trait 路径。
            let actual_target = self.parse_type()?;
            // 从第一个类型里提取 trait 路径。
            let trait_path = match first_type.kind {
                TypeKind::Path(path) => path,
                TypeKind::PathWithArgs { path, args } if args.is_empty() => path,
                _ => {
                    return Err(CompileError::ParseError(
                        ParseError::expected_trait_path_in_impl(),
                    ));
                }
            };
            (actual_target, Some(trait_path))
        } else {
            // `impl Type` 表示固有 impl，不带 trait。
            (first_type, None)
        };

        self.parse_optional_where_clause(&mut type_params, &[TokenKind::LBrace])?;

        self.expect(TokenKind::LBrace)?;

        let mut items = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            self.consume(TokenKind::PubKw);
            let method_async = self.consume(TokenKind::AsyncKw).is_some();
            self.expect(TokenKind::DefKw)?;

            let name = self.expect_ident()?;
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

            if self.check_self_param() {
                self_param = Some(self.parse_self_param()?);
                self.consume(TokenKind::Comma);
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

            items.push(Function {
                vis: Visibility::Private,
                name,
                type_params,
                params,
                self_param,
                return_type,
                precondition: precondition.map(Box::new),
                postcondition: postcondition.map(Box::new),
                body,
                is_async: method_async,
                abi: None,
                is_unsafe: false,
                no_mangle: false,
                export_name: None,
                span: self.current_span(),
            });
        }

        Ok(DeclKind::Impl(Impl {
            vis,
            type_params,
            target_type,
            trait_path,
            items,
            span: self.current_span(),
        }))
    }

    /// 解析类型别名声明。
    fn parse_type_alias_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 解析泛型参数。
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        self.parse_optional_where_clause(&mut type_params, &[TokenKind::Assign, TokenKind::Eq])?;

        if self.consume(TokenKind::Assign).is_none() && self.consume(TokenKind::Eq).is_none() {
            self.expect(TokenKind::Assign)?;
        }
        let ty = self.parse_type()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(DeclKind::TypeAlias(TypeAlias {
            vis,
            name,
            type_params,
            ty,
            span: self.current_span(),
        }))
    }

    /// 解析常量声明。
    fn parse_const_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(DeclKind::Const(Const {
            vis,
            name,
            ty,
            value: Box::new(value),
            span: self.current_span(),
        }))
    }

    /// 解析静态变量声明。
    fn parse_static_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let is_mut = self.consume(TokenKind::MutKw).is_some();
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(DeclKind::Static(Static {
            vis,
            is_mut,
            name,
            ty,
            value: Box::new(value),
            span: self.current_span(),
        }))
    }

    /// 解析导入声明。
    fn parse_import_decl(&mut self) -> Result<DeclKind> {
        let path = self.parse_path()?;

        let (kind, alias) = if self.consume(TokenKind::AsKw).is_some() {
            (ImportKind::Simple, Some(self.expect_ident()?))
        } else if self.consume(TokenKind::LBrace).is_some() {
            // 选择性导入。
            let mut names = Vec::new();
            while !self.is_eof() {
                if self.consume(TokenKind::RBrace).is_some() {
                    break;
                }
                names.push(self.expect_ident()?);
                self.consume(TokenKind::Comma);
            }
            (ImportKind::Selective(names), None)
        } else if self.consume(TokenKind::Star).is_some() {
            // 通配符导入。
            self.expect(TokenKind::FromKw)?;
            (ImportKind::Wildcard, None)
        } else {
            (ImportKind::Simple, None)
        };

        self.expect(TokenKind::Semicolon)?;

        Ok(DeclKind::Import(Import {
            path,
            alias,
            kind,
            span: self.current_span(),
        }))
    }
}
