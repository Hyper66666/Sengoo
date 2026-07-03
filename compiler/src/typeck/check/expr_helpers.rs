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
            SymbolKind::Function { ty, .. } => Ok(self.infer.instantiate_with_fresh_vars(ty)),
            _ => {
                if let Some(ty) = symbol.get_ty() {
                    Ok(self.infer.instantiate(ty))
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
        } else if let Some(result) = self.check_enum_variant_constructor(path, &[]) {
            result
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

    pub(super) fn check_enum_variant_constructor(
        &mut self,
        path: &Path,
        args: &[Expr],
    ) -> Option<TyResult<Ty>> {
        if path.segments.len() != 2 {
            return None;
        }
        let enum_name = &path.segments[0].name;
        let variant_name = &path.segments[1].name;
        let variants = self.enum_variants.get(enum_name)?.clone();
        let span_lo = path.segments[0].span.lo;
        let span_hi = path.segments[1].span.hi;

        if !variants.iter().any(|variant| variant == variant_name) {
            return Some(Err(TypeckError::diagnostic(
                "unknown-enum-variant",
                format!("enum `{enum_name}` has no variant `{variant_name}`"),
                span_lo,
                span_hi,
            )));
        }

        let raw_field_tys = self
            .enum_variant_field_tys
            .get(enum_name)
            .and_then(|variants| variants.get(variant_name))
            .cloned()
            .unwrap_or_default();
        let generic_meta = self.generic_type_metas.get(enum_name).cloned();
        let (field_tys, generic_param_placeholders) = if let Some(meta) = generic_meta.as_ref() {
            let mut template_parts = raw_field_tys.clone();
            for param in &meta.params {
                template_parts.push(Ty::new(0, TyKind::Var(param.var_id)));
            }
            let template = Ty::new(0, TyKind::Tuple(template_parts));
            let instantiated = self.infer.instantiate_with_fresh_vars(&template);
            let TyKind::Tuple(mut parts) = instantiated.kind else {
                return Some(Err(TypeckError::Other(
                    "internal error: generic enum instantiation expected tuple".to_string(),
                )));
            };
            let param_placeholders = parts.split_off(raw_field_tys.len());
            (parts, param_placeholders)
        } else {
            (raw_field_tys, Vec::new())
        };
        if field_tys.len() != args.len() {
            return Some(Err(TypeckError::diagnostic(
                "enum-variant-arity",
                format!(
                    "enum variant `{enum_name}::{variant_name}` expects {} argument(s), found {}",
                    field_tys.len(),
                    args.len()
                ),
                span_lo,
                span_hi,
            )));
        }

        for (index, (expected, arg)) in field_tys.iter().zip(args.iter()).enumerate() {
            let actual = match self.check_expr(arg) {
                Ok(ty) => ty,
                Err(error) => return Some(Err(error)),
            };
            if self.infer.unify(expected, &actual).is_err() {
                let (span_lo, span_hi) = expression_subject_span(arg);
                return Some(Err(TypeckError::diagnostic(
                    "enum-variant-type",
                    format!(
                        "argument {} for `{enum_name}::{variant_name}` has type {}, expected {}",
                        index + 1,
                        actual.kind,
                        expected.kind
                    ),
                    span_lo,
                    span_hi,
                )));
            }
        }

        if let Some(meta) = generic_meta.as_ref() {
            let mut enum_args = Vec::with_capacity(meta.params.len());
            let mut resolved_by_old_id = std::collections::HashMap::new();
            for (param, placeholder) in meta.params.iter().zip(generic_param_placeholders.iter()) {
                let mut concrete_ty = self.infer.apply_subst(placeholder);
                if matches!(concrete_ty.kind, TyKind::Var(_)) {
                    if let Some(default_ty) = &param.default {
                        concrete_ty = self.substitute_ty_vars(default_ty, &resolved_by_old_id);
                        concrete_ty = self.infer.apply_subst(&concrete_ty);
                    } else {
                        return Some(Err(TypeckError::Other(format!(
                            "cannot infer generic argument `{}` for enum `{}` variant `{}`",
                            param.name, enum_name, variant_name
                        ))));
                    }
                }
                for bound in &param.bounds {
                    let concrete_key = type_key(&concrete_ty);
                    if !self.impl_registry.implements_trait(bound, &concrete_key) {
                        return Some(Err(Self::unsatisfied_trait_bound_error(
                            format!("enum `{enum_name}` variant `{variant_name}`"),
                            &concrete_key,
                            bound,
                            &param.name,
                            span_lo,
                            span_hi,
                        )));
                    }
                }
                resolved_by_old_id.insert(param.var_id, concrete_ty.clone());
                enum_args.push(concrete_ty);
            }
            return Some(Ok(self.env.new_ty(TyKind::Adt {
                name: enum_name.clone(),
                args: enum_args,
            })));
        }

        let enum_ty = self
            .env
            .lookup(enum_name)
            .and_then(|symbol| symbol.get_ty())
            .cloned()
            .unwrap_or_else(|| {
                self.env.new_ty(TyKind::Adt {
                    name: enum_name.clone(),
                    args: Vec::new(),
                })
            });
        Some(Ok(enum_ty))
    }

    pub(super) fn check_binary(&mut self, op: &BinOp, left: &Expr, right: &Expr) -> TyResult<Ty> {
        let left_ty = self.check_expr(left)?;
        let right_ty = self.check_expr(right)?;

        if matches!(op, BinOp::Add)
            && self.is_owned_string_ty(&left_ty)
            && Self::is_borrowed_str_ty(&right_ty)
        {
            return Ok(left_ty);
        }

        if matches!(
            op,
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        ) && (Self::is_async_context_ty(&left_ty) || Self::is_async_context_ty(&right_ty))
        {
            return Err(TypeckError::Other(
                "AsyncContext is poll-scoped and cannot be compared".to_string(),
            ));
        }

        let is_owned_string_pair = matches!(
            (&left_ty.kind, &right_ty.kind),
            (
                TyKind::Adt {
                    name: left_name, ..
                },
                TyKind::Adt {
                    name: right_name, ..
                }
            ) if left_name == "String" && right_name == "String"
        );
        if is_owned_string_pair
            && !matches!(
                op,
                BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
            )
        {
            return Err(TypeckError::TypeMismatch {
                expected: right_ty.kind.clone(),
                found: left_ty.kind.clone(),
            });
        }

        let types_compatible = match (&left_ty.kind, &right_ty.kind) {
            (
                TyKind::Adt {
                    name: left_name, ..
                },
                TyKind::Adt {
                    name: right_name, ..
                },
            ) if left_name == "String" || right_name == "String" => {
                left_name == right_name
                    && matches!(
                        op,
                        BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
                    )
            }
            _ if left_ty.kind == right_ty.kind => true,
            (TyKind::Adt { name, .. }, TyKind::Str)
                if name == "String" && matches!(op, BinOp::Add) =>
            {
                true
            }
            (TyKind::Adt { name, .. }, TyKind::Ref(false, inner))
                if name == "String"
                    && matches!(inner.kind, TyKind::Str)
                    && matches!(op, BinOp::Add) =>
            {
                true
            }
            (TyKind::Ref(false, inner), TyKind::Adt { name, .. })
                if matches!(inner.kind, TyKind::Str)
                    && name == "String"
                    && matches!(op, BinOp::Add) =>
            {
                true
            }
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
            (TyKind::Ref(false, inner), TyKind::Adt { name, .. })
                if matches!(inner.kind, TyKind::Str)
                    && name == "String"
                    && matches!(op, BinOp::Add) =>
            {
                right_ty.clone()
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

    fn is_borrowed_str_ty(ty: &Ty) -> bool {
        matches!(&ty.kind, TyKind::Ref(false, inner) if matches!(inner.kind, TyKind::Str))
    }

    fn is_owned_string_ty(&self, ty: &Ty) -> bool {
        self.env
            .owned_string_ty
            .as_ref()
            .is_some_and(|canonical| canonical.kind == ty.kind)
            || matches!(&ty.kind, TyKind::Adt { name, args } if name == "String" && args.is_empty())
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
        self.ensure_assignable_target(target)?;
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    pub(super) fn check_assign_op(
        &mut self,
        op: &AssignOp,
        target: &Expr,
        value: &Expr,
    ) -> TyResult<Ty> {
        self.ensure_assignable_target(target)?;
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        if matches!(op, AssignOp::AddAssign)
            && matches!(&target_ty.kind, TyKind::Adt { name, .. } if name == "String")
            && matches!(&value_ty.kind, TyKind::Ref(false, inner) if matches!(inner.kind, TyKind::Str))
        {
            return Ok(self.env.unit_ty());
        }
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    fn ensure_assignable_target(&self, target: &Expr) -> TyResult<()> {
        let binding = match &target.kind {
            ExprKind::Ident(ident) => Some((ident.name.as_str(), ident.span)),
            ExprKind::Path(path) => path
                .as_simple()
                .map(|ident| (ident.name.as_str(), ident.span)),
            ExprKind::Index { base, .. } | ExprKind::Field { base, .. } => {
                return self.ensure_assignable_target(base);
            }
            _ => None,
        };

        let Some((name, span)) = binding else {
            return Ok(());
        };
        let Some(symbol) = self.env.lookup(name) else {
            return Ok(());
        };
        let SymbolKind::Var { is_mut, .. } = &symbol.kind else {
            return Ok(());
        };
        if *is_mut {
            return Ok(());
        }

        Err(TypeckError::diagnostic(
            "immutable-assignment",
            format!("cannot assign to immutable binding `{name}`; declare it with `let mut`"),
            span.lo,
            span.hi,
        ))
    }

    pub(super) fn check_index(&mut self, base: &Expr, index: &Expr) -> TyResult<Ty> {
        let base_ty = self.check_expr(base)?;

        if let ExprKind::Range {
            start: Some(start),
            end: Some(end),
            inclusive: false,
        } = &index.kind
        {
            let start_ty = self.check_expr(start)?;
            let end_ty = self.check_expr(end)?;
            if !start_ty.is_int() || !end_ty.is_int() {
                let (span_lo, span_hi) = expression_subject_span(index);
                return Err(TypeckError::diagnostic(
                    "invalid-string-slice-index",
                    "string slice bounds must be integers".to_string(),
                    span_lo,
                    span_hi,
                ));
            }

            let is_string_slice_base = matches!(&base_ty.kind, TyKind::Adt { name, .. } if name == "String")
                || matches!(&base_ty.kind, TyKind::Ref(false, inner) if matches!(inner.kind, TyKind::Str))
                || matches!(&base_ty.kind, TyKind::Str);
            if is_string_slice_base {
                return Ok(self.env.new_ty(TyKind::Adt {
                    name: "String".to_string(),
                    args: Vec::new(),
                }));
            }
        }

        let index_ty = self.check_expr(index)?;

        if !index_ty.is_int() {
            let (span_lo, span_hi) = expression_subject_span(index);
            return Err(TypeckError::diagnostic(
                "invalid-array-index",
                format!("array index must be an integer, found {}", index_ty.kind),
                span_lo,
                span_hi,
            ));
        }

        if let (TyKind::Array(_, len), ExprKind::Literal(Literal::Int(value))) =
            (&base_ty.kind, &index.kind)
        {
            if *value < 0 || (*value as usize) >= *len {
                let (span_lo, span_hi) = expression_subject_span(index);
                return Err(TypeckError::diagnostic(
                    "array-index-out-of-bounds",
                    format!("array index {value} is out of bounds for length {len}"),
                    span_lo,
                    span_hi,
                ));
            }
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
            TyKind::Tuple(types) => tuple_field_index(&name.name)
                .and_then(|index| types.get(index).cloned())
                .ok_or_else(|| TypeckError::FieldNotFound {
                    type_name: base_ty.kind.to_string(),
                    field_name: name.name.clone(),
                }),
            TyKind::Adt {
                name: type_name,
                args,
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
        if elem_types
            .iter()
            .any(|ty| self.contains_future_escape_ty(ty))
        {
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
        let mut seen = std::collections::HashSet::new();
        if let Some(duplicate) = params
            .iter()
            .find(|param| !seen.insert(param.name.as_str()))
        {
            return Err(TypeckError::diagnostic(
                "duplicate-closure-parameter",
                format!(
                    "closure parameter `{}` is declared more than once",
                    duplicate.name
                ),
                duplicate.span.lo,
                duplicate.span.hi,
            ));
        }

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

fn tuple_field_index(field: &str) -> Option<usize> {
    field.parse::<usize>().ok().or(match field {
        "x" | "left" | "r" => Some(0),
        "y" | "right" | "g" => Some(1),
        "z" | "b" => Some(2),
        "w" | "a" => Some(3),
        _ => None,
    })
}

fn expression_subject_span(expr: &Expr) -> (u32, u32) {
    match &expr.kind {
        ExprKind::Ident(ident) => (ident.span.lo, ident.span.hi),
        ExprKind::Path(path) if !path.segments.is_empty() => (
            path.segments[0].span.lo,
            path.segments
                .last()
                .map_or(expr.span.hi, |ident| ident.span.hi),
        ),
        ExprKind::Literal(Literal::Bool(value)) => {
            let len = if *value { 4 } else { 5 };
            (expr.span.lo, expr.span.lo + len)
        }
        ExprKind::Literal(Literal::Int(value)) => {
            let len = value.to_string().len() as u32;
            (expr.span.lo, expr.span.lo + len)
        }
        _ => (expr.span.lo, expr.span.hi),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Span;

    #[test]
    fn check_ident_reports_undefined_variable() {
        let mut checker = TypeChecker::new();
        let err = checker
            .check_ident(&Ident::new("missing", Span::new(0, 0)))
            .unwrap_err();
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
        assert!(
            matches!(err, TypeckError::Other(msg) if msg.contains("future values cannot escape"))
        );
    }
}
