use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;

use super::super::Parser;

impl<'source> Parser<'source> {
    /// 解析类声明。
    pub(super) fn parse_class_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 解析泛型参数。
        let mut type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // 解析继承/实现列表：`class Child: Base, TraitA` 或 `class Service: TraitA`.
        let (extends, implements) = if self.consume(TokenKind::Colon).is_some() {
            let first = self.parse_path()?;
            let mut extra_paths = Vec::new();
            while self.consume(TokenKind::Comma).is_some() {
                extra_paths.push(self.parse_path()?);
            }
            let implements = extra_paths
                .into_iter()
                .map(crate::ast::TraitBound::new)
                .collect::<Vec<_>>();
            (Some(first), implements)
        } else {
            (None, Vec::new())
        };

        self.parse_optional_where_clause(&mut type_params, &[TokenKind::LBrace])?;

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
    pub(super) fn parse_trait_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
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
    pub(super) fn parse_impl_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
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

        let (target_type, trait_path, trait_args) = if self.consume(TokenKind::ForKw).is_some() {
            // `impl Trait for Type` 中，`first_type` 实际上是 trait 路径。
            let actual_target = self.parse_type()?;
            // 从第一个类型里提取 trait 路径。
            let (trait_path, trait_args) = match first_type.kind {
                TypeKind::Path(path) => (path, Vec::new()),
                TypeKind::PathWithArgs { path, args } => (path, args),
                _ => {
                    return Err(CompileError::ParseError(
                        ParseError::expected_trait_path_in_impl(),
                    ));
                }
            };
            (actual_target, Some(trait_path), trait_args)
        } else {
            // `impl Type` 表示固有 impl，不带 trait。
            (first_type, None, Vec::new())
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
            trait_args,
            items,
            span: self.current_span(),
        }))
    }
}
