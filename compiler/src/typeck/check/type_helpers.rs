use super::*;

impl TypeChecker {
    pub(super) fn path_name(&self, path: &Path) -> TyResult<String> {
        path.as_simple()
            .map(|ident| ident.name.clone())
            .ok_or_else(|| TypeckError::UndefinedType {
                name: path
                    .segments
                    .iter()
                    .map(|seg| seg.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            })
    }

    pub(super) fn builtin_type_by_name(&mut self, name: &str) -> Option<Ty> {
        Some(match name {
            "()" => self.env.unit_ty(),
            "bool" => self.env.bool_ty(),
            "i8" => self.env.int_ty(IntKind::I8),
            "i16" => self.env.int_ty(IntKind::I16),
            "i32" => self.env.int_ty(IntKind::I32),
            "i64" => self.env.int_ty(IntKind::I64),
            "i128" => self.env.int_ty(IntKind::I128),
            "isize" => self.env.int_ty(IntKind::ISize),
            "u8" => self.env.int_ty(IntKind::U8),
            "u16" => self.env.int_ty(IntKind::U16),
            "u32" => self.env.int_ty(IntKind::U32),
            "u64" => self.env.int_ty(IntKind::U64),
            "u128" => self.env.int_ty(IntKind::U128),
            "usize" => self.env.int_ty(IntKind::USize),
            "f32" => self.env.float_ty(FloatKind::F32),
            "f64" => self.env.float_ty(FloatKind::F64),
            "str" => self.env.str_ty(),
            "char" => self.env.new_ty(TyKind::Char),
            "!" => self.env.never_ty(),
            _ => return None,
        })
    }

    pub(super) fn resolve_generic_type_args(
        &self,
        type_name: &str,
        meta: &GenericTypeMeta,
        explicit_args: Vec<Ty>,
    ) -> TyResult<Vec<Ty>> {
        if explicit_args.len() > meta.params.len() {
            return Err(TypeckError::Other(format!(
                "type {} expects at most {} generic arguments, found {}",
                type_name,
                meta.params.len(),
                explicit_args.len()
            )));
        }

        let mut resolved = Vec::with_capacity(meta.params.len());
        let mut subst = HashMap::<TyVarId, Ty>::new();

        for (index, param) in meta.params.iter().enumerate() {
            let current = if let Some(arg) = explicit_args.get(index) {
                arg.clone()
            } else if let Some(default_ty) = &param.default {
                self.substitute_ty_vars(default_ty, &subst)
            } else {
                return Err(TypeckError::Other(format!(
                    "missing generic argument {} for type {}",
                    param.name, type_name
                )));
            };

            for bound in &param.bounds {
                let concrete_key = type_key(&current);
                if !self.impl_registry.implements_trait(bound, &concrete_key) {
                    return Err(TypeckError::Other(format!(
                        "generic constraint violated in type {}: {} does not implement {} for {}",
                        type_name, current, bound, param.name
                    )));
                }
            }

            subst.insert(param.var_id, current.clone());
            resolved.push(current);
        }

        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Span;

    #[test]
    fn builtin_type_by_name_resolves_i64() {
        let mut checker = TypeChecker::new();
        let ty = checker.builtin_type_by_name("i64").expect("builtin i64 should exist");
        assert!(matches!(ty.kind, TyKind::Int(IntKind::I64)));
    }

    #[test]
    fn check_path_type_rejects_explicit_args_on_nongeneric_type() {
        let mut checker = TypeChecker::new();
        let ty = checker.env.new_ty(TyKind::Adt { name: "Foo".to_string(), args: vec![] });
        checker.env.insert_type("Foo".to_string(), ty);
        let path = Path::from_ident(Ident::new("Foo", Span::new(0, 0)));
        let explicit = vec![checker.env.int_ty(IntKind::I64)];
        let err = checker.check_path_type(&path, explicit).unwrap_err();
        assert!(matches!(err, TypeckError::Other(msg) if msg.contains("type Foo is not generic")));
    }
}
