//! 类型推断
//!
//! 使用统一算法进行类型推断

use crate::typeck::env::TypeEnv;
use crate::typeck::ty::{Subst, Ty, TyKind, TyVarId, TypeckError};

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
        Self {
            env: TypeEnv::new(),
            subst: Subst::new(),
            errors: Vec::new(),
        }
    }

    pub fn with_env(env: TypeEnv) -> Self {
        Self {
            env,
            subst: Subst::new(),
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
            if let Some(ty) = symbol.get_ty() {
                Ok(self.instantiate(ty.clone()))
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
    pub fn instantiate(&mut self, ty: Ty) -> Ty {
        self.apply_subst(&ty)
    }

    /// 应用替换到类型
    pub fn apply_subst(&self, ty: &Ty) -> Ty {
        self.subst_apply(&self.subst, ty)
    }

    /// 使用指定替换应用类型
    fn subst_apply(&self, subst: &Subst, ty: &Ty) -> Ty {
        match &ty.kind {
            TyKind::Var(id) => {
                if let Some(replacement) = subst.get(*id) {
                    // 递归应用替换
                    self.subst_apply(subst, replacement)
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

    /// 统一两个类型
    pub fn unify(&mut self, ty1: &Ty, ty2: &Ty) -> Result<Subst, TypeckError> {
        let ty1 = self.apply_subst(ty1);
        let ty2 = self.apply_subst(ty2);

        // 快速路径：相同类型
        if ty1 == ty2 {
            return Ok(self.subst.clone());
        }

        match (&ty1.kind, &ty2.kind) {
            // 错误类型：直接返回，避免错误级联
            (TyKind::Error, _) | (_, TyKind::Error) => Ok(self.subst.clone()),

            // Never 类型（底类型）：与任何类型兼容
            // Never 是所有类型的子类型，因此 unify(Never, T) 和 unify(T, Never) 都应成功
            (TyKind::Never, _) | (_, TyKind::Never) => Ok(self.subst.clone()),

            // 类型变量
            (TyKind::Var(id), _) => {
                self.bind_var(*id, &ty2);
                Ok(self.subst.clone())
            }
            (_, TyKind::Var(id)) => {
                self.bind_var(*id, &ty1);
                Ok(self.subst.clone())
            }

            // 相同类型构造器
            (TyKind::Unit, TyKind::Unit)
            | (TyKind::Bool, TyKind::Bool)
            | (TyKind::Char, TyKind::Char)
            | (TyKind::Str, TyKind::Str)
            | (TyKind::Byte, TyKind::Byte)
            | (TyKind::Bytes, TyKind::Bytes) => Ok(self.subst.clone()),

            (TyKind::Int(i1), TyKind::Int(i2)) if i1 == i2 => Ok(self.subst.clone()),
            (TyKind::Float(f1), TyKind::Float(f2)) if f1 == f2 => Ok(self.subst.clone()),

            // 引用类型
            (TyKind::Ref(m1, t1), TyKind::Ref(m2, t2)) if m1 == m2 => self.unify(t1, t2),
            (TyKind::Ref(_, _), TyKind::Ref(_, _)) => Err(TypeckError::TypeMismatch {
                expected: ty2.kind.clone(),
                found: ty1.kind.clone(),
            }),

            // 指针类型
            (TyKind::Ptr(t1), TyKind::Ptr(t2)) => self.unify(t1, t2),

            // 元组类型
            (TyKind::Tuple(ts1), TyKind::Tuple(ts2)) if ts1.len() == ts2.len() => {
                let mut subst = self.subst.clone();
                for (t1, t2) in ts1.iter().zip(ts2.iter()) {
                    let old_subst = self.subst.clone();
                    self.subst = subst.clone();
                    match self.unify(t1, t2) {
                        Ok(s) => subst = subst.union(s),
                        Err(e) => {
                            self.subst = old_subst;
                            return Err(e);
                        }
                    }
                }
                self.subst = subst;
                Ok(self.subst.clone())
            }
            (TyKind::Tuple(ts1), TyKind::Tuple(ts2)) => Err(TypeckError::TypeMismatch {
                expected: TyKind::Tuple(ts2.clone()),
                found: TyKind::Tuple(ts1.clone()),
            }),

            // 数组类型
            (TyKind::Array(e1, n1), TyKind::Array(e2, n2)) if n1 == n2 => self.unify(e1, e2),
            (TyKind::Array(_, n1), TyKind::Array(_, n2)) => Err(TypeckError::TypeMismatch {
                expected: TyKind::Array(Box::new(ty2.clone()), *n2),
                found: TyKind::Array(Box::new(ty1.clone()), *n1),
            }),

            // 切片类型
            (TyKind::Slice(e1), TyKind::Slice(e2)) => self.unify(e1, e2),

            // 函数类型
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
                let mut subst = self.subst.clone();
                for (param1, param2) in p1.iter().zip(p2.iter()) {
                    let old_subst = self.subst.clone();
                    self.subst = subst.clone();
                    match self.unify(param1, param2) {
                        Ok(s) => subst = subst.union(s),
                        Err(e) => {
                            self.subst = old_subst;
                            return Err(e);
                        }
                    }
                }
                // 统一返回类型
                let old_subst = self.subst.clone();
                self.subst = subst.clone();
                match self.unify(r1, r2) {
                    Ok(s) => subst = subst.union(s),
                    Err(e) => {
                        self.subst = old_subst;
                        return Err(e);
                    }
                }
                self.subst = subst;
                Ok(self.subst.clone())
            }

            // ADT 类型（简化处理）
            (TyKind::Adt { name: n1, args: a1 }, TyKind::Adt { name: n2, args: a2 })
                if n1 == n2 =>
            {
                if a1.len() != a2.len() {
                    return Err(TypeckError::TypeMismatch {
                        expected: ty2.kind.clone(),
                        found: ty1.kind.clone(),
                    });
                }
                let mut subst = self.subst.clone();
                for (arg1, arg2) in a1.iter().zip(a2.iter()) {
                    let old_subst = self.subst.clone();
                    self.subst = subst.clone();
                    match self.unify(arg1, arg2) {
                        Ok(s) => subst = subst.union(s),
                        Err(e) => {
                            self.subst = old_subst;
                            return Err(e);
                        }
                    }
                }
                self.subst = subst;
                Ok(self.subst.clone())
            }

            // 其他情况：类型不匹配
            _ => Err(TypeckError::TypeMismatch {
                expected: ty2.kind.clone(),
                found: ty1.kind.clone(),
            }),
        }
    }

    /// 绑定类型变量
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
        self.subst = Subst::new();
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
