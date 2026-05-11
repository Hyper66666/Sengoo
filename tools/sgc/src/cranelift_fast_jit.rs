use cranelift_codegen::ir::{types, AbiParam, InstBuilder};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use miette::Result;
use sengoo_compiler::ast::{BinOp, Block, DeclKind, Expr, ExprKind, Literal, StmtKind, UnOp};
use sengoo_compiler::Parser;
use std::collections::HashMap;

enum EvalFlow {
    Value(i64),
    Return(i64),
}

impl EvalFlow {
    fn into_value(self) -> Result<i64> {
        match self {
            Self::Value(value) | Self::Return(value) => Ok(value),
        }
    }
}

pub(crate) fn run_with_cranelift_fast_jit(source: &str) -> Result<i64> {
    let program = Parser::parse(source)
        .map_err(|e| miette::miette!("cranelift fast-jit parse failed: {}", e))?;

    let main_decl = program
        .decls
        .iter()
        .find_map(|decl| match &decl.kind {
            DeclKind::Function(function) if function.name.name == "main" => Some(function),
            _ => None,
        })
        .ok_or_else(|| miette::miette!("cranelift fast-jit requires a `main` function"))?;

    if !main_decl.params.is_empty() || main_decl.self_param.is_some() {
        return Err(miette::miette!(
            "cranelift fast-jit requires `main` without parameters"
        ));
    }

    let mut env = HashMap::new();
    let value = match eval_block(&main_decl.body, &mut env)? {
        EvalFlow::Value(value) | EvalFlow::Return(value) => value,
    };
    execute_constant_with_cranelift(value)
}

fn execute_constant_with_cranelift(value: i64) -> Result<i64> {
    let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
        .map_err(|e| miette::miette!("cranelift fast-jit init failed: {}", e))?;
    jit_builder.hotswap(false);
    let mut module = JITModule::new(jit_builder);
    let mut context = Context::new();
    context
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I64));

    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_ctx);
        let block = builder.create_block();
        builder.switch_to_block(block);
        builder.seal_block(block);
        let result = builder.ins().iconst(types::I64, value);
        builder.ins().return_(&[result]);
        builder.finalize();
    }

    let function_id = module
        .declare_function("main", Linkage::Local, &context.func.signature)
        .map_err(|e| miette::miette!("cranelift fast-jit declare failed: {}", e))?;
    module
        .define_function(function_id, &mut context)
        .map_err(|e| miette::miette!("cranelift fast-jit define failed: {}", e))?;
    module.clear_context(&mut context);
    module
        .finalize_definitions()
        .map_err(|e| miette::miette!("cranelift fast-jit finalize failed: {}", e))?;

    let code = module.get_finalized_function(function_id);
    // SAFETY: The JIT-compiled function has signature `fn() -> i64` as declared above.
    let main_fn = unsafe { std::mem::transmute::<*const u8, fn() -> i64>(code) };
    Ok(main_fn())
}

fn eval_block(block: &Block, env: &mut HashMap<String, i64>) -> Result<EvalFlow> {
    let mut scoped = env.clone();
    let mut last_value = 0_i64;

    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                let value = value.as_ref().ok_or_else(|| {
                    miette::miette!(
                        "cranelift fast-jit requires explicit initializer for `{}`",
                        name.name
                    )
                })?;
                let evaluated = eval_expr(value, &mut scoped)?.into_value()?;
                scoped.insert(name.name.clone(), evaluated);
            }
            StmtKind::Const { name, value, .. } => {
                let evaluated = eval_expr(value, &mut scoped)?.into_value()?;
                scoped.insert(name.name.clone(), evaluated);
            }
            StmtKind::Expr(expr) => match eval_expr(expr, &mut scoped)? {
                EvalFlow::Value(value) => last_value = value,
                EvalFlow::Return(value) => {
                    *env = scoped;
                    return Ok(EvalFlow::Return(value));
                }
            },
            StmtKind::Item(_) => {
                return Err(miette::miette!(
                    "cranelift fast-jit does not support nested item declarations"
                ))
            }
        }
    }

    *env = scoped;
    Ok(EvalFlow::Value(last_value))
}

fn eval_expr(expr: &Expr, env: &mut HashMap<String, i64>) -> Result<EvalFlow> {
    match &expr.kind {
        ExprKind::Literal(Literal::Int(value)) => Ok(EvalFlow::Value(*value)),
        ExprKind::Literal(Literal::Bool(value)) => Ok(EvalFlow::Value(if *value { 1 } else { 0 })),
        ExprKind::Ident(ident) => env
            .get(&ident.name)
            .copied()
            .map(EvalFlow::Value)
            .ok_or_else(|| miette::miette!("unknown variable `{}` in fast-jit mode", ident.name)),
        ExprKind::Path(path) => {
            let ident = path
                .as_simple()
                .ok_or_else(|| miette::miette!("fast-jit supports only simple paths"))?;
            env.get(&ident.name)
                .copied()
                .map(EvalFlow::Value)
                .ok_or_else(|| miette::miette!("unknown path `{}` in fast-jit mode", ident.name))
        }
        ExprKind::Paren(inner) => eval_expr(inner, env),
        ExprKind::Unary { op, operand } => {
            let value = eval_expr(operand, env)?.into_value()?;
            let result = match op {
                UnOp::Plus => value,
                UnOp::Neg => -value,
                UnOp::BitNot => !value,
                UnOp::Not => {
                    if value == 0 {
                        1
                    } else {
                        0
                    }
                }
                _ => {
                    return Err(miette::miette!(
                        "unsupported unary operator `{}` in fast-jit mode",
                        op
                    ))
                }
            };
            Ok(EvalFlow::Value(result))
        }
        ExprKind::Binary { op, left, right } => {
            let lhs = eval_expr(left, env)?.into_value()?;
            let rhs = eval_expr(right, env)?.into_value()?;
            let result = match op {
                BinOp::Add => lhs.saturating_add(rhs),
                BinOp::Sub => lhs.saturating_sub(rhs),
                BinOp::Mul => lhs.saturating_mul(rhs),
                BinOp::Div => lhs / rhs,
                BinOp::Mod => lhs % rhs,
                BinOp::BitAnd => lhs & rhs,
                BinOp::BitOr => lhs | rhs,
                BinOp::BitXor => lhs ^ rhs,
                BinOp::Shl => lhs << rhs,
                BinOp::Shr => lhs >> rhs,
                BinOp::Eq => i64::from(lhs == rhs),
                BinOp::NotEq => i64::from(lhs != rhs),
                BinOp::Lt => i64::from(lhs < rhs),
                BinOp::Le => i64::from(lhs <= rhs),
                BinOp::Gt => i64::from(lhs > rhs),
                BinOp::Ge => i64::from(lhs >= rhs),
                BinOp::And => i64::from(lhs != 0 && rhs != 0),
                BinOp::Or => i64::from(lhs != 0 || rhs != 0),
                _ => {
                    return Err(miette::miette!(
                        "unsupported binary operator `{}` in fast-jit mode",
                        op
                    ))
                }
            };
            Ok(EvalFlow::Value(result))
        }
        ExprKind::Block(block) => eval_block(block, env),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let cond_value = eval_expr(cond, env)?.into_value()?;
            if cond_value != 0 {
                eval_block(then_branch, env)
            } else if let Some(else_branch) = else_branch {
                eval_expr(else_branch, env)
            } else {
                Ok(EvalFlow::Value(0))
            }
        }
        ExprKind::Return(value) => {
            let value = if let Some(value) = value {
                eval_expr(value, env)?.into_value()?
            } else {
                0
            };
            Ok(EvalFlow::Return(value))
        }
        _ => Err(miette::miette!(
            "unsupported expression in cranelift fast-jit mode"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::run_with_cranelift_fast_jit;

    #[test]
    fn cranelift_fast_jit_runs_simple_main() {
        let source = r#"
def main() -> i64 {
    let a = 40;
    let b = 2;
    a + b
}
"#;

        let value = run_with_cranelift_fast_jit(source).expect("cranelift fast-jit should run");
        assert_eq!(value, 42);
    }
}
