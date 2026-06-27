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
use crate::mir::async_dispatch_helpers::{
    build_async_dispatch_registry_with_extras, AsyncDispatchRegistry, OPTIONAL_ASYNC_DISPATCH_NAMES,
};
use crate::mir::async_origin_helpers::{
    infer_async_base_name_from_instructions, infer_last_async_start_base,
};
use crate::mir::build_enum_defs;
use crate::mir::concrete_type_helpers::collect_concrete_named_types_with_impl_variants;
use crate::mir::function_sig_helpers::{build_function_sig, build_hir_function_sig_with_enums};
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
    bind_mir_subst_from_hir_type, hir_type_to_mir_with_structs_and_subst,
};
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirBinOp, MirConstant, MirFunction, MirUnOp,
    Terminator, MIR_BOOL, MIR_I64, MIR_UNIT,
};
use crate::symbol::SymbolId;
use crate::type_naming::mir_type_instance_name as mir_type_to_instance_name;
use std::collections::{HashMap, HashSet};

mod aggregate_expr_helpers;
mod assert_callsite_helpers;
mod assignment_helpers;
mod async_methods;
mod block_async_expr_helpers;
mod block_state_methods;
mod body_dispatch_methods;
mod builtin_helpers;
mod call_emission_helpers;
mod call_expr_helpers;
mod call_invocation_helpers;
mod call_target_helpers;
mod context_methods;
mod contract_methods;
mod drop_glue_helpers;
mod entry;
mod enum_expr_helpers;
mod for_expr_helpers;
mod function_lowering;
mod if_expr_helpers;
mod lambda_expr_helpers;
mod let_stmt_helpers;
mod literal_op_methods;
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
mod pattern_methods;
mod pointer_expr_helpers;
mod print_methods;
mod return_expr_helpers;
mod try_expr_helpers;
mod while_expr_helpers;
use self::aggregate_expr_helpers::{
    lower_array_expr, lower_field_expr, lower_index_expr, lower_struct_expr, lower_tuple_expr,
};
use self::assignment_helpers::{lower_assign_expr, lower_assign_op_expr};
use self::block_async_expr_helpers::{lower_async_block_expr, lower_await_expr, lower_block_expr};
use self::call_emission_helpers::emit_call_from_plan;
use self::call_expr_helpers::lower_call_expr;
use self::call_invocation_helpers::build_call_invocation_plan;
use self::call_target_helpers::{CallTargetPlan, CallTargetResolution};
use self::enum_expr_helpers::lower_enum_construct_expr;
use self::for_expr_helpers::lower_for_expr;
use self::function_lowering::lower_function;
use self::if_expr_helpers::lower_if_expr;
use self::lambda_expr_helpers::{lower_lambda_expr, lower_lambda_expr_with_expected};
use self::let_stmt_helpers::lower_let_stmt;
use self::loop_control_helpers::{lower_break_expr, lower_continue_expr};
use self::loop_expr_helpers::lower_loop_expr;
use self::match_expr_helpers::lower_match_expr;
use self::method_expr_helpers::lower_method_call_expr;
use self::op_expr_helpers::{
    lower_binary_expr, lower_logical_and_expr, lower_logical_or_expr, lower_unary_expr,
};
use self::pointer_expr_helpers::{lower_deref_expr, lower_ref_expr};
use self::return_expr_helpers::lower_return_expr;
use self::try_expr_helpers::{lower_try_block_expr, lower_try_expr};
use self::while_expr_helpers::lower_while_expr;
pub use entry::{lower_hir, lower_hir_with_options};
pub use options::{AssertCallsiteContext, MirLowerOptions};

/// 循环上下文，记录 `break/continue` 目标基本块。
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    /// break目标基本块的索引。
    break_block: usize,
    /// continue目标基本块的索引。
    continue_block: usize,
    drop_scope_depth: usize,
}

/// 函数签名信息，存储函数名、参数数量和参数类型。
#[derive(Debug, Clone)]
struct DropBinding {
    local: Local,
    field_path: Vec<u32>,
    drop_func: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FunctionSig {
    pub(crate) ret_type: MIRType,
    pub(crate) param_count: usize,
    /// 函数参数数量（不含环境指针参数）。
    #[allow(dead_code)]
    pub(crate) env: Vec<(String, MIRType)>,
}

fn http_server_request_mir_type() -> MIRType {
    MIRType::Struct {
        name: "HttpServerRequest".to_string(),
        fields: vec![("handle".to_string(), MIR_I64)],
    }
}

fn http_server_next_request_outcome_mir_type() -> MIRType {
    MIRType::Struct {
        name: "HttpServerNextRequestOutcome".to_string(),
        fields: vec![
            ("is_ok".to_string(), MIR_BOOL),
            ("value".to_string(), http_server_request_mir_type()),
            ("error".to_string(), MIR_I64),
        ],
    }
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
    /// 会话级共享的函数签名基表（只读借用，永不克隆）。
    ///
    /// 函数签名表拆分为 `base`（会话级不可变基表）+ `overlay`（本上下文私有覆盖表）。
    /// 常规函数降级只读借用基表；插入单态化/lambda/async 实例时只写入 `overlay`（O(1)）。
    /// 嵌套上下文（lambda/async/单态化）继承时只克隆体量很小的 `overlay`，绝不整表克隆
    /// 基表，因此即便在 lambda/async/generic 密集场景下降级仍保持 O(n)。
    function_sigs_base: &'a HashMap<String, FunctionSig>,
    /// 本上下文私有的函数签名覆盖表（仅含本次降级新产生的实例）。
    function_sigs_overlay: HashMap<String, FunctionSig>,
    /// lambda环境信息表，按名称索引。
    lambda_environments: HashMap<String, LambdaEnv>,
    /// 局部变量名与MIR类型的映射表。
    type_names: HashMap<Local, String>,
    /// 已知函数名基集合（只读借用，永不克隆）。拆分约定同 `function_sigs_base`。
    known_functions_base: &'a HashSet<String>,
    /// 本上下文私有的已知函数名覆盖集合（仅含本次降级新产生的名字）。
    known_functions_overlay: HashSet<String>,
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
    try_scope_stack: Vec<try_expr_helpers::TryScope>,
    drop_bindings: Vec<DropBinding>,
    drop_scope_markers: Vec<usize>,
    moved_drop_locals: HashSet<Local>,
    moved_drop_fields: HashSet<(Local, Vec<u32>)>,
}

impl<'a> LoweringContext<'a> {
    /// 查找函数签名：先看本上下文 `overlay`，再回落到会话级基表。
    fn function_sig(&self, name: &str) -> Option<&FunctionSig> {
        self.function_sigs_overlay
            .get(name)
            .or_else(|| self.function_sigs_base.get(name))
    }

    /// 是否为已知函数：`overlay` 或基集合命中即可。
    fn is_known_function(&self, name: &str) -> bool {
        self.known_functions_overlay.contains(name) || self.known_functions_base.contains(name)
    }

    /// 遍历全部已知函数名（基集合 + `overlay`，两者互不相交）。
    fn known_function_names(&self) -> impl Iterator<Item = &String> {
        self.known_functions_base
            .iter()
            .chain(self.known_functions_overlay.iter())
    }

    /// 向本上下文 `overlay` 登记一个函数签名（O(1)，绝不克隆基表）。
    fn insert_function_sig(&mut self, name: String, sig: FunctionSig) {
        self.function_sigs_overlay.insert(name, sig);
    }

    /// 向本上下文 `overlay` 登记一个已知函数名（O(1)）。
    fn insert_known_function(&mut self, name: String) {
        self.known_functions_overlay.insert(name);
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
}
