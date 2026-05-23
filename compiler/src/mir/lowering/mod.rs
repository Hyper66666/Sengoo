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
mod async_methods;
mod block_async_expr_helpers;
mod block_state_methods;
mod body_lowering_helpers;
mod body_dispatch_methods;
mod builtin_helpers;
mod call_emission_helpers;
mod call_expr_helpers;
mod call_invocation_helpers;
mod call_target_helpers;
mod contract_methods;
mod context_methods;
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
    /// 返回当前上下文的指令列表的可变引用。
    /// 根据参数列表收集表达式中的自由变量及其对应 `Local`。
    fn collect_free_vars(
        &self,
        params: &[String],
        body: &crate::hir::HIRExpr,
    ) -> Vec<(String, Local)> {
        collect_free_vars(body, params, &self.local_names)
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

