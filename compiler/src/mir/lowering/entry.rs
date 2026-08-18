use super::*;
use crate::mir::dyn_dispatch::DynMethodSlot;
use crate::mir::EnumDefMap;

/// Per-trait vtable slot layout, keyed by trait name.
type TraitMethodOrder = HashMap<String, Vec<DynMethodSlot>>;
/// Per-function `&dyn Trait` parameter expectations, keyed by function name.
type DynParamTraits = HashMap<String, Vec<Option<String>>>;

pub fn lower_hir(items: &[HIRItem]) -> Result<Vec<MirFunction>, String> {
    lower_hir_with_options(items, MirLowerOptions::default())
}

pub fn lower_hir_with_options(
    items: &[HIRItem],
    options: MirLowerOptions,
) -> Result<Vec<MirFunction>, String> {
    let target_pointer_width = options.target_pointer_width;
    crate::mir::type_mapping_helpers::with_target_pointer_width(target_pointer_width, || {
        lower_hir_with_options_scoped(items, options)
    })
}

fn lower_hir_with_options_scoped(
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
            HIRItem::Enum(enum_item) => {
                known_named_types.insert(enum_item.name.clone());
            }
            _ => {}
        }
    }
    let concrete_named_types =
        collect_concrete_named_types_with_impl_variants(items, &known_named_types);
    let enum_defs = build_enum_defs(items, &struct_defs);
    let concrete_type_registry = ConcreteTypeRegistry::new(&struct_defs, &concrete_named_types);
    // Keep enum declarations visible to nested type binders (method
    // specialization sees only struct_defs otherwise) for this lowering run.
    let _scoped_enum_defs =
        crate::mir::type_mapping_helpers::ScopedEnumDefs::install(enum_defs.clone());
    let options = options.with_enum_defs(enum_defs);
    let (trait_method_order, dyn_param_traits) =
        build_dyn_dispatch_metadata(items, &trait_defs, &struct_defs, &options.enum_defs);
    let options = options.with_dyn_dispatch_metadata(trait_method_order, dyn_param_traits);
    let inherent_method_templates = collect_inherent_method_templates(items);
    let concrete_inherent_method_names =
        collect_concrete_inherent_method_names(items, &known_named_types);
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
                    build_hir_function_sig_with_enums(
                        &fn_item.return_type,
                        fn_item.params.len(),
                        &struct_defs,
                        &options.enum_defs,
                    ),
                );
            }
            HIRItem::ExternBlock(extern_block) => {
                for extern_item in &extern_block.items {
                    if let hir::HIRExternItem::Function(extern_fn) = extern_item {
                        known_functions.insert(extern_fn.name.clone());
                        known_function_sigs.insert(
                            extern_fn.name.clone(),
                            build_hir_function_sig_with_enums(
                                &extern_fn.return_type,
                                extern_fn.params.len(),
                                &struct_defs,
                                &options.enum_defs,
                            ),
                        );
                    }
                }
            }
            HIRItem::Impl(impl_item) => {
                let impl_is_concrete =
                    hir_type_is_concrete(&impl_item.target_type, &known_named_types);
                for impl_item in
                    expand_impl_variants(impl_item, &concrete_named_types, &known_named_types)
                {
                    for method in &impl_item.items {
                        if method.type_params.is_empty()
                            && matches!(method.return_type.kind, hir::HIRTypeKind::Named { .. })
                            && hir_type_is_concrete(&method.return_type, &known_named_types)
                        {
                            concrete_type_registry.register_instance(
                                crate::type_naming::hir_type_instance_name(&method.return_type),
                                method.return_type.clone(),
                            );
                        }
                    }
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
                                build_hir_function_sig_with_enums(
                                    &registration.return_type,
                                    registration.explicit_param_count,
                                    &struct_defs,
                                    &options.enum_defs,
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
                            if !impl_is_concrete && is_terminal_iterator_method(&method.name) {
                                continue;
                            }
                            if !method.type_params.is_empty() {
                                continue;
                            }
                            if !impl_is_concrete
                                && concrete_inherent_method_names.contains(&method.name)
                            {
                                continue;
                            }
                            known_functions.insert(method.name.clone());
                            known_function_sigs.insert(
                                method.name.clone(),
                                build_hir_function_sig_with_enums(
                                    &method.return_type,
                                    explicit_hir_method_param_count(method),
                                    &struct_defs,
                                    &options.enum_defs,
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
                let impl_is_concrete =
                    hir_type_is_concrete(&impl_item.target_type, &known_named_types);
                for impl_item in
                    expand_impl_variants(impl_item, &concrete_named_types, &known_named_types)
                {
                    if impl_item.trait_name.is_some() {
                        continue;
                    }
                    for method in &impl_item.items {
                        if !impl_is_concrete && is_terminal_iterator_method(&method.name) {
                            continue;
                        }
                        if !method.type_params.is_empty() {
                            continue;
                        }
                        if !impl_is_concrete
                            && concrete_inherent_method_names.contains(&method.name)
                        {
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

    dedup_mir_functions_by_name(&mut results);

    let shims = synthesize_dyn_vtable_shims(&options, &results);
    results.extend(shims);

    let mut owned_drop_traits: Vec<String> = options
        .dyn_owned_drop_requests
        .borrow()
        .iter()
        .cloned()
        .collect();
    owned_drop_traits.sort();
    for trait_name in owned_drop_traits {
        results.push(synthesize_owned_dyn_drop_helper(&trait_name));
    }

    coverage_helpers::inject_coverage_registration(&mut results, &options);

    Ok(results)
}

fn dedup_mir_functions_by_name(functions: &mut Vec<MirFunction>) {
    let mut seen = HashSet::new();
    functions.retain(|function| seen.insert(function.name.clone()));
}

fn collect_concrete_inherent_method_names(
    items: &[HIRItem],
    known_named_types: &HashSet<String>,
) -> HashSet<String> {
    let concrete_named_types = HashMap::new();
    let mut names = HashSet::new();
    for item in items {
        let HIRItem::Impl(impl_item) = item else {
            continue;
        };
        if impl_item.trait_name.is_some()
            || !hir_type_is_concrete(&impl_item.target_type, known_named_types)
        {
            continue;
        }
        for impl_item in expand_impl_variants(impl_item, &concrete_named_types, known_named_types) {
            for method in impl_item.items {
                names.insert(method.name);
            }
        }
    }
    names
}

/// Collect dyn-dispatch metadata: per-trait vtable slot layout (sorted method
/// names + return types) and, per free function, which parameters are `&dyn
/// Trait`.
fn build_dyn_dispatch_metadata(
    items: &[HIRItem],
    trait_defs: &HashMap<String, &HIRTrait>,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    enum_defs: &EnumDefMap,
) -> (TraitMethodOrder, DynParamTraits) {
    use crate::mir::dyn_dispatch::dyn_trait_of_hir_param;
    use crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums;

    let empty_subst = HashMap::new();
    let mut trait_method_order = HashMap::new();
    for (trait_name, trait_def) in trait_defs {
        let mut slots: Vec<DynMethodSlot> = trait_def
            .items
            .iter()
            .filter_map(|item| match item {
                crate::hir::HIRTraitItem::Function(f) => Some(DynMethodSlot {
                    name: f.name.clone(),
                    ret: hir_type_to_mir_with_structs_and_enums(
                        &f.return_type,
                        struct_defs,
                        enum_defs,
                        &empty_subst,
                    ),
                }),
                _ => None,
            })
            .collect();
        slots.sort_by(|a, b| a.name.cmp(&b.name));
        slots.dedup_by(|a, b| a.name == b.name);
        trait_method_order.insert((*trait_name).clone(), slots);
    }

    let mut dyn_param_traits = HashMap::new();
    for item in items {
        if let HIRItem::Function(fn_item) = item {
            let traits: Vec<Option<String>> = fn_item
                .params
                .iter()
                .map(|p| dyn_trait_of_hir_param(&p.ty))
                .collect();
            if traits.iter().any(Option::is_some) {
                dyn_param_traits.insert(fn_item.name.clone(), traits);
            }
        }
    }

    (trait_method_order, dyn_param_traits)
}

/// Synthesize by-pointer dispatch shims for every `(trait, concrete)` pair seen
/// at a coercion site. Each shim loads the concrete receiver from the data
/// pointer and forwards to the monomorphic `Type_Trait_method` implementation,
/// keeping dynamic dispatch ABI-compatible with the direct call path.
fn synthesize_dyn_vtable_shims(
    options: &MirLowerOptions,
    results: &[MirFunction],
) -> Vec<MirFunction> {
    use crate::mir::dyn_dispatch::{vtable_shim_name, VTABLE_METHOD_BASE};

    let by_name: HashMap<&str, &MirFunction> =
        results.iter().map(|f| (f.name.as_str(), f)).collect();

    let mut pairs: Vec<(String, String)> = options
        .dyn_vtable_requests
        .borrow()
        .iter()
        .cloned()
        .collect();
    pairs.sort();

    let mut shims = Vec::new();
    for (trait_name, type_prefix) in pairs {
        let Some(slots) = options.trait_method_order.get(&trait_name) else {
            continue;
        };
        let concrete_ty = slots.iter().find_map(|method| {
            let eager_name = format!("{}_{}_{}", type_prefix, trait_name, method.name);
            by_name
                .get(eager_name.as_str())
                .and_then(|eager_fn| eager_fn.params.first())
                .map(dyn_concrete_object_ty)
        });
        if let Some(concrete_ty) = concrete_ty.clone() {
            let drop_func = format!("{}_Drop_drop", type_prefix);
            shims.push(synthesize_dyn_drop_thunk(
                &trait_name,
                &type_prefix,
                &concrete_ty,
                by_name
                    .contains_key(drop_func.as_str())
                    .then_some(drop_func),
            ));
        }
        for (slot, method) in slots.iter().enumerate() {
            let eager_name = format!("{}_{}_{}", type_prefix, trait_name, method.name);
            let Some(eager_fn) = by_name.get(eager_name.as_str()) else {
                continue;
            };
            let Some((concrete_ty, method_params)) = eager_fn.params.split_first() else {
                continue;
            };
            let ret_ty = eager_fn.return_type.clone();
            let concrete_ty = concrete_ty.clone();

            let mut shim_params = Vec::with_capacity(method_params.len() + 1);
            shim_params.push(MIRType::Ptr(Box::new(MIRType::Int(8))));
            shim_params.extend(method_params.iter().cloned());

            let shim_name = vtable_shim_name(
                &trait_name,
                &type_prefix,
                VTABLE_METHOD_BASE + slot,
                &method.name,
            );
            let mut shim = MirFunction::new(shim_name, shim_params, ret_ty.clone());

            // param 1 = data pointer (i8*); reinterpret it to the concrete
            // receiver ABI. Value receivers need a load; reference receivers
            // are already pointer-shaped and must not be loaded again.
            let data_param = Local::new(1, LocalKind::Param);
            let self_val = if matches!(concrete_ty, MIRType::Ref(_) | MIRType::Ptr(_)) {
                let typed_ref = shim.add_local(LocalKind::Temp, concrete_ty.clone());
                shim.push_inst_to_block(
                    0,
                    Instruction::Cast {
                        destination: typed_ref,
                        value: data_param,
                        to: concrete_ty.clone(),
                    },
                );
                typed_ref
            } else {
                let typed_ptr =
                    shim.add_local(LocalKind::Temp, MIRType::Ptr(Box::new(concrete_ty.clone())));
                shim.push_inst_to_block(
                    0,
                    Instruction::Cast {
                        destination: typed_ptr,
                        value: data_param,
                        to: MIRType::Ptr(Box::new(concrete_ty.clone())),
                    },
                );
                let self_val = shim.add_local(LocalKind::Temp, concrete_ty);
                shim.push_inst_to_block(
                    0,
                    Instruction::Load {
                        destination: self_val,
                        source: typed_ptr,
                    },
                );
                self_val
            };

            let mut call_args = vec![self_val];
            for i in 0..method_params.len() {
                call_args.push(Local::new(2 + i, LocalKind::Param));
            }
            let ret_local = shim.add_local(LocalKind::Temp, ret_ty);
            shim.push_inst_to_block(
                0,
                Instruction::Call {
                    destination: ret_local,
                    func: eager_name,
                    args: call_args,
                },
            );
            shim.basic_blocks[0].set_terminator(Terminator::Return(Some(ret_local)));
            shims.push(shim);
        }
    }
    shims
}

fn dyn_concrete_object_ty(receiver_ty: &MIRType) -> MIRType {
    match receiver_ty {
        MIRType::Ref(inner) | MIRType::Ptr(inner) => inner.as_ref().clone(),
        other => other.clone(),
    }
}

/// Synthesize the per-trait owned `dyn Trait` drop helper
/// `__dyn_Trait_Drop_drop(fat: __dyn_Trait)`. It loads the vtable drop slot
/// from the fat pointer and, when non-null, calls the erased drop thunk with
/// the data pointer, dispatching to the concrete `Drop` impl.
fn synthesize_owned_dyn_drop_helper(trait_name: &str) -> MirFunction {
    use crate::mir::dyn_dispatch::{dyn_fat_ptr_type, dyn_struct_name, VTABLE_DROP_SLOT};

    let fat_ty = dyn_fat_ptr_type(trait_name);
    let name = format!("{}_Drop_drop", dyn_struct_name(trait_name));
    let mut helper = MirFunction::new(name, vec![fat_ty], MIR_UNIT);

    let fat_param = Local::new(1, LocalKind::Param);
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let i64_ptr = MIRType::Ptr(Box::new(MIR_I64));

    let data = helper.add_local(LocalKind::Temp, i8_ptr.clone());
    helper.push_inst_to_block(
        0,
        Instruction::Extract {
            destination: data,
            value: fat_param,
            index: 0,
        },
    );
    let vtable_i8 = helper.add_local(LocalKind::Temp, i8_ptr);
    helper.push_inst_to_block(
        0,
        Instruction::Extract {
            destination: vtable_i8,
            value: fat_param,
            index: 1,
        },
    );
    let vtable_words = helper.add_local(LocalKind::Temp, i64_ptr.clone());
    helper.push_inst_to_block(
        0,
        Instruction::Cast {
            destination: vtable_words,
            value: vtable_i8,
            to: i64_ptr,
        },
    );
    let slot = helper.add_local(LocalKind::Temp, MIR_I64);
    helper.push_inst_to_block(
        0,
        Instruction::Assign {
            destination: slot,
            value: MirConstant::Int(VTABLE_DROP_SLOT as i64),
        },
    );
    let slot_addr = helper.add_local(LocalKind::Temp, MIRType::Ptr(Box::new(MIR_I64)));
    helper.push_inst_to_block(
        0,
        Instruction::IndexAddr {
            destination: slot_addr,
            base: vtable_words,
            index: slot,
        },
    );
    let fnptr = helper.add_local(LocalKind::Temp, MIR_I64);
    helper.push_inst_to_block(
        0,
        Instruction::Load {
            destination: fnptr,
            source: slot_addr,
        },
    );
    let zero = helper.add_local(LocalKind::Temp, MIR_I64);
    helper.push_inst_to_block(
        0,
        Instruction::Assign {
            destination: zero,
            value: MirConstant::Int(0),
        },
    );
    let has_drop = helper.add_local(LocalKind::Temp, MIR_BOOL);
    helper.push_inst_to_block(
        0,
        Instruction::Binary {
            destination: has_drop,
            op: MirBinOp::Ne,
            left: fnptr,
            right: zero,
        },
    );

    let call_block = helper.add_block();
    let exit_block = helper.add_block();
    helper.basic_blocks[0].set_terminator(Terminator::If {
        cond: has_drop,
        then_block: call_block,
        else_block: exit_block,
    });

    let unit = helper.add_local(LocalKind::Temp, MIR_UNIT);
    helper.push_inst_to_block(
        call_block,
        Instruction::CallIndirect {
            destination: unit,
            func_ptr: fnptr,
            args: vec![data],
        },
    );
    helper.basic_blocks[call_block].set_terminator(Terminator::Goto(exit_block));
    helper.basic_blocks[exit_block].set_terminator(Terminator::Return(None));
    helper
}

fn synthesize_dyn_drop_thunk(
    trait_name: &str,
    type_prefix: &str,
    concrete_ty: &MIRType,
    drop_func: Option<String>,
) -> MirFunction {
    let (size, align) = crate::codegen::common::mir_type_size_align(concrete_ty);
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let thunk_name =
        crate::mir::dyn_dispatch::vtable_drop_shim_name(trait_name, type_prefix, size, align);
    let mut thunk = MirFunction::new(thunk_name, vec![i8_ptr.clone()], MIR_UNIT);

    let data_param = Local::new(1, LocalKind::Param);
    let typed_ptr = thunk.add_local(LocalKind::Temp, MIRType::Ptr(Box::new(concrete_ty.clone())));
    thunk.push_inst_to_block(
        0,
        Instruction::Cast {
            destination: typed_ptr,
            value: data_param,
            to: MIRType::Ptr(Box::new(concrete_ty.clone())),
        },
    );

    // Concrete `{Type}_Drop_drop` glue takes the receiver by value (matching
    // the drop-glue ABI used for scope-exit drops), so load the payload before
    // calling it.
    if let Some(drop_func) = drop_func {
        let payload = thunk.add_local(LocalKind::Temp, concrete_ty.clone());
        thunk.push_inst_to_block(
            0,
            Instruction::Load {
                destination: payload,
                source: typed_ptr,
            },
        );
        let destination = thunk.add_local(LocalKind::Temp, MIR_UNIT);
        thunk.push_inst_to_block(
            0,
            Instruction::Call {
                destination,
                func: drop_func,
                args: vec![payload],
            },
        );
    }
    thunk.basic_blocks[0].set_terminator(Terminator::Return(None));
    thunk
}
