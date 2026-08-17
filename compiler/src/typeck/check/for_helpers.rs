use super::*;
use crate::ast::pattern::{Pattern, PatternKind};
use crate::lexer::Span;

#[derive(Clone, Copy)]
enum ForAdapter {
    Direct,
    Method(&'static str),
}

impl TypeChecker {
    pub(super) fn check_for(
        &mut self,
        pattern: &Pattern,
        iter: &Expr,
        body: &Block,
        for_span: Span,
    ) -> TyResult<Ty> {
        let elem_ty = match &iter.kind {
            ExprKind::Range { start, end, .. } => {
                let range_ty = self.env.int_ty(IntKind::I64);
                if let Some(start) = start.as_deref() {
                    let start_ty = self.check_expr(start)?;
                    self.infer.unify(&start_ty, &range_ty)?;
                }
                if let Some(end) = end.as_deref() {
                    let end_ty = self.check_expr(end)?;
                    self.infer.unify(&end_ty, &range_ty)?;
                }
                range_ty
            }
            _ => {
                let iter_ty = self.check_expr(iter)?;
                match &iter_ty.kind {
                    TyKind::Array(elem, _) | TyKind::Slice(elem) => (**elem).clone(),
                    _ => {
                        let adapter = self.for_loop_adapter(&iter_ty)?;
                        let desugared = desugar_iterator_for(
                            pattern,
                            iter,
                            body,
                            adapter,
                            for_span,
                            self.env.fresh_synthetic_span(),
                            self.env.fresh_synthetic_span(),
                        );
                        self.env.record_desugared_for(for_span, desugared.clone());
                        self.check_expr(&desugared)?;
                        return Ok(self.env.unit_ty());
                    }
                }
            }
        };

        self.env.push_scope();

        let var_name = match &pattern.kind {
            PatternKind::Ident(name) => name.name.clone(),
            PatternKind::Wildcard => "_loop".to_string(),
            _ => "_loop".to_string(),
        };

        self.env.insert_var(var_name, elem_ty);
        self.check_block(body)?;
        self.env.pop_scope();

        Ok(self.env.unit_ty())
    }

    fn for_loop_adapter(&mut self, iter_ty: &Ty) -> TyResult<ForAdapter> {
        let iter_ty = self.infer.apply_subst(iter_ty);
        let iter_ty = match &iter_ty.kind {
            TyKind::Ref(_, inner) => inner.as_ref().clone(),
            _ => iter_ty,
        };
        if self.iterator_item_ty(&iter_ty).is_some() {
            return Ok(ForAdapter::Direct);
        }
        if let TyKind::Adt { name, .. } = &iter_ty.kind {
            if matches!(name.as_str(), "HashMap" | "BTreeMap")
                && self.has_zero_arg_method(&iter_ty, "entries")
            {
                return Ok(ForAdapter::Method("entries"));
            }
        }
        if self.has_zero_arg_method(&iter_ty, "iter") {
            return Ok(ForAdapter::Method("iter"));
        }
        Err(TypeckError::Other(
            "for loop expects an array, slice, range, collection, or iterator".to_string(),
        ))
    }

    fn iterator_item_ty(&mut self, ty: &Ty) -> Option<Ty> {
        let ty = self.infer.apply_subst(ty);
        let ty = match &ty.kind {
            TyKind::Ref(_, inner) => inner.as_ref().clone(),
            _ => ty,
        };
        let keys = [type_key(&ty), self.generic_lookup_key(&ty)];
        for key in keys {
            let impls = self
                .impl_registry
                .get_trait_impls("Iterator", &key)
                .to_vec();
            for impl_info in impls {
                let mut subst = HashMap::new();
                if !self.match_generic_impl_target(&impl_info.target_type, &ty, &mut subst) {
                    continue;
                }
                if let Some(item) = impl_info.assoc_types.get("Item") {
                    return Some(self.substitute_ty_vars(item, &subst));
                }
            }
        }
        let next_ty = self
            .impl_registry
            .lookup_inherent_method(&type_key(&ty), "next")
            .cloned()
            .map(|fn_ty| self.instantiate_method_function_ty(&fn_ty, &HashMap::new()))
            .or_else(|| self.lookup_generic_inherent_method(&ty, "next"));
        next_ty
            .filter(|fn_ty| fn_ty.param_types.is_empty())
            .and_then(|fn_ty| option_payload_ty(&fn_ty.return_type))
    }

    fn has_zero_arg_method(&mut self, ty: &Ty, name: &str) -> bool {
        let fn_ty = self
            .impl_registry
            .lookup_inherent_method(&type_key(ty), name)
            .cloned()
            .map(|fn_ty| self.instantiate_method_function_ty(&fn_ty, &HashMap::new()))
            .or_else(|| self.lookup_generic_inherent_method(ty, name));
        fn_ty.is_some_and(|fn_ty| fn_ty.param_types.is_empty())
    }
}

fn option_payload_ty(ty: &Ty) -> Option<Ty> {
    match &ty.kind {
        TyKind::Adt { name, args } if name == "Option" && args.len() == 1 => Some(args[0].clone()),
        _ => None,
    }
}

fn desugar_iterator_for(
    pattern: &Pattern,
    iter: &Expr,
    body: &Block,
    adapter: ForAdapter,
    for_span: Span,
    adapter_span: Span,
    next_span: Span,
) -> Expr {
    let source = match adapter {
        ForAdapter::Direct => iter.clone(),
        ForAdapter::Method(method) => Expr::method_call(
            iter.clone(),
            Ident::new(method, adapter_span),
            Vec::new(),
            adapter_span,
        ),
    };
    let iter_ident = Ident::new("__sg_for_iter", for_span);
    let let_iter = Stmt::new(
        StmtKind::Let {
            name: iter_ident.clone(),
            ty: None,
            value: Some(Box::new(source)),
            is_mut: false,
        },
        for_span,
    );
    let next_call = Expr::method_call(
        Expr::ident("__sg_for_iter", for_span),
        Ident::new("next", next_span),
        Vec::new(),
        next_span,
    );
    let some_arm = MatchArm::new(
        vec![Pattern::new(
            PatternKind::TupleStruct {
                path: Path::from_str("Some", for_span),
                patterns: vec![pattern.clone()],
            },
            for_span,
        )],
        Expr::block(body.clone()),
        for_span,
    );
    let none_arm = MatchArm::new(
        vec![Pattern::new(
            PatternKind::Path(Path::from_str("None", for_span)),
            for_span,
        )],
        Expr::break_expr(None, for_span),
        for_span,
    );
    let match_expr = Expr::match_expr(next_call, vec![some_arm, none_arm], for_span);
    let loop_expr = Expr::loop_expr(Block::new(vec![Stmt::expr(match_expr)], for_span), for_span);
    Expr::block(Block::new(vec![let_iter, Stmt::expr(loop_expr)], for_span))
}
