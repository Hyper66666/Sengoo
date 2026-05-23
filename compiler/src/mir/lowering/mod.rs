//! HIR到MIR的降级器，将高级中间表示转换为低级中间表示。

//!
//! Lowering is single-threaded. Shared mutable lowering state uses
//! `Rc<RefCell<_>>` so nested lowering contexts clone cheap handles instead
//! of deep-copying async function sets or concrete type registries.
use super::generic_methods::{
    collect_inherent_method_templates, collect_trait_method_templates_for_impl,
    ConcreteTypeRegistry, InherentMethodTemplate, TraitMethodTemplate,
};
use crate::hir::HIRTrait;
use crate::hir::{self, HIRBody, HIRExpr, HIRItem, HIRLiteral, HIRStmt, HIRType};
use crate::method_resolution::explicit_hir_method_param_count;
use crate::mir::async_dispatch_helpers::{build_async_dispatch_registry, AsyncDispatchRegistry};
use crate::mir::async_origin_helpers::{
    infer_async_base_name_from_instructions, infer_last_async_start_base,
};
use crate::mir::concrete_type_helpers::collect_concrete_named_types_with_impl_variants;
use crate::mir::direct_call_helpers::collect_direct_call_names;
use crate::mir::function_sig_helpers::{build_function_sig, build_hir_function_sig};
use crate::mir::impl_specialization_helpers::{
    expand_impl_variants, impl_type_prefix, resolve_inherent_method_specialization,
};
use crate::mir::local_type_helpers::collect_local_types;
use crate::mir::lowering_helpers::{
    collect_free_vars, collect_free_vars_in_body, collect_named_symbols,
};
use crate::mir::method_specialization_helpers::resolve_trait_method_specialization;
use crate::mir::pattern_helpers::{
    build_match_switch_plan, pattern_binding_plan, pattern_match_plan, PatternBindingPlan,
    PatternMatchPlan,
};
use crate::mir::type_helpers::is_void_like;
use crate::mir::type_mapping_helpers::{
    bind_mir_subst_from_hir_type, hir_type_to_mir_with_structs,
    hir_type_to_mir_with_structs_and_subst,
};
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirBinOp, MirConstant, MirFunction, MirUnOp,
    Terminator, MIR_BOOL, MIR_I64, MIR_UNIT,
};
use crate::symbol::SymbolId;
use crate::type_naming::mir_type_instance_name as mir_type_to_instance_name;
use std::collections::{HashMap, HashSet};

mod aggregate_expr_helpers;
mod assignment_helpers;
mod block_async_expr_helpers;
mod body_lowering_helpers;
mod builtin_helpers;
mod call_emission_helpers;
mod call_expr_helpers;
mod call_invocation_helpers;
mod call_target_helpers;
mod entry;
mod for_expr_helpers;
mod function_lowering;
mod if_expr_helpers;
mod lambda_expr_helpers;
mod let_stmt_helpers;
mod loop_control_helpers;
mod loop_expr_helpers;
mod match_expr_helpers;
mod method_builtin_helpers;
mod method_call_helpers;
mod method_expr_helpers;
mod named_call_helpers;
mod non_named_call_helpers;
mod op_expr_helpers;
mod options;
mod pointer_expr_helpers;
mod while_expr_helpers;
pub use entry::{lower_hir, lower_hir_with_options};
pub use options::MirLowerOptions;
use self::aggregate_expr_helpers::{
    lower_array_expr, lower_field_expr, lower_index_expr, lower_struct_expr,
};
use self::assignment_helpers::{lower_assign_expr, lower_assign_op_expr};
use self::block_async_expr_helpers::{lower_async_block_expr, lower_await_expr, lower_block_expr};
use self::call_emission_helpers::emit_call_from_plan;
use self::call_expr_helpers::lower_call_expr;
use self::call_invocation_helpers::build_call_invocation_plan;
use self::call_target_helpers::CallTargetResolution;
use self::for_expr_helpers::lower_for_expr;
use self::function_lowering::lower_function;
use self::if_expr_helpers::lower_if_expr;
use self::lambda_expr_helpers::lower_lambda_expr;
use self::let_stmt_helpers::lower_let_stmt;
use self::loop_control_helpers::{lower_break_expr, lower_continue_expr};
use self::loop_expr_helpers::lower_loop_expr;
use self::match_expr_helpers::lower_match_expr;
use self::method_expr_helpers::lower_method_call_expr;
use self::op_expr_helpers::{
    lower_binary_expr, lower_logical_and_expr, lower_logical_or_expr, lower_unary_expr,
};
use self::pointer_expr_helpers::{lower_deref_expr, lower_ref_expr};
use self::while_expr_helpers::lower_while_expr;

fn mir_local_name(local: Local) -> String {
    match local.kind {
        LocalKind::Param => format!("%l_{}", local.id),
        LocalKind::Temp => format!("%t_{}", local.id),
        LocalKind::User => format!("%u_{}", local.id),
        LocalKind::Return => format!("%ret_{}", local.id),
    }
}

/// 循环上下文，记录 `break/continue` 目标基本块。
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    /// break目标基本块的索引。
    break_block: usize,
    /// continue目标基本块的索引。
    continue_block: usize,
}

/// 函数签名信息，存储函数名、参数数量和参数类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FunctionSig {
    pub(crate) ret_type: MIRType,
    pub(crate) param_count: usize,
    /// 函数参数数量（不含环境指针参数）。
    #[allow(dead_code)]
    pub(crate) env: Vec<(String, MIRType)>,
}

/// Lambda 捕获环境。
struct LambdaEnv {
    /// 捕获变量列表，保存变量名及其对应的局部变量 `Local`。
    vars: Vec<(String, Local)>,
    /// 自由变量列表，用于lambda捕获分析。
    #[allow(dead_code)]
    env_type: MIRType,
    /// 捕获环境数组对应的MIR局部变量（Local句柄）。
    env_ptr_local: Option<Local>,
}

/// MIR lowering 上下文。
struct LoweringContext<'a> {
    mir_fn: &'a mut MirFunction,
    /// 当前正在降级的MIR函数的可变引用。
    local_names: HashMap<String, Local>,
    local_symbols: HashMap<SymbolId, Local>,
    contract_param_bindings: Vec<(String, SymbolId, Local)>,
    /// 当前基本块的索引（None表示未设置）。
    current_block: Option<usize>,
    /// 错误信息列表，记录降级过程中遇到的错误。
    errors: Vec<String>,
    /// 循环上下文栈，记录嵌套循环的 `break/continue` 目标。
    loop_stack: Vec<LoopContext>,
    /// 循环嵌套栈，支持多层循环的break/continue。
    lambda_counter: &'a mut usize,
    /// 存储lambda上下文中Lambda函数计数器的引用。
    lambda_functions: Vec<MirFunction>,
    /// lambda名称到Local的映射，用于lambda引用解析。
    lambda_names: HashMap<Local, String>,
    /// lambda函数集合，存储生成的所有lambda MIR函数。
    function_sigs: HashMap<String, FunctionSig>,
    /// lambda环境信息表，按名称索引。
    lambda_environments: HashMap<String, LambdaEnv>,
    /// 局部变量名与MIR类型的映射表。
    type_names: HashMap<Local, String>,
    /// 已知函数名集合，用于快速判断标识符是否表示函数调用。
    known_functions: HashSet<String>,
    struct_defs: &'a HashMap<String, &'a hir::HIRStruct>,
    concrete_type_registry: ConcreteTypeRegistry,
    options: MirLowerOptions,
    inherent_method_templates: &'a [InherentMethodTemplate],
    trait_method_templates: &'a [TraitMethodTemplate],
    async_dispatch_registry: AsyncDispatchRegistry,
    /// Maps a Local → async function base name when that local holds a future
    /// handle produced by a `foo__start(...)` call. Propagated through let
    /// bindings so that `let f = async_fn(); await f` resolves correctly.
    future_origins: HashMap<Local, String>,
}

impl<'a> LoweringContext<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        mir_fn: &'a mut MirFunction,
        lambda_counter: &'a mut usize,
        known_functions: &'a HashSet<String>,
        known_function_sigs: &HashMap<String, FunctionSig>,
        struct_defs: &'a HashMap<String, &'a hir::HIRStruct>,
        concrete_type_registry: ConcreteTypeRegistry,
        options: MirLowerOptions,
        inherent_method_templates: &'a [InherentMethodTemplate],
        trait_method_templates: &'a [TraitMethodTemplate],
    ) -> Self {
        let async_names = options
            .async_functions
            .borrow()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let async_dispatch_registry = build_async_dispatch_registry(async_names);
        Self {
            mir_fn,
            local_names: HashMap::new(),
            local_symbols: HashMap::new(),
            contract_param_bindings: Vec::new(),
            current_block: None,
            errors: Vec::new(),
            loop_stack: Vec::new(),
            lambda_counter,
            lambda_functions: Vec::new(),
            lambda_names: HashMap::new(),
            function_sigs: known_function_sigs.clone(),
            lambda_environments: HashMap::new(),
            type_names: HashMap::new(),
            known_functions: known_functions.clone(),
            struct_defs,
            concrete_type_registry,
            options,
            inherent_method_templates,
            trait_method_templates,
            async_dispatch_registry,
            future_origins: HashMap::new(),
        }
    }

    fn async_dispatch_kind_id(&mut self, base_name: &str) -> Option<i64> {
        self.async_dispatch_registry.kind_id(base_name).or_else(|| {
            self.errors.push(format!(
                "unable to assign a stable async dispatch id for future origin `{base_name}` during MIR lowering"
            ));
            None
        })
    }

    fn lower_materialized_method(&mut self, specialized: hir::HIRFunction) -> Option<String> {
        if self.known_functions.contains(&specialized.name) {
            return Some(specialized.name);
        }

        self.function_sigs.insert(
            specialized.name.clone(),
            build_hir_function_sig(
                &specialized.return_type,
                explicit_hir_method_param_count(&specialized),
                self.struct_defs,
            ),
        );
        self.known_functions.insert(specialized.name.clone());

        match lower_function(
            &specialized,
            self.lambda_counter,
            &self.known_functions,
            &self.function_sigs,
            self.struct_defs,
            self.concrete_type_registry.clone(),
            &self.options,
            self.inherent_method_templates,
            self.trait_method_templates,
        ) {
            Ok((mir_fn, nested)) => {
                self.lambda_functions.push(mir_fn);
                self.lambda_functions.extend(nested);
                Some(specialized.name)
            }
            Err(error) => {
                self.errors.push(error);
                None
            }
        }
    }

    fn try_materialize_inherent_method(
        &mut self,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
    ) -> Option<String> {
        let actual_arg_types =
            collect_local_types(arg_locals, |local| self.get_local_type(local).clone());
        let specialized = resolve_inherent_method_specialization(
            self.inherent_method_templates,
            method,
            receiver_ty,
            &actual_arg_types,
            self.struct_defs,
            &mut self.concrete_type_registry,
        )?;

        self.lower_materialized_method(specialized)
    }

    fn try_materialize_trait_method(
        &mut self,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
        type_display: &str,
    ) -> Result<Option<String>, String> {
        let actual_arg_types =
            collect_local_types(arg_locals, |local| self.get_local_type(local).clone());
        let specialized = resolve_trait_method_specialization(
            self.trait_method_templates,
            method,
            receiver_ty,
            &actual_arg_types,
            self.struct_defs,
            &mut self.concrete_type_registry,
            type_display,
        )?;

        match specialized {
            Some(specialized) => Ok(self.lower_materialized_method(specialized)),
            None => Ok(None),
        }
    }

    fn infer_struct_literal_type(
        &mut self,
        name: &str,
        field_locals: &HashMap<String, Local>,
    ) -> Option<MIRType> {
        let def = self.struct_defs.get(name)?;
        let mut subst: HashMap<String, MIRType> = HashMap::new();
        for field in &def.fields {
            let local = field_locals.get(&field.name)?;
            bind_mir_subst_from_hir_type(
                &field.ty,
                self.get_local_type(*local),
                self.struct_defs,
                &mut subst,
            );
        }

        if !def.type_params.is_empty()
            && !def
                .type_params
                .iter()
                .all(|type_param| subst.contains_key(&type_param.name))
        {
            return None;
        }

        let (instance_name, concrete_hir_ty) = if def.type_params.is_empty() {
            (
                name.to_string(),
                HIRType::named(name.to_string(), Vec::new()),
            )
        } else {
            let mut parts: Vec<String> = Vec::with_capacity(def.type_params.len());
            let mut concrete_args: Vec<HIRType> = Vec::with_capacity(def.type_params.len());

            for type_param in &def.type_params {
                let Some(mir_arg) = subst.get(&type_param.name) else {
                    self.errors.push(format!(
                        "struct literal {}: generic type parameter {} could not be inferred during MIR lowering",
                        name, type_param.name
                    ));
                    return None;
                };
                parts.push(mir_type_to_instance_name(mir_arg));

                let Some(hir_arg) = self.concrete_type_registry.hir_type_for_mir(mir_arg) else {
                    self.errors.push(format!(
                        "struct literal {}: concrete type argument for {} could not be resolved during MIR lowering",
                        name, type_param.name
                    ));
                    return None;
                };
                concrete_args.push(hir_arg);
            }

            (
                format!("{}_{}", name, parts.join("_")),
                HIRType::named(name.to_string(), concrete_args),
            )
        };
        self.concrete_type_registry
            .register_instance(instance_name.clone(), concrete_hir_ty);

        Some(MIRType::Struct {
            name: instance_name,
            fields: def
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        hir_type_to_mir_with_structs_and_subst(&field.ty, self.struct_defs, &subst),
                    )
                })
                .collect(),
        })
    }

    fn lambda_name(&mut self) -> String {
        let name = format!("$__lambda{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }

    fn async_block_name(&mut self) -> String {
        let name = format!("$__async_block{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }

    /// 将循环上下文压入循环嵌套栈。
    fn push_loop(&mut self, break_block: usize, continue_block: usize) {
        self.loop_stack.push(LoopContext {
            break_block,
            continue_block,
        });
    }

    /// 返回当前上下文的指令列表的可变引用。
    /// 根据参数列表收集表达式中的自由变量及其对应 `Local`。
    fn collect_free_vars(
        &self,
        params: &[String],
        body: &crate::hir::HIRExpr,
    ) -> Vec<(String, Local)> {
        collect_free_vars(body, params, &self.local_names)
    }

    fn collect_async_block_free_vars(&self, body: &crate::hir::HIRBody) -> Vec<(String, Local)> {
        collect_free_vars_in_body(body, &self.local_names)
    }

    fn lower_async_block(&mut self, body: &HIRBody) -> Local {
        let async_block_name = self.async_block_name();
        let free_vars = self.collect_async_block_free_vars(body);
        let capture_types: Vec<MIRType> = free_vars
            .iter()
            .map(|(_, local)| self.get_local_type(*local).clone())
            .collect();
        let capture_args: Vec<Local> = free_vars.iter().map(|(_, local)| *local).collect();

        let capture_arity = capture_types.len();
        let mut async_fn = MirFunction::new(async_block_name.clone(), capture_types, MIR_UNIT);
        async_fn.is_async = true;
        let async_start = async_fn.start_block;

        let mut async_ctx = LoweringContext::new(
            &mut async_fn,
            self.lambda_counter,
            &self.known_functions,
            &self.function_sigs,
            self.struct_defs,
            self.concrete_type_registry.clone(),
            self.options.clone(),
            self.inherent_method_templates,
            self.trait_method_templates,
        );
        async_ctx.current_block = Some(async_start);

        for (index, (var_name, outer_local)) in free_vars.iter().enumerate() {
            let param_local = Local::new(index + 1, LocalKind::Param);
            async_ctx.local_names.insert(var_name.clone(), param_local);
            if let Some(type_name) = self.type_names.get(outer_local).cloned() {
                async_ctx.type_names.insert(param_local, type_name);
            }
            if let Some(origin) = self.future_origins.get(outer_local).cloned() {
                async_ctx.future_origins.insert(param_local, origin);
            }
        }

        let result_local = async_ctx.lower_body_to_block_val(body, async_start);
        let result_ty = async_ctx.get_local_type(result_local).clone();
        async_ctx.mir_fn.return_type = result_ty.clone();
        if let Some((_, slot_ty)) = async_ctx.mir_fn.locals.get_mut(0) {
            *slot_ty = result_ty.clone();
        }

        let cur = async_ctx.current_block();
        let already_terminated = async_ctx
            .mir_fn
            .block_mut(cur)
            .is_some_and(|block| block.terminator.is_some());
        if !already_terminated {
            if matches!(result_ty, MIRType::Unit) {
                async_ctx.set_terminator(Terminator::Return(None));
            } else {
                async_ctx.set_terminator(Terminator::Return(Some(result_local)));
            }
        }

        let async_errors = std::mem::take(&mut async_ctx.errors);
        let nested_functions = std::mem::take(&mut async_ctx.lambda_functions);
        drop(async_ctx);

        if !async_errors.is_empty() {
            self.errors.push(format!(
                "async block lowering failed for '{}':\n  {}",
                async_block_name,
                async_errors.join("\n  ")
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        self.known_functions.insert(async_block_name.clone());
        self.options
            .async_functions
            .borrow_mut()
            .insert(async_block_name.clone());
        self.function_sigs.insert(
            async_block_name.clone(),
            build_function_sig(result_ty.clone(), capture_arity, vec![]),
        );

        self.lambda_functions.push(async_fn);
        self.lambda_functions.extend(nested_functions);

        let future_local = self.add_local(None, LocalKind::Temp, result_ty);
        self.push_inst(Instruction::Call {
            destination: future_local,
            func: format!("{}__start", async_block_name),
            args: capture_args,
        });
        self.future_origins.insert(future_local, async_block_name);
        future_local
    }

    /// 弹出当前循环的break/continue目标。
    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    /// 获取当前循环的break目标块索引。
    fn get_break_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.break_block)
    }

    /// 获取当前循环的continue目标块索引。
    fn get_continue_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.continue_block)
    }

    /// 添加一个新的局部变量并返回其Local句柄。
    fn add_local(&mut self, name: Option<String>, kind: LocalKind, ty: MIRType) -> Local {
        let local = self.mir_fn.add_local(kind, ty);
        if let Some(name) = name {
            self.local_names.insert(name, local);
        }
        local
    }

    fn bind_local_symbol(&mut self, symbol: SymbolId, local: Local) {
        if symbol.is_valid() {
            self.local_symbols.insert(symbol, local);
        }
    }

    /// 获取局部变量的MIR类型。
    fn get_local_type(&self, local: Local) -> &MIRType {
        if let Some((_, ty)) = self.mir_fn.locals.get(local.index()) {
            ty
        } else {
            &MIR_UNIT
        }
    }

    /// 获取局部变量的类型信息。
    /// 解析名称和符号ID对应的局部变量，或创建新的局部变量。
    fn resolve_local(&mut self, name: &str, symbol: SymbolId) -> Local {
        if symbol.is_valid() {
            if let Some(&local) = self.local_symbols.get(&symbol) {
                return local;
            }
        }
        match self.local_names.get(name) {
            Some(&local) => local,
            None => {
                // 变量未定义时报告错误并返回临时变量。
                self.errors.push(format!("undefined variable: '{}'", name));
                // 错误处理：返回一个unit类型的临时local。
                self.mir_fn.add_local(LocalKind::Temp, MIR_UNIT)
            }
        }
    }

    /// 创建一个新的基本块并返回其索引。
    fn new_block(&mut self) -> usize {
        self.mir_fn.add_block()
    }

    /// 设置当前基本块为指定块。
    fn set_current_block(&mut self, block: usize) {
        self.current_block = Some(block);
    }

    fn current_block_or_error(&mut self, context: &str) -> usize {
        match self.current_block {
            Some(block) => block,
            None => {
                self.errors.push(format!(
                    "internal MIR lowering error: no current block set while {context}"
                ));
                self.mir_fn.start_block
            }
        }
    }

    /// 返回当前正在生成的基本块索引。
    fn current_block(&self) -> usize {
        debug_assert!(self.current_block.is_some(), "no current block set");
        self.current_block.unwrap_or(self.mir_fn.start_block)
    }

    fn propagate_future_origin_through_phi(
        &mut self,
        destination: Local,
        incoming: &[(Local, usize)],
    ) {
        if !matches!(self.get_local_type(destination), MIRType::Future(_)) {
            return;
        }

        let mut resolved = Vec::with_capacity(incoming.len());
        for (local, _) in incoming {
            let Some(origin) = self.future_origins.get(local).cloned() else {
                return;
            };
            resolved.push(origin);
        }

        let Some(first) = resolved.first().cloned() else {
            return;
        };
        if resolved.iter().all(|origin| origin == &first) {
            self.future_origins.insert(destination, first);
        }
    }

    /// Check if two types are compatible for binary operations and, if not,
    /// try to insert Cast instructions to reconcile them.  Returns the
    /// (possibly cast) left and right locals whose types now match, or pushes
    /// an error and returns the originals unchanged.
    fn reconcile_binary_operand_types(&mut self, left: Local, right: Local) -> (Local, Local) {
        let left_ty = self.get_local_type(left).clone();
        let right_ty = self.get_local_type(right).clone();

        // 若两侧类型已经相同，无需调和。
        if left_ty == right_ty {
            return (left, right);
        }

        // Determine if a cast between two types is valid and, if so,
        // which direction to cast (returns the common target type).
        match (&left_ty, &right_ty) {
            // 对整数和浮点数类型进行隐式类型提升（宽化）。
            (MIRType::Int(a), MIRType::Int(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::Int(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }

            // 两个浮点数操作数：选择较大位宽的类型。
            (MIRType::Float(a), MIRType::Float(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::Float(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }

            // 整数与浮点数混合：将整数转为浮点数。
            (MIRType::Int(_), MIRType::Float(b)) => {
                let target_ty = MIRType::Float(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Float(a), MIRType::Int(_)) => {
                let target_ty = MIRType::Float(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }

            // 布尔与整数混合：将bool转为对应位宽的整数。
            (MIRType::Bool, MIRType::Int(b)) => {
                let target_ty = MIRType::Int(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Int(a), MIRType::Bool) => {
                let target_ty = MIRType::Int(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }

            // 其他类型组合：无需自动转换，直接返回左侧类型。
            _ => {
                self.errors.push(format!(
                    "type mismatch in binary operation: left operand has type {:?}, right operand has type {:?}",
                    left_ty, right_ty
                ));
                (left, right)
            }
        }
    }

    /// Insert a Cast instruction that converts `source` to `target_ty`,
    /// returning the new local that holds the cast result.
    fn insert_cast(&mut self, source: Local, target_ty: MIRType) -> Local {
        let dest = self.add_local(None, LocalKind::Temp, target_ty.clone());
        self.push_inst(Instruction::Cast {
            destination: dest,
            value: source,
            to: target_ty,
        });
        dest
    }

    /// 向当前基本块追加一条MIR指令。
    fn push_inst(&mut self, inst: Instruction) {
        let block_id = self.current_block_or_error("emitting MIR instruction");
        self.mir_fn.push_inst_to_block(block_id, inst);
    }

    /// 向当前基本块追加terminator终止指令。
    fn set_terminator(&mut self, term: Terminator) {
        let block_id = self.current_block_or_error("emitting MIR terminator");
        if let Some(block) = self.mir_fn.block_mut(block_id) {
            block.set_terminator(term);
        }
    }

    fn inject_precondition_check(&mut self, precondition: &HIRExpr, entry_block: usize) -> usize {
        self.set_current_block(entry_block);
        let cond_local = self.lower_contract_condition(precondition, None);
        let pass_block = self.new_block();
        let fail_block = self.new_block();
        self.set_terminator(Terminator::If {
            cond: cond_local,
            then_block: pass_block,
            else_block: fail_block,
        });
        self.set_current_block(fail_block);
        self.set_terminator(Terminator::Unreachable);
        pass_block
    }

    fn inject_postcondition_checks(&mut self, postcondition: &HIRExpr) {
        let return_sites = self
            .mir_fn
            .basic_blocks
            .iter()
            .enumerate()
            .filter_map(|(block_id, block)| match &block.terminator {
                Some(Terminator::Return(value)) => Some((block_id, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();

        for (return_block, return_value) in return_sites {
            let Some(return_local) = return_value else {
                continue;
            };

            let check_block = self.new_block();
            let success_block = self.new_block();
            let fail_block = self.new_block();

            if let Some(block) = self.mir_fn.block_mut(return_block) {
                block.set_terminator(Terminator::Goto(check_block));
            }

            self.set_current_block(check_block);
            let cond_local = self.lower_contract_condition(postcondition, Some(return_local));
            self.set_terminator(Terminator::If {
                cond: cond_local,
                then_block: success_block,
                else_block: fail_block,
            });

            self.set_current_block(success_block);
            self.set_terminator(Terminator::Return(Some(return_local)));

            self.set_current_block(fail_block);
            self.set_terminator(Terminator::Unreachable);
        }
    }

    fn lower_contract_condition(
        &mut self,
        condition: &HIRExpr,
        result_local: Option<Local>,
    ) -> Local {
        let mut saved_name_bindings = Vec::<(String, Option<Local>)>::new();
        let mut saved_symbol_bindings = Vec::<(SymbolId, Option<Local>)>::new();

        for (name, symbol, local) in &self.contract_param_bindings {
            let previous_name = self.local_names.insert(name.clone(), *local);
            saved_name_bindings.push((name.clone(), previous_name));
            if symbol.is_valid() {
                let previous_symbol = self.local_symbols.insert(*symbol, *local);
                saved_symbol_bindings.push((*symbol, previous_symbol));
            }
        }

        if let Some(result_local) = result_local {
            let result_name = "result".to_string();
            let previous_result_name = self.local_names.insert(result_name.clone(), result_local);
            saved_name_bindings.push((result_name, previous_result_name));

            let mut result_symbols = Vec::new();
            collect_named_symbols(condition, "result", &mut result_symbols);
            for symbol in result_symbols {
                if symbol.is_valid() {
                    let previous_symbol = self.local_symbols.insert(symbol, result_local);
                    saved_symbol_bindings.push((symbol, previous_symbol));
                }
            }
        }

        let cond_local = self.lower_expr(condition);

        for (symbol, previous) in saved_symbol_bindings.into_iter().rev() {
            if let Some(local) = previous {
                self.local_symbols.insert(symbol, local);
            } else {
                self.local_symbols.remove(&symbol);
            }
        }
        for (name, previous) in saved_name_bindings.into_iter().rev() {
            if let Some(local) = previous {
                self.local_names.insert(name, local);
            } else {
                self.local_names.remove(&name);
            }
        }

        cond_local
    }

    /// 将HIR函数体降级为基本块（不计算返回值）。
    fn lower_body_to_block(&mut self, body: &HIRBody, target_block: usize) {
        self.lower_body_to_block_with_return(body, target_block, true);
    }

    /// 将HIR函数体降级为基本块，计算块值（返回最后一个表达式）。
    fn lower_body_to_block_val(&mut self, body: &HIRBody, target_block: usize) -> Local {
        self.set_current_block(target_block);

        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }

        if let Some(expr) = &body.expr {
            self.lower_expr(expr)
        } else {
            self.add_local(None, LocalKind::Temp, MIR_UNIT)
        }
    }

    /// 将HIR函数体降级为基本块，并在末尾插入return指令。
    fn lower_body_to_block_with_return(
        &mut self,
        body: &HIRBody,
        target_block: usize,
        add_return: bool,
    ) {
        self.set_current_block(target_block);

        // 降级函数体的所有语句到当前基本块。
        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }

        // 若块尾存在表达式，则先降级该表达式并视情况插入 return。
        if let Some(expr) = &body.expr {
            let result_local = self.lower_expr(expr);
            if add_return {
                // Only add return if the current block doesn't already have a
                // terminator (e.g. set by break/continue/return inside the expr).
                let cur = self.current_block();
                let already_terminated = self
                    .mir_fn
                    .block_mut(cur)
                    .is_some_and(|b| b.terminator.is_some());
                if !already_terminated {
                    // 为函数体末尾生成隐式return指令。
                    // 检查是否为main函数的隐式返回情况。
                    let is_main_with_unit_body = self.mir_fn.name == "main"
                        && matches!(self.mir_fn.return_type, MIRType::Int(_))
                        && matches!(*self.get_local_type(result_local), MIRType::Unit);

                    if is_main_with_unit_body {
                        self.set_terminator(Terminator::Return(None));
                    } else {
                        self.set_terminator(Terminator::Return(Some(result_local)));
                    }
                }
            }
        // 若需要添加return终止符则插入return指令。
        } else if add_return {
            // 当需要添加return且最后一个块未终止时，插入return指令。
            // Only set return if the current block doesn't already have a
            // terminator (e.g. set by break/continue/return in a statement).
            let cur = self.current_block();
            let already_terminated = self
                .mir_fn
                .block_mut(cur)
                .is_some_and(|b| b.terminator.is_some());
            if !already_terminated {
                self.set_terminator(Terminator::Return(None));
            }
        }
    }

    /// 将单条HIR语句降级为MIR指令序列。
    fn lower_stmt(&mut self, stmt: &HIRStmt) {
        match stmt {
            HIRStmt::Let {
                name,
                symbol,
                ty,
                value,
                is_mut,
            } => lower_let_stmt(self, name, *symbol, ty, value.as_ref(), *is_mut),
            HIRStmt::Expr(expr) => {
                self.lower_expr(expr);
            }
            HIRStmt::Item => {}
        }
    }

    /// 生成运行时打印调用的指令（用于调试输出）。
    fn emit_runtime_print_call(&mut self, func: &str, arg_local: Local) {
        let call_local = self.add_local(None, LocalKind::Temp, MIR_UNIT);
        self.push_inst(Instruction::Call {
            destination: call_local,
            func: func.to_string(),
            args: vec![arg_local],
        });
    }

    fn emit_print_str_literal(&mut self, text: &str) {
        let str_local = self.lower_literal(&HIRLiteral::String(text.to_string()));
        self.emit_runtime_print_call("sengoo_print_str", str_local);
    }

    fn emit_print_value(&mut self, value_local: Local, value_ty: &MIRType) {
        match value_ty {
            MIRType::Struct { name, fields } => {
                self.emit_print_str_literal(&format!("{} {{ ", name));

                for (index, (field_name, field_ty)) in fields.iter().enumerate() {
                    if index > 0 {
                        self.emit_print_str_literal(", ");
                    }
                    self.emit_print_str_literal(&format!("{}: ", field_name));

                    let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
                    self.push_inst(Instruction::Extract {
                        destination: field_local,
                        value: value_local,
                        index: index as u32,
                    });

                    self.emit_print_value(field_local, field_ty);
                }

                self.emit_print_str_literal(" }");
            }
            MIRType::Int(_) => self.emit_runtime_print_call("sengoo_print_i64", value_local),
            MIRType::Bool => self.emit_runtime_print_call("sengoo_print_bool", value_local),
            MIRType::Float(_) => self.emit_runtime_print_call("sengoo_print_f64", value_local),
            MIRType::Ptr(_) | MIRType::Ref(_) => {
                self.emit_runtime_print_call("sengoo_print_str", value_local)
            }
            _ => {
                self.errors.push(format!(
                    "print: unsupported MIR type for lowering: {:?}",
                    value_ty
                ));
            }
        }
    }

    fn lower_expr(&mut self, expr: &HIRExpr) -> Local {
        match expr {
            HIRExpr::Lit(lit) => self.lower_literal(lit),
            HIRExpr::Var { name, symbol } => self.resolve_local(name, *symbol),
            HIRExpr::Unary(op, operand) => lower_unary_expr(self, op, operand),
            HIRExpr::Binary(op, left, right) => lower_binary_expr(self, op, left, right),
            HIRExpr::Block(body) => lower_block_expr(self, body),
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => lower_if_expr(self, cond, then_branch, else_branch.as_deref()),
            HIRExpr::Loop(body) => lower_loop_expr(self, body),
            HIRExpr::While { cond, body } => lower_while_expr(self, cond, body),
            HIRExpr::For {
                var_name,
                iter,
                body,
                ..
            } => lower_for_expr(self, var_name, iter, body),
            HIRExpr::Call { func, args } => lower_call_expr(self, func, args),
            HIRExpr::And(left, right) => lower_logical_and_expr(self, left, right),
            HIRExpr::Or(left, right) => lower_logical_or_expr(self, left, right),
            HIRExpr::Break(value) => lower_break_expr(self, value.as_deref()),
            HIRExpr::Continue => lower_continue_expr(self),
            HIRExpr::Assign { target, value } => lower_assign_expr(self, target, value),
            HIRExpr::AssignOp { target, op, value } => {
                lower_assign_op_expr(self, target, op, value)
            }
            HIRExpr::Array(elems) => lower_array_expr(self, elems),
            HIRExpr::Index { base, index } => lower_index_expr(self, base, index),
            HIRExpr::Struct { name, fields } => lower_struct_expr(self, name, fields),

            HIRExpr::Field { base, field } => {
                let base_local = self.lower_expr(base);
                lower_field_expr(self, base_local, field)
            }
            HIRExpr::Ref(_is_mut, expr) => lower_ref_expr(self, expr),
            HIRExpr::Deref(expr) => lower_deref_expr(self, expr),
            HIRExpr::Lambda { params, body } => lower_lambda_expr(self, params, body),
            HIRExpr::Match { scrutinee, arms } => lower_match_expr(self, scrutinee, arms),
            HIRExpr::MethodCall {
                receiver,
                method,
                args,
            } => lower_method_call_expr(self, receiver, method, args),
            HIRExpr::Await(inner) => lower_await_expr(self, inner),
            HIRExpr::AsyncBlock(body) => lower_async_block_expr(self, body),
            _ => self.add_local(None, LocalKind::Temp, MIR_UNIT),
        }
    }

    fn infer_poll_func_from_last_call(&self) -> String {
        let block = &self.mir_fn.basic_blocks[self.current_block()];
        let instructions = block
            .instructions
            .iter()
            .map(|inst_id| self.mir_fn.instruction(*inst_id));
        infer_last_async_start_base(instructions).unwrap_or_else(|| "unknown".to_string())
    }

    /// Resolve the async function base name for a given future handle local.
    ///
    /// Resolution order:
    ///  1. Direct lookup in `future_origins` — covers `await async_fn(args)`.
    ///  2. If the handle came from a `Load { destination: handle, source: src }`,
    ///     look up `src` in `future_origins` — covers `let f = async_fn(); await f`.
    ///  3. Fall back to backward-scan heuristic via `infer_poll_func_from_last_call`.
    fn resolve_async_base_name(&self, handle: Local) -> String {
        let block = &self.mir_fn.basic_blocks[self.current_block()];
        let instructions = block
            .instructions
            .iter()
            .map(|inst_id| self.mir_fn.instruction(*inst_id));

        infer_async_base_name_from_instructions(handle, instructions, &self.future_origins)
            .unwrap_or_else(|| self.infer_poll_func_from_last_call())
    }

    /// 从枚举模式中提取判别值（discriminant）并生成匹配判断逻辑。
    /// 判断给定值是否匹配 HIR 模式，用于运行时合约检查。
    fn matches_pattern(&mut self, pat: &crate::hir::HIRPattern, value: Local) -> Local {
        let result = self.add_local(None, LocalKind::Temp, MIR_BOOL);

        match pattern_match_plan(pat) {
            PatternMatchPlan::AlwaysTrue => {
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
            PatternMatchPlan::EqLiteral(lit) => {
                let lit_local = self.lower_literal(&lit);
                self.push_inst(Instruction::Binary {
                    destination: result,
                    op: MirBinOp::Eq,
                    left: value,
                    right: lit_local,
                });
                result
            }
        }
    }

    /// 将HIR模式绑定降级为MIR，生成对应的局部变量绑定指令。
    /// 将模式绑定降级到MIR，生成模式匹配的局部变量绑定指令。
    fn lower_pattern_bindings(&mut self, pat: &crate::hir::HIRPattern, enum_value: Local) {
        match pattern_binding_plan(pat) {
            PatternBindingPlan::Ignore => {}
            PatternBindingPlan::BindWhole(name) => {
                let _ = self.add_local(Some(name), LocalKind::User, MIR_I64);
            }
            PatternBindingPlan::BindTupleFields(fields) => {
                let payload_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                self.push_inst(Instruction::ExtractPayload {
                    destination: payload_local,
                    source: enum_value,
                });
                for (index, name) in fields {
                    let field_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                    self.push_inst(Instruction::Extract {
                        destination: field_local,
                        value: payload_local,
                        index,
                    });
                    let bound_local = self.add_local(Some(name), LocalKind::User, MIR_I64);
                    self.push_inst(Instruction::Store {
                        destination: bound_local,
                        value: field_local,
                    });
                }
            }
        }
    }

    /// 将HIR字面量降级为MIR常量指令。
    fn lower_literal(&mut self, lit: &HIRLiteral) -> Local {
        let constant = match lit {
            HIRLiteral::Int(n) => MirConstant::Int(*n),
            HIRLiteral::Float(f) => MirConstant::Float(*f),
            HIRLiteral::String(s) => MirConstant::String(s.clone()),
            HIRLiteral::Bool(b) => MirConstant::Bool(*b),
            HIRLiteral::Char(c) => MirConstant::Char(*c),
            HIRLiteral::Null => MirConstant::Unit,
            HIRLiteral::Bytes(b) => MirConstant::Bytes(b.clone()),
            HIRLiteral::Uint(u) => MirConstant::Uint(*u),
        };
        let ty = constant.ty();
        let local = self.add_local(None, LocalKind::Temp, ty);
        self.push_inst(Instruction::Assign {
            destination: local,
            value: constant,
        });
        local
    }

    /// 将HIR一元运算符转换为MIR一元运算符。
    fn lower_un_op(&self, op: &hir::HIRUnaryOp) -> MirUnOp {
        match op {
            hir::HIRUnaryOp::Neg => MirUnOp::Neg,
            hir::HIRUnaryOp::Not => MirUnOp::Not,
            hir::HIRUnaryOp::BitNot => MirUnOp::BitNot,
            hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut | hir::HIRUnaryOp::Deref => MirUnOp::Neg,
        }
    }

    /// 将HIR二元运算符转换为MIR二元运算符。
    fn lower_bin_op(&self, op: &hir::HIRBinaryOp) -> MirBinOp {
        match op {
            hir::HIRBinaryOp::Add => MirBinOp::Add,
            hir::HIRBinaryOp::Sub => MirBinOp::Sub,
            hir::HIRBinaryOp::Mul => MirBinOp::Mul,
            hir::HIRBinaryOp::Div => MirBinOp::Div,
            hir::HIRBinaryOp::Mod => MirBinOp::Rem,
            hir::HIRBinaryOp::BitAnd => MirBinOp::BitAnd,
            hir::HIRBinaryOp::BitOr => MirBinOp::BitOr,
            hir::HIRBinaryOp::BitXor => MirBinOp::BitXor,
            hir::HIRBinaryOp::Shl => MirBinOp::Shl,
            hir::HIRBinaryOp::Shr => MirBinOp::Shr,
            hir::HIRBinaryOp::LogAnd => MirBinOp::LogAnd,
            hir::HIRBinaryOp::LogOr => MirBinOp::LogOr,
            hir::HIRBinaryOp::Eq => MirBinOp::Eq,
            hir::HIRBinaryOp::NotEq => MirBinOp::Ne,
            hir::HIRBinaryOp::Lt => MirBinOp::Lt,
            hir::HIRBinaryOp::Gt => MirBinOp::Gt,
            hir::HIRBinaryOp::Le => MirBinOp::Le,
            hir::HIRBinaryOp::Ge => MirBinOp::Ge,
            hir::HIRBinaryOp::Assign => MirBinOp::Add,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn set_terminator_without_current_block_records_error_instead_of_panicking() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );

        let result = catch_unwind(AssertUnwindSafe(|| {
            ctx.set_terminator(Terminator::Return(None));
        }));

        assert!(
            result.is_ok(),
            "set_terminator should not panic without a current block"
        );
        assert!(
            ctx.errors
                .iter()
                .any(|err| err.contains("no current block set while emitting MIR terminator")),
            "expected lowering error to be recorded, got {:?}",
            ctx.errors
        );
    }
}
