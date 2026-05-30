use crate::ast::*;
use crate::lexer::TokenKind;
use crate::Result;

use super::super::Parser;

impl<'source> Parser<'source> {
    /// 解析结构体声明。
    pub(super) fn parse_struct_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
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
    pub(super) fn parse_enum_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
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
}
