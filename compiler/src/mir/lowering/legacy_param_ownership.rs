//! Conservative ownership summaries for established by-value handle borrows.
//!
//! Legacy wrappers retain their source ABI (`String`, `Buffer`, ...), but a
//! parameter which only observes its handle must not silently consume the
//! caller's owner. Unknown/recursive calls and every whole-value escape remain
//! transfers. This is intentionally not general interprocedural lifetime
//! inference, and never treats an extern value parameter as a proven borrow.

use super::*;

pub(super) type BorrowedParams = HashMap<String, Vec<bool>>;

#[derive(Clone, Debug, Default)]
pub(crate) struct ParameterContracts {
    pub(crate) observed: BorrowedParams,
    pub(crate) projected: BorrowedParams,
    pub(crate) native_observers: HashSet<String>,
}

#[cfg(test)]
pub(crate) fn infer_from_items(items: &[HIRItem]) -> BorrowedParams {
    infer_contracts_from_items(items).observed
}

pub(crate) fn infer_contracts_from_items(items: &[HIRItem]) -> ParameterContracts {
    let mut definitions = Vec::new();
    for item in items {
        match item {
            HIRItem::Function(function) if function.type_params.is_empty() => {
                definitions.push(function.clone());
            }
            HIRItem::Impl(implementation) if implementation.trait_name.is_none() => {
                for function in &implementation.items {
                    if function.type_params.is_empty() {
                        definitions.push(function.clone());
                    }
                }
            }
            _ => {}
        }
    }
    infer(
        &definitions,
        &super::native_borrow_contracts::declared_observers(items),
    )
}

pub(super) fn infer(
    functions: &[hir::HIRFunction],
    native_observers: &HashSet<String>,
) -> ParameterContracts {
    let mut identity_counts = HashMap::new();
    for function in functions {
        *identity_counts
            .entry(function.name.as_str())
            .or_insert(0usize) += 1;
    }
    let definitions: HashMap<&str, &hir::HIRFunction> = functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect();
    let mut summaries: BorrowedParams = functions
        .iter()
        .map(|function| {
            (
                function.name.clone(),
                function.params.iter().map(explicit_borrow).collect(),
            )
        })
        .collect();
    for (name, params) in &mut summaries {
        if identity_counts[name.as_str()] != 1 {
            params.fill(false);
        }
    }
    let mut projected = summaries.clone();
    // Start with no inferred borrows. A recursive cycle without an independently
    // established observation summary stays consuming rather than guessing.
    loop {
        let mut changed = false;
        for function in functions {
            for (index, parameter) in function.params.iter().enumerate() {
                if !(legacy_type(&parameter.ty)
                    || matches!(parameter.ty.kind, hir::HIRTypeKind::Int(hir::IntKind::I64)))
                    || function.name.ends_with("_Drop_drop")
                    || contains_borrow(&function.return_type)
                    || identity_counts[function.name.as_str()] != 1
                {
                    continue;
                }
                let mut locals = HashMap::new();
                for parameter in &function.params {
                    locals.insert(parameter.symbol, parameter.ty.clone());
                }
                let analysis = Analysis {
                    parameter: parameter.symbol,
                    definitions: &definitions,
                    summaries: &summaries,
                    projections: &projected,
                    capture_check: false,
                    native_observers,
                    projection_context: false,
                };
                let observed = !summaries[&function.name][index]
                    && analysis.body(&function.body, false, &mut locals.clone());
                let projection = !projected[&function.name][index]
                    && matches!(
                        function.return_type.kind,
                        hir::HIRTypeKind::Int(hir::IntKind::I64)
                    )
                    && analysis
                        .in_projection()
                        .body(&function.body, true, &mut locals);
                if observed {
                    summaries.get_mut(&function.name).unwrap()[index] = true;
                    projected.get_mut(&function.name).unwrap()[index] = true;
                    changed = true;
                }
                if projection {
                    projected.get_mut(&function.name).unwrap()[index] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    ParameterContracts {
        observed: summaries,
        projected,
        native_observers: native_observers
            .iter()
            .filter(|name| !definitions.contains_key(name.as_str()))
            .cloned()
            .collect(),
    }
}

fn explicit_borrow(parameter: &hir::HIRParam) -> bool {
    parameter.is_borrowed || matches!(parameter.ty.kind, hir::HIRTypeKind::Ref(..))
}

fn legacy_type(ty: &HIRType) -> bool {
    let hir::HIRTypeKind::Named { name, .. } = &ty.kind else {
        return false;
    };
    // JsonDoc is deliberately omitted: it already has owning parameter ABI.
    name != "JsonDoc"
        && LoweringContext::is_legacy_idempotent_handle_mir_type(&MIRType::Struct {
            name: name.clone(),
            fields: Vec::new(),
        })
}

#[derive(Clone, Copy)]
struct Analysis<'a> {
    parameter: SymbolId,
    definitions: &'a HashMap<&'a str, &'a hir::HIRFunction>,
    summaries: &'a BorrowedParams,
    capture_check: bool,
    native_observers: &'a HashSet<String>,
    projections: &'a BorrowedParams,
    projection_context: bool,
}

impl Analysis<'_> {
    fn in_projection(&self) -> Self {
        Self {
            projection_context: true,
            ..*self
        }
    }
    fn body(
        &self,
        body: &HIRBody,
        tail_borrowed: bool,
        locals: &mut HashMap<SymbolId, HIRType>,
    ) -> bool {
        for statement in &body.stmts {
            match statement {
                HIRStmt::Let { symbol, value, .. } => {
                    if value
                        .as_ref()
                        .is_some_and(|value| !self.expr(value, false, locals))
                    {
                        return false;
                    }
                    // Local inference may be unavailable in the declaration
                    // prepass. Do not infer receiver identity from local types
                    // later and silently widen the checked parameter contract.
                    locals.insert(*symbol, HIRType::new(hir::HIRTypeKind::Error));
                }
                HIRStmt::Expr(value) if !self.expr(value, true, locals) => return false,
                _ => {}
            }
        }
        body.expr
            .as_deref()
            .is_none_or(|value| self.expr(value, tail_borrowed, locals))
    }

    fn expr(
        &self,
        expr: &HIRExpr,
        borrowed: bool,
        locals: &mut HashMap<SymbolId, HIRType>,
    ) -> bool {
        match expr {
            HIRExpr::Var { symbol, .. } => {
                *symbol != self.parameter || (borrowed && !self.capture_check)
            }
            HIRExpr::Lit(_) | HIRExpr::Continue => true,
            // A primitive handle projection still carries owner identity. It
            // is observable in comparisons or a proven native borrow, but may
            // not be returned/stored/reconstructed as an independent owner.
            HIRExpr::Field { base, .. } => self.expr(base, borrowed, locals),
            HIRExpr::Ref(_, base) => self.expr(base, borrowed, locals),
            HIRExpr::Call { func, args, .. } => {
                let HIRExpr::Var { name, symbol } = func.as_ref() else {
                    return self.expr(func, false, locals)
                        && args.iter().all(|arg| self.expr(arg, false, locals));
                };
                args.iter().enumerate().all(|(index, arg)| {
                    let selected = if self.projection_context && borrowed {
                        self.projections
                    } else {
                        self.summaries
                    };
                    let inferred = selected
                        .get(name)
                        .and_then(|params| params.get(index))
                        .copied()
                        .unwrap_or(false);
                    let native = !self.definitions.contains_key(name.as_str())
                        && self.native_observers.contains(name)
                        && (super::native_borrow_contracts::observes_argument(
                            name,
                            index,
                            args.len(),
                        ) || self.projection_context
                            && borrowed
                            && super::native_borrow_contracts::projects_argument(
                                name,
                                index,
                                args.len(),
                            ));
                    let observes = !locals.contains_key(symbol) && (inferred || native);
                    let context = if observes {
                        self.in_projection()
                    } else {
                        *self
                    };
                    context.expr(arg, observes, locals)
                })
            }
            HIRExpr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let target = self.method_target(receiver, method, locals);
                let selected = if self.projection_context && borrowed {
                    self.projections
                } else {
                    self.summaries
                };
                let policy = target.as_deref().and_then(|name| selected.get(name));
                let receiver_borrowed =
                    !matches!(method.as_str(), "drop" | "free" | "close" | "into_iter")
                        && policy
                            .and_then(|params| params.first())
                            .copied()
                            .unwrap_or(false);
                self.expr(receiver, receiver_borrowed, locals)
                    && args.iter().enumerate().all(|(index, arg)| {
                        let observes = policy
                            .and_then(|params| params.get(index + 1))
                            .copied()
                            .unwrap_or(false);
                        self.expr(arg, observes, locals)
                    })
            }
            HIRExpr::Struct { fields, .. } => fields
                .iter()
                .all(|(_, value)| self.expr(value, false, locals)),
            HIRExpr::EnumConstruct { args, .. } | HIRExpr::Array(args) | HIRExpr::Tuple(args) => {
                args.iter().all(|arg| self.expr(arg, false, locals))
            }
            HIRExpr::Return(value) | HIRExpr::Break(value) => value
                .as_deref()
                .is_none_or(|value| self.expr(value, false, locals)),
            HIRExpr::Block(body) | HIRExpr::TryBlock(body) => {
                self.body(body, borrowed, &mut locals.clone())
            }
            HIRExpr::Loop(body) => self.body(body, true, &mut locals.clone()),
            HIRExpr::While { cond, body } => {
                self.expr(cond, true, locals) && self.body(body, true, &mut locals.clone())
            }
            HIRExpr::For { iter, body, .. } => {
                self.expr(iter, false, locals) && self.body(body, true, &mut locals.clone())
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr(cond, true, locals)
                    && self.body(then_branch, borrowed, &mut locals.clone())
                    && else_branch
                        .as_deref()
                        .is_none_or(|body| self.body(body, borrowed, &mut locals.clone()))
            }
            HIRExpr::Match { scrutinee, arms } => {
                self.expr(scrutinee, false, locals)
                    && arms.iter().all(|arm| {
                        arm.guard
                            .as_ref()
                            .is_none_or(|guard| self.expr(guard, true, locals))
                            && self.expr(&arm.body, borrowed, &mut locals.clone())
                    })
            }
            HIRExpr::Assign { target, value } | HIRExpr::AssignOp { target, value, .. } => {
                self.expr(target, true, locals) && self.expr(value, false, locals)
            }
            HIRExpr::Index { base, index } => {
                self.expr(base, borrowed, locals) && self.expr(index, true, locals)
            }
            HIRExpr::Unary(_, value)
            | HIRExpr::Cast(value, _)
            | HIRExpr::Ascribe(value, _)
            | HIRExpr::Deref(value)
            | HIRExpr::Await(value)
            | HIRExpr::Try(value) => self.expr(value, borrowed, locals),
            HIRExpr::Binary(op, left, right) => {
                let observes = matches!(
                    op,
                    hir::HIRBinaryOp::Eq
                        | hir::HIRBinaryOp::NotEq
                        | hir::HIRBinaryOp::Lt
                        | hir::HIRBinaryOp::Le
                        | hir::HIRBinaryOp::Gt
                        | hir::HIRBinaryOp::Ge
                );
                self.expr(left, observes, locals) && self.expr(right, observes, locals)
            }
            HIRExpr::And(left, right) | HIRExpr::Or(left, right) => {
                self.expr(left, borrowed, locals) && self.expr(right, borrowed, locals)
            }
            HIRExpr::Range { start, end, .. } => {
                start
                    .as_deref()
                    .is_none_or(|value| self.expr(value, false, locals))
                    && end
                        .as_deref()
                        .is_none_or(|value| self.expr(value, false, locals))
            }
            // A capture may escape the call, including captures reached through
            // primitive fields. No closure is inferred to borrow a legacy owner.
            HIRExpr::Lambda { body, .. } => !mentions(body, self.parameter),
            HIRExpr::AsyncBlock(body) => !body_mentions(body, self.parameter),
        }
    }

    fn method_target(
        &self,
        receiver: &HIRExpr,
        method: &str,
        locals: &HashMap<SymbolId, HIRType>,
    ) -> Option<String> {
        let mut ty = self.expr_type(receiver, locals)?;
        while let hir::HIRTypeKind::Ref(_, inner) | hir::HIRTypeKind::Ptr(inner) = ty.kind {
            ty = *inner;
        }
        let hir::HIRTypeKind::Named { name, .. } = ty.kind else {
            return None;
        };
        let target = format!("{name}_{method}");
        self.definitions
            .contains_key(target.as_str())
            .then_some(target)
    }

    fn expr_type(&self, expr: &HIRExpr, locals: &HashMap<SymbolId, HIRType>) -> Option<HIRType> {
        match expr {
            HIRExpr::Var { symbol, .. } => locals.get(symbol).cloned(),
            HIRExpr::Call { func, .. } => {
                let HIRExpr::Var { name, .. } = func.as_ref() else {
                    return None;
                };
                self.definitions
                    .get(name.as_str())
                    .map(|function| function.return_type.clone())
            }
            HIRExpr::Try(value) => {
                let ty = self.expr_type(value, locals)?;
                let hir::HIRTypeKind::Named { name, args } = ty.kind else {
                    return None;
                };
                matches!(name.as_str(), "Result" | "Option")
                    .then(|| args.first().cloned())
                    .flatten()
            }
            _ => None,
        }
    }
}

fn body_mentions(body: &HIRBody, parameter: SymbolId) -> bool {
    let definitions = HashMap::new();
    let summaries = HashMap::new();
    let native_observers = HashSet::new();
    let projections = HashMap::new();
    let analysis = Analysis {
        parameter,
        definitions: &definitions,
        summaries: &summaries,
        capture_check: true,
        native_observers: &native_observers,
        projections: &projections,
        projection_context: false,
    };
    !analysis.body(body, false, &mut HashMap::new())
}

fn mentions(expr: &HIRExpr, parameter: SymbolId) -> bool {
    body_mentions(&HIRBody::with_expr(expr.clone()), parameter)
}

fn contains_borrow(ty: &HIRType) -> bool {
    match &ty.kind {
        hir::HIRTypeKind::Ref(..) | hir::HIRTypeKind::Ptr(..) => true,
        hir::HIRTypeKind::Named { args, .. } => args.iter().any(contains_borrow),
        hir::HIRTypeKind::Tuple(fields) => fields.iter().any(contains_borrow),
        hir::HIRTypeKind::Array(inner, _) | hir::HIRTypeKind::Slice(inner) => {
            contains_borrow(inner)
        }
        hir::HIRTypeKind::Fn { .. }
        | hir::HIRTypeKind::AssocProjection { .. }
        | hir::HIRTypeKind::TraitObject(_) => true,
        _ => false,
    }
}
