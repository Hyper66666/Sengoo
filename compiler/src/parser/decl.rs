//! 澹版槑瑙ｆ瀽

use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;

use super::Parser;

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
    /// 瑙ｆ瀽澹版槑
    pub(super) fn parse_decl(&mut self) -> Result<Decl> {
        let lo = self.current_span().lo;
        let vis = self.parse_visibility()?;

        let kind = match self.current().map(|t| &t.kind) {
            // 鍑芥暟 (Python 椋庢牸: def)
            Some(TokenKind::DefKw) => {
                self.advance();
                self.parse_function_decl(vis)?
            }

            // 缁撴瀯浣?
            Some(TokenKind::StructKw) => {
                self.advance();
                self.parse_struct_decl(vis)?
            }

            // 鏋氫妇
            Some(TokenKind::EnumKw) => {
                self.advance();
                self.parse_enum_decl(vis)?
            }

            // 绫?
            Some(TokenKind::ClassKw) => {
                self.advance();
                self.parse_class_decl(vis)?
            }

            // trait
            Some(TokenKind::TraitKw) => {
                self.advance();
                self.parse_trait_decl(vis)?
            }

            // impl 鍧?
            Some(TokenKind::ImplKw) => {
                self.advance();
                self.parse_impl_decl(vis)?
            }

            // 绫诲瀷鍒悕
            Some(TokenKind::TypeKw) => {
                self.advance();
                self.parse_type_alias_decl(vis)?
            }

            // 甯搁噺
            Some(TokenKind::ConstKw) => {
                self.advance();
                self.parse_const_decl(vis)?
            }

            // 闈欐€佸彉閲?
            Some(TokenKind::StaticKw) => {
                self.advance();
                self.parse_static_decl(vis)?
            }

            // 瀵煎叆
            Some(TokenKind::ImportKw) => {
                self.advance();
                self.parse_import_decl()?
            }

            _ => {
                return Err(CompileError::ParseError(ParseError::expected_declaration()));
            }
        };

        Ok(Decl::new(kind, self.span_at(lo)))
    }

    /// 瑙ｆ瀽鍙鎬?
    fn parse_visibility(&mut self) -> Result<Visibility> {
        if self.consume(TokenKind::PubKw).is_some() {
            Ok(Visibility::Public)
        } else {
            self.consume(TokenKind::PrivKw);
            Ok(Visibility::Private)
        }
    }

    /// 瑙ｆ瀽鍑芥暟澹版槑
    fn parse_function_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 绫诲瀷鍙傛暟
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

        // 妫€鏌?self 鍙傛暟
        if self.check_self_param() {
            self_param = Some(self.parse_self_param()?);
            if self.consume(TokenKind::Comma).is_some() {
                // 缁х画瑙ｆ瀽鍏朵粬鍙傛暟
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

        let body = self.parse_block()?;

        Ok(DeclKind::Function(Function {
            vis,
            name,
            type_params,
            params,
            self_param,
            return_type,
            body,
            is_async: false,
            span: self.current_span(),
        }))
    }

    /// 妫€鏌ユ槸鍚︽槸 self 鍙傛暟
    fn check_self_param(&self) -> bool {
        if let Some(token) = self.current() {
            match &token.kind {
                TokenKind::SelfLowerKw | TokenKind::MutKw => {
                    // 妫€鏌ュ悗闈㈡槸鍚︽槸 self
                    if let TokenKind::MutKw = &token.kind {
                        self.check_peek(TokenKind::SelfLowerKw)
                    } else {
                        true
                    }
                }
                TokenKind::And => {
                    // &self 鎴?&mut self
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

    /// 瑙ｆ瀽 self 鍙傛暟
    fn parse_self_param(&mut self) -> Result<SelfParam> {
        if self.consume(TokenKind::And).is_some() {
            // &self 鎴?&mut self
            let is_mut = self.consume(TokenKind::MutKw).is_some();
            self.expect(TokenKind::SelfLowerKw)?;
            Ok(if is_mut {
                SelfParam::BorrowedMut
            } else {
                SelfParam::Borrowed
            })
        } else {
            // self, mut self
            let is_mut = self.consume(TokenKind::MutKw).is_some();
            self.expect(TokenKind::SelfLowerKw)?;
            Ok(if is_mut {
                SelfParam::OwnedMut
            } else {
                SelfParam::Owned
            })
        }
    }

    /// 瑙ｆ瀽鍙傛暟
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

    /// 瑙ｆ瀽绫诲瀷鍙傛暟
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

    /// 瑙ｆ瀽缁撴瀯浣撳０鏄?
    fn parse_struct_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 绫诲瀷鍙傛暟
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
            // 鍛藉悕瀛楁缁撴瀯浣?
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
            // 鍏冪粍缁撴瀯浣?
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
            // 鍗曞厓缁撴瀯浣?
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

    /// 瑙ｆ瀽鏋氫妇澹版槑
    fn parse_enum_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 绫诲瀷鍙傛暟
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
                // 缁撴瀯浣撳彉浣?
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
                // 鍏冪粍鍙樹綋
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
                // 甯﹀垽鍒€肩殑鍙樹綋
                let value = self.parse_expr()?;
                self.consume(TokenKind::Comma);
                (Vec::new(), Some(Box::new(value)))
            } else {
                // 鍗曞厓鍙樹綋
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

    /// 瑙ｆ瀽绫诲０鏄?
    fn parse_class_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 绫诲瀷鍙傛暟
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // 缁ф壙
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

        // V1: trait conformance is expressed through impl Trait for Type.
        let implements = Vec::new();

        self.expect(TokenKind::LBrace)?;

        let mut members = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // 鍙互鏄瓧娈垫垨鏂规硶
            let member_vis = self.parse_visibility()?;

            if self.consume(TokenKind::DefKw).is_some() {
                // 鏂规硶 (Python 椋庢牸: def)
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

                let body = self.parse_block()?;

                members.push(ClassMember::Method(Function {
                    vis: member_vis,
                    name,
                    type_params,
                    params,
                    self_param,
                    return_type,
                    body,
                    is_async: false,
                    span: self.current_span(),
                }));
            } else {
                // 瀛楁
                let name = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                self.consume(TokenKind::Semicolon);
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

    /// 瑙ｆ瀽 trait 澹版槑
    fn parse_trait_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 绫诲瀷鍙傛暟
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // 绾︽潫
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

            // 鍑芥暟鎴栧父閲?(Python 椋庢牸: def)
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

                let body = self.parse_block()?;

                items.push(TraitItem::Function(Function {
                    vis: Visibility::Private,
                    name,
                    type_params,
                    params,
                    self_param,
                    return_type,
                    body,
                    is_async: false,
                    span: self.current_span(),
                }));
            } else if self.consume(TokenKind::ConstKw).is_some() {
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

    /// 瑙ｆ瀽 impl 鍧?
    fn parse_impl_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        // 绫诲瀷鍙傛暟
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // Parse the first type. For `impl Type { ... }` this is the target type.
        // For `impl Trait for Type { ... }` this is actually the trait path, and
        // the real target type comes after `for`.
        let first_type = self.parse_type()?;

        let (target_type, trait_path) = if self.consume(TokenKind::ForKw).is_some() {
            // `impl Trait for Type` 鈥?first_type is the trait, parse the real target type
            let actual_target = self.parse_type()?;
            // Extract the path from the first type (the trait)
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
            // `impl Type` 鈥?inherent impl, no trait
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

            let body = self.parse_block()?;

            items.push(Function {
                vis: Visibility::Private,
                name,
                type_params,
                params,
                self_param,
                return_type,
                body,
                is_async: false,
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

    /// 瑙ｆ瀽绫诲瀷鍒悕
    fn parse_type_alias_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 绫诲瀷鍙傛暟
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

    /// 瑙ｆ瀽甯搁噺澹版槑
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

    /// 瑙ｆ瀽闈欐€佸彉閲忓０鏄?
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

    /// 瑙ｆ瀽瀵煎叆澹版槑
    fn parse_import_decl(&mut self) -> Result<DeclKind> {
        let path = self.parse_path()?;

        let (kind, alias) = if self.consume(TokenKind::AsKw).is_some() {
            (ImportKind::Simple, Some(self.expect_ident()?))
        } else if self.consume(TokenKind::LBrace).is_some() {
            // 閫夋嫨鎬у鍏?
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
            // 閫氶厤绗﹀鍏?
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
