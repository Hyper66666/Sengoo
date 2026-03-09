use super::MIRType;
use crate::hir::{self, HIRItem, HIRParam, HIRTrait, HIRTraitItem, HIRType};
use crate::symbol::SymbolId;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default)]
pub(crate) struct ConcreteTypeRegistry {
    hir_by_instance_name: HashMap<String, HIRType>,
}

impl ConcreteTypeRegistry {
    pub(crate) fn new(
        struct_defs: &HashMap<String, &hir::HIRStruct>,
        concrete_named_types: &HashMap<String, HIRType>,
    ) -> Self {
        let mut registry = Self::default();
        for (name, def) in struct_defs {
            if def.type_params.is_empty() {
                registry.register_instance(name.clone(), HIRType::named(name.clone(), Vec::new()));
            }
        }
        for (instance_name, ty) in concrete_named_types {
            registry.register_instance(instance_name.clone(), ty.clone());
        }
        registry
    }

    pub(crate) fn register_instance(&mut self, instance_name: String, ty: HIRType) {
        self.hir_by_instance_name.insert(instance_name, ty);
    }

    pub(crate) fn hir_type_for_mir(&self, ty: &MIRType) -> Option<HIRType> {
        match ty {
            MIRType::Unit => Some(HIRType::unit()),
            MIRType::Never => Some(HIRType::never()),
            MIRType::Bool => Some(HIRType::bool()),
            MIRType::Int(8) => Some(HIRType::int(crate::hir::IntKind::I8)),
            MIRType::Int(16) => Some(HIRType::int(crate::hir::IntKind::I16)),
            MIRType::Int(32) => Some(HIRType::int(crate::hir::IntKind::I32)),
            MIRType::Int(64) => Some(HIRType::int(crate::hir::IntKind::I64)),
            MIRType::Float(32) => Some(HIRType::float(crate::hir::FloatKind::F32)),
            MIRType::Float(64) => Some(HIRType::float(crate::hir::FloatKind::F64)),
            MIRType::Ptr(inner) => {
                if matches!(inner.as_ref(), MIRType::Int(8)) {
                    Some(HIRType::reference(false, HIRType::str()))
                } else {
                    self.hir_type_for_mir(inner).map(HIRType::pointer)
                }
            }
            MIRType::Ref(inner) => {
                if matches!(inner.as_ref(), MIRType::Int(8)) {
                    Some(HIRType::reference(false, HIRType::str()))
                } else {
                    self.hir_type_for_mir(inner)
                        .map(|inner| HIRType::reference(false, inner))
                }
            }
            MIRType::Array(inner, len) => self
                .hir_type_for_mir(inner)
                .map(|inner| HIRType::array(inner, *len as usize)),
            MIRType::Tuple(items) => {
                let mut hir_items = Vec::with_capacity(items.len());
                for item in items {
                    hir_items.push(self.hir_type_for_mir(item)?);
                }
                Some(HIRType::tuple(hir_items))
            }
            MIRType::Struct { name, .. } => self.hir_by_instance_name.get(name).cloned(),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct InherentMethodTemplate {
    pub(crate) target_type: HIRType,
    pub(crate) method: hir::HIRFunction,
}

pub(crate) fn collect_inherent_method_templates(items: &[HIRItem]) -> Vec<InherentMethodTemplate> {
    let mut templates = Vec::new();
    for item in items {
        if let HIRItem::Impl(impl_item) = item {
            if impl_item.trait_name.is_some() {
                continue;
            }
            for method in &impl_item.items {
                templates.push(InherentMethodTemplate {
                    target_type: impl_item.target_type.clone(),
                    method: method.clone(),
                });
            }
        }
    }
    templates
}

#[derive(Clone)]
pub(crate) struct TraitMethodTemplate {
    pub(crate) target_type: HIRType,
    pub(crate) trait_name: String,
    pub(crate) method: hir::HIRFunction,
}

pub(crate) struct TraitMethodTemplateCollection {
    pub(crate) templates: Vec<TraitMethodTemplate>,
    pub(crate) implemented_method_names: HashSet<String>,
}

pub(crate) fn collect_trait_method_templates_for_impl(
    impl_item: &hir::HIRImpl,
    trait_def: Option<&HIRTrait>,
    type_prefix: &str,
) -> TraitMethodTemplateCollection {
    let Some(trait_name) = impl_item.trait_name.as_ref() else {
        return TraitMethodTemplateCollection {
            templates: Vec::new(),
            implemented_method_names: HashSet::new(),
        };
    };

    let mut templates = Vec::new();
    let mut implemented_method_names = HashSet::new();

    for method in &impl_item.items {
        let original_method_name = method
            .name
            .strip_prefix(&format!("{}_", type_prefix))
            .unwrap_or(&method.name);
        implemented_method_names.insert(original_method_name.to_string());
        if method.type_params.is_empty() {
            continue;
        }

        let mut template_method = method.clone();
        template_method.name = original_method_name.to_string();
        templates.push(TraitMethodTemplate {
            target_type: impl_item.target_type.clone(),
            trait_name: trait_name.clone(),
            method: template_method,
        });
    }

    if let Some(trait_def) = trait_def {
        for trait_item in &trait_def.items {
            let HIRTraitItem::Function(trait_fn) = trait_item else {
                continue;
            };
            if implemented_method_names.contains(&trait_fn.name) || trait_fn.type_params.is_empty() {
                continue;
            }

            let mut params = Vec::new();
            let has_self = trait_fn.params.iter().any(|param| param.name == "self");
            if !has_self {
                params.push(HIRParam::new(
                    "self".to_string(),
                    SymbolId::INVALID,
                    impl_item.target_type.clone(),
                ));
            }
            params.extend(trait_fn.params.iter().cloned());

            templates.push(TraitMethodTemplate {
                target_type: impl_item.target_type.clone(),
                trait_name: trait_name.clone(),
                method: hir::HIRFunction {
                    name: trait_fn.name.clone(),
                    type_params: trait_fn.type_params.clone(),
                    params,
                    return_type: trait_fn.return_type.clone(),
                    precondition: trait_fn.precondition.clone(),
                    postcondition: trait_fn.postcondition.clone(),
                    body: trait_fn.body.clone(),
                    is_async: trait_fn.is_async,
                    abi: trait_fn.abi.clone(),
                    is_unsafe: trait_fn.is_unsafe,
                    no_mangle: trait_fn.no_mangle,
                    export_name: trait_fn.export_name.clone(),
                    is_pub: trait_fn.is_pub,
                },
            });
        }
    }

    TraitMethodTemplateCollection {
        templates,
        implemented_method_names,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{HIRBody, HIRTypeParam};

    fn empty_body() -> HIRBody {
        HIRBody::new()
    }

    fn i64_ty() -> HIRType {
        HIRType::int(crate::hir::IntKind::I64)
    }

    #[test]
    fn collect_trait_method_templates_for_impl_tracks_generic_impl_and_default_methods() {
        let trait_def = HIRTrait {
            name: "WrapValue".to_string(),
            type_params: Vec::new(),
            items: vec![
                HIRTraitItem::Function(hir::HIRFunction {
                    name: "wrap".to_string(),
                    type_params: vec![HIRTypeParam {
                        name: "T".to_string(),
                        bounds: Vec::new(),
                        default: None,
                    }],
                    params: vec![HIRParam::new(
                        "value".to_string(),
                        SymbolId::INVALID,
                        HIRType::named("T".to_string(), Vec::new()),
                    )],
                    return_type: HIRType::named(
                        "Wrap".to_string(),
                        vec![HIRType::named("T".to_string(), Vec::new())],
                    ),
                    precondition: None,
                    postcondition: None,
                    body: empty_body(),
                    is_async: false,
                    abi: None,
                    is_unsafe: false,
                    no_mangle: false,
                    export_name: None,
                    is_pub: false,
                }),
                HIRTraitItem::Function(hir::HIRFunction {
                    name: "mix".to_string(),
                    type_params: vec![HIRTypeParam {
                        name: "U".to_string(),
                        bounds: Vec::new(),
                        default: None,
                    }],
                    params: vec![HIRParam::new(
                        "value".to_string(),
                        SymbolId::INVALID,
                        HIRType::named("U".to_string(), Vec::new()),
                    )],
                    return_type: HIRType::named("U".to_string(), Vec::new()),
                    precondition: None,
                    postcondition: None,
                    body: empty_body(),
                    is_async: false,
                    abi: None,
                    is_unsafe: false,
                    no_mangle: false,
                    export_name: None,
                    is_pub: false,
                }),
            ],
            is_pub: false,
        };

        let impl_item = hir::HIRImpl {
            target_type: i64_ty(),
            trait_name: Some("WrapValue".to_string()),
            items: vec![hir::HIRFunction {
                name: "i64_wrap".to_string(),
                type_params: vec![HIRTypeParam {
                    name: "T".to_string(),
                    bounds: Vec::new(),
                    default: None,
                }],
                params: vec![
                    HIRParam::new("self".to_string(), SymbolId::INVALID, i64_ty()),
                    HIRParam::new(
                        "value".to_string(),
                        SymbolId::INVALID,
                        HIRType::named("T".to_string(), Vec::new()),
                    ),
                ],
                return_type: HIRType::named(
                    "Wrap".to_string(),
                    vec![HIRType::named("T".to_string(), Vec::new())],
                ),
                precondition: None,
                postcondition: None,
                body: empty_body(),
                is_async: false,
                abi: None,
                is_unsafe: false,
                no_mangle: false,
                export_name: None,
                is_pub: false,
            }],
        };

        let collected =
            collect_trait_method_templates_for_impl(&impl_item, Some(&trait_def), "i64");

        assert!(collected.implemented_method_names.contains("wrap"));
        assert_eq!(collected.templates.len(), 2);
        assert_eq!(collected.templates[0].method.name, "wrap");
        assert_eq!(collected.templates[1].method.name, "mix");
        assert_eq!(collected.templates[1].method.params[0].name, "self");
    }
}