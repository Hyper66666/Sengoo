//! HIR �?MIR 的转�?

use crate::hir::{
    self, HIRBody, HIRExpr, HIRItem, HIRLiteral, HIRParam, HIRStmt, HIRType, HIRTypeKind,
};
use crate::hir::{HIRTrait, HIRTraitItem};
use crate::mir::{
    bb::CallArg, BasicBlock, Instruction, Local, LocalKind, MIRType, MirBinOp, MirConstant,
    MirFunction, MirUnOp, Terminator, MIR_BOOL, MIR_I64, MIR_UNIT,
};
use std::collections::{HashMap, HashSet};

/// �?HIRType 转换为类型前缀字符串（用于方法名修饰）
fn hir_type_to_prefix(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Int(ik) => format!("i{}", ik.bits()),
        HIRTypeKind::Float(fk) => format!("f{}", fk.bits()),
        HIRTypeKind::Bool => "bool".to_string(),
        HIRTypeKind::Unit => "unit".to_string(),
        HIRTypeKind::Named { name, .. } => name.clone(),
        _ => "unknown".to_string(),
    }
}

/// �?HIR 模块转换�?MIR 函数集合
pub fn lower_hir(items: &[HIRItem]) -> Result<Vec<MirFunction>, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut lambda_counter = 0;

    // Collect trait definitions for default method resolution
    let mut trait_defs: HashMap<String, &HIRTrait> = HashMap::new();
    for item in items {
        if let HIRItem::Trait(trait_item) = item {
            trait_defs.insert(trait_item.name.clone(), trait_item);
        }
    }

    // First pass: collect all known function names (top-level functions and impl methods)
    let mut known_functions: HashSet<String> = HashSet::new();
    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                known_functions.insert(fn_item.name.clone());
            }
            HIRItem::Impl(impl_item) => {
                let type_prefix = hir_type_to_prefix(&impl_item.target_type);
                if let Some(trait_name) = &impl_item.trait_name {
                    // Collect method names that are explicitly implemented
                    let mut impl_method_names: HashSet<String> = HashSet::new();
                    for method in &impl_item.items {
                        let original_method_name = method
                            .name
                            .strip_prefix(&format!("{}_", type_prefix))
                            .unwrap_or(&method.name);
                        impl_method_names.insert(original_method_name.to_string());
                        let three_part_name =
                            format!("{}_{}_{}", type_prefix, trait_name, original_method_name);
                        known_functions.insert(three_part_name);
                    }

                    // Also register default methods from the trait definition
                    // that are not overridden by the impl
                    if let Some(trait_def) = trait_defs.get(trait_name.as_str()) {
                        for trait_item in &trait_def.items {
                            if let HIRTraitItem::Function(trait_fn) = trait_item {
                                if !impl_method_names.contains(&trait_fn.name) {
                                    // This trait method has a default implementation
                                    // and is not overridden �?register it
                                    let three_part_name =
                                        format!("{}_{}_{}", type_prefix, trait_name, trait_fn.name);
                                    known_functions.insert(three_part_name);
                                }
                            }
                        }
                    }
                } else {
                    for method in &impl_item.items {
                        // Inherent impl: method names are already mangled as TypePrefix_MethodName by HIR lowering
                        known_functions.insert(method.name.clone());
                    }
                }
            }
            _ => {}
        }
    }

    // Second pass: lower all items
    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                match lower_function(fn_item, &mut lambda_counter, &known_functions) {
                    Ok((mir_fn, lambdas)) => {
                        results.push(mir_fn);
                        results.extend(lambdas);
                    }
                    Err(e) => errors.push(e),
                }
            }
            HIRItem::Impl(impl_item) => {
                let type_prefix = hir_type_to_prefix(&impl_item.target_type);
                // 处理 impl 块中的方�?
                let mut impl_method_names: HashSet<String> = HashSet::new();
                for method in &impl_item.items {
                    if let Some(trait_name) = &impl_item.trait_name {
                        // Trait impl: rename method to three-part mangled name
                        // {TypePrefix}_{TraitName}_{MethodName}
                        let original_method_name = method
                            .name
                            .strip_prefix(&format!("{}_", type_prefix))
                            .unwrap_or(&method.name);
                        impl_method_names.insert(original_method_name.to_string());
                        let three_part_name =
                            format!("{}_{}_{}", type_prefix, trait_name, original_method_name);
                        let mut renamed_method = method.clone();
                        renamed_method.name = three_part_name;
                        match lower_function(&renamed_method, &mut lambda_counter, &known_functions)
                        {
                            Ok((mir_fn, lambdas)) => {
                                results.push(mir_fn);
                                results.extend(lambdas);
                            }
                            Err(e) => errors.push(e),
                        }
                    } else {
                        // Inherent impl: use existing two-part mangled name
                        match lower_function(method, &mut lambda_counter, &known_functions) {
                            Ok((mir_fn, lambdas)) => {
                                results.push(mir_fn);
                                results.extend(lambdas);
                            }
                            Err(e) => errors.push(e),
                        }
                    }
                }

                // Handle default trait method implementations:
                // For trait impls, check if any trait methods are missing from the impl
                // and generate wrapper functions for default implementations.
                if let Some(trait_name) = &impl_item.trait_name {
                    if let Some(trait_def) = trait_defs.get(trait_name.as_str()) {
                        for trait_item in &trait_def.items {
                            if let HIRTraitItem::Function(trait_fn) = trait_item {
                                if !impl_method_names.contains(&trait_fn.name) {
                                    // This method was not overridden �?use the default implementation.
                                    // Create a new HIRFunction with:
                                    // - Three-part mangled name: {TypePrefix}_{TraitName}_{MethodName}
                                    // - self parameter added with the impl target type
                                    // - The default body from the trait definition
                                    let three_part_name =
                                        format!("{}_{}_{}", type_prefix, trait_name, trait_fn.name);

                                    // Build the parameter list: add self as first param if the
                                    // trait method has a self parameter (check if any param is named "self"
                                    // or if the original AST had a self_param).
                                    // Since trait methods lowered via lower_function (without self_type)
                                    // don't get a self param, we need to add it ourselves.
                                    let mut params = Vec::new();
                                    let has_self = trait_fn.params.iter().any(|p| p.name == "self");
                                    if !has_self {
                                        // The trait method likely takes self but it wasn't added
                                        // during HIR lowering (since lower_function was used without
                                        // self_type). Add self with the impl target type.
                                        params.push(HIRParam::new(
                                            "self".to_string(),
                                            impl_item.target_type.clone(),
                                        ));
                                    }
                                    params.extend(trait_fn.params.iter().cloned());

                                    let default_fn = hir::HIRFunction {
                                        name: three_part_name,
                                        type_params: trait_fn.type_params.clone(),
                                        params,
                                        return_type: trait_fn.return_type.clone(),
                                        body: trait_fn.body.clone(),
                                        is_async: trait_fn.is_async,
                                        is_pub: trait_fn.is_pub,
                                    };

                                    match lower_function(
                                        &default_fn,
                                        &mut lambda_counter,
                                        &known_functions,
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
                    }
                }
            }
            // 其他 HIR 项（Struct, Enum, Trait 等）暂时跳过
            _ => {}
        }
    }

    if !errors.is_empty() {
        return Err(format!("MIR lowering failed:\n{}", errors.join("\n")));
    }

    Ok(results)
}

/// �?HIR 函数转换�?MIR 函数
/// 返回 (主函�? Lambda 辅助函数列表)
fn lower_function(
    fn_item: &hir::HIRFunction,
    lambda_counter: &mut usize,
    known_functions: &HashSet<String>,
) -> Result<(MirFunction, Vec<MirFunction>), String> {
    let params: Vec<MIRType> = fn_item.params.iter().map(|p| p.ty.clone().into()).collect();
    let return_type: MIRType = fn_item.return_type.clone().into();

    let mut mir_fn = MirFunction::new(fn_item.name.clone(), params, return_type);
    let start_block = mir_fn.start_block; // 保存 start_block
    let mut ctx = LoweringContext::new(&mut mir_fn, lambda_counter, known_functions);

    // 参数已经被添加到 locals 中，需要记录它们的名称
    for (i, param) in fn_item.params.iter().enumerate() {
        let local = Local::new(i + 1, LocalKind::Param);
        ctx.local_names.insert(param.name.clone(), local);
    }

    // 降低函数体到已有的入口块
    ctx.lower_body_to_block(&fn_item.body, start_block);

    // 检查是否有错误发生
    if !ctx.errors.is_empty() {
        return Err(format!(
            "MIR lowering errors in function '{}':\n  {}",
            fn_item.name,
            ctx.errors.join("\n  ")
        ));
    }

    // 提取 lambda_functions，释放对 mir_fn 的借用
    let lambda_functions = ctx.lambda_functions;
    Ok((mir_fn, lambda_functions))
}

/// 循环上下文，用于 break/continue
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    /// break 跳转到的目标�?
    break_block: usize,
    /// continue 跳转到的目标�?
    continue_block: usize,
}

/// 函数签名信息（返回类型）
#[derive(Clone)]
struct FunctionSig {
    ret_type: MIRType,
    /// 捕获的自由变量（名称, 类型�?
    env: Vec<(String, MIRType)>,
}

/// Lambda 环境信息
struct LambdaEnv {
    /// 环境变量名称和对应的 Local
    vars: Vec<(String, Local)>,
    /// 环境结构体类型（用于代码生成�?
    env_type: MIRType,
    /// 环境指针�?Local（在调用时使用）
    env_ptr_local: Option<Local>,
}

/// 转换上下�?
struct LoweringContext<'a> {
    mir_fn: &'a mut MirFunction,
    /// 名称到局部变量的映射
    local_names: HashMap<String, Local>,
    /// 当前基本�?
    current_block: Option<usize>,
    /// 收集的错误信�?
    errors: Vec<String>,
    /// 循环栈，用于处理 break/continue
    loop_stack: Vec<LoopContext>,
    /// Lambda 计数器（用于生成唯一名称�?
    lambda_counter: &'a mut usize,
    /// 生成�?Lambda 辅助函数
    lambda_functions: Vec<MirFunction>,
    /// Local �?Lambda 函数名的映射
    lambda_names: HashMap<Local, String>,
    /// 函数名到签名的映�?
    function_sigs: HashMap<String, FunctionSig>,
    /// Lambda 函数名到环境信息的映�?
    lambda_environments: HashMap<String, LambdaEnv>,
    /// 映射 Local �?原始类型名称（用于结构体方法调用解析�?
    type_names: HashMap<Local, String>,
    /// 已知的函数名集合（用于方法调用验证）
    known_functions: &'a HashSet<String>,
}

impl<'a> LoweringContext<'a> {
    fn new(
        mir_fn: &'a mut MirFunction,
        lambda_counter: &'a mut usize,
        known_functions: &'a HashSet<String>,
    ) -> Self {
        Self {
            mir_fn,
            local_names: HashMap::new(),
            current_block: None,
            errors: Vec::new(),
            loop_stack: Vec::new(),
            lambda_counter,
            lambda_functions: Vec::new(),
            lambda_names: HashMap::new(),
            function_sigs: HashMap::new(),
            lambda_environments: HashMap::new(),
            type_names: HashMap::new(),
            known_functions,
        }
    }

    /// 生成唯一�?Lambda 函数�?
    fn lambda_name(&mut self) -> String {
        let name = format!("$__lambda{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }

    /// 进入循环，将 break/continue 目标推入�?
    fn push_loop(&mut self, break_block: usize, continue_block: usize) {
        self.loop_stack.push(LoopContext {
            break_block,
            continue_block,
        });
    }

    /// 收集 Lambda body 中使用的自由变量（非参数的外部变量）
    /// 返回自由变量名称列表和对应的 Local
    fn collect_free_vars(
        &self,
        params: &[String],
        body: &crate::hir::HIRExpr,
    ) -> Vec<(String, Local)> {
        use crate::hir::HIRExpr;

        let param_names: std::collections::HashSet<String> = params.iter().cloned().collect();

        let mut free_vars = Vec::new();
        self.collect_vars_from_expr(body, &param_names, &mut free_vars);
        free_vars
    }

    /// 递归收集表达式中使用的自由变�?
    fn collect_vars_from_expr(
        &self,
        expr: &crate::hir::HIRExpr,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        use crate::hir::HIRExpr;

        match expr {
            HIRExpr::Var(name) => {
                // 如果是变量且不是参数，则是自由变�?
                if !param_names.contains(name) {
                    if let Some(&local) = self.local_names.get(name) {
                        if !free_vars.iter().any(|(n, _)| n == name) {
                            free_vars.push((name.clone(), local));
                        }
                    }
                }
            }
            HIRExpr::Lit(_) => {}
            HIRExpr::Unary(_, operand) => {
                self.collect_vars_from_expr(operand, param_names, free_vars);
            }
            HIRExpr::Binary(_op, left, right) => {
                self.collect_vars_from_expr(left, param_names, free_vars);
                self.collect_vars_from_expr(right, param_names, free_vars);
            }
            HIRExpr::Call { func, args } => {
                self.collect_vars_from_expr(func, param_names, free_vars);
                for arg in args {
                    self.collect_vars_from_expr(arg, param_names, free_vars);
                }
            }
            HIRExpr::Lambda {
                params: inner_params,
                body: inner_body,
            } => {
                // 内部 Lambda 有自己的参数集合
                let inner_param_names: std::collections::HashSet<String> =
                    inner_params.iter().cloned().collect();
                self.collect_vars_from_expr(inner_body, &inner_param_names, free_vars);
            }
            HIRExpr::Block(body) => {
                for stmt in &body.stmts {
                    self.collect_vars_from_stmt(stmt, param_names, free_vars);
                }
                if let Some(expr) = &body.expr {
                    self.collect_vars_from_expr(expr, param_names, free_vars);
                }
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_vars_from_expr(cond, param_names, free_vars);
                // then_branch �?else_branch �?HIRBody，需要特殊处�?
                self.collect_vars_from_body(then_branch, param_names, free_vars);
                if let Some(else_b) = else_branch {
                    self.collect_vars_from_body(else_b, param_names, free_vars);
                }
            }
            HIRExpr::Loop(body) => {
                self.collect_vars_from_body(body, param_names, free_vars);
            }
            HIRExpr::While { cond, body } => {
                self.collect_vars_from_expr(cond, param_names, free_vars);
                self.collect_vars_from_body(body, param_names, free_vars);
            }
            HIRExpr::Break(_) | HIRExpr::Continue => {}
            HIRExpr::Array(elems) => {
                for elem in elems {
                    self.collect_vars_from_expr(elem, param_names, free_vars);
                }
            }
            HIRExpr::Index { base, index } => {
                self.collect_vars_from_expr(base, param_names, free_vars);
                self.collect_vars_from_expr(index, param_names, free_vars);
            }
            HIRExpr::Struct { fields, .. } => {
                for (_, field_val) in fields {
                    self.collect_vars_from_expr(field_val, param_names, free_vars);
                }
            }
            HIRExpr::Field { base, .. } => {
                self.collect_vars_from_expr(base, param_names, free_vars);
            }
            HIRExpr::For { var, iter, body } => {
                self.collect_vars_from_expr(iter, param_names, free_vars);
                // for 变量在循环体内是绑定的，不算自由变量
                let mut extended_params = param_names.clone();
                extended_params.insert(var.clone());
                self.collect_vars_from_body(body, &extended_params, free_vars);
            }
            HIRExpr::Assign { target, value } => {
                self.collect_vars_from_expr(target, param_names, free_vars);
                self.collect_vars_from_expr(value, param_names, free_vars);
            }
            HIRExpr::AssignOp {
                target,
                op: _,
                value,
            } => {
                self.collect_vars_from_expr(target, param_names, free_vars);
                self.collect_vars_from_expr(value, param_names, free_vars);
            }
            HIRExpr::And(left, right) | HIRExpr::Or(left, right) => {
                self.collect_vars_from_expr(left, param_names, free_vars);
                self.collect_vars_from_expr(right, param_names, free_vars);
            }
            HIRExpr::MethodCall { receiver, args, .. } => {
                self.collect_vars_from_expr(receiver, param_names, free_vars);
                for arg in args {
                    self.collect_vars_from_expr(arg, param_names, free_vars);
                }
            }
            _ => {
                // 其他表达式类型暂不处�?
            }
        }
    }

    /// �?HIRBody 中收集变�?
    fn collect_vars_from_body(
        &self,
        body: &crate::hir::HIRBody,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        for stmt in &body.stmts {
            self.collect_vars_from_stmt(stmt, param_names, free_vars);
        }
        if let Some(expr) = &body.expr {
            self.collect_vars_from_expr(expr, param_names, free_vars);
        }
    }

    /// 从语句中收集变量
    fn collect_vars_from_stmt(
        &self,
        stmt: &crate::hir::HIRStmt,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        use crate::hir::HIRStmt;

        match stmt {
            HIRStmt::Let { name, value, .. } => {
                if let Some(v) = value {
                    self.collect_vars_from_expr(v, param_names, free_vars);
                }
                // let 绑定的变量不是自由变�?
            }
            HIRStmt::Expr(expr) => {
                self.collect_vars_from_expr(expr, param_names, free_vars);
            }
            HIRStmt::Item => {}
        }
    }

    /// 退出循�?
    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    /// 获取当前循环�?break 目标�?
    fn get_break_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.break_block)
    }

    /// 获取当前循环�?continue 目标�?
    fn get_continue_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.continue_block)
    }

    /// 添加新的局部变�?
    fn add_local(&mut self, name: Option<String>, kind: LocalKind, ty: MIRType) -> Local {
        let local = self.mir_fn.add_local(kind, ty);
        if let Some(name) = name {
            self.local_names.insert(name, local);
        }
        local
    }

    /// 获取局部变量的类型（返回引用，避免不必要的 clone�?
    fn get_local_type(&self, local: Local) -> &MIRType {
        if let Some((_, ty)) = self.mir_fn.locals.get(local.id) {
            ty
        } else {
            &MIR_UNIT
        }
    }

    /// 解析局部变�?
    /// 如果变量未定义，记录错误并返回一个占位符 local
    fn resolve_local(&mut self, name: &str) -> Local {
        match self.local_names.get(name) {
            Some(&local) => local,
            None => {
                // 记录错误
                self.errors.push(format!("undefined variable: '{}'", name));
                // 返回一个占位符 local，让编译继续
                self.mir_fn.add_local(LocalKind::Temp, MIR_UNIT)
            }
        }
    }

    /// 创建新的基本�?
    fn new_block(&mut self) -> usize {
        self.mir_fn.add_block()
    }

    /// 设置当前基本�?
    fn set_current_block(&mut self, block: usize) {
        self.current_block = Some(block);
    }

    /// 获取当前基本�?
    fn current_block(&self) -> usize {
        self.current_block.expect("no current block set")
    }

    /// Check if two types are compatible for binary operations and, if not,
    /// try to insert Cast instructions to reconcile them.  Returns the
    /// (possibly cast) left and right locals whose types now match, or pushes
    /// an error and returns the originals unchanged.
    fn reconcile_binary_operand_types(&mut self, left: Local, right: Local) -> (Local, Local) {
        let left_ty = self.get_local_type(left).clone();
        let right_ty = self.get_local_type(right).clone();

        // Types already match �?nothing to do.
        if left_ty == right_ty {
            return (left, right);
        }

        // Determine if a cast between two types is valid and, if so,
        // which direction to cast (returns the common target type).
        match (&left_ty, &right_ty) {
            // Int widening: smaller int �?larger int
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

            // Float widening: smaller float �?larger float
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

            // Int �?Float promotion (either direction)
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

            // Bool �?Int promotion (either direction)
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

            // Incompatible types �?report an error and return originals.
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

    /// 添加指令到当前基本块
    fn push_inst(&mut self, inst: Instruction) {
        let block_id = self.current_block();
        if let Some(block) = self.mir_fn.block_mut(block_id) {
            block.push(inst);
        }
    }

    /// 设置当前基本块的终止�?
    fn set_terminator(&mut self, term: Terminator) {
        let block_id = self.current_block();
        if let Some(block) = self.mir_fn.block_mut(block_id) {
            block.set_terminator(term);
        }
    }

    /// 降低 HIR 块到指定�?
    fn lower_body_to_block(&mut self, body: &HIRBody, target_block: usize) {
        self.lower_body_to_block_with_return(body, target_block, true);
    }

    /// 降低 HIR 块到指定块，不添�?return，返回最终表达式�?Local（如果有�?
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

    /// 降低 HIR 块到指定块（控制是否添加 return�?
    fn lower_body_to_block_with_return(
        &mut self,
        body: &HIRBody,
        target_block: usize,
        add_return: bool,
    ) {
        self.set_current_block(target_block);

        // 降低所有语�?
        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }

        // 处理最终表达式
        if let Some(expr) = &body.expr {
            let result_local = self.lower_expr(expr);
            if add_return {
                // Only add return if the current block doesn't already have a
                // terminator (e.g. set by break/continue/return inside the expr).
                let cur = self.current_block();
                let already_terminated = self
                    .mir_fn
                    .block_mut(cur)
                    .map_or(false, |b| b.terminator.is_some());
                if !already_terminated {
                    // 检查是否是 main 函数且返回类型是整数
                    // 如果是且表达式结果是 unit 类型，则不返�?unit 值，而是返回 None（代码生成器会返�?0�?
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
            // 如果 add_return = false，不添加 terminator（由表达式自己设置，�?break�?
        } else if add_return {
            // 没有表达式但需�?return，添加空 return
            // Only set return if the current block doesn't already have a
            // terminator (e.g. set by break/continue/return in a statement).
            let cur = self.current_block();
            let already_terminated = self
                .mir_fn
                .block_mut(cur)
                .map_or(false, |b| b.terminator.is_some());
            if !already_terminated {
                self.set_terminator(Terminator::Return(None));
            }
        }
    }

    /// 降低 HIR �?
    fn lower_body(&mut self, body: &HIRBody) -> usize {
        let entry_block = self.new_block();
        self.lower_body_to_block(body, entry_block);
        entry_block
    }

    /// 降低 HIR 语句
    fn lower_stmt(&mut self, stmt: &HIRStmt) {
        match stmt {
            HIRStmt::Let {
                name,
                ty,
                value,
                is_mut,
            } => {
                let kind = if *is_mut {
                    LocalKind::User
                } else {
                    LocalKind::User
                };
                let mir_ty = ty.clone().into();

                if let Some(value_expr) = value {
                    // 先降低表达式得到�?
                    let value_local = self.lower_expr(value_expr);

                    // 检查是否是 Lambda（克隆以避免借用冲突�?
                    let lambda_name = self.lambda_names.get(&value_local).cloned();

                    if let Some(ln) = lambda_name {
                        // 这是一�?Lambda，需要创建环境并存储捕获的变�?
                        let local = self.add_local(Some(name.clone()), kind, mir_ty);

                        // �?Lambda 名称映射到新�?local（用于调用时查找�?
                        self.lambda_names.insert(local, ln.clone());

                        // 检�?Lambda 是否有环境变量需要捕�?
                        // 需要克�?vars 以避免借用检查问�?
                        let env_vars = self
                            .lambda_environments
                            .get(&ln)
                            .map(|env| env.vars.clone())
                            .unwrap_or_default();

                        if !env_vars.is_empty() {
                            // 创建环境结构�?
                            // 环境是一个数组，每个捕获的变量按顺序存储
                            let env_elem_ty = MIR_I64;
                            let env_ty = MIRType::Array(
                                Box::new(env_elem_ty.clone()),
                                env_vars.len() as u64,
                            );

                            // 分配环境空间 - 使用 User 类型以便正确 alloca
                            let env_local = self.mir_fn.add_local(LocalKind::User, env_ty);

                            // 存储每个捕获的变量到环境�?
                            for (i, (var_name, _var_local)) in env_vars.iter().enumerate() {
                                // 从当前上下文获取捕获变量�?local
                                if let Some(&captured_local) = self.local_names.get(var_name) {
                                    // 获取环境变量的地址
                                    let elem_addr_local = self.add_local(
                                        None,
                                        LocalKind::Temp,
                                        MIRType::Ptr(Box::new(env_elem_ty.clone())),
                                    );
                                    let index_local =
                                        self.add_local(None, LocalKind::Temp, MIR_I64);
                                    self.push_inst(Instruction::Assign {
                                        destination: index_local,
                                        value: MirConstant::Int(i as i64),
                                    });
                                    self.push_inst(Instruction::IndexAddr {
                                        destination: elem_addr_local,
                                        base: env_local,
                                        index: index_local,
                                    });

                                    // 加载捕获的变量�?
                                    let captured_value_local =
                                        self.add_local(None, LocalKind::Temp, env_elem_ty.clone());
                                    self.push_inst(Instruction::Load {
                                        destination: captured_value_local,
                                        source: captured_local,
                                    });

                                    // 存储到环�?
                                    self.push_inst(Instruction::Store {
                                        destination: elem_addr_local,
                                        value: captured_value_local,
                                    });
                                }
                            }

                            // 获取环境的地址（作为指针传递给 Lambda�?
                            // 直接使用 mir_fn.add_local 而不�?add_local，避免将环境变量添加�?local_names
                            let env_ptr_local = self
                                .mir_fn
                                .add_local(LocalKind::Temp, MIRType::Ptr(Box::new(env_elem_ty)));
                            self.push_inst(Instruction::AddrOf {
                                destination: env_ptr_local,
                                source: env_local,
                            });

                            // 将环境指针存储到 lambda_environments 中，以便在调用时使用
                            if let Some(env_mut) = self.lambda_environments.get_mut(&ln) {
                                env_mut.env_ptr_local = Some(env_ptr_local);
                            } else {
                                self.errors.push(format!(
                                    "MIR lowering: lambda environment not found for '{}' in Let binding",
                                    ln
                                ));
                            }
                        }
                    } else {
                        // 普通值，创建 local 并存�?
                        // 特殊处理：如果右值是数组类型的用户变量，直接重命名它而不是创建新变量
                        let value_ty = self.get_local_type(value_local).clone();
                        let value_info_opt = self
                            .mir_fn
                            .locals
                            .iter()
                            .find(|(l, _)| l == &value_local)
                            .map(|(l, _t)| l.clone());

                        let value_info = match value_info_opt {
                            Some(info) => info,
                            None => {
                                self.errors.push(format!(
                                    "MIR lowering: local info not found for local {:?} in Let binding for '{}'",
                                    value_local, name
                                ));
                                // Fall through to the normal path with a new local
                                let local = self.add_local(Some(name.clone()), kind, mir_ty);
                                if let Some(type_name) = self.type_names.get(&value_local).cloned()
                                {
                                    self.type_names.insert(local, type_name);
                                }
                                self.push_inst(Instruction::Store {
                                    destination: local,
                                    value: value_local,
                                });
                                return;
                            }
                        };

                        if matches!(value_ty, MIRType::Array(_, _))
                            && value_info.kind == LocalKind::User
                        {
                            // 右值是数组类型的用户变量，直接将其重命名为目标变量
                            // �?local_names 中删除旧的映射，添加新的映射
                            self.local_names.insert(name.clone(), value_local);
                            // 不生�?Store 指令
                        } else {
                            // 普通值，创建 local 并存�?
                            // 使用值的实际类型（如�?HIR 类型不够精确，例如结构体类型�?
                            let actual_ty = if matches!(value_ty, MIRType::Struct { .. }) {
                                value_ty.clone()
                            } else {
                                mir_ty
                            };
                            let local = self.add_local(Some(name.clone()), kind, actual_ty);
                            // 传播类型名称：如果右值有类型名称，将其传播到新的 local
                            if let Some(type_name) = self.type_names.get(&value_local).cloned() {
                                self.type_names.insert(local, type_name);
                            }
                            self.push_inst(Instruction::Store {
                                destination: local,
                                value: value_local,
                            });
                        }
                    }
                } else {
                    // 没有初始值的 let 绑定
                    let _local = self.add_local(Some(name.clone()), kind, mir_ty);
                }
            }
            HIRStmt::Expr(expr) => {
                self.lower_expr(expr);
            }
            HIRStmt::Item => {}
        }
    }

    /// 降低 HIR 表达�?
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

                let fields = fields.clone();
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

    fn lower_builtin_print(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "print expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let arg_local = arg_locals[0];
        let arg_ty = self.get_local_type(arg_local).clone();
        self.emit_print_value(arg_local, &arg_ty);
        self.add_local(None, LocalKind::Temp, MIR_UNIT)
    }

    fn lower_expr(&mut self, expr: &HIRExpr) -> Local {
        match expr {
            HIRExpr::Lit(lit) => self.lower_literal(lit),
            HIRExpr::Var(name) => self.resolve_local(name),
            HIRExpr::Unary(op, operand) => {
                // 特殊处理引用和解引用运算�?
                match op {
                    hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut => {
                        // &expr - 获取表达式的地址
                        let expr_local = self.lower_expr(operand);
                        let expr_ty = self.get_local_type(expr_local).clone();

                        // 创建指针类型
                        let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
                        let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);

                        // 使用 AddrOf 指令获取地址
                        self.push_inst(Instruction::AddrOf {
                            destination: ptr_local,
                            source: expr_local,
                        });

                        ptr_local
                    }
                    hir::HIRUnaryOp::Deref => {
                        // *ptr - 解引�?
                        let ptr_local = self.lower_expr(operand);
                        let ptr_ty = self.get_local_type(ptr_local).clone();

                        let elem_ty = match ptr_ty {
                            MIRType::Ptr(inner) | MIRType::Ref(inner) => (*inner).clone(),
                            _ => MIR_I64,
                        };

                        let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                        self.push_inst(Instruction::Load {
                            destination: result_local,
                            source: ptr_local,
                        });

                        result_local
                    }
                    _ => {
                        // 其他一元运算符
                        let operand_local = self.lower_expr(operand);
                        let mir_op = self.lower_un_op(op);
                        let local = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Unary {
                            destination: local,
                            op: mir_op,
                            operand: operand_local,
                        });
                        local
                    }
                }
            }
            HIRExpr::Binary(op, left, right) => {
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let mir_op = self.lower_bin_op(op);

                // String concatenation: when both operands are string type
                // (Ptr(Int(8))) and the operation is Add, generate a call to
                // sengoo_str_concat instead of a binary add instruction.
                if mir_op == MirBinOp::Add {
                    let is_string_concat = {
                        let left_ty = self.get_local_type(left_local);
                        let right_ty = self.get_local_type(right_local);
                        let is_string = |ty: &MIRType| matches!(ty, MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)));
                        is_string(left_ty) && is_string(right_ty)
                    };
                    if is_string_concat {
                        let result_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
                        let result_local = self.add_local(None, LocalKind::Temp, result_ty);
                        self.push_inst(Instruction::Call {
                            destination: result_local,
                            func: "sengoo_str_concat".to_string(),
                            args: vec![left_local, right_local],
                        });
                        return result_local;
                    }
                }

                // String comparison: when both operands are string type
                // (Ptr(Int(8))) and the operation is Eq or Ne, generate a call
                // to sengoo_str_eq instead of a binary comparison instruction.
                // sengoo_str_eq returns i64 (1=equal, 0=not equal), so we
                // convert to bool by comparing the result with 0.
                if mir_op == MirBinOp::Eq || mir_op == MirBinOp::Ne {
                    let is_string_cmp = {
                        let left_ty = self.get_local_type(left_local);
                        let right_ty = self.get_local_type(right_local);
                        let is_string = |ty: &MIRType| matches!(ty, MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)));
                        is_string(left_ty) && is_string(right_ty)
                    };
                    if is_string_cmp {
                        // Call sengoo_str_eq(left, right) -> i64
                        let call_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Call {
                            destination: call_result,
                            func: "sengoo_str_eq".to_string(),
                            args: vec![left_local, right_local],
                        });

                        // Create constant 0 for comparison
                        let zero = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Assign {
                            destination: zero,
                            value: MirConstant::Int(0),
                        });

                        // Convert i64 result to bool:
                        // For Eq: result != 0 means strings are equal �?true
                        // For Ne: result == 0 means strings are not equal �?true
                        let cmp_op = if mir_op == MirBinOp::Eq {
                            MirBinOp::Ne
                        } else {
                            MirBinOp::Eq
                        };
                        let bool_result = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                        self.push_inst(Instruction::Binary {
                            destination: bool_result,
                            op: cmp_op,
                            left: call_result,
                            right: zero,
                        });

                        return bool_result;
                    }
                }

                // 比较和逻辑操作返回 bool，其他操作返�?int(64)
                // Before generating the binary instruction, reconcile operand
                // types: insert Cast instructions for compatible mismatches or
                // record an error for incompatible types (Requirement 7.4).
                let (left_local, right_local) =
                    self.reconcile_binary_operand_types(left_local, right_local);

                // Determine the result type based on the (possibly cast) operand type.
                let operand_ty = self.get_local_type(left_local).clone();
                let result_ty = match mir_op {
                    MirBinOp::Eq
                    | MirBinOp::Ne
                    | MirBinOp::Lt
                    | MirBinOp::Le
                    | MirBinOp::Gt
                    | MirBinOp::Ge
                    | MirBinOp::LogAnd
                    | MirBinOp::LogOr => MIR_BOOL,
                    _ => operand_ty,
                };
                let local = self.add_local(None, LocalKind::Temp, result_ty);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: mir_op,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Block(body) => {
                let entry = self.lower_body(body);
                Local::new(0, LocalKind::Return)
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let then_block = self.new_block();
                let else_block = self.new_block();
                let join_block = self.new_block();

                let cond_local = self.lower_expr(cond);
                self.set_terminator(Terminator::If {
                    cond: cond_local,
                    then_block,
                    else_block,
                });

                // 降低 then 分支
                let then_val = self.lower_body_to_block_val(then_branch, then_block);
                let then_end = self.current_block();
                if let Some(block) = self.mir_fn.block_mut(then_end) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(join_block));
                    }
                }

                // 降低 else 分支
                if let Some(e) = else_branch {
                    let else_val = self.lower_body_to_block_val(e, else_block);
                    let else_end = self.current_block();
                    if let Some(block) = self.mir_fn.block_mut(else_end) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(join_block));
                        }
                    }

                    // 在 join_block 合并两个分支结果。
                    // 注意：LLVM 不允许 `phi void`，因此 Unit 类型不生成 Phi。
                    self.set_current_block(join_block);
                    let then_ty = self.get_local_type(then_val).clone();
                    let is_void_like = match &then_ty {
                        MIRType::Unit | MIRType::Never => true,
                        MIRType::Tuple(fields) if fields.is_empty() => true,
                        _ => false,
                    };
                    if is_void_like {
                        self.add_local(None, LocalKind::Temp, MIR_UNIT)
                    } else {
                        let result = self.add_local(None, LocalKind::Temp, then_ty);
                        self.push_inst(Instruction::Phi {
                            destination: result,
                            incoming: vec![(then_val, then_end), (else_val, else_end)],
                        });
                        result
                    }
                } else {
                    // 没有 else 分支，else_block 直接跳转�?join_block
                    if let Some(block) = self.mir_fn.block_mut(else_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(join_block));
                        }
                    }
                    self.set_current_block(join_block);
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Loop(body) => {
                let loop_block = self.new_block();
                let exit_block = self.new_block();

                self.set_terminator(Terminator::Goto(loop_block));

                // 进入循环上下文：break -> exit_block, continue -> loop_block
                self.push_loop(exit_block, loop_block);

                // 降低 body �?loop_block（不添加 return�?
                self.lower_body_to_block_with_return(body, loop_block, false);

                // 退出循环上下文
                self.pop_loop();

                // After lowering the body, the current block may differ from
                // loop_block (e.g. when the body contains `if` or other control
                // flow that creates new blocks).  We need to ensure that every
                // block reachable at the end of the body that lacks a terminator
                // unconditionally branches back to loop_block.
                let end_block = self.current_block();
                if end_block != loop_block {
                    // The body introduced extra blocks; make sure the final
                    // block loops back.
                    if let Some(block) = self.mir_fn.block_mut(end_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(loop_block));
                        }
                    }
                }

                // Also ensure loop_block itself loops back when it has no
                // terminator (simple body with no control flow).
                if let Some(block) = self.mir_fn.block_mut(loop_block) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(loop_block));
                    }
                }

                self.set_current_block(exit_block);
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::While { cond, body } => {
                let cond_block = self.new_block();
                let body_block = self.new_block();
                let exit_block = self.new_block();

                self.set_terminator(Terminator::Goto(cond_block));

                // 降低条件表达式到 cond_block
                self.set_current_block(cond_block);
                let cond_local = self.lower_expr(cond);
                self.set_terminator(Terminator::If {
                    cond: cond_local,
                    then_block: body_block,
                    else_block: exit_block,
                });

                // 进入循环上下文：break -> exit_block, continue -> cond_block
                self.push_loop(exit_block, cond_block);

                // 降低 body �?body_block（不添加 return�?
                self.lower_body_to_block_with_return(body, body_block, false);

                // 退出循环上下文
                self.pop_loop();

                // body 结束后跳转回 cond_block
                // 注意：body 可能包含控制流（�?if/else），导致 current_block 不再�?body_block
                // 需要在 body 的最后一个活跃块上设�?Goto(cond_block)
                let body_end_block = self.current_block();
                if body_end_block != body_block {
                    // body 包含控制流，最后一个块不是 body_block
                    if let Some(block) = self.mir_fn.block_mut(body_end_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(cond_block));
                        }
                    }
                }
                // 也检�?body_block 本身（简�?body 的情况）
                if let Some(block) = self.mir_fn.block_mut(body_block) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(cond_block));
                    }
                }

                self.set_current_block(exit_block);
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::For { var, iter, body } => {
                // 检查是否为范围迭代
                match iter.as_ref() {
                    HIRExpr::Range {
                        start,
                        end,
                        inclusive,
                    } => {
                        // for x in start..end { body }  降低�?while 循环
                        let cond_block = self.new_block();
                        let body_block = self.new_block();
                        let inc_block = self.new_block(); // 增加循环变量的块
                        let exit_block = self.new_block();

                        // 降低 start �?end
                        let start_local = if let Some(s) = start {
                            self.lower_expr(s)
                        } else {
                            // 默认�?0 开�?
                            let zero = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Assign {
                                destination: zero,
                                value: MirConstant::Int(0),
                            });
                            zero
                        };

                        let end_local = if let Some(e) = end {
                            self.lower_expr(e)
                        } else {
                            // 没有结束值，创建一个占位符（无限循环）
                            let max = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Assign {
                                destination: max,
                                value: MirConstant::Int(i64::MAX),
                            });
                            max
                        };

                        // 创建循环变量并初始化�?start
                        let loop_var = self.add_local(Some(var.clone()), LocalKind::User, MIR_I64);
                        self.push_inst(Instruction::Store {
                            destination: loop_var,
                            value: start_local,
                        });

                        // 跳转到条件块
                        self.set_terminator(Terminator::Goto(cond_block));

                        // 条件块：检查循环变�?< end
                        self.set_current_block(cond_block);
                        let loop_var_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: loop_var_loaded,
                            source: loop_var,
                        });

                        let end_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: end_loaded,
                            source: end_local,
                        });

                        // 比较操作
                        let cond_local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                        let compare_op = if *inclusive {
                            MirBinOp::Le
                        } else {
                            MirBinOp::Lt
                        };
                        self.push_inst(Instruction::Binary {
                            destination: cond_local,
                            op: compare_op,
                            left: loop_var_loaded,
                            right: end_loaded,
                        });

                        self.set_terminator(Terminator::If {
                            cond: cond_local,
                            then_block: body_block,
                            else_block: exit_block,
                        });

                        // 进入循环上下文：break -> exit_block, continue -> inc_block
                        self.push_loop(exit_block, inc_block);

                        // 循环体（不添�?return�?
                        self.lower_body_to_block_with_return(body, body_block, false);

                        // 退出循环上下文
                        self.pop_loop();

                        // body_block 结束后跳转到 inc_block
                        if let Some(block) = self.mir_fn.block_mut(body_block) {
                            if block.terminator.is_none() {
                                block.set_terminator(Terminator::Goto(inc_block));
                            }
                        }

                        // 增加块：增加循环变量
                        self.set_current_block(inc_block);
                        let inc_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: inc_loaded,
                            source: loop_var,
                        });

                        let one = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Assign {
                            destination: one,
                            value: MirConstant::Int(1),
                        });

                        let inc_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Binary {
                            destination: inc_result,
                            op: MirBinOp::Add,
                            left: inc_loaded,
                            right: one,
                        });

                        self.push_inst(Instruction::Store {
                            destination: loop_var,
                            value: inc_result,
                        });

                        // 跳转回条件块
                        self.set_terminator(Terminator::Goto(cond_block));

                        self.set_current_block(exit_block);
                        self.add_local(None, LocalKind::Temp, MIR_UNIT)
                    }
                    _ => {
                        // 尝试数组迭代: for x in [1, 2, 3] �?for x in arr
                        let iter_local = self.lower_expr(iter);
                        let iter_ty = self.get_local_type(iter_local).clone();

                        match iter_ty {
                            MIRType::Array(elem_ty, len) => {
                                // 数组迭代: for x in arr { body }
                                let cond_block = self.new_block();
                                let body_block = self.new_block();
                                let inc_block = self.new_block();
                                let exit_block = self.new_block();

                                // 创建索引变量并初始化�?0
                                // 索引变量需要在循环中更新，使用 User 类型
                                let index_var = self.add_local(None, LocalKind::User, MIR_I64);
                                let init_val = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: init_val,
                                    value: MirConstant::Int(0),
                                });
                                self.push_inst(Instruction::Store {
                                    destination: index_var,
                                    value: init_val,
                                });

                                // 创建循环变量（与数组元素类型相同�?
                                let loop_var = self.add_local(
                                    Some(var.clone()),
                                    LocalKind::User,
                                    (*elem_ty).clone(),
                                );

                                // 创建数组长度常量
                                let len_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: len_local,
                                    value: MirConstant::Int(len as i64),
                                });

                                // 跳转到条件块
                                self.set_terminator(Terminator::Goto(cond_block));

                                // 条件块：检�?index < len
                                self.set_current_block(cond_block);
                                let index_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: index_loaded,
                                    source: index_var,
                                });

                                let len_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: len_loaded,
                                    source: len_local,
                                });

                                // 比较 index < len
                                let cond_local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                                self.push_inst(Instruction::Binary {
                                    destination: cond_local,
                                    op: MirBinOp::Lt,
                                    left: index_loaded,
                                    right: len_loaded,
                                });

                                self.set_terminator(Terminator::If {
                                    cond: cond_local,
                                    then_block: body_block,
                                    else_block: exit_block,
                                });

                                // 进入循环上下�?
                                self.push_loop(exit_block, inc_block);

                                // 循环体：首先加载 arr[index] 到循环变�?
                                self.set_current_block(body_block);

                                // 计算元素地址: &arr[index]
                                // �?load index_var（User local）到 Temp，再传给 IndexAddr
                                let index_for_addr = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: index_for_addr,
                                    source: index_var,
                                });
                                let elem_addr_local = self.add_local(
                                    None,
                                    LocalKind::Temp,
                                    MIRType::Ptr(elem_ty.clone()),
                                );
                                self.push_inst(Instruction::IndexAddr {
                                    destination: elem_addr_local,
                                    base: iter_local,
                                    index: index_for_addr,
                                });

                                // 加载元素值到循环变量
                                let elem_loaded =
                                    self.add_local(None, LocalKind::Temp, (*elem_ty).clone());
                                self.push_inst(Instruction::Load {
                                    destination: elem_loaded,
                                    source: elem_addr_local,
                                });

                                // 存储到循环变�?
                                self.push_inst(Instruction::Store {
                                    destination: loop_var,
                                    value: elem_loaded,
                                });

                                // 降低循环�?
                                self.lower_body_to_block_with_return(body, body_block, false);

                                // 退出循环上下文
                                self.pop_loop();

                                // body_block 结束后跳转到 inc_block
                                if let Some(block) = self.mir_fn.block_mut(body_block) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(inc_block));
                                    }
                                }

                                // 增加块：index++
                                self.set_current_block(inc_block);
                                let inc_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: inc_loaded,
                                    source: index_var,
                                });

                                let one = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: one,
                                    value: MirConstant::Int(1),
                                });

                                let inc_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Binary {
                                    destination: inc_result,
                                    op: MirBinOp::Add,
                                    left: inc_loaded,
                                    right: one,
                                });

                                self.push_inst(Instruction::Store {
                                    destination: index_var,
                                    value: inc_result,
                                });

                                // 跳转回条件块
                                self.set_terminator(Terminator::Goto(cond_block));

                                self.set_current_block(exit_block);
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            }
                            _ => {
                                // 不支持的迭代器类�?
                                self.errors.push(format!(
                                    "for loop: unsupported iterator type: {:?}",
                                    iter_ty
                                ));
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            }
                        }
                    }
                }
            }
            HIRExpr::Call { func, args } => {
                let arg_locals: Vec<Local> = args.iter().map(|a| self.lower_expr(a)).collect();

                // 获取函数名和返回类型，支�?Lambda 调用
                let (func_name, ret_type, env_ptr_local) = match func.as_ref() {
                    HIRExpr::Var(name) => {
                        // Prefer local function-valued variables (e.g. lambdas) over builtins.
                        if let Some(&var_local) = self.local_names.get(name) {
                            if let Some(lambda_name) = self.lambda_names.get(&var_local) {
                                let ret = self
                                    .function_sigs
                                    .get(lambda_name)
                                    .map(|sig| sig.ret_type.clone())
                                    .unwrap_or(MIR_I64);

                                let env_ptr = self
                                    .lambda_environments
                                    .get(lambda_name)
                                    .and_then(|env| env.env_ptr_local);

                                (lambda_name.clone(), ret, env_ptr)
                            } else {
                                (name.clone(), MIR_I64, None)
                            }
                        } else if name == "print" {
                            return self.lower_builtin_print(&arg_locals);
                        } else {
                            (name.clone(), MIR_I64, None)
                        }
                    }
                    _ => (String::new(), MIR_UNIT, None),
                };

                let local = self.add_local(None, LocalKind::Temp, ret_type);

                // 如果有环境指针，将其作为第一个参数传�?
                let mut final_args = Vec::new();
                if let Some(env_ptr) = env_ptr_local {
                    final_args.push(env_ptr);
                }
                final_args.extend(arg_locals);

                self.push_inst(Instruction::Call {
                    destination: local,
                    func: func_name,
                    args: final_args,
                });
                local
            }
            HIRExpr::And(left, right) => {
                // 短路逻辑�?- 简化为二元运算
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: MirBinOp::LogAnd,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Or(left, right) => {
                // 短路逻辑�?- 简化为二元运算
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: MirBinOp::LogOr,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Break(value) => {
                // 处理 break
                if let Some(target) = self.get_break_target() {
                    // 降低可选的返回�?
                    if let Some(v) = value {
                        self.lower_expr(v);
                    }
                    self.set_terminator(Terminator::Break { target });
                    // break 后不可达，返回一个占位符 Local
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                } else {
                    self.errors.push("break outside of loop".to_string());
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Continue => {
                // 处理 continue
                if let Some(target) = self.get_continue_target() {
                    self.set_terminator(Terminator::Continue { target });
                    // continue 后不可达，返回一个占位符 Local
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                } else {
                    self.errors.push("continue outside of loop".to_string());
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Assign { target, value } => {
                // 赋值表达式: target = value
                // 降低右�?
                let value_local = self.lower_expr(value);

                // 降低左�?�?获取目标变量
                match target.as_ref() {
                    HIRExpr::Var(name) => {
                        let target_local = self.resolve_local(name);
                        if value_local == target_local {
                            // Skip no-op self-assignment (`x = x`) to reduce temp churn.
                            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                        }
                        // 传播类型名称：如果右值有类型名称，将其传播到目标 local
                        if let Some(type_name) = self.type_names.get(&value_local).cloned() {
                            self.type_names.insert(target_local, type_name);
                        }
                        self.push_inst(Instruction::Store {
                            destination: target_local,
                            value: value_local,
                        });
                    }
                    HIRExpr::Index { base, index } => {
                        // 数组元素赋�? arr[i] = value
                        let base_local = self.lower_expr(base);
                        let index_local = self.lower_expr(index);

                        // 计算元素地址
                        let base_ty = self.get_local_type(base_local).clone();
                        let elem_ty = match &base_ty {
                            MIRType::Array(elem, _) => (**elem).clone(),
                            _ => {
                                self.errors
                                    .push("index assignment on non-array type".to_string());
                                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                            }
                        };

                        let addr_local =
                            self.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(elem_ty)));
                        self.push_inst(Instruction::IndexAddr {
                            destination: addr_local,
                            base: base_local,
                            index: index_local,
                        });

                        // 存储值到计算出的地址
                        self.push_inst(Instruction::Store {
                            destination: addr_local,
                            value: value_local,
                        });
                    }
                    _ => {
                        self.errors.push(format!("unsupported assignment target"));
                    }
                }
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::AssignOp { target, op, value } => {
                // 复合赋值表达式: target op= value (e.g., x += 1)
                // 降低右�?
                let value_local = self.lower_expr(value);

                match target.as_ref() {
                    HIRExpr::Var(name) => {
                        let target_local = self.resolve_local(name);
                        // 加载当前�?
                        let target_ty = self.get_local_type(target_local).clone();
                        let current_val = self.add_local(None, LocalKind::Temp, target_ty.clone());
                        self.push_inst(Instruction::Load {
                            destination: current_val,
                            source: target_local,
                        });
                        // 执行运算
                        let mir_op = self.lower_bin_op(op);
                        let result = self.add_local(None, LocalKind::Temp, target_ty);
                        self.push_inst(Instruction::Binary {
                            destination: result,
                            op: mir_op,
                            left: current_val,
                            right: value_local,
                        });
                        // 存储结果
                        self.push_inst(Instruction::Store {
                            destination: target_local,
                            value: result,
                        });
                    }
                    HIRExpr::Index { base, index } => {
                        // 数组元素复合赋�? arr[i] += value
                        let base_local = self.lower_expr(base);
                        let index_local = self.lower_expr(index);

                        // 计算元素地址
                        let base_ty = self.get_local_type(base_local).clone();
                        let elem_ty = match &base_ty {
                            MIRType::Array(elem, _) => (**elem).clone(),
                            _ => {
                                self.errors.push(
                                    "index compound assignment on non-array type".to_string(),
                                );
                                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                            }
                        };

                        let addr_local = self.add_local(
                            None,
                            LocalKind::Temp,
                            MIRType::Ptr(Box::new(elem_ty.clone())),
                        );
                        self.push_inst(Instruction::IndexAddr {
                            destination: addr_local,
                            base: base_local,
                            index: index_local,
                        });

                        // 加载当前元素�?
                        let current_val = self.add_local(None, LocalKind::Temp, elem_ty.clone());
                        self.push_inst(Instruction::Load {
                            destination: current_val,
                            source: addr_local,
                        });

                        // 执行运算
                        let mir_op = self.lower_bin_op(op);
                        let result = self.add_local(None, LocalKind::Temp, elem_ty);
                        self.push_inst(Instruction::Binary {
                            destination: result,
                            op: mir_op,
                            left: current_val,
                            right: value_local,
                        });

                        // 存储结果回元素地址
                        self.push_inst(Instruction::Store {
                            destination: addr_local,
                            value: result,
                        });
                    }
                    _ => {
                        self.errors
                            .push(format!("unsupported compound assignment target"));
                    }
                }
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::Array(elems) => {
                // 数组字面�?[a, b, c]
                // 降低每个元素并收集它们的 locals
                let elem_locals: Vec<Local> = elems.iter().map(|e| self.lower_expr(e)).collect();

                // 确定元素类型和数组类�?
                let elem_ty = if let Some(first_local) = elem_locals.first() {
                    self.get_local_type(*first_local).clone()
                } else {
                    MIR_UNIT
                };
                let array_ty = MIRType::Array(Box::new(elem_ty), elems.len() as u64);

                // 数组需要在内存中分配空间，使用 User 类型
                let array_local = self.add_local(None, LocalKind::User, array_ty.clone());
                self.push_inst(Instruction::Aggregate {
                    destination: array_local,
                    fields: elem_locals,
                    ty: array_ty,
                });

                array_local
            }
            HIRExpr::Index { base, index } => {
                // 数组索引 arr[i]
                let base_local = self.lower_expr(base);
                let index_local = self.lower_expr(index);

                // 获取数组类型以确定元素类�?
                let base_ty = self.get_local_type(base_local).clone();
                let elem_ty = match base_ty {
                    MIRType::Array(elem, _) => *elem,
                    _ => MIR_UNIT,
                };

                // 创建 IndexAddr 指令来计算元素地址
                let addr_local = self.add_local(
                    None,
                    LocalKind::Temp,
                    MIRType::Ptr(Box::new(elem_ty.clone())),
                );
                self.push_inst(Instruction::IndexAddr {
                    destination: addr_local,
                    base: base_local,
                    index: index_local,
                });

                // 从地址加载�?
                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Load {
                    destination: result_local,
                    source: addr_local,
                });

                result_local
            }
            HIRExpr::Struct { name, fields } => {
                // 结构体实例化 Struct { field1: val1, field2: val2 }
                let field_locals: Vec<Local> = fields
                    .iter()
                    .map(|(_, expr)| self.lower_expr(expr))
                    .collect();

                // 使用 MIRType::Struct 表示结构体，包含字段名和类型
                let struct_fields: Vec<(String, MIRType)> = fields
                    .iter()
                    .zip(field_locals.iter())
                    .map(|((field_name, _), &local)| {
                        (field_name.clone(), self.get_local_type(local).clone())
                    })
                    .collect();
                let struct_ty = MIRType::Struct {
                    name: name.clone(),
                    fields: struct_fields,
                };

                let struct_local = self.add_local(None, LocalKind::Temp, struct_ty.clone());
                self.push_inst(Instruction::Aggregate {
                    destination: struct_local,
                    fields: field_locals,
                    ty: struct_ty,
                });

                // 记录结构体类型名称，用于后续方法调用解析
                if !name.is_empty() {
                    self.type_names.insert(struct_local, name.clone());
                }

                struct_local
            }
            HIRExpr::Field { base, field } => {
                // 字段访问 obj.field
                let base_local = self.lower_expr(base);

                // 对于使用 Tuple 表示的结构体，使用索引访�?
                // 临时方案：硬编码常见字段名到索引的映�?
                let field_index = match field.as_str() {
                    "x" | "left" | "r" => 0,
                    "y" | "right" | "g" => 1,
                    "z" | "b" => 2,
                    "w" | "a" => 3,
                    _ => 0,
                };

                let base_ty = self.get_local_type(base_local).clone();
                let elem_ty = match &base_ty {
                    MIRType::Tuple(ref tys) if field_index < tys.len() => tys[field_index].clone(),
                    MIRType::Struct { fields, .. } if field_index < fields.len() => {
                        fields[field_index].1.clone()
                    }
                    _ => MIR_I64,
                };

                // 结构�?元组是值类型，使用 Extract (extractvalue) 而非 FieldAddr+Load
                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Extract {
                    destination: result_local,
                    value: base_local,
                    index: field_index as u32,
                });

                result_local
            }
            HIRExpr::Ref(_is_mut, expr) => {
                // 引用 &expr - 暂时返回表达式的地址
                let expr_local = self.lower_expr(expr);
                let expr_ty = self.get_local_type(expr_local).clone();

                // 创建指针类型
                let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
                let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);

                // 对于局部变量，获取其地址（使�?IndexAddr with index 0�?
                let zero_index = self.add_local(None, LocalKind::Temp, MIR_I64);
                self.push_inst(Instruction::Assign {
                    destination: zero_index,
                    value: MirConstant::Int(0),
                });

                self.push_inst(Instruction::IndexAddr {
                    destination: ptr_local,
                    base: expr_local,
                    index: zero_index,
                });

                ptr_local
            }
            HIRExpr::Deref(expr) => {
                // 解引�?*ptr
                let ptr_local = self.lower_expr(expr);
                let ptr_ty = self.get_local_type(ptr_local).clone();

                let elem_ty = match ptr_ty {
                    MIRType::Ptr(inner) | MIRType::Ref(inner) => *inner,
                    _ => MIR_UNIT,
                };

                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Load {
                    destination: result_local,
                    source: ptr_local,
                });

                result_local
            }
            HIRExpr::Lambda { params, body } => {
                // Lambda 闭包 |args| body
                // 创建一个辅助函数并返回函数引用

                // 生成唯一�?Lambda 函数�?
                let lambda_name = self.lambda_name();

                // 收集自由变量（环境捕获）
                let free_vars = self.collect_free_vars(params, body);

                // Lambda 类型：默认参数和返回类型都是 i64
                let mut param_types: Vec<MIRType> = (0..params.len()).map(|_| MIR_I64).collect();
                let ret_type = MIR_I64;

                // 如果有自由变量，添加环境参数作为第一个参�?
                let env_param_offset = if free_vars.is_empty() {
                    0
                } else {
                    // 环境参数：使用结构体类型表示捕获的环�?
                    // 简化：使用 i64* 指针指向环境
                    param_types.insert(0, MIRType::Ptr(Box::new(MIR_I64)));
                    1
                };

                // 创建 Lambda 辅助函数
                let mut lambda_fn =
                    MirFunction::new(lambda_name.clone(), param_types.clone(), ret_type.clone());
                let lambda_start = lambda_fn.start_block;
                let mut lambda_ctx =
                    LoweringContext::new(&mut lambda_fn, self.lambda_counter, self.known_functions);
                // Set current block for Lambda function entry
                lambda_ctx.current_block = Some(lambda_start);

                // 绑定环境参数�?Lambda 参数�?Lambda 函数
                if !free_vars.is_empty() {
                    // 第一个参数是环境（指针）
                    let env_local = Local::new(1, LocalKind::Param);
                    let env_ptr_name = "__env".to_string();
                    lambda_ctx
                        .local_names
                        .insert(env_ptr_name.clone(), env_local);

                    // 从环境加载捕获的变量
                    // 环境是一个结构体，每个捕获的变量按顺序存�?
                    for (i, (var_name, _)) in free_vars.iter().enumerate() {
                        // 为捕获的变量创建一�?local
                        let captured_local =
                            lambda_ctx.add_local(Some(var_name.clone()), LocalKind::Temp, MIR_I64);

                        // 从环境指针加载变�?
                        // 使用 getelementptr �?load
                        let index_local = lambda_ctx.add_local(None, LocalKind::Temp, MIR_I64);
                        lambda_ctx.push_inst(Instruction::Assign {
                            destination: index_local,
                            value: MirConstant::Int(i as i64),
                        });

                        let ptr_local = lambda_ctx.add_local(
                            None,
                            LocalKind::Temp,
                            MIRType::Ptr(Box::new(MIR_I64)),
                        );
                        lambda_ctx.push_inst(Instruction::IndexAddr {
                            destination: ptr_local,
                            base: env_local,
                            index: index_local,
                        });

                        // 加载�?
                        lambda_ctx.push_inst(Instruction::Load {
                            destination: captured_local,
                            source: ptr_local,
                        });

                        // 将捕获的变量绑定到名称（这样 body 中就可以直接使用了）
                        lambda_ctx
                            .local_names
                            .insert(var_name.clone(), captured_local);
                    }

                    // 绑定 Lambda 参数（偏�?1，因为环境参数占用了位置 1�?
                    for (i, param_name) in params.iter().enumerate() {
                        let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
                        lambda_ctx.local_names.insert(param_name.clone(), local);
                    }
                } else {
                    // 没有环境，正常绑定参�?
                    for (i, param_name) in params.iter().enumerate() {
                        let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
                        lambda_ctx.local_names.insert(param_name.clone(), local);
                    }
                }

                // 降低 body �?Lambda 函数
                // Lambda body �?HIRExpr，需要包装成 HIRBody
                use crate::hir::HIRBody;
                let lambda_body = HIRBody {
                    stmts: vec![],
                    expr: Some(body.clone()),
                };
                lambda_ctx.lower_body_to_block(&lambda_body, lambda_start);

                // 将生成的 Lambda 函数添加到列表中
                self.lambda_functions.push(lambda_fn);

                // 记录环境信息
                if !free_vars.is_empty() {
                    let env_var_types: Vec<(String, MIRType)> = free_vars
                        .iter()
                        .map(|(name, local)| (name.clone(), self.get_local_type(*local).clone()))
                        .collect();
                    self.lambda_environments.insert(
                        lambda_name.clone(),
                        LambdaEnv {
                            vars: free_vars.clone(),
                            env_type: MIRType::Ptr(Box::new(MIR_I64)),
                            env_ptr_local: None, // 稍后�?Let lowering 中设�?
                        },
                    );

                    // 记录函数签名（包含环境信息）
                    self.function_sigs.insert(
                        lambda_name.clone(),
                        FunctionSig {
                            ret_type: ret_type.clone(),
                            env: env_var_types,
                        },
                    );
                } else {
                    // 记录函数签名（无环境�?
                    self.function_sigs.insert(
                        lambda_name.clone(),
                        FunctionSig {
                            ret_type: ret_type.clone(),
                            env: vec![],
                        },
                    );
                }

                // 创建一个临�?local 来存�?Lambda 函数�?
                // 使用整数类型作为 Lambda 的表示（函数指针�?
                let lambda_local = self.add_local(None, LocalKind::Temp, MIR_I64);

                // 存储 Local -> Lambda 函数名的映射
                self.lambda_names.insert(lambda_local, lambda_name.clone());

                lambda_local
            }
            HIRExpr::Match { scrutinee, arms } => {
                // 模式匹配 match scrutinee { arms... }
                // 降低判定表达�?
                let scrutinee_local = self.lower_expr(scrutinee);

                // 获取判定值的类型，检查是否为枚举
                let scrutinee_ty = self.get_local_type(scrutinee_local).clone();

                match scrutinee_ty {
                    MIRType::Enum { ref variants, .. } => {
                        // 枚举模式匹配
                        // 提取判别�?
                        let discr_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Discriminant {
                            destination: discr_local,
                            source: scrutinee_local,
                        });

                        // 为每个分支创建基本块
                        let arm_blocks: Vec<usize> =
                            arms.iter().map(|_| self.new_block()).collect();
                        let join_block = self.new_block();

                        // 创建 Switch 终止�?
                        // 收集 (判别�? 目标�? 映射
                        let mut targets = Vec::new();
                        for (i, arm) in arms.iter().enumerate() {
                            let discr_value = self.extract_discriminant_from_pattern(&arm.pat);
                            if let Some(value) = discr_value {
                                targets.push((value, arm_blocks[i]));
                            }
                        }

                        self.set_terminator(Terminator::Switch {
                            discr: discr_local,
                            targets,
                            otherwise: join_block,
                        });

                        // 降低每个分支
                        for (i, arm) in arms.iter().enumerate() {
                            let arm_block = arm_blocks[i];
                            self.set_current_block(arm_block);

                            // 如果模式绑定了变量，从枚举中提取载荷
                            self.lower_pattern_bindings(&arm.pat, scrutinee_local);

                            // 降低分支主体
                            let arm_result = self.lower_expr(&arm.body);

                            // 跳转到合并块
                            if let Some(block) = self.mir_fn.block_mut(arm_block) {
                                if block.terminator.is_none() {
                                    block.set_terminator(Terminator::Goto(join_block));
                                }
                            }
                        }

                        // 设置合并�?
                        self.set_current_block(join_block);
                        // TODO: 实现 phi 指令来正确合并各分支的返回�?
                        self.add_local(None, LocalKind::Temp, MIR_I64)
                    }
                    _ => {
                        // 非枚举类型的模式匹配 - 简化为 if-else �?
                        let result_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                        let join_block = self.new_block();

                        for (i, arm) in arms.iter().enumerate() {
                            let is_last = i == arms.len() - 1;

                            if is_last {
                                // 最后一个分支直接执�?
                                let arm_result = self.lower_expr(&arm.body);
                                // TODO: 将结果存储到 result_local
                            } else {
                                // 非最后分支需要检查条�?
                                let then_block = self.new_block();
                                let next_arm_block = self.new_block();

                                // 简化处理：对于字面量模式，检查相�?
                                // 对于通配符模式，直接跳转
                                let should_take = self.matches_pattern(&arm.pat, scrutinee_local);
                                self.set_terminator(Terminator::If {
                                    cond: should_take,
                                    then_block,
                                    else_block: next_arm_block,
                                });

                                // 执行当前分支
                                self.set_current_block(then_block);
                                self.lower_expr(&arm.body);
                                if let Some(block) = self.mir_fn.block_mut(then_block) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(join_block));
                                    }
                                }

                                // 继续下一个分�?
                                self.set_current_block(next_arm_block);
                            }
                        }

                        self.set_current_block(join_block);
                        result_local
                    }
                }
            }
            HIRExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                // 方法调用 receiver.method(args)
                // 降低为普通函数调�? TypeName_method(receiver, args)

                // 降低接收�?
                let receiver_local = self.lower_expr(receiver);
                let receiver_ty = self.get_local_type(receiver_local).clone();

                // 降低参数
                let arg_locals: Vec<Local> = args.iter().map(|a| self.lower_expr(a)).collect();

                // String built-in method handling: when receiver is a string
                // (Ptr to i8), intercept known methods and generate runtime calls.
                if let MIRType::Ptr(inner) = &receiver_ty {
                    if let MIRType::Int(8) = inner.as_ref() {
                        if method == "len" {
                            // Generate call to sengoo_str_len(receiver) -> i64
                            let result_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Call {
                                destination: result_local,
                                func: "sengoo_str_len".to_string(),
                                args: vec![receiver_local],
                            });
                            return result_local;
                        }
                    }
                }

                // 生成方法函数名：TypeName_method
                // 遵循 Sengoo 命名约定
                // 首先检�?type_names 以获取实际的结构体类型名�?
                let method_func_name = if let Some(type_name) = self.type_names.get(&receiver_local)
                {
                    format!("{}_{}", type_name, method)
                } else {
                    // 回退到基�?MIRType 的解析（处理内置类型�?
                    match &receiver_ty {
                        MIRType::Int(bits) => {
                            format!("i{}_{}", bits, method)
                        }
                        MIRType::Float(bits) => {
                            format!("f{}_{}", bits, method)
                        }
                        MIRType::Bool => {
                            format!("bool_{}", method)
                        }
                        MIRType::Array(_, _) => {
                            format!("array_{}", method)
                        }
                        MIRType::Tuple(_) => {
                            format!("tuple_{}", method)
                        }
                        MIRType::Ptr(inner) | MIRType::Ref(inner) => {
                            // 指针/引用类型的方�?
                            match inner.as_ref() {
                                MIRType::Int(bits) => format!("i{}_ptr_{}", bits, method),
                                MIRType::Float(bits) => format!("f{}_ptr_{}", bits, method),
                                MIRType::Bool => format!("bool_ptr_{}", method),
                                _ => format!("ptr_{}", method),
                            }
                        }
                        _ => {
                            // 对于未知类型，默认使�?i64 方法�?
                            format!("i64_{}", method)
                        }
                    }
                };

                // Determine the type name for the error message
                let type_display = if let Some(type_name) = self.type_names.get(&receiver_local) {
                    type_name.clone()
                } else {
                    match &receiver_ty {
                        MIRType::Int(bits) => format!("i{}", bits),
                        MIRType::Float(bits) => format!("f{}", bits),
                        MIRType::Bool => "bool".to_string(),
                        MIRType::Array(_, _) => "array".to_string(),
                        MIRType::Tuple(_) => "tuple".to_string(),
                        MIRType::Ptr(_) | MIRType::Ref(_) => "ptr".to_string(),
                        _ => format!("{:?}", receiver_ty),
                    }
                };

                // Check if the method exists in any known function.
                // First try the two-part inherent impl name (e.g. "i64_show").
                // If not found, search for a three-part trait impl name matching
                // "{type_prefix}_{TraitName}_{method}" in known_functions.
                let resolved_func_name = if self.known_functions.contains(&method_func_name) {
                    method_func_name.clone()
                } else {
                    // Build the type prefix used for matching three-part names
                    let type_prefix = if let Some(type_name) = self.type_names.get(&receiver_local)
                    {
                        type_name.clone()
                    } else {
                        match &receiver_ty {
                            MIRType::Int(bits) => format!("i{}", bits),
                            MIRType::Float(bits) => format!("f{}", bits),
                            MIRType::Bool => "bool".to_string(),
                            MIRType::Array(_, _) => "array".to_string(),
                            MIRType::Tuple(_) => "tuple".to_string(),
                            MIRType::Ptr(inner) | MIRType::Ref(inner) => match inner.as_ref() {
                                MIRType::Int(bits) => format!("i{}_ptr", bits),
                                MIRType::Float(bits) => format!("f{}_ptr", bits),
                                MIRType::Bool => "bool_ptr".to_string(),
                                _ => "ptr".to_string(),
                            },
                            _ => "i64".to_string(),
                        }
                    };

                    // Search for three-part mangled names: {type_prefix}_{TraitName}_{method}
                    let suffix = format!("_{}", method);
                    let prefix = format!("{}_", type_prefix);
                    let found = self.known_functions.iter().find(|name| {
                        name.starts_with(&prefix)
                            && name.ends_with(&suffix)
                            && *name != &method_func_name
                            && {
                                // Ensure there is a middle part (trait name) between
                                // the type prefix and the method name suffix.
                                let middle = &name[prefix.len()..name.len() - suffix.len()];
                                !middle.is_empty()
                            }
                    });

                    match found {
                        Some(trait_func_name) => trait_func_name.clone(),
                        None => {
                            self.errors.push(format!(
                                "method '{}' not found for type '{}'",
                                method, type_display
                            ));
                            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                        }
                    }
                };

                // 确定返回类型（默�?i64�?
                let ret_type = MIR_I64;
                let result_local = self.add_local(None, LocalKind::Temp, ret_type);

                // 构建参数列表：receiver + args
                let mut call_args = vec![receiver_local];
                call_args.extend(arg_locals);

                // 生成 Call 指令
                self.push_inst(Instruction::Call {
                    destination: result_local,
                    func: resolved_func_name,
                    args: call_args,
                });

                result_local
            }
            // 其他未实现的 HIR 表达式类型，返回占位�?
            _ => self.add_local(None, LocalKind::Temp, MIR_UNIT),
        }
    }

    /// 从模式中提取判别�?
    /// 对于字面量模式返�?Some(value)，其他返�?None
    fn extract_discriminant_from_pattern(&self, pat: &crate::hir::HIRPattern) -> Option<u32> {
        use crate::hir::HIRPattern;
        match pat {
            HIRPattern::Lit(lit) => match lit {
                HIRLiteral::Int(n) if *n >= 0 && *n < u32::MAX as i64 => Some(*n as u32),
                _ => None,
            },
            HIRPattern::Wild => None,
            HIRPattern::Var { .. } => None,
            _ => None,
        }
    }

    /// 检查值是否匹配模�?
    /// 返回一个包含布尔结果的 Local
    fn matches_pattern(&mut self, pat: &crate::hir::HIRPattern, value: Local) -> Local {
        use crate::hir::HIRPattern;
        let result = self.add_local(None, LocalKind::Temp, MIR_BOOL);

        match pat {
            HIRPattern::Wild => {
                // 通配符总是匹配
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
            HIRPattern::Lit(lit) => {
                // 字面量模式：比较�?
                let lit_local = self.lower_literal(lit);
                self.push_inst(Instruction::Binary {
                    destination: result,
                    op: MirBinOp::Eq,
                    left: value,
                    right: lit_local,
                });
                result
            }
            HIRPattern::Var { .. } => {
                // 变量模式总是匹配
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
            _ => {
                // 其他模式暂不实现
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
        }
    }

    /// 降低模式绑定
    /// 如果模式包含变量绑定，从枚举中提取载荷并绑定
    fn lower_pattern_bindings(&mut self, pat: &crate::hir::HIRPattern, enum_value: Local) {
        use crate::hir::HIRPattern;
        match pat {
            HIRPattern::Var { name, .. } => {
                // 简单变量绑定：整个枚举值绑定到变量
                let _ = self.add_local(Some(name.clone()), LocalKind::User, MIR_I64);
            }
            HIRPattern::Tuple(patterns) => {
                // 元组模式：从枚举中提取载�?
                if !patterns.is_empty() {
                    let payload_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                    self.push_inst(Instruction::ExtractPayload {
                        destination: payload_local,
                        source: enum_value,
                    });
                    // TODO: 绑定元组中的每个元素
                }
            }
            _ => {
                // 其他模式暂不处理
            }
        }
    }

    /// 降低字面�?
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

    /// 降低一元操作符
    fn lower_un_op(&self, op: &hir::HIRUnaryOp) -> MirUnOp {
        match op {
            hir::HIRUnaryOp::Neg => MirUnOp::Neg,
            hir::HIRUnaryOp::Not => MirUnOp::Not,
            hir::HIRUnaryOp::BitNot => MirUnOp::BitNot,
            hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut | hir::HIRUnaryOp::Deref => MirUnOp::Neg,
        }
    }

    /// 降低二元操作�?
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
