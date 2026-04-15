use super::*;

impl TypeChecker {
    pub(super) fn check_literal(&mut self, lit: &Literal) -> TyResult<Ty> {
        Ok(match lit {
            Literal::Int(_) => self.env.int_ty(IntKind::I64),
            Literal::Float(_) => self.env.float_ty(FloatKind::F64),
            Literal::String(_) => {
                let str_ty = self.env.str_ty();
                self.env.ref_ty(false, str_ty)
            }
            Literal::Char(_) => self.env.new_ty(TyKind::Char),
            Literal::Bytes(_) => self.env.new_ty(TyKind::Bytes),
            Literal::Bool(_) => self.env.bool_ty(),
            Literal::Null => self.env.new_ty(TyKind::Adt {
                name: "Option".to_string(),
                args: vec![],
            }),
            Literal::Unit => self.env.unit_ty(),
        })
    }

    pub(super) fn check_ident(&mut self, ident: &Ident) -> TyResult<Ty> {
        let symbol = if let Some(symbol) = self.env.lookup(&ident.name) {
            symbol.clone()
        } else {
            return Err(TypeckError::UndefinedVariable {
                name: ident.name.clone(),
            });
        };

        match &symbol.kind {
            SymbolKind::Function { ty, .. } => Ok(self.infer.instantiate_with_fresh_vars(ty.clone())),
            _ => {
                if let Some(ty) = symbol.get_ty() {
                    Ok(self.infer.instantiate(ty.clone()))
                } else {
                    Err(TypeckError::UndefinedVariable {
                        name: ident.name.clone(),
                    })
                }
            }
        }
    }

    pub(super) fn check_path(&mut self, path: &Path) -> TyResult<Ty> {
        if let Some(ident) = path.as_simple() {
            self.check_ident(ident)
        } else {
            Err(TypeckError::UndefinedVariable {
                name: path
                    .segments
                    .iter()
                    .map(|seg| seg.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            })
        }
    }

    pub(super) fn check_binary(&mut self, op: &BinOp, left: &Expr, right: &Expr) -> TyResult<Ty> {
        let left_ty = self.check_expr(left)?;
        let right_ty = self.check_expr(right)?;

        let types_compatible = match (&left_ty.kind, &right_ty.kind) {
            _ if left_ty.kind == right_ty.kind => true,
            (TyKind::Int(a), TyKind::Int(b)) if a != b && a.is_signed() && b.is_signed() => {
                matches!(
                    op,
                    BinOp::Add
                        | BinOp::Sub
                        | BinOp::Mul
                        | BinOp::Div
                        | BinOp::Mod
                        | BinOp::BitAnd
                        | BinOp::BitOr
                        | BinOp::BitXor
                        | BinOp::Shl
                        | BinOp::Shr
                        | BinOp::Eq
                        | BinOp::NotEq
                        | BinOp::Lt
                        | BinOp::Le
                        | BinOp::Gt
                        | BinOp::Ge
                )
            }
            (TyKind::Float(_), TyKind::Float(_)) => matches!(
                op,
                BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod
                    | BinOp::Eq
                    | BinOp::NotEq
                    | BinOp::Lt
                    | BinOp::Le
                    | BinOp::Gt
                    | BinOp::Ge
            ),
            _ => false,
        };

        if !types_compatible {
            self.infer
                .unify(&left_ty, &right_ty)
                .map_err(|_| TypeckError::TypeMismatch {
                    expected: right_ty.kind.clone(),
                    found: left_ty.kind.clone(),
                })?;
        }

        let result_ty = match (&left_ty.kind, &right_ty.kind) {
            (TyKind::Int(a), TyKind::Int(b)) if a != b && a.is_signed() && b.is_signed() => {
                let wider = if a.bits() >= b.bits() { *a } else { *b };
                self.env.int_ty(wider)
            }
            (TyKind::Float(a), TyKind::Float(b)) if a != b => {
                use crate::typeck::ty::FloatKind;
                let wider = match (a, b) {
                    (FloatKind::F64, _) | (_, FloatKind::F64) => FloatKind::F64,
                    _ => FloatKind::F32,
                };
                self.env.float_ty(wider)
            }
            _ => left_ty.clone(),
        };

        Ok(match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => result_ty,
            BinOp::And | BinOp::Or => self.env.bool_ty(),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => result_ty,
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.env.bool_ty()
            }
            BinOp::Pipe | BinOp::Compose | BinOp::Range | BinOp::RangeInclusive => result_ty,
        })
    }

    pub(super) fn check_unary(&mut self, op: &UnOp, operand: &Expr) -> TyResult<Ty> {
        let ty = self.check_expr(operand)?;
        Ok(match op {
            UnOp::Neg | UnOp::Not | UnOp::Plus | UnOp::BitNot => ty.clone(),
            UnOp::Deref => {
                if let Some(inner) = ty.ref_inner() {
                    inner.clone()
                } else {
                    return Err(TypeckError::TypeMismatch {
                        expected: TyKind::Ref(false, Box::new(self.env.error_ty())),
                        found: ty.kind.clone(),
                    });
                }
            }
            UnOp::Ref => self.env.ref_ty(false, ty),
            UnOp::RefMut | UnOp::DerefMut => self.env.ref_ty(true, ty),
        })
    }

    pub(super) fn check_assign(&mut self, target: &Expr, value: &Expr) -> TyResult<Ty> {
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    pub(super) fn check_assign_op(&mut self, _op: &AssignOp, target: &Expr, value: &Expr) -> TyResult<Ty> {
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    pub(super) fn check_index(&mut self, base: &Expr, index: &Expr) -> TyResult<Ty> {
        let base_ty = self.check_expr(base)?;
        let index_ty = self.check_expr(index)?;

        if !index_ty.is_int() {
            return Err(TypeckError::TypeMismatch {
                expected: TyKind::Int(IntKind::ISize),
                found: index_ty.kind.clone(),
            });
        }

        Ok(match &base_ty.kind {
            TyKind::Array(elem, _) => (**elem).clone(),
            TyKind::Slice(elem) => (**elem).clone(),
            TyKind::Tuple(types) if !types.is_empty() => types[0].clone(),
            _ => self.env.error_ty(),
        })
    }

    pub(super) fn check_field(&mut self, base: &Expr, name: &Ident) -> TyResult<Ty> {
        let base_ty = self.check_expr(base)?;

        match &base_ty.kind {
            TyKind::Adt {
                name: type_name, args
            } => {
                let field_defs =
                    self.struct_field_defs
                        .get(type_name)
                        .cloned()
                        .ok_or_else(|| TypeckError::FieldNotFound {
                            type_name: type_name.clone(),
                            field_name: name.name.clone(),
                        })?;

                let field_ty = field_defs
                    .into_iter()
                    .find(|(field_name, _)| field_name == &name.name)
                    .map(|(_, field_ty)| field_ty)
                    .ok_or_else(|| TypeckError::FieldNotFound {
                        type_name: type_name.clone(),
                        field_name: name.name.clone(),
                    })?;

                if let Some(type_params) = self.struct_type_params.get(type_name).cloned() {
                    if !type_params.is_empty() && type_params.len() == args.len() {
                        self.env.push_scope();
                        for (type_param, concrete_ty) in type_params.iter().zip(args.iter()) {
                            self.env
                                .insert_type(type_param.name.name.clone(), concrete_ty.clone());
                        }
                        let resolved = self.check_type(&field_ty);
                        self.env.pop_scope();
                        resolved
                    } else {
                        self.check_type(&field_ty)
                    }
                } else {
                    self.check_type(&field_ty)
                }
            }
            _ => Err(TypeckError::FieldNotFound {
                type_name: base_ty.kind.to_string(),
                field_name: name.name.clone(),
            }),
        }
    }

    pub(super) fn check_tuple(&mut self, elems: &[Expr]) -> TyResult<Ty> {
        let elem_types = elems
            .iter()
            .map(|e| self.check_expr(e))
            .collect::<TyResult<Vec<_>>>()?;
        if elem_types.iter().any(|ty| self.contains_future_escape_ty(ty)) {
            return Err(Self::future_escape_error());
        }
        Ok(self.env.tuple_ty(elem_types))
    }

    pub(super) fn check_array(&mut self, elems: &[Expr]) -> TyResult<Ty> {
        if elems.is_empty() {
            return Ok(self.env.array_ty(self.infer.fresh_ty_var(), 0));
        }

        let first_ty = self.check_expr(&elems[0])?;
        if self.contains_future_escape_ty(&first_ty) {
            return Err(Self::future_escape_error());
        }
        for elem in &elems[1..] {
            let ty = self.check_expr(elem)?;
            if self.contains_future_escape_ty(&ty) {
                return Err(Self::future_escape_error());
            }
            self.infer.unify(&first_ty, &ty)?;
        }

        Ok(self.env.array_ty(first_ty, elems.len()))
    }

    pub(super) fn check_lambda(&mut self, params: &[Ident], body: &Expr) -> TyResult<Ty> {
        let param_tys: Vec<Ty> = params.iter().map(|_| self.infer.fresh_ty_var()).collect();

        self.env.push_scope();
        for (param, ty) in params.iter().zip(param_tys.iter()) {
            self.env.insert_var(param.name.clone(), ty.clone());
        }
        let body_ty = self.check_expr(body)?;
        self.env.pop_scope();

        Ok(self.env.fn_ty(param_tys, body_ty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Span;

    #[test]
    fn check_ident_reports_undefined_variable() {
        let mut checker = TypeChecker::new();
        let err = checker.check_ident(&Ident::new("missing", Span::new(0, 0))).unwrap_err();
        assert!(matches!(err, TypeckError::UndefinedVariable { name } if name == "missing"));
    }

    #[test]
    fn check_binary_prefers_wider_signed_int_type() {
        let mut checker = TypeChecker::new();
        let lhs_ty = checker.env.int_ty(IntKind::I32);
        let rhs_ty = checker.env.int_ty(IntKind::I64);
        checker.env.insert_var("a".to_string(), lhs_ty);
        checker.env.insert_var("b".to_string(), rhs_ty);

        let expr = Expr::binary(
            BinOp::Add,
            Expr::ident("a", Span::new(0, 0)),
            Expr::ident("b", Span::new(0, 0)),
            Span::new(0, 0),
        );

        let ty = checker.check_expr(&expr).unwrap();
        assert!(matches!(ty.kind, TyKind::Int(IntKind::I64)));
    }

    #[test]
    fn check_array_rejects_future_escape_values() {
        let mut checker = TypeChecker::new();
        let inner_ty = checker.env.int_ty(IntKind::I64);
        let future_ty = checker.env.new_ty(TyKind::Future(Box::new(inner_ty)));
        checker.env.insert_var("f".to_string(), future_ty);

        let expr = Expr::array(vec![Expr::ident("f", Span::new(0, 0))], Span::new(0, 0));
        let err = checker.check_expr(&expr).unwrap_err();
        assert!(matches!(err, TypeckError::Other(msg) if msg.contains("future values cannot escape")));
    }
}