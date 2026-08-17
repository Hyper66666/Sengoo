use super::*;
use crate::ast::{AssignOp, BinOp, TypeKind};
use crate::lexer::Span;

impl TypeChecker {
    #[inline(never)]
    pub(super) fn check_vec_bang(
        &mut self,
        elements: &[Expr],
        count: Option<&Expr>,
        span: Span,
    ) -> TyResult<Ty> {
        let elem_ty = if let Some(count) = count {
            let Some(value) = elements.first() else {
                return Err(TypeckError::diagnostic(
                    "invalid-vec-macro",
                    "`vec![value; count]` requires a repeated value",
                    span.lo,
                    span.hi,
                ));
            };
            let value_ty = self.check_expr(value)?;
            let count_ty = self.check_expr(count)?;
            self.infer
                .unify(&count_ty, &self.env.int_ty(IntKind::I64))?;
            value_ty
        } else if elements.is_empty() {
            self.expected_vec_elem_ty().ok_or_else(|| {
                TypeckError::diagnostic(
                    "invalid-vec-macro",
                    "cannot infer element type for `vec![]`; annotate the binding or add elements",
                    span.lo,
                    span.hi,
                )
            })?
        } else {
            let mut elem_ty = self.check_expr(&elements[0])?;
            for element in &elements[1..] {
                let next_ty = self.check_expr(element)?;
                self.infer.unify(&elem_ty, &next_ty)?;
                elem_ty = self.infer.apply_subst(&elem_ty);
            }
            elem_ty
        };
        let elem_ty = self.infer.apply_subst(&elem_ty);
        let vec_ty = self.env.new_ty(TyKind::Adt {
            name: "Vec".to_string(),
            args: vec![elem_ty.clone()],
        });
        let elem_ast = ty_to_ast_type(&elem_ty, span).ok_or_else(|| {
            TypeckError::diagnostic(
                "invalid-vec-macro",
                "cannot infer element type for `vec!`",
                span.lo,
                span.hi,
            )
        })?;
        let desugared = if let Some(count) = count {
            desugar_vec_repeat(elements[0].clone(), count.clone(), span, elem_ast)
        } else {
            desugar_vec_elements(elements.to_vec(), span, elem_ast)
        };
        self.env.record_desugared_for(span, desugared.clone());
        self.check_expr(&desugared)?;
        Ok(vec_ty)
    }

    fn expected_vec_elem_ty(&self) -> Option<Ty> {
        let expected = self.expected_return_types.last()?;
        let expected = self.infer.apply_subst(expected);
        match &expected.kind {
            TyKind::Adt { name, args } if name == "Vec" && args.len() == 1 => Some(args[0].clone()),
            _ => None,
        }
    }
}

fn ty_to_ast_type(ty: &Ty, span: Span) -> Option<Type> {
    Some(match &ty.kind {
        TyKind::Int(kind) => Type::simple(kind.to_string(), span),
        TyKind::Float(kind) => Type::simple(kind.to_string(), span),
        TyKind::Bool => Type::simple("bool", span),
        TyKind::Str => Type::simple("str", span),
        TyKind::Char => Type::simple("char", span),
        TyKind::Byte => Type::simple("u8", span),
        TyKind::Unit => Type::unit(span),
        TyKind::Adt { name, args } if args.is_empty() => Type::simple(name.clone(), span),
        TyKind::Adt { name, args } => Type::new(
            TypeKind::PathWithArgs {
                path: Path::from_str(name.clone(), span),
                args: args
                    .iter()
                    .map(|arg| ty_to_ast_type(arg, span))
                    .collect::<Option<Vec<_>>>()?,
            },
            span,
        ),
        TyKind::Ref(is_mut, inner) => Type::ref_(ty_to_ast_type(inner, span)?, *is_mut, span),
        TyKind::Tuple(items) => Type::tuple(
            items
                .iter()
                .map(|item| ty_to_ast_type(item, span))
                .collect::<Option<Vec<_>>>()?,
            span,
        ),
        _ => return None,
    })
}

fn vec_new_call(span: Span) -> Expr {
    Expr::call(
        Expr::path(Path::from_str("vec_new", span)),
        Vec::new(),
        span,
    )
}

fn vec_annotation(elem_ty: Type, span: Span) -> Type {
    Type::new(
        TypeKind::PathWithArgs {
            path: Path::from_str("Vec", span),
            args: vec![elem_ty],
        },
        span,
    )
}

fn desugar_vec_elements(elements: Vec<Expr>, span: Span, elem_ty: Type) -> Expr {
    let vec_name = Ident::new("__sg_vec", span);
    let mut stmts = vec![Stmt::new(
        StmtKind::Let {
            name: vec_name.clone(),
            ty: Some(vec_annotation(elem_ty, span)),
            value: Some(Box::new(vec_new_call(span))),
            is_mut: true,
        },
        span,
    )];
    for element in elements {
        stmts.push(Stmt::expr(Expr::method_call(
            Expr::ident("__sg_vec", span),
            Ident::new("push", span),
            vec![element],
            span,
        )));
    }
    stmts.push(Stmt::expr(Expr::ident("__sg_vec", span)));
    Expr::block(Block::new(stmts, span))
}

fn desugar_vec_repeat(value: Expr, count: Expr, span: Span, elem_ty: Type) -> Expr {
    let push = Stmt::expr(Expr::method_call(
        Expr::ident("__sg_vec", span),
        Ident::new("push", span),
        vec![value],
        span,
    ));
    let bump = Stmt::expr(Expr::new(
        ExprKind::AssignOp {
            op: AssignOp::AddAssign,
            target: Box::new(Expr::ident("__sg_i", span)),
            value: Box::new(Expr::literal(Literal::Int(1), span)),
        },
        span,
    ));
    let while_expr = Expr::new(
        ExprKind::While {
            cond: Box::new(Expr::binary(
                BinOp::Lt,
                Expr::ident("__sg_i", span),
                Expr::ident("__sg_n", span),
                span,
            )),
            body: Block::new(vec![push, bump], span),
        },
        span,
    );
    Expr::block(Block::new(
        vec![
            Stmt::new(
                StmtKind::Let {
                    name: Ident::new("__sg_vec", span),
                    ty: Some(vec_annotation(elem_ty, span)),
                    value: Some(Box::new(vec_new_call(span))),
                    is_mut: true,
                },
                span,
            ),
            Stmt::new(
                StmtKind::Let {
                    name: Ident::new("__sg_i", span),
                    ty: None,
                    value: Some(Box::new(Expr::literal(Literal::Int(0), span))),
                    is_mut: true,
                },
                span,
            ),
            Stmt::new(
                StmtKind::Let {
                    name: Ident::new("__sg_n", span),
                    ty: None,
                    value: Some(Box::new(count)),
                    is_mut: false,
                },
                span,
            ),
            Stmt::expr(while_expr),
            Stmt::expr(Expr::ident("__sg_vec", span)),
        ],
        span,
    ))
}
