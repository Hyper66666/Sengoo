//! 类型推断
//!
//! 使用统一算法进行类型推断

use crate::typeck::env::TypeEnv;
use crate::typeck::ty::{Subst, Ty, TyKind, TyVarId, TypeckError};
use std::collections::HashMap;

/// 类型推断器
#[derive(Debug)]
pub struct TypeInfer {
    /// 类型环境
    env: TypeEnv,
    /// 当前的类型替换
    subst: Subst,
    /// 收集的错误
    errors: Vec<TypeckError>,
}

impl TypeInfer {
    pub fn new() -> Self {
        let env = TypeEnv::new();
        let subst = Subst::new(env.interner());
        Self {
            env,
            subst,
            errors: Vec::new(),
        }
    }

    pub fn with_env(env: TypeEnv) -> Self {
        let subst = Subst::new(env.interner());
        Self {
            env,
            subst,
            errors: Vec::new(),
        }
    }

    /// 获取类型环境的引用
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// 获取类型环境的可变引用
    pub fn env_mut(&mut self) -> &mut TypeEnv {
        &mut self.env
    }

    /// 获取替换的引用
    pub fn subst(&self) -> &Subst {
        &self.subst
    }

    /// 获取收集的错误
    pub fn errors(&self) -> &[TypeckError] {
        &self.errors
    }

    /// 推断变量的类型
    pub fn instantiate_var(&mut self, name: &str) -> Result<Ty, TypeckError> {
        if let Some(symbol) = self.env.lookup(name) {
            if let Some(ty) = symbol.get_ty().cloned() {
                Ok(self.instantiate(&ty))
            } else {
                Err(TypeckError::UndefinedVariable {
                    name: name.to_string(),
                })
            }
        } else {
            Err(TypeckError::UndefinedVariable {
                name: name.to_string(),
            })
        }
    }

    /// 实例化类型（替换类型变量）
    pub fn instantiate(&mut self, ty: &Ty) -> Ty {
        self.apply_subst(ty)
    }

    /// Instantiate a polymorphic type with fresh type variables so each
    /// use-site can infer independently.
    pub fn instantiate_with_fresh_vars(&mut self, ty: &Ty) -> Ty {
        self.instantiate_with_fresh_vars_and_map(ty).0
    }

    /// Instantiate and return a map from original type variable id to
    /// instantiated fresh type variable id.
    pub fn instantiate_with_fresh_vars_and_map(
        &mut self,
        ty: &Ty,
    ) -> (Ty, HashMap<TyVarId, TyVarId>) {
        let mut var_map = HashMap::new();
        let instantiated = self.instantiate_fresh_impl(ty, &mut var_map);
        let mut id_map = HashMap::new();
        for (old_id, mapped_ty) in var_map {
            if let TyKind::Var(new_id) = mapped_ty.kind {
                id_map.insert(old_id, new_id);
            }
        }
        (instantiated, id_map)
    }

    /// 应用替换到类型
    pub fn apply_subst(&self, ty: &Ty) -> Ty {
        self.subst_apply(&self.subst, ty)
    }

    /// 使用指定替换应用类型
    fn subst_apply(&self, subst: &Subst, ty: &Ty) -> Ty {
        match &ty.kind {
            TyKind::Var(id) => {
                // Slice E 后返回 owned `Ty`（materialize 自 InternedTyId），
                // 递归调用需要取其引用。
                if let Some(replacement) = subst.get(*id) {
                    self.subst_apply(subst, &replacement)
                } else {
                    ty.clone()
                }
            }
            TyKind::Ref(m, inner) => Ty::new(
                ty.id,
                TyKind::Ref(*m, Box::new(self.subst_apply(subst, inner))),
            ),
            TyKind::Ptr(inner) => {
                Ty::new(ty.id, TyKind::Ptr(Box::new(self.subst_apply(subst, inner))))
            }
            TyKind::Array(elem, n) => Ty::new(
                ty.id,
                TyKind::Array(Box::new(self.subst_apply(subst, elem)), *n),
            ),
            TyKind::Slice(elem) => Ty::new(
                ty.id,
                TyKind::Slice(Box::new(self.subst_apply(subst, elem))),
            ),
            TyKind::Tuple(types) => {
                let new_types = types.iter().map(|t| self.subst_apply(subst, t)).collect();
                Ty::new(ty.id, TyKind::Tuple(new_types))
            }
            TyKind::Fn {
                params,
                ret,
                is_variadic,
            } => {
                let new_params = params.iter().map(|t| self.subst_apply(subst, t)).collect();
                let new_ret = Box::new(self.subst_apply(subst, ret));
                Ty::new(
                    ty.id,
                    TyKind::Fn {
                        params: new_params,
                        ret: new_ret,
                        is_variadic: *is_variadic,
                    },
                )
            }
            TyKind::Adt { name, args } => {
                let new_args = args.iter().map(|t| self.subst_apply(subst, t)).collect();
                Ty::new(
                    ty.id,
                    TyKind::Adt {
                        name: name.clone(),
                        args: new_args,
                    },
                )
            }
            _ => ty.clone(),
        }
    }

    fn instantiate_fresh_impl(&mut self, ty: &Ty, var_map: &mut HashMap<TyVarId, Ty>) -> Ty {
        match &ty.kind {
            TyKind::Var(id) => {
                if let Some(mapped) = var_map.get(id) {
                    mapped.clone()
                } else {
                    let fresh = self.fresh_ty_var();
                    var_map.insert(*id, fresh.clone());
                    fresh
                }
            }
            TyKind::Ref(m, inner) => Ty::new(
                ty.id,
                TyKind::Ref(*m, Box::new(self.instantiate_fresh_impl(inner, var_map))),
            ),
            TyKind::Ptr(inner) => Ty::new(
                ty.id,
                TyKind::Ptr(Box::new(self.instantiate_fresh_impl(inner, var_map))),
            ),
            TyKind::Array(elem, n) => Ty::new(
                ty.id,
                TyKind::Array(Box::new(self.instantiate_fresh_impl(elem, var_map)), *n),
            ),
            TyKind::Slice(elem) => Ty::new(
                ty.id,
                TyKind::Slice(Box::new(self.instantiate_fresh_impl(elem, var_map))),
            ),
            TyKind::Tuple(types) => Ty::new(
                ty.id,
                TyKind::Tuple(
                    types
                        .iter()
                        .map(|t| self.instantiate_fresh_impl(t, var_map))
                        .collect(),
                ),
            ),
            TyKind::Fn {
                params,
                ret,
                is_variadic,
            } => Ty::new(
                ty.id,
                TyKind::Fn {
                    params: params
                        .iter()
                        .map(|t| self.instantiate_fresh_impl(t, var_map))
                        .collect(),
                    ret: Box::new(self.instantiate_fresh_impl(ret, var_map)),
                    is_variadic: *is_variadic,
                },
            ),
            TyKind::Adt { name, args } => Ty::new(
                ty.id,
                TyKind::Adt {
                    name: name.clone(),
                    args: args
                        .iter()
                        .map(|t| self.instantiate_fresh_impl(t, var_map))
                        .collect(),
                },
            ),
            _ => ty.clone(),
        }
    }

    /// 统一两个类型
    pub fn unify(&mut self, ty1: &Ty, ty2: &Ty) -> Result<Subst, TypeckError> {
        self.unify_in_place(ty1, ty2)?;
        Ok(self.subst.clone())
    }

    fn unify_in_place(&mut self, ty1: &Ty, ty2: &Ty) -> Result<(), TypeckError> {
        let ty1 = self.apply_subst(ty1);
        let ty2 = self.apply_subst(ty2);

        if ty1 == ty2 {
            return Ok(());
        }

        match (&ty1.kind, &ty2.kind) {
            (TyKind::Error, _) | (_, TyKind::Error) => Ok(()),
            (TyKind::Never, _) | (_, TyKind::Never) => Ok(()),

            (TyKind::Var(id), _) => {
                self.bind_var(*id, &ty2);
                Ok(())
            }
            (_, TyKind::Var(id)) => {
                self.bind_var(*id, &ty1);
                Ok(())
            }

            (TyKind::Unit, TyKind::Unit)
            | (TyKind::Bool, TyKind::Bool)
            | (TyKind::Char, TyKind::Char)
            | (TyKind::Str, TyKind::Str)
            | (TyKind::Byte, TyKind::Byte)
            | (TyKind::Bytes, TyKind::Bytes) => Ok(()),

            (TyKind::Int(i1), TyKind::Int(i2)) if i1 == i2 => Ok(()),
            (TyKind::Float(f1), TyKind::Float(f2)) if f1 == f2 => Ok(()),

            (TyKind::Ref(m1, t1), TyKind::Ref(m2, t2)) if m1 == m2 => self.unify_in_place(t1, t2),
            (TyKind::Ref(_, _), TyKind::Ref(_, _)) => Err(TypeckError::TypeMismatch {
                expected: ty2.kind.clone(),
                found: ty1.kind.clone(),
            }),

            (TyKind::Ptr(t1), TyKind::Ptr(t2)) => self.unify_in_place(t1, t2),

            (TyKind::Tuple(ts1), TyKind::Tuple(ts2)) if ts1.len() == ts2.len() => {
                for (t1, t2) in ts1.iter().zip(ts2.iter()) {
                    let checkpoint = self.subst.clone();
                    if let Err(e) = self.unify_in_place(t1, t2) {
                        self.subst = checkpoint;
                        return Err(e);
                    }
                }
                Ok(())
            }
            (TyKind::Tuple(ts1), TyKind::Tuple(ts2)) => Err(TypeckError::TypeMismatch {
                expected: TyKind::Tuple(ts2.clone()),
                found: TyKind::Tuple(ts1.clone()),
            }),

            (TyKind::Array(e1, n1), TyKind::Array(e2, n2)) if n1 == n2 => {
                self.unify_in_place(e1, e2)
            }
            (TyKind::Array(_, n1), TyKind::Array(_, n2)) => Err(TypeckError::TypeMismatch {
                expected: TyKind::Array(Box::new(ty2.clone()), *n2),
                found: TyKind::Array(Box::new(ty1.clone()), *n1),
            }),

            (TyKind::Slice(e1), TyKind::Slice(e2)) => self.unify_in_place(e1, e2),

            (
                TyKind::Fn {
                    params: p1,
                    ret: r1,
                    ..
                },
                TyKind::Fn {
                    params: p2,
                    ret: r2,
                    ..
                },
            ) => {
                if p1.len() != p2.len() {
                    return Err(TypeckError::ArgumentCountMismatch {
                        expected: p1.len(),
                        found: p2.len(),
                    });
                }
                for (param1, param2) in p1.iter().zip(p2.iter()) {
                    let checkpoint = self.subst.clone();
                    if let Err(e) = self.unify_in_place(param1, param2) {
                        self.subst = checkpoint;
                        return Err(e);
                    }
                }
                let checkpoint = self.subst.clone();
                if let Err(e) = self.unify_in_place(r1, r2) {
                    self.subst = checkpoint;
                    return Err(e);
                }
                Ok(())
            }

            (TyKind::Adt { name: n1, args: a1 }, TyKind::Adt { name: n2, args: a2 })
                if n1 == n2 =>
            {
                if a1.len() != a2.len() {
                    return Err(TypeckError::TypeMismatch {
                        expected: ty2.kind.clone(),
                        found: ty1.kind.clone(),
                    });
                }
                for (arg1, arg2) in a1.iter().zip(a2.iter()) {
                    let checkpoint = self.subst.clone();
                    if let Err(e) = self.unify_in_place(arg1, arg2) {
                        self.subst = checkpoint;
                        return Err(e);
                    }
                }
                Ok(())
            }

            _ => Err(TypeckError::TypeMismatch {
                expected: ty2.kind.clone(),
                found: ty1.kind.clone(),
            }),
        }
    }
    fn bind_var(&mut self, var_id: TyVarId, ty: &Ty) {
        // 检查是否会出现循环
        if ty.contains_var(var_id) {
            self.errors.push(TypeckError::CyclicType);
            // 绑定到错误类型以避免无限循环
            self.subst.insert(var_id, self.env.error_ty());
            return;
        }

        self.subst.insert(var_id, ty.clone());
    }

    /// 推断新的类型变量
    pub fn fresh_ty_var(&mut self) -> Ty {
        self.env.new_ty_var()
    }

    /// 添加错误
    pub fn add_error(&mut self, error: TypeckError) {
        self.errors.push(error);
    }

    /// 是否有错误
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 重置替换
    pub fn reset_subst(&mut self) {
        self.subst = Subst::new(self.env.interner());
    }
}

impl Default for TypeInfer {
    fn default() -> Self {
        Self::new()
    }
}

/// 检查类型是否包含指定的类型变量
trait ContainsVar {
    fn contains_var(&self, var_id: TyVarId) -> bool;
}

impl ContainsVar for Ty {
    fn contains_var(&self, var_id: TyVarId) -> bool {
        match &self.kind {
            TyKind::Var(id) => *id == var_id,
            TyKind::Ref(_, t) => t.contains_var(var_id),
            TyKind::Ptr(t) => t.contains_var(var_id),
            TyKind::Array(t, _) => t.contains_var(var_id),
            TyKind::Slice(t) => t.contains_var(var_id),
            TyKind::Tuple(ts) => ts.iter().any(|t| t.contains_var(var_id)),
            TyKind::Fn { params, ret, .. } => {
                params.iter().any(|t| t.contains_var(var_id)) || ret.contains_var(var_id)
            }
            TyKind::Adt { args, .. } => args.iter().any(|t| t.contains_var(var_id)),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeck::ty::IntKind;

    fn ty(kind: TyKind) -> Ty {
        Ty::new(0, kind)
    }

    fn var(id: TyVarId) -> Ty {
        ty(TyKind::Var(id))
    }

    fn i32_ty() -> Ty {
        ty(TyKind::Int(IntKind::I32))
    }

    fn bool_ty() -> Ty {
        ty(TyKind::Bool)
    }

    #[test]
    fn failed_tuple_unify_preserves_prior_child_bindings() {
        let mut infer = TypeInfer::new();
        let var = var(0);
        let left = ty(TyKind::Tuple(vec![var.clone(), var]));
        let right = ty(TyKind::Tuple(vec![i32_ty(), bool_ty()]));

        let result = infer.unify(&left, &right);

        assert!(matches!(result, Err(TypeckError::TypeMismatch { .. })));
        // Slice E: subst.get 返回 owned `Ty`。materialize 后 origin tag = 0，
        // 与 helper `i32_ty()` 构造的 `Ty::new(0, ...)` 一致。
        assert_eq!(infer.subst().get(0), Some(i32_ty()));
    }

    #[test]
    fn failed_fn_unify_preserves_prior_param_bindings() {
        let mut infer = TypeInfer::new();
        let var = var(0);
        let left = ty(TyKind::Fn {
            params: vec![var.clone(), var.clone()],
            ret: Box::new(var),
            is_variadic: false,
        });
        let right = ty(TyKind::Fn {
            params: vec![i32_ty(), bool_ty()],
            ret: Box::new(i32_ty()),
            is_variadic: false,
        });

        let result = infer.unify(&left, &right);

        assert!(matches!(result, Err(TypeckError::TypeMismatch { .. })));
        assert_eq!(infer.subst().get(0), Some(i32_ty()));
    }
}
