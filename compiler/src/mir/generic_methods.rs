use super::MIRType;
use crate::hir::{self, HIRItem, HIRParam, HIRTrait, HIRTraitItem, HIRType};
use crate::method_resolution::explicit_hir_method_param_count;
use crate::symbol::SymbolId;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct ConcreteTypeRegistry {
    inner: Rc<RefCell<ConcreteTypeRegistryInner>>,
}

#[derive(Debug, Default)]
struct ConcreteTypeRegistryInner {
    hir_by_instance_name: HashMap<String, HIRType>,
}

impl ConcreteTypeRegistry {
    pub(crate) fn new(
        struct_defs: &HashMap<String, &hir::HIRStruct>,
        concrete_named_types: &HashMap<String, HIRType>,
    ) -> Self {
        let registry = Self::default();
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

    pub(crate) fn register_instance(&self, instance_name: String, ty: HIRType) {
        self.inner
            .borrow_mut()
            .hir_by_instance_name
            .insert(instance_name, ty);
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
            MIRType::Struct { name, .. } => {
                self.inner.borrow().hir_by_instance_name.get(name).cloned()
            }
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

#[derive(Clone)]
pub(crate) struct EagerTraitMethod {
    pub(crate) function: hir::HIRFunction,
    pub(crate) explicit_param_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EagerTraitMethodRegistration {
    pub(crate) name: String,
    pub(crate) return_type: HIRType,
    pub(crate) explicit_param_count: usize,
}

pub(crate) struct TraitMethodLoweringPlan {
    pub(crate) templates: Vec<TraitMethodTemplate>,
    pub(crate) eager_methods: Vec<EagerTraitMethod>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) implemented_method_names: HashSet<String>,
}

impl TraitMethodLoweringPlan {
    pub(crate) fn eager_registrations(&self) -> Vec<EagerTraitMethodRegistration> {
        self.eager_methods
            .iter()
            .map(|method| EagerTraitMethodRegistration {
                name: method.function.name.clone(),
                return_type: method.function.return_type.clone(),
                explicit_param_count: method.explicit_param_count,
            })
            .collect()
    }
}

pub(crate) fn collect_trait_method_templates_for_impl(
    impl_item: &hir::HIRImpl,
    trait_def: Option<&HIRTrait>,
    type_prefix: &str,
) -> TraitMethodLoweringPlan {
    let Some(trait_name) = impl_item.trait_name.as_ref() else {
        return TraitMethodLoweringPlan {
            templates: Vec::new(),
            eager_methods: Vec::new(),
            implemented_method_names: HashSet::new(),
        };
    };

    let mut templates = Vec::new();
    let mut eager_methods = Vec::new();
    let mut implemented_method_names = HashSet::new();

    for method in &impl_item.items {
        let original_method_name = method
            .name
            .strip_prefix(&format!("{}_", type_prefix))
            .unwrap_or(&method.name);
        implemented_method_names.insert(original_method_name.to_string());

        if method.type_params.is_empty() {
            let mut eager_method = method.clone();
            eager_method.name = format!("{}_{}_{}", type_prefix, trait_name, original_method_name);
            eager_methods.push(EagerTraitMethod {
                explicit_param_count: explicit_hir_method_param_count(&eager_method),
                function: eager_method,
            });
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
            if implemented_method_names.contains(&trait_fn.name) {
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

            if trait_fn.type_params.is_empty() {
                let eager_function = hir::HIRFunction {
                    name: format!("{}_{}_{}", type_prefix, trait_name, trait_fn.name),
                    type_params: Vec::new(),
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
                };
                eager_methods.push(EagerTraitMethod {
                    explicit_param_count: explicit_hir_method_param_count(&eager_function),
                    function: eager_function,
                });
                continue;
            }

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

    TraitMethodLoweringPlan {
        templates,
        eager_methods,
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

    #[test]
    fn concrete_type_registry_clone_shares_registered_instances() {
        let registry = ConcreteTypeRegistry::default();
        let cloned = registry.clone();

        cloned.register_instance(
            "Box_i64".to_string(),
            HIRType::named("Box".to_string(), vec![i64_ty()]),
        );

        let resolved = registry.hir_type_for_mir(&MIRType::Struct {
            name: "Box_i64".to_string(),
            fields: vec![("value".to_string(), MIRType::Int(64))],
        });

        assert_eq!(
            resolved,
            Some(HIRType::named("Box".to_string(), vec![i64_ty()]))
        );
    }

    #[test]
    fn collect_trait_method_templates_for_impl_collects_eager_trait_functions() {
        let trait_def = HIRTrait {
            name: "WrapValue".to_string(),
            type_params: Vec::new(),
            items: vec![
                HIRTraitItem::Function(hir::HIRFunction {
                    name: "id".to_string(),
                    type_params: Vec::new(),
                    params: vec![HIRParam::new(
                        "self".to_string(),
                        SymbolId::INVALID,
                        i64_ty(),
                    )],
                    return_type: i64_ty(),
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
                    name: "fallback".to_string(),
                    type_params: Vec::new(),
                    params: vec![HIRParam::new(
                        "self".to_string(),
                        SymbolId::INVALID,
                        i64_ty(),
                    )],
                    return_type: i64_ty(),
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
            ],
            is_pub: false,
        };

        let impl_item = hir::HIRImpl {
            target_type: i64_ty(),
            trait_name: Some("WrapValue".to_string()),
            items: vec![hir::HIRFunction {
                name: "i64_id".to_string(),
                type_params: Vec::new(),
                params: vec![HIRParam::new(
                    "self".to_string(),
                    SymbolId::INVALID,
                    i64_ty(),
                )],
                return_type: i64_ty(),
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

        let eager_names = collected
            .eager_methods
            .iter()
            .map(|method| method.function.name.as_str())
            .collect::<HashSet<_>>();

        assert!(eager_names.contains("i64_WrapValue_id"));
        assert!(eager_names.contains("i64_WrapValue_fallback"));
        assert!(!eager_names.contains("i64_WrapValue_wrap"));
        assert_eq!(collected.templates.len(), 1);
        assert_eq!(collected.templates[0].method.name, "wrap");
    }

    #[test]
    fn collect_trait_method_templates_for_impl_exposes_eager_registrations() {
        let trait_def = HIRTrait {
            name: "WrapValue".to_string(),
            type_params: Vec::new(),
            items: vec![
                HIRTraitItem::Function(hir::HIRFunction {
                    name: "id".to_string(),
                    type_params: Vec::new(),
                    params: vec![HIRParam::new(
                        "self".to_string(),
                        SymbolId::INVALID,
                        i64_ty(),
                    )],
                    return_type: i64_ty(),
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
                    name: "fallback".to_string(),
                    type_params: Vec::new(),
                    params: vec![HIRParam::new(
                        "self".to_string(),
                        SymbolId::INVALID,
                        i64_ty(),
                    )],
                    return_type: i64_ty(),
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
                name: "i64_id".to_string(),
                type_params: Vec::new(),
                params: vec![HIRParam::new(
                    "self".to_string(),
                    SymbolId::INVALID,
                    i64_ty(),
                )],
                return_type: i64_ty(),
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
        let registrations = collected.eager_registrations();

        assert!(registrations.contains(&EagerTraitMethodRegistration {
            name: "i64_WrapValue_id".to_string(),
            return_type: i64_ty(),
            explicit_param_count: 0,
        }));
        assert!(registrations.contains(&EagerTraitMethodRegistration {
            name: "i64_WrapValue_fallback".to_string(),
            return_type: i64_ty(),
            explicit_param_count: 0,
        }));
    }
}
