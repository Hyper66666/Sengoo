use super::*;
use crate::ast::pattern::{Pattern, PatternKind};
use crate::ast::{Expr, MatchArm};
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
struct PatternBindings {
    names: Vec<(String, Ty)>,
}

impl TypeChecker {
    pub(super) fn check_match_expr(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        match_span: crate::lexer::Span,
    ) -> TyResult<Ty> {
        let scrutinee_ty_raw = self.check_expr(scrutinee)?;
        let scrutinee_ty = self.infer.apply_subst(&scrutinee_ty_raw);

        let mut arm_types = Vec::new();
        let mut has_catch_all = false;

        for arm in arms {
            if has_catch_all {
                return Err(TypeckError::UnreachableMatchArm {
                    span_lo: arm.span.lo,
                    span_hi: arm.span.hi,
                });
            }

            if arm.patterns.len() > 1 {
                self.check_alternative_pattern_bindings(&arm.patterns, &scrutinee_ty, arm.span)?;
            } else if let Some(pattern) = arm.patterns.first() {
                self.check_pattern_or_bindings(pattern, &scrutinee_ty)?;
            }

            if arm
                .patterns
                .iter()
                .any(|pat| self.pattern_is_catch_all(pat, arm.guard.is_none()))
            {
                has_catch_all = true;
            }

            self.env.push_scope();
            for pattern in &arm.patterns {
                self.bind_pattern_vars(pattern, &scrutinee_ty)?;
            }
            if let Some(guard) = &arm.guard {
                let guard_ty = self.check_expr(guard)?;
                self.infer.unify(&guard_ty, &self.env.bool_ty())?;
                if !matches!(self.infer.apply_subst(&guard_ty).kind, TyKind::Bool) {
                    return Err(TypeckError::GuardNotBool {
                        span_lo: guard.span.lo,
                        span_hi: guard.span.hi,
                    });
                }
            }
            let arm_ty = self.check_expr(&arm.body)?;
            self.env.pop_scope();
            arm_types.push(arm_ty);
        }

        if !has_catch_all {
            if let Some(missing) = self.missing_enum_variants(&scrutinee_ty, arms) {
                if !missing.is_empty() {
                    return Err(TypeckError::NonExhaustiveMatch {
                        missing,
                        span_lo: match_span.lo,
                        span_hi: match_span.hi,
                    });
                }
            }
        }

        let result_ty = arm_types
            .first()
            .cloned()
            .unwrap_or_else(|| self.env.unit_ty());
        for arm_ty in &arm_types {
            self.infer.unify(&result_ty, arm_ty)?;
        }

        Ok(result_ty)
    }

    fn pattern_is_catch_all(&self, pat: &Pattern, unguarded: bool) -> bool {
        if !unguarded {
            return false;
        }
        matches!(pat.kind, PatternKind::Wildcard)
    }

    fn check_alternative_pattern_bindings(
        &mut self,
        patterns: &[Pattern],
        scrutinee_ty: &Ty,
        span: crate::lexer::Span,
    ) -> TyResult<()> {
        let mut expected: Option<PatternBindings> = None;
        for pat in patterns {
            let bindings = self.collect_pattern_bindings(pat, scrutinee_ty)?;
            if let Some(exp) = &expected {
                if !bindings_compatible(exp, &bindings) {
                    return Err(TypeckError::OrPatternBindingMismatch {
                        span_lo: span.lo,
                        span_hi: span.hi,
                    });
                }
            } else {
                expected = Some(bindings);
            }
        }
        Ok(())
    }

    fn check_pattern_or_bindings(&mut self, pat: &Pattern, scrutinee_ty: &Ty) -> TyResult<()> {
        match &pat.kind {
            PatternKind::Or(alts) => {
                let mut expected: Option<PatternBindings> = None;
                for alt in alts {
                    let bindings = self.collect_pattern_bindings(alt, scrutinee_ty)?;
                    if let Some(exp) = &expected {
                        if !bindings_compatible(exp, &bindings) {
                            return Err(TypeckError::OrPatternBindingMismatch {
                                span_lo: pat.span.lo,
                                span_hi: pat.span.hi,
                            });
                        }
                    } else {
                        expected = Some(bindings);
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn bind_pattern_vars(&mut self, pat: &Pattern, scrutinee_ty: &Ty) -> TyResult<()> {
        let bindings = self.collect_pattern_bindings(pat, scrutinee_ty)?;
        for (name, ty) in bindings.names {
            self.env.insert_var(name, ty);
        }
        Ok(())
    }

    fn scrutinee_enum_name(scrutinee_ty: &Ty) -> Option<String> {
        match &scrutinee_ty.kind {
            TyKind::Adt { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    fn collect_pattern_bindings(
        &mut self,
        pat: &Pattern,
        scrutinee_ty: &Ty,
    ) -> TyResult<PatternBindings> {
        let scrutinee_enum = Self::scrutinee_enum_name(scrutinee_ty);
        match &pat.kind {
            PatternKind::Wildcard => Ok(PatternBindings::default()),
            PatternKind::Literal(_) => Ok(PatternBindings::default()),
            PatternKind::Ident(ident) => Ok(PatternBindings {
                names: vec![(ident.name.clone(), scrutinee_ty.clone())],
            }),
            PatternKind::Path(_) => Ok(PatternBindings::default()),
            PatternKind::TupleStruct { path, patterns } => {
                let mut names = Vec::new();
                if let Some(field_tys) =
                    self.enum_variant_field_tys_for_path(path, scrutinee_enum.as_deref())
                {
                    for (sub, field_ty) in patterns.iter().zip(field_tys.iter()) {
                        names.extend(self.collect_pattern_bindings(sub, field_ty)?.names);
                    }
                } else {
                    for sub in patterns {
                        names.extend(self.collect_pattern_bindings(sub, scrutinee_ty)?.names);
                    }
                }
                Ok(PatternBindings { names })
            }
            PatternKind::Struct { path, fields, .. } => {
                let mut names = Vec::new();
                if let Some(field_tys) =
                    self.enum_variant_field_tys_for_path(path, scrutinee_enum.as_deref())
                {
                    for (field, field_ty) in fields.iter().zip(field_tys.iter()) {
                        names.extend(
                            self.collect_pattern_bindings(&field.pattern, field_ty)?
                                .names,
                        );
                    }
                } else {
                    for field in fields {
                        names.extend(
                            self.collect_pattern_bindings(&field.pattern, scrutinee_ty)?
                                .names,
                        );
                    }
                }
                Ok(PatternBindings { names })
            }
            PatternKind::Tuple(patterns) => {
                let mut names = Vec::new();
                for sub in patterns {
                    names.extend(self.collect_pattern_bindings(sub, scrutinee_ty)?.names);
                }
                Ok(PatternBindings { names })
            }
            PatternKind::Or(alts) => {
                let first = alts
                    .first()
                    .ok_or_else(|| TypeckError::Other("empty or-pattern".to_string()))?;
                self.collect_pattern_bindings(first, scrutinee_ty)
            }
            _ => Ok(PatternBindings::default()),
        }
    }

    fn missing_enum_variants(&self, scrutinee_ty: &Ty, arms: &[MatchArm]) -> Option<Vec<String>> {
        let TyKind::Adt { name, .. } = &scrutinee_ty.kind else {
            return None;
        };
        let variants = self.enum_variants.get(name)?;
        let mut covered = HashSet::new();
        for arm in arms {
            for pat in &arm.patterns {
                self.collect_covered_variants(pat, &mut covered);
            }
        }
        let missing: Vec<String> = variants
            .iter()
            .filter(|variant| !covered.contains(*variant))
            .cloned()
            .collect();
        if missing.is_empty() {
            None
        } else {
            Some(missing)
        }
    }

    fn collect_covered_variants(&self, pat: &Pattern, covered: &mut HashSet<String>) {
        match &pat.kind {
            PatternKind::Path(path) | PatternKind::TupleStruct { path, .. } => {
                if let Some(ident) = path.segments.last() {
                    covered.insert(ident.name.clone());
                }
            }
            PatternKind::Or(alts) => {
                for alt in alts {
                    self.collect_covered_variants(alt, covered);
                }
            }
            _ => {}
        }
    }
}

impl TypeChecker {
    fn enum_variant_field_tys_for_path(
        &self,
        path: &crate::ast::Path,
        scrutinee_enum: Option<&str>,
    ) -> Option<Vec<Ty>> {
        let variant_name = path.segments.last()?.name.clone();
        let enum_name = if path.segments.len() >= 2 {
            path.segments[0].name.clone()
        } else {
            scrutinee_enum?.to_string()
        };
        self.enum_variant_field_tys
            .get(&enum_name)?
            .get(&variant_name)
            .cloned()
    }
}

fn bindings_compatible(left: &PatternBindings, right: &PatternBindings) -> bool {
    if left.names.len() != right.names.len() {
        return false;
    }
    for (l_name, l_ty) in &left.names {
        let Some((_, r_ty)) = right.names.iter().find(|(name, _)| name == l_name) else {
            return false;
        };
        if l_ty.kind != r_ty.kind {
            return false;
        }
    }
    true
}
