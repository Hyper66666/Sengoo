use cranelift_codegen::ir::{types, AbiParam, InstBuilder};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use miette::Result;
use sengoo_compiler::mir::{self, MIRType, MirBinOp, MirConstant, MirFunction};
use std::collections::HashMap;

pub(crate) fn run_with_cranelift_fast_jit(source: &str) -> Result<i64> {
    run_with_optional_host_probe(source, None)
}

#[cfg(test)]
pub(crate) fn run_with_cranelift_fast_jit_with_host_probe<F>(
    source: &str,
    probe_name: &str,
    mut probe: F,
) -> Result<i64>
where
    F: FnMut() -> i64,
{
    run_with_optional_host_probe(
        source,
        Some(HostProbe {
            name: probe_name,
            callback: &mut probe,
        }),
    )
}

fn run_with_optional_host_probe(source: &str, probe: Option<HostProbe<'_>>) -> Result<i64> {
    let (mir_fns, _ffi_codegen) =
        crate::pipeline::compile_source_to_mir_bundle_for_fast_jit(source, 1)
            .map_err(|e| miette::miette!("cranelift fast-jit MIR frontend failed: {}", e))?;

    let value = MirFastInterpreter::new(&mir_fns, probe).run_main()?;
    execute_constant_with_cranelift(value)
}

struct HostProbe<'a> {
    name: &'a str,
    callback: &'a mut dyn FnMut() -> i64,
}

#[derive(Clone, Debug)]
enum MirValue {
    Unit,
    I64(i64),
    Bool(bool),
    Aggregate(Vec<MirValue>),
}

impl MirValue {
    fn as_i64(&self) -> Result<i64> {
        match self {
            Self::I64(value) => Ok(*value),
            Self::Bool(value) => Ok(i64::from(*value)),
            Self::Unit => Ok(0),
            Self::Aggregate(_) => Err(miette::miette!(
                "cranelift fast-jit cannot return aggregate values yet"
            )),
        }
    }

    fn truthy(&self) -> Result<bool> {
        Ok(self.as_i64()? != 0)
    }
}

struct MirFastInterpreter<'a, 'probe> {
    functions: HashMap<&'a str, &'a MirFunction>,
    probe: Option<HostProbe<'probe>>,
}

impl<'a, 'probe> MirFastInterpreter<'a, 'probe> {
    fn new(mir_fns: &'a [MirFunction], probe: Option<HostProbe<'probe>>) -> Self {
        Self {
            functions: mir_fns
                .iter()
                .map(|function| (function.name.as_str(), function))
                .collect(),
            probe,
        }
    }

    fn run_main(mut self) -> Result<i64> {
        let value = self.call_function("main", Vec::new())?;
        value.as_i64()
    }

    fn call_function(&mut self, name: &str, args: Vec<MirValue>) -> Result<MirValue> {
        if self.probe.as_ref().is_some_and(|probe| probe.name == name) {
            let probe = self.probe.as_mut().expect("probe presence checked above");
            return Ok(MirValue::I64((probe.callback)()));
        }

        let function = *self
            .functions
            .get(name)
            .ok_or_else(|| miette::miette!("cranelift fast-jit cannot resolve `{}`", name))?;
        self.execute_function(function, args)
    }

    fn execute_function(
        &mut self,
        function: &MirFunction,
        args: Vec<MirValue>,
    ) -> Result<MirValue> {
        let mut locals = function
            .locals
            .iter()
            .map(|(_, ty)| default_value_for_type(ty))
            .collect::<Vec<_>>();
        for (index, value) in args.into_iter().enumerate() {
            let local_index = index + 1;
            if local_index < locals.len() {
                locals[local_index] = value;
            }
        }

        let mut block_id = function.start_block;
        loop {
            let block = function.basic_blocks.get(block_id).ok_or_else(|| {
                miette::miette!("cranelift fast-jit missing MIR block {}", block_id)
            })?;
            for instruction in function.block_instructions(block) {
                self.execute_instruction(function, instruction, &mut locals)?;
            }

            match block.terminator.as_ref() {
                Some(mir::Terminator::Return(Some(local))) => {
                    return Ok(local_value(&locals, *local)?.clone());
                }
                Some(mir::Terminator::Return(None)) => return Ok(MirValue::Unit),
                Some(mir::Terminator::Goto(target)) => block_id = *target,
                Some(mir::Terminator::If {
                    cond,
                    then_block,
                    else_block,
                }) => {
                    block_id = if local_value(&locals, *cond)?.truthy()? {
                        *then_block
                    } else {
                        *else_block
                    };
                }
                Some(mir::Terminator::Call {
                    func,
                    args,
                    destination,
                    target,
                }) => {
                    let call_args = args
                        .iter()
                        .map(|arg| call_arg_value(&locals, arg))
                        .collect::<Result<Vec<_>>>()?;
                    let value = self.call_function(func, call_args)?;
                    assign_local(&mut locals, *destination, value)?;
                    block_id = *target;
                }
                Some(other) => {
                    return Err(miette::miette!(
                        "cranelift fast-jit unsupported MIR terminator: {:?}",
                        other
                    ))
                }
                None => {
                    return Err(miette::miette!(
                        "cranelift fast-jit block {} has no terminator",
                        block_id
                    ))
                }
            }
        }
    }

    fn execute_instruction(
        &mut self,
        _function: &MirFunction,
        instruction: &mir::Instruction,
        locals: &mut [MirValue],
    ) -> Result<()> {
        match instruction {
            mir::Instruction::Assign { destination, value } => {
                assign_local(locals, *destination, constant_to_value(value)?)?;
            }
            mir::Instruction::Unary {
                destination,
                op,
                operand,
            } => {
                let value = local_value(locals, *operand)?.as_i64()?;
                let result = match op {
                    mir::MirUnOp::Neg => -value,
                    mir::MirUnOp::Not => i64::from(value == 0),
                    mir::MirUnOp::BitNot => !value,
                };
                assign_local(locals, *destination, MirValue::I64(result))?;
            }
            mir::Instruction::Binary {
                destination,
                op,
                left,
                right,
            } => {
                let lhs = local_value(locals, *left)?.as_i64()?;
                let rhs = local_value(locals, *right)?.as_i64()?;
                assign_local(
                    locals,
                    *destination,
                    MirValue::I64(eval_binary(*op, lhs, rhs)?),
                )?;
            }
            mir::Instruction::Aggregate {
                destination,
                fields,
                ..
            } => {
                let values = fields
                    .iter()
                    .map(|local| local_value(locals, *local).cloned())
                    .collect::<Result<Vec<_>>>()?;
                assign_local(locals, *destination, MirValue::Aggregate(values))?;
            }
            mir::Instruction::Extract {
                destination,
                value,
                index,
            } => {
                let MirValue::Aggregate(fields) = local_value(locals, *value)? else {
                    return Err(miette::miette!(
                        "cranelift fast-jit extract requires an aggregate"
                    ));
                };
                let field = fields.get(*index as usize).cloned().ok_or_else(|| {
                    miette::miette!("cranelift fast-jit extract index {} out of range", index)
                })?;
                assign_local(locals, *destination, field)?;
            }
            mir::Instruction::Call {
                destination,
                func,
                args,
            } => {
                let call_args = args
                    .iter()
                    .map(|local| local_value(locals, *local).cloned())
                    .collect::<Result<Vec<_>>>()?;
                let value = self.call_function(func, call_args)?;
                assign_local(locals, *destination, value)?;
            }
            mir::Instruction::Cast {
                destination, value, ..
            }
            | mir::Instruction::Bitcast {
                destination, value, ..
            }
            | mir::Instruction::Load {
                destination,
                source: value,
            }
            | mir::Instruction::AddrOf {
                destination,
                source: value,
            } => {
                assign_local(locals, *destination, local_value(locals, *value)?.clone())?;
            }
            mir::Instruction::Store { destination, value } => {
                assign_local(locals, *destination, local_value(locals, *value)?.clone())?;
            }
            mir::Instruction::Nop => {}
            other => {
                return Err(miette::miette!(
                    "cranelift fast-jit unsupported MIR instruction: {:?}",
                    other
                ))
            }
        }
        Ok(())
    }
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

fn assign_local(locals: &mut [MirValue], local: mir::Local, value: MirValue) -> Result<()> {
    let slot = locals
        .get_mut(local.index())
        .ok_or_else(|| miette::miette!("cranelift fast-jit missing local {}", local.index()))?;
    *slot = value;
    Ok(())
}

fn local_value(locals: &[MirValue], local: mir::Local) -> Result<&MirValue> {
    locals
        .get(local.index())
        .ok_or_else(|| miette::miette!("cranelift fast-jit missing local {}", local.index()))
}

fn call_arg_value(locals: &[MirValue], arg: &mir::CallArg) -> Result<MirValue> {
    match arg {
        mir::CallArg::Local(local) => Ok(local_value(locals, *local)?.clone()),
        mir::CallArg::Constant(constant) => constant_to_value(constant),
    }
}

fn default_value_for_type(ty: &MIRType) -> MirValue {
    match ty {
        MIRType::Bool => MirValue::Bool(false),
        MIRType::Struct { fields, .. } => MirValue::Aggregate(
            fields
                .iter()
                .map(|(_, field_ty)| default_value_for_type(field_ty))
                .collect(),
        ),
        MIRType::Tuple(fields) => {
            MirValue::Aggregate(fields.iter().map(default_value_for_type).collect())
        }
        MIRType::Unit | MIRType::Never => MirValue::Unit,
        _ => MirValue::I64(0),
    }
}

fn constant_to_value(value: &MirConstant) -> Result<MirValue> {
    match value {
        MirConstant::Unit => Ok(MirValue::Unit),
        MirConstant::Bool(value) => Ok(MirValue::Bool(*value)),
        MirConstant::Int(value) => Ok(MirValue::I64(*value)),
        MirConstant::Uint(value) => Ok(MirValue::I64(*value as i64)),
        MirConstant::Char(value) => Ok(MirValue::I64(*value as i64)),
        MirConstant::GlobalRef(_) => Ok(MirValue::I64(0)),
        other => Err(miette::miette!(
            "cranelift fast-jit unsupported constant: {:?}",
            other
        )),
    }
}

fn eval_binary(op: MirBinOp, lhs: i64, rhs: i64) -> Result<i64> {
    Ok(match op {
        MirBinOp::Add => lhs.saturating_add(rhs),
        MirBinOp::Sub => lhs.saturating_sub(rhs),
        MirBinOp::Mul => lhs.saturating_mul(rhs),
        MirBinOp::Div => lhs / rhs,
        MirBinOp::Rem => lhs % rhs,
        MirBinOp::BitAnd => lhs & rhs,
        MirBinOp::BitOr => lhs | rhs,
        MirBinOp::BitXor => lhs ^ rhs,
        MirBinOp::Shl => lhs << rhs,
        MirBinOp::Shr => lhs >> rhs,
        MirBinOp::LogAnd => i64::from(lhs != 0 && rhs != 0),
        MirBinOp::LogOr => i64::from(lhs != 0 || rhs != 0),
        MirBinOp::Eq => i64::from(lhs == rhs),
        MirBinOp::Ne => i64::from(lhs != rhs),
        MirBinOp::Lt => i64::from(lhs < rhs),
        MirBinOp::Gt => i64::from(lhs > rhs),
        MirBinOp::Le => i64::from(lhs <= rhs),
        MirBinOp::Ge => i64::from(lhs >= rhs),
    })
}

#[cfg(test)]
mod tests {
    use super::{run_with_cranelift_fast_jit, run_with_cranelift_fast_jit_with_host_probe};
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    #[test]
    fn cranelift_fast_jit_runs_user_drop_from_mir() {
        static DROP_PROBE_COUNT: AtomicUsize = AtomicUsize::new(0);
        DROP_PROBE_COUNT.store(0, Ordering::SeqCst);

        let source = r#"
extern "C" {
    fn sg_test_drop_probe() -> i64;
}

struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
        sg_test_drop_probe();
    }
}

def main() -> i64 {
    let resource: Resource = Resource { handle: 7 };
    42
}
"#;

        let value =
            run_with_cranelift_fast_jit_with_host_probe(source, "sg_test_drop_probe", || {
                DROP_PROBE_COUNT.fetch_add(1, Ordering::SeqCst);
                0
            })
            .expect("cranelift fast-jit should execute MIR drop glue");

        assert_eq!(value, 42);
        assert_eq!(DROP_PROBE_COUNT.load(Ordering::SeqCst), 1);
    }
}
