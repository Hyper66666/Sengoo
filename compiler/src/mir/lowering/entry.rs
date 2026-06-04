use super::*;

pub fn lower_hir(items: &[HIRItem]) -> Result<Vec<MirFunction>, String> {
    lower_hir_with_options(items, MirLowerOptions::default())
}

pub fn lower_hir_with_options(
    items: &[HIRItem],
    options: MirLowerOptions,
) -> Result<Vec<MirFunction>, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut lambda_counter = 0;
    let generic_function_templates = items
        .iter()
        .filter_map(|item| match item {
            HIRItem::Function(fn_item) if !fn_item.type_params.is_empty() => {
                Some((fn_item.name.clone(), fn_item.clone()))
            }
            _ => None,
        })
        .collect();
    let options = options.with_generic_function_templates(generic_function_templates);

    let mut trait_defs: HashMap<String, &HIRTrait> = HashMap::new();
    let mut struct_defs: HashMap<String, &hir::HIRStruct> = HashMap::new();
    let mut known_named_types: HashSet<String> = HashSet::new();
    for item in items {
        match item {
            HIRItem::Trait(trait_item) => {
                trait_defs.insert(trait_item.name.clone(), trait_item);
            }
            HIRItem::Struct(struct_item) => {
                known_named_types.insert(struct_item.name.clone());
                struct_defs.insert(struct_item.name.clone(), struct_item);
            }
            _ => {}
        }
    }
    let concrete_named_types =
        collect_concrete_named_types_with_impl_variants(items, &known_named_types);
    let enum_defs = build_enum_defs(items, &struct_defs);
    let concrete_type_registry = ConcreteTypeRegistry::new(&struct_defs, &concrete_named_types);
    let options = options.with_enum_defs(enum_defs);
    let inherent_method_templates = collect_inherent_method_templates(items);
    let mut trait_method_templates: Vec<TraitMethodTemplate> = Vec::new();
    let mut eager_trait_functions: Vec<hir::HIRFunction> = Vec::new();

    let mut known_functions: HashSet<String> = HashSet::new();
    let mut known_function_sigs: HashMap<String, FunctionSig> = HashMap::new();
    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                known_functions.insert(fn_item.name.clone());
                known_function_sigs.insert(
                    fn_item.name.clone(),
                    build_hir_function_sig(
                        &fn_item.return_type,
                        fn_item.params.len(),
                        &struct_defs,
                    ),
                );
            }
            HIRItem::ExternBlock(extern_block) => {
                for extern_item in &extern_block.items {
                    if let hir::HIRExternItem::Function(extern_fn) = extern_item {
                        known_functions.insert(extern_fn.name.clone());
                        known_function_sigs.insert(
                            extern_fn.name.clone(),
                            build_hir_function_sig(
                                &extern_fn.return_type,
                                extern_fn.params.len(),
                                &struct_defs,
                            ),
                        );
                    }
                }
            }
            HIRItem::Impl(impl_item) => {
                for impl_item in
                    expand_impl_variants(impl_item, &concrete_named_types, &known_named_types)
                {
                    let type_prefix = impl_type_prefix(&impl_item.target_type);
                    if let Some(trait_name) = &impl_item.trait_name {
                        let collected = collect_trait_method_templates_for_impl(
                            &impl_item,
                            trait_defs.get(trait_name.as_str()).copied(),
                            &type_prefix,
                        );
                        for registration in collected.eager_registrations() {
                            known_function_sigs.insert(
                                registration.name.clone(),
                                build_hir_function_sig(
                                    &registration.return_type,
                                    registration.explicit_param_count,
                                    &struct_defs,
                                ),
                            );
                            known_functions.insert(registration.name);
                        }
                        eager_trait_functions.extend(
                            collected
                                .eager_methods
                                .into_iter()
                                .map(|method| method.function),
                        );
                        trait_method_templates.extend(collected.templates);
                    } else {
                        for method in &impl_item.items {
                            if !method.type_params.is_empty() {
                                continue;
                            }
                            known_functions.insert(method.name.clone());
                            known_function_sigs.insert(
                                method.name.clone(),
                                build_hir_function_sig(
                                    &method.return_type,
                                    explicit_hir_method_param_count(method),
                                    &struct_defs,
                                ),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                if !fn_item.type_params.is_empty() {
                    continue;
                }
                match lower_function(
                    fn_item,
                    &mut lambda_counter,
                    &known_functions,
                    &known_function_sigs,
                    &struct_defs,
                    concrete_type_registry.clone(),
                    &options,
                    &inherent_method_templates,
                    &trait_method_templates,
                    HashSet::new(),
                    HashMap::new(),
                ) {
                    Ok((mir_fn, lambdas)) => {
                        results.push(mir_fn);
                        results.extend(lambdas);
                    }
                    Err(e) => errors.push(e),
                }
            }
            HIRItem::Impl(impl_item) => {
                for impl_item in
                    expand_impl_variants(impl_item, &concrete_named_types, &known_named_types)
                {
                    if impl_item.trait_name.is_some() {
                        continue;
                    }
                    for method in &impl_item.items {
                        if !method.type_params.is_empty() {
                            continue;
                        }
                        match lower_function(
                            method,
                            &mut lambda_counter,
                            &known_functions,
                            &known_function_sigs,
                            &struct_defs,
                            concrete_type_registry.clone(),
                            &options,
                            &inherent_method_templates,
                            &trait_method_templates,
                            HashSet::new(),
                            HashMap::new(),
                        ) {
                            Ok((mir_fn, lambdas)) => {
                                results.push(mir_fn);
                                results.extend(lambdas);
                            }
                            Err(e) => errors.push(e),
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for function in eager_trait_functions {
        match lower_function(
            &function,
            &mut lambda_counter,
            &known_functions,
            &known_function_sigs,
            &struct_defs,
            concrete_type_registry.clone(),
            &options,
            &inherent_method_templates,
            &trait_method_templates,
            HashSet::new(),
            HashMap::new(),
        ) {
            Ok((mir_fn, lambdas)) => {
                results.push(mir_fn);
                results.extend(lambdas);
            }
            Err(e) => errors.push(e),
        }
    }

    if !errors.is_empty() {
        return Err(format!("MIR lowering failed:\n{}", errors.join("\n")));
    }

    Ok(results)
}
