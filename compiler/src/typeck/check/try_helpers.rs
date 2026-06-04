use super::*;

/// Where `?` may propagate errors (function return type or `try {}` boundary).
#[derive(Debug, Clone)]
pub(super) enum PropagationContext {
    Result {
        err: Ty,
    },
    Option {
        inner: Ty,
    },
    /// Interior of `try { }`; success/error types are fixed when the block finishes.
    TryBlockInfer {
        err: Ty,
        inner: Ty,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TryBlockMode {
    Unknown,
    Result,
    Option,
}

impl TypeChecker {
    pub(super) fn propagation_from_ty(ty: &Ty) -> Option<PropagationContext> {
        Self::peel_result_ty_static(ty)
            .map(|(_, err)| PropagationContext::Result { err })
            .or_else(|| {
                Self::peel_option_ty_static(ty).map(|inner| PropagationContext::Option { inner })
            })
    }

    fn peel_result_ty_static(ty: &Ty) -> Option<(Ty, Ty)> {
        match &ty.kind {
            TyKind::Adt { name, args } if name == "Result" && args.len() == 2 => {
                Some((args[0].clone(), args[1].clone()))
            }
            _ => None,
        }
    }

    fn peel_option_ty_static(ty: &Ty) -> Option<Ty> {
        match &ty.kind {
            TyKind::Adt { name, args } if name == "Option" && args.len() == 1 => {
                Some(args[0].clone())
            }
            _ => None,
        }
    }

    pub(super) fn push_propagation(&mut self, ctx: PropagationContext) {
        self.propagation_stack.push(ctx);
    }

    pub(super) fn pop_propagation(&mut self) {
        self.propagation_stack.pop();
    }

    fn current_propagation(&self) -> Option<&PropagationContext> {
        self.propagation_stack.last()
    }

    fn set_try_block_mode(&mut self, mode: TryBlockMode) {
        if let Some(active) = self.try_block_mode_stack.last_mut() {
            if *active == TryBlockMode::Unknown {
                *active = mode;
            }
        }
    }

    pub(super) fn check_try_expr(&mut self, operand: &Expr) -> TyResult<Ty> {
        let op_ty = self.check_expr(operand)?;
        let op_ty = self.infer.apply_subst(&op_ty);
        let ctx = self.current_propagation().cloned().ok_or_else(|| {
            TypeckError::InvalidQuestionMark {
                message: "the `?` operator can only be used in a function, closure, or `try` block that returns `Result` or `Option`".to_string(),
                span_lo: operand.span.lo,
                span_hi: operand.span.hi,
            }
        })?;

        if let Some((ok_ty, err_ty)) = Self::peel_result_ty_static(&self.infer.apply_subst(&op_ty))
        {
            return match ctx {
                PropagationContext::Result {
                    err: expected_err, ..
                } => {
                    self.infer.unify(&err_ty, &expected_err)?;
                    Ok(ok_ty)
                }
                PropagationContext::TryBlockInfer {
                    err: expected_err, ..
                } => {
                    self.infer.unify(&err_ty, &expected_err)?;
                    self.set_try_block_mode(TryBlockMode::Result);
                    Ok(ok_ty)
                }
                PropagationContext::Option { .. } => Err(TypeckError::InvalidQuestionMark {
                    message: "cannot use `?` on `Result` in a context that expects `Option`"
                        .to_string(),
                    span_lo: operand.span.lo,
                    span_hi: operand.span.hi,
                }),
            };
        }

        if let Some(inner_ty) = Self::peel_option_ty_static(&self.infer.apply_subst(&op_ty)) {
            return match ctx {
                PropagationContext::Option {
                    inner: expected_inner,
                } => {
                    self.infer.unify(&inner_ty, &expected_inner)?;
                    Ok(inner_ty)
                }
                PropagationContext::TryBlockInfer {
                    inner: expected_inner,
                    ..
                } => {
                    self.infer.unify(&inner_ty, &expected_inner)?;
                    self.set_try_block_mode(TryBlockMode::Option);
                    Ok(inner_ty)
                }
                PropagationContext::Result { .. } => Err(TypeckError::InvalidQuestionMark {
                    message: "cannot use `?` on `Option` in a context that expects `Result`"
                        .to_string(),
                    span_lo: operand.span.lo,
                    span_hi: operand.span.hi,
                }),
            };
        }

        Err(TypeckError::InvalidQuestionMark {
            message: "the `?` operator requires a `Result` or `Option` value".to_string(),
            span_lo: operand.span.lo,
            span_hi: operand.span.hi,
        })
    }

    pub(super) fn check_try_block_expr(&mut self, block: &Block) -> TyResult<Ty> {
        let err_ty = self.infer.fresh_ty_var();
        let inner_ty = self.infer.fresh_ty_var();

        self.try_block_mode_stack.push(TryBlockMode::Unknown);
        self.push_propagation(PropagationContext::TryBlockInfer {
            err: err_ty.clone(),
            inner: inner_ty,
        });

        let body_ty = self.check_block(block)?;
        self.pop_propagation();

        let mode = self
            .try_block_mode_stack
            .pop()
            .unwrap_or(TryBlockMode::Unknown);

        match mode {
            TryBlockMode::Option => {
                if Self::peel_option_ty_static(&body_ty).is_some() {
                    Ok(body_ty)
                } else {
                    Ok(self.env.new_ty(TyKind::Adt {
                        name: "Option".to_string(),
                        args: vec![body_ty],
                    }))
                }
            }
            TryBlockMode::Result | TryBlockMode::Unknown => {
                if Self::peel_result_ty_static(&body_ty).is_some() {
                    Ok(body_ty)
                } else {
                    Ok(self.env.new_ty(TyKind::Adt {
                        name: "Result".to_string(),
                        args: vec![body_ty, err_ty],
                    }))
                }
            }
        }
    }

    pub(super) fn with_propagation<R>(
        &mut self,
        ctx: Option<PropagationContext>,
        f: impl FnOnce(&mut Self) -> TyResult<R>,
    ) -> TyResult<R> {
        let pushed = ctx.is_some();
        if let Some(ctx) = ctx {
            self.push_propagation(ctx);
        }
        let result = f(self);
        if pushed {
            self.pop_propagation();
        }
        result
    }
}
