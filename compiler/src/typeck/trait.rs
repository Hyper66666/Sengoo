//! Trait 注册表
//!
//! 管理 Trait 定义和 Impl 块的注册和查询。

use crate::typeck::ty::{Ty, TyVarId};
use std::collections::HashMap;
use std::sync::Arc;

/// Trait 信息
#[derive(Debug, Clone)]
pub struct TraitInfo {
    /// Trait 名称
    pub name: String,
    /// 类型参数
    pub type_params: Vec<String>,
    /// 方法签名：(方法名, 参数类型, 返回类型)
    pub methods: HashMap<String, MethodSig>,
    /// 关联常量
    pub consts: HashMap<String, Ty>,
    /// 关联类型
    pub assoc_types: Vec<String>,
    /// 是否公开
    pub is_pub: bool,
}

impl TraitInfo {
    pub fn new(name: String, type_params: Vec<String>, is_pub: bool) -> Self {
        Self {
            name,
            type_params,
            methods: HashMap::new(),
            consts: HashMap::new(),
            assoc_types: Vec::new(),
            is_pub,
        }
    }

    /// 添加方法
    pub fn add_method(&mut self, name: String, sig: MethodSig) {
        self.methods.insert(name, sig);
    }

    /// 添加常量
    pub fn add_const(&mut self, name: String, ty: Ty) {
        self.consts.insert(name, ty);
    }

    /// 添加关联类型
    pub fn add_assoc_type(&mut self, name: String) {
        self.assoc_types.push(name);
    }

    /// 检查是否包含指定方法
    pub fn has_method(&self, name: &str) -> bool {
        self.methods.contains_key(name)
    }

    /// 获取方法签名
    pub fn get_method(&self, name: &str) -> Option<&MethodSig> {
        self.methods.get(name)
    }
}

/// 方法签名
#[derive(Debug, Clone)]
pub struct MethodSig {
    pub has_self: bool,
    pub param_types: Vec<Ty>,
    pub return_type: Ty,
    pub generic_params: Vec<TyVarId>,
    pub has_default: bool,
}

impl MethodSig {
    pub fn new(
        has_self: bool,
        param_types: Vec<Ty>,
        return_type: Ty,
        generic_params: Vec<TyVarId>,
    ) -> Self {
        Self {
            has_self,
            param_types,
            return_type,
            generic_params,
            has_default: false,
        }
    }

    pub fn with_default(
        has_self: bool,
        param_types: Vec<Ty>,
        return_type: Ty,
        generic_params: Vec<TyVarId>,
    ) -> Self {
        Self {
            has_self,
            param_types,
            return_type,
            generic_params,
            has_default: true,
        }
    }
}/// Impl 信息
#[derive(Debug, Clone)]
pub struct ImplInfo {
    /// 目标类型
    pub target_type: Ty,
    /// 实现的 Trait（None 表示固有 impl）
    pub trait_name: Option<String>,
    /// 方法：(方法名, 函数类型)
    pub methods: HashMap<String, FunctionTy>,
    /// 关联常量
    pub consts: HashMap<String, Ty>,
    /// 关联类型
    pub assoc_types: HashMap<String, Ty>,
}

impl ImplInfo {
    pub fn new(target_type: Ty, trait_name: Option<String>) -> Self {
        Self {
            target_type,
            trait_name,
            methods: HashMap::new(),
            consts: HashMap::new(),
            assoc_types: HashMap::new(),
        }
    }

    /// 添加方法
    pub fn add_method(&mut self, name: String, ty: FunctionTy) {
        self.methods.insert(name, ty);
    }

    /// 添加常量
    pub fn add_const(&mut self, name: String, ty: Ty) {
        self.consts.insert(name, ty);
    }

    /// 添加关联类型
    pub fn add_assoc_type(&mut self, name: String, ty: Ty) {
        self.assoc_types.insert(name, ty);
    }

    /// 是否包含指定方法
    pub fn has_method(&self, name: &str) -> bool {
        self.methods.contains_key(name)
    }

    /// 获取方法类型
    pub fn get_method(&self, name: &str) -> Option<&FunctionTy> {
        self.methods.get(name)
    }
}

/// 函数类型
#[derive(Debug, Clone)]
pub struct FunctionTy {
    /// 是否有 self 参数
    pub has_self: bool,
    /// 参数类型
    pub param_types: Vec<Ty>,
    /// 返回类型
    pub return_type: Ty,
    pub generic_params: Vec<TyVarId>,
}

impl FunctionTy {
    pub fn new(has_self: bool, param_types: Vec<Ty>, return_type: Ty) -> Self {
        Self {
            has_self,
            param_types,
            return_type,
            generic_params: Vec::new(),
        }
    }

    pub fn with_generic_params(
        has_self: bool,
        param_types: Vec<Ty>,
        return_type: Ty,
        generic_params: Vec<TyVarId>,
    ) -> Self {
        Self {
            has_self,
            param_types,
            return_type,
            generic_params,
        }
    }
}

/// Trait 注册表
#[derive(Debug, Clone)]
pub struct TraitRegistry {
    /// 所有 Trait 定义 (trait_name -> TraitInfo)
    traits: HashMap<String, Arc<TraitInfo>>,
}

impl TraitRegistry {
    pub fn new() -> Self {
        Self {
            traits: HashMap::new(),
        }
    }

    /// 注册 Trait
    pub fn register(&mut self, info: TraitInfo) -> Option<Arc<TraitInfo>> {
        let name = info.name.clone();
        let info = Arc::new(info);
        self.traits.insert(name, info.clone())
    }

    /// 获取 Trait 信息
    pub fn get(&self, name: &str) -> Option<Arc<TraitInfo>> {
        self.traits.get(name).cloned()
    }

    /// 检查 Trait 是否存在
    pub fn contains(&self, name: &str) -> bool {
        self.traits.contains_key(name)
    }

    /// 获取所有 Trait 名称
    pub fn all_traits(&self) -> Vec<String> {
        self.traits.keys().cloned().collect()
    }
}

impl Default for TraitRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Impl 注册表
#[derive(Debug, Clone)]
pub struct ImplRegistry {
    /// 固有 impl (type_key -> Vec<ImplInfo>)
    inherent_impls: HashMap<String, Vec<ImplInfo>>,
    /// Trait impl (trait_name -> type_key -> ImplInfo)
    trait_impls: HashMap<String, HashMap<String, ImplInfo>>,
}

impl ImplRegistry {
    pub fn new() -> Self {
        Self {
            inherent_impls: HashMap::new(),
            trait_impls: HashMap::new(),
        }
    }

    /// 注册固有 impl
    pub fn register_inherent(&mut self, type_key: String, info: ImplInfo) {
        self.inherent_impls
            .entry(type_key)
            .or_default()
            .push(info);
    }

    /// 注册 Trait impl
    pub fn register_trait_impl(&mut self, trait_name: String, type_key: String, info: ImplInfo) {
        self.trait_impls
            .entry(trait_name)
            .or_default()
            .insert(type_key, info);
    }

    /// 查找固有 impl 的方法
    pub fn lookup_inherent_method(&self, type_key: &str, method_name: &str) -> Option<&FunctionTy> {
        self.inherent_impls.get(type_key).and_then(|impls| {
            impls
                .iter()
                .find_map(|impl_info| impl_info.get_method(method_name))
        })
    }

    /// 查找 Trait impl 的方法
    pub fn lookup_trait_method(
        &self,
        trait_name: &str,
        type_key: &str,
        method_name: &str,
    ) -> Option<&FunctionTy> {
        self.trait_impls
            .get(trait_name)
            .and_then(|type_map| type_map.get(type_key))
            .and_then(|impl_info| impl_info.get_method(method_name))
    }

    /// 检查类型是否实现了指定 Trait
    pub fn implements_trait(&self, trait_name: &str, type_key: &str) -> bool {
        self.trait_impls
            .get(trait_name)
            .map(|type_map| type_map.contains_key(type_key))
            .unwrap_or(false)
    }

    /// 获取类型的所有固有 impl
    pub fn get_inherent_impls(&self, type_key: &str) -> Vec<&ImplInfo> {
        self.inherent_impls
            .get(type_key)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }
}

impl Default for ImplRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 生成类型的键（用于 HashMap）
pub fn type_key(ty: &Ty) -> String {
    match &ty.kind {
        crate::typeck::ty::TyKind::Unit => "()".to_string(),
        crate::typeck::ty::TyKind::Bool => "bool".to_string(),
        crate::typeck::ty::TyKind::Int(int_kind) => int_kind.to_string(),
        crate::typeck::ty::TyKind::Float(float_kind) => float_kind.to_string(),
        crate::typeck::ty::TyKind::Str => "str".to_string(),
        crate::typeck::ty::TyKind::Char => "char".to_string(),
        crate::typeck::ty::TyKind::Adt { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                format!(
                    "{}<{}>",
                    name,
                    args.iter().map(type_key).collect::<Vec<_>>().join(",")
                )
            }
        }
        crate::typeck::ty::TyKind::Ref(_, inner) => format!("&{}", type_key(inner)),
        crate::typeck::ty::TyKind::Ptr(inner) => format!("*{}", type_key(inner)),
        crate::typeck::ty::TyKind::Array(elem, len) => format!("[{}; {}]", type_key(elem), len),
        crate::typeck::ty::TyKind::Slice(elem) => format!("[{}]", type_key(elem)),
        crate::typeck::ty::TyKind::Tuple(types) => {
            if types.is_empty() {
                "()".to_string()
            } else {
                format!(
                    "({})",
                    types.iter().map(type_key).collect::<Vec<_>>().join(", ")
                )
            }
        }
        crate::typeck::ty::TyKind::Fn { .. } => "fn".to_string(),
        crate::typeck::ty::TyKind::Never => "!".to_string(),
        crate::typeck::ty::TyKind::Var(_) | crate::typeck::ty::TyKind::Error => "?".to_string(),
        _ => "<unknown>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeck::ty::{IntKind, TyKind};

    #[test]
    fn test_trait_registry() {
        let mut registry = TraitRegistry::new();

        let trait_info = TraitInfo::new("Display".to_string(), vec![], true);
        registry.register(trait_info);

        assert!(registry.contains("Display"));
        assert!(!registry.contains("Debug"));
    }

    #[test]
    fn test_impl_registry() {
        let mut registry = ImplRegistry::new();

        let ty = Ty::new(0, TyKind::Int(IntKind::I32));
        let impl_info = ImplInfo::new(ty.clone(), Some("Display".to_string()));
        registry.register_trait_impl("Display".to_string(), "i32".to_string(), impl_info);

        assert!(registry.implements_trait("Display", "i32"));
        assert!(!registry.implements_trait("Debug", "i32"));
    }

    #[test]
    fn test_type_key() {
        let ty = Ty::new(0, TyKind::Int(IntKind::I32));
        assert_eq!(type_key(&ty), "i32");

        let tuple_ty = Ty::new(
            1,
            TyKind::Tuple(vec![
                Ty::new(2, TyKind::Int(IntKind::I32)),
                Ty::new(3, TyKind::Bool),
            ]),
        );
        assert_eq!(type_key(&tuple_ty), "(i32, bool)");
    }
}
