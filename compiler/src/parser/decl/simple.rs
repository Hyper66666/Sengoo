use crate::ast::*;
use crate::lexer::TokenKind;
use crate::Result;

use super::super::Parser;

impl<'source> Parser<'source> {
    /// 解析类型别名声明。
    pub(super) fn parse_type_alias_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
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
    pub(super) fn parse_const_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
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
    pub(super) fn parse_static_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
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
    pub(super) fn parse_import_decl(&mut self) -> Result<DeclKind> {
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
