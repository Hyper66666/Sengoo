//! 声明解析

use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::{Keyword, TokenKind};
use crate::Result;

use super::Parser;

impl<'source> Parser<'source> {
    /// 解析声明
    pub(super) fn parse_decl(&mut self) -> Result<Decl> {
        let lo = self.current_span().lo;
        let vis = self.parse_visibility()?;

        let kind = match self.current().map(|t| &t.kind) {
            // 函数 (Python 风格: def)
            Some(TokenKind::DefKw) => {
                self.advance();
                self.parse_function_decl(vis)?
            }

            // 结构体
            Some(TokenKind::StructKw) => {
                self.advance();
                self.parse_struct_decl(vis)?
            }

            // 枚举
            Some(TokenKind::EnumKw) => {
                self.advance();
                self.parse_enum_decl(vis)?
            }

            // 类
            Some(TokenKind::ClassKw) => {
                self.advance();
                self.parse_class_decl(vis)?
            }

            // trait
            Some(TokenKind::TraitKw) => {
                self.advance();
                self.parse_trait_decl(vis)?
            }

            // impl 块
            Some(TokenKind::ImplKw) => {
                self.advance();
                self.parse_impl_decl(vis)?
            }

            // 类型别名
            Some(TokenKind::TypeKw) => {
                self.advance();
                self.parse_type_alias_decl(vis)?
            }

            // 常量
            Some(TokenKind::ConstKw) => {
                self.advance();
                self.parse_const_decl(vis)?
            }

            // 静态变量
            Some(TokenKind::StaticKw) => {
                self.advance();
                self.parse_static_decl(vis)?
            }

            // 导入
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

    /// 解析可见性
    fn parse_visibility(&mut self) -> Result<Visibility> {
        if self.consume(TokenKind::PubKw).is_some() {
            Ok(Visibility::Public)
        } else {
            self.consume(TokenKind::PrivKw);
            Ok(Visibility::Private)
        }
    }

    /// 解析函数声明
    fn parse_function_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 类型参数
        let type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LParen)?;

        let mut params = Vec::new();
        let mut self_param = None;

        // 检查 self 参数
        if self.check_self_param() {
            self_param = Some(self.parse_self_param()?);
            if self.consume(TokenKind::Comma).is_some() {
                // 继续解析其他参数
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

    /// 检查是否是 self 参数
    fn check_self_param(&self) -> bool {
        if let Some(token) = self.current() {
            match &token.kind {
                TokenKind::SelfLowerKw | TokenKind::MutKw => {
                    // 检查后面是否是 self
                    if let TokenKind::MutKw = &token.kind {
                        self.check_peek(TokenKind::SelfLowerKw)
                    } else {
                        true
                    }
                }
                TokenKind::And => {
                    // &self 或 &mut self
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

    /// 解析 self 参数
    fn parse_self_param(&mut self) -> Result<SelfParam> {
        if self.consume(TokenKind::And).is_some() {
            // &self 或 &mut self
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

    /// 解析参数
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

    /// 解析类型参数
    fn parse_type_params(&mut self) -> Result<Vec<TypeParam>> {
        let mut params = Vec::new();

        while !self.is_eof() {
            if self.check(TokenKind::Gt) || self.check(TokenKind::Shr) {
                break;
            }

            let name = self.expect_ident()?;
            let mut param = TypeParam::new(name);

            // TODO: 解析约束 `: Trait`

            params.push(param);

            self.consume(TokenKind::Comma);
        }

        Ok(params)
    }

    /// 解析结构体声明
    fn parse_struct_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 类型参数
        let type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        let mut fields = Vec::new();

        if self.consume(TokenKind::LBrace).is_some() {
            // 命名字段结构体
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
            // 元组结构体
            let mut index = 0;
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

                index += 1;
                self.consume(TokenKind::Comma);
            }

            self.consume(TokenKind::Semicolon);
        } else {
            // 单元结构体
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

    /// 解析枚举声明
    fn parse_enum_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 类型参数
        let type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;

        let mut variants = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            let name = self.expect_ident()?;

            let (fields, discriminant) = if self.consume(TokenKind::LBrace).is_some() {
                // 结构体变体
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
                // 元组变体
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
                // 带判别值的变体
                let value = self.parse_expr()?;
                self.consume(TokenKind::Comma);
                (Vec::new(), Some(Box::new(value)))
            } else {
                // 单元变体
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

    /// 解析类声明
    fn parse_class_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 类型参数
        let type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // 继承
        let extends = if self.consume(TokenKind::Colon).is_some() {
            Some(self.parse_path()?)
        } else {
            None
        };

        // 实现的 trait
        let mut implements = Vec::new();
        if self.consume(TokenKind::Colon).is_some() {
            loop {
                if !matches!(self.current().map(|t| &t.kind), Some(TokenKind::LBrace)) {
                    let path = self.parse_path()?;
                    implements.push(crate::ast::TraitBound::new(path));
                } else {
                    break;
                }
                self.consume(TokenKind::Comma);
            }
        }

        self.expect(TokenKind::LBrace)?;

        let mut members = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // 可以是字段或方法
            let member_vis = self.parse_visibility()?;

            if self.consume(TokenKind::DefKw).is_some() {
                // 方法 (Python 风格: def)
                let name = self.expect_ident()?;
                let type_params = Vec::new();

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
                // 字段
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

    /// 解析 trait 声明
    fn parse_trait_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 类型参数
        let type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        // 约束
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

        self.expect(TokenKind::LBrace)?;

        let mut items = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // 函数或常量 (Python 风格: def)
            if self.consume(TokenKind::DefKw).is_some() {
                let name = self.expect_ident()?;
                let type_params = Vec::new();

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

    /// 解析 impl 块
    fn parse_impl_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        // 类型参数
        let type_params = if self.consume(TokenKind::Lt).is_some() {
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
            // `impl Trait for Type` — first_type is the trait, parse the real target type
            let actual_target = self.parse_type()?;
            // Extract the path from the first type (the trait)
            let trait_path = match first_type.kind {
                TypeKind::Path(path) => path,
                _ => {
                    return Err(CompileError::ParseError(
                        ParseError::expected_trait_path_in_impl(),
                    ));
                }
            };
            (actual_target, Some(trait_path))
        } else {
            // `impl Type` — inherent impl, no trait
            (first_type, None)
        };

        self.expect(TokenKind::LBrace)?;

        let mut items = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            self.consume(TokenKind::PubKw);
            self.expect(TokenKind::DefKw)?;

            let name = self.expect_ident()?;
            let type_params = Vec::new();

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

    /// 解析类型别名
    fn parse_type_alias_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        // 类型参数
        let type_params = if self.consume(TokenKind::Lt).is_some() {
            let params = self.parse_type_params()?;
            self.expect(TokenKind::Gt)?;
            params
        } else {
            Vec::new()
        };

        self.expect(TokenKind::Eq)?;
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

    /// 解析常量声明
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

    /// 解析静态变量声明
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

    /// 解析导入声明
    fn parse_import_decl(&mut self) -> Result<DeclKind> {
        let path = self.parse_path()?;

        let (kind, alias) = if self.consume(TokenKind::AsKw).is_some() {
            (ImportKind::Simple, Some(self.expect_ident()?))
        } else if self.consume(TokenKind::LBrace).is_some() {
            // 选择性导入
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
            // 通配符导入
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

    /// 解析模块声明
    fn parse_module_decl(&mut self, vis: Visibility) -> Result<DeclKind> {
        let name = self.expect_ident()?;

        let items = if self.consume(TokenKind::LBrace).is_some() {
            let mut items = Vec::new();
            while !self.is_eof() {
                if self.consume(TokenKind::RBrace).is_some() {
                    break;
                }
                items.push(self.parse_decl()?);
            }
            items
        } else {
            self.expect(TokenKind::Semicolon)?;
            Vec::new()
        };

        Ok(DeclKind::Module(Module {
            vis,
            name,
            items,
            span: self.current_span(),
        }))
    }
}
