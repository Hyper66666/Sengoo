use cranelift_codegen::ir::{
    condcodes::IntCC, types, AbiParam, InstBuilder, TrapCode, Type as ClifType, Value,
};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use miette::Result;
use sengoo_compiler::ast::{
    BinOp, Block, DeclKind, Expr, ExprKind, Literal, StmtKind, Type, TypeKind, UnOp,
};
use sengoo_compiler::Parser;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrimitiveType {
    Bool,
    Integer { bits: u8, signed: bool },
}

impl PrimitiveType {
    const I64: Self = Self::Integer {
        bits: 64,
        signed: true,
    };
    const U64: Self = Self::Integer {
        bits: 64,
        signed: false,
    };

    fn from_ast(ty: &Type) -> Result<Self> {
        let TypeKind::Path(path) = &ty.kind else {
            return Err(miette::miette!(
                "cranelift fast-jit supports only primitive scalar types"
            ));
        };
        let name = path
            .as_simple()
            .map(|ident| ident.name.as_str())
            .ok_or_else(|| miette::miette!("fast-jit requires a simple primitive type"))?;
        match name {
            "bool" => Ok(Self::Bool),
            "i8" => Ok(Self::integer(8, true)),
            "i16" => Ok(Self::integer(16, true)),
            "i32" => Ok(Self::integer(32, true)),
            "i64" | "isize" => Ok(Self::integer(64, true)),
            "u8" => Ok(Self::integer(8, false)),
            "u16" => Ok(Self::integer(16, false)),
            "u32" => Ok(Self::integer(32, false)),
            "u64" | "usize" => Ok(Self::integer(64, false)),
            _ => Err(miette::miette!(
                "unsupported primitive type `{name}` in cranelift fast-jit mode"
            )),
        }
    }

    const fn integer(bits: u8, signed: bool) -> Self {
        Self::Integer { bits, signed }
    }

    fn clif_type(self) -> ClifType {
        match self {
            Self::Bool | Self::Integer { bits: 8, .. } => types::I8,
            Self::Integer { bits: 16, .. } => types::I16,
            Self::Integer { bits: 32, .. } => types::I32,
            Self::Integer { bits: 64, .. } => types::I64,
            Self::Integer { bits, .. } => unreachable!("unsupported integer width {bits}"),
        }
    }

    fn bits(self) -> u8 {
        match self {
            Self::Bool => 8,
            Self::Integer { bits, .. } => bits,
        }
    }

    fn is_signed(self) -> bool {
        matches!(self, Self::Integer { signed: true, .. })
    }

    fn is_integer(self) -> bool {
        matches!(self, Self::Integer { .. })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Integer {
                bits: 8,
                signed: true,
            } => "i8",
            Self::Integer {
                bits: 16,
                signed: true,
            } => "i16",
            Self::Integer {
                bits: 32,
                signed: true,
            } => "i32",
            Self::Integer {
                bits: 64,
                signed: true,
            } => "i64/isize",
            Self::Integer {
                bits: 8,
                signed: false,
            } => "u8",
            Self::Integer {
                bits: 16,
                signed: false,
            } => "u16",
            Self::Integer {
                bits: 32,
                signed: false,
            } => "u32",
            Self::Integer {
                bits: 64,
                signed: false,
            } => "u64/usize",
            Self::Integer { .. } => "integer",
        }
    }
}

#[derive(Clone, Copy)]
struct TypedValue {
    value: Value,
    ty: PrimitiveType,
}

enum EmitFlow {
    Value(TypedValue),
    Return(TypedValue),
}

impl EmitFlow {
    fn into_value(self) -> Result<TypedValue> {
        match self {
            Self::Value(value) => Ok(value),
            Self::Return(_) => Err(miette::miette!(
                "return is only supported as a standalone fast-jit expression"
            )),
        }
    }
}

pub(crate) struct CraneliftExecution {
    pub(crate) value: i64,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) ir: String,
}

pub(crate) fn run_with_cranelift_fast_jit(source: &str, opt_level: u8) -> Result<i64> {
    Ok(compile_and_run_with_cranelift_fast_jit(source, opt_level)?.value)
}

pub(crate) fn compile_and_run_with_cranelift_fast_jit(
    source: &str,
    opt_level: u8,
) -> Result<CraneliftExecution> {
    if opt_level > 3 {
        return Err(miette::miette!(
            "invalid Cranelift fast-jit optimization level: {opt_level}"
        ));
    }

    let program = Parser::parse(source)
        .map_err(|e| miette::miette!("cranelift fast-jit parse failed: {e}"))?;
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

    let declared_return = main_decl
        .return_type
        .as_ref()
        .map(PrimitiveType::from_ast)
        .transpose()?;
    let mut module = create_jit_module()?;
    let mut context = Context::new();
    context
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I64));

    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_ctx);
        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let result = {
            let mut emitter = NumericEmitter {
                builder: &mut builder,
                env: HashMap::new(),
                trap_overflow: opt_level <= 1,
            };
            let result = match emitter.emit_block(&main_decl.body)? {
                EmitFlow::Value(value) | EmitFlow::Return(value) => value,
            };
            if let Some(expected) = declared_return {
                if result.ty != expected {
                    return Err(miette::miette!(
                        "cranelift fast-jit main returns {}, but `{}` was declared",
                        result.ty.label(),
                        expected.label()
                    ));
                }
            }
            emitter.extend_to_abi(result)
        };
        builder.ins().return_(&[result]);
        builder.finalize();
    }

    let ir = context.func.display().to_string();
    let function_id = module
        .declare_function("main", Linkage::Local, &context.func.signature)
        .map_err(|e| miette::miette!("cranelift fast-jit declare failed: {e}"))?;
    module
        .define_function(function_id, &mut context)
        .map_err(|e| miette::miette!("cranelift fast-jit define failed: {e}"))?;
    module.clear_context(&mut context);
    module
        .finalize_definitions()
        .map_err(|e| miette::miette!("cranelift fast-jit finalize failed: {e}"))?;

    let code = module.get_finalized_function(function_id);
    // SAFETY: The JIT function is declared and emitted as `fn() -> i64` above.
    let main_fn = unsafe { std::mem::transmute::<*const u8, fn() -> i64>(code) };
    Ok(CraneliftExecution {
        value: main_fn(),
        ir,
    })
}

fn create_jit_module() -> Result<JITModule> {
    let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
        .map_err(|e| miette::miette!("cranelift fast-jit init failed: {e}"))?;
    jit_builder.hotswap(false);
    Ok(JITModule::new(jit_builder))
}

struct NumericEmitter<'a, 'b> {
    builder: &'a mut FunctionBuilder<'b>,
    env: HashMap<String, TypedValue>,
    trap_overflow: bool,
}

impl NumericEmitter<'_, '_> {
    fn emit_block(&mut self, block: &Block) -> Result<EmitFlow> {
        let mut last = None;
        for stmt in &block.stmts {
            match &stmt.kind {
                StmtKind::Let {
                    name, ty, value, ..
                } => {
                    let expr = value.as_ref().ok_or_else(|| {
                        miette::miette!(
                            "cranelift fast-jit requires explicit initializer for `{}`",
                            name.name
                        )
                    })?;
                    let value = self.emit_expr(expr)?.into_value()?;
                    let value = if let Some(ty) = ty {
                        let target = PrimitiveType::from_ast(ty)?;
                        self.cast(value, target)
                    } else {
                        value
                    };
                    self.env.insert(name.name.clone(), value);
                }
                StmtKind::Const { name, ty, value } => {
                    let value = self.emit_expr(value)?.into_value()?;
                    let value = self.cast(value, PrimitiveType::from_ast(ty)?);
                    self.env.insert(name.name.clone(), value);
                }
                StmtKind::Expr(expr) => match self.emit_expr(expr)? {
                    EmitFlow::Value(value) => last = Some(value),
                    return_flow @ EmitFlow::Return(_) => return Ok(return_flow),
                },
                StmtKind::Item(_) => {
                    return Err(miette::miette!(
                        "cranelift fast-jit does not support nested item declarations"
                    ));
                }
            }
        }

        last.map(EmitFlow::Value).ok_or_else(|| {
            miette::miette!("cranelift fast-jit main requires a value-producing expression")
        })
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<EmitFlow> {
        let value = match &expr.kind {
            ExprKind::Literal(Literal::Int(value)) => TypedValue {
                value: self.builder.ins().iconst(types::I64, *value),
                ty: PrimitiveType::I64,
            },
            ExprKind::Literal(Literal::Uint(value)) => TypedValue {
                value: self.builder.ins().iconst(types::I64, *value as i64),
                ty: PrimitiveType::U64,
            },
            ExprKind::Literal(Literal::Bool(value)) => TypedValue {
                value: self.builder.ins().iconst(types::I8, i64::from(*value)),
                ty: PrimitiveType::Bool,
            },
            ExprKind::Ident(ident) => self.lookup(&ident.name)?,
            ExprKind::Path(path) => {
                let ident = path
                    .as_simple()
                    .ok_or_else(|| miette::miette!("fast-jit supports only simple paths"))?;
                self.lookup(&ident.name)?
            }
            ExprKind::Paren(inner) => return self.emit_expr(inner),
            ExprKind::Unary { op, operand } => {
                let operand = self.emit_expr(operand)?.into_value()?;
                self.emit_unary(*op, operand)?
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.emit_expr(left)?.into_value()?;
                let right = self.emit_expr(right)?.into_value()?;
                self.emit_binary(*op, left, right)?
            }
            ExprKind::Cast { expr, ty } => {
                let value = self.emit_expr(expr)?.into_value()?;
                self.cast(value, PrimitiveType::from_ast(ty)?)
            }
            ExprKind::Assign { target, value } => {
                let ExprKind::Ident(target) = &target.kind else {
                    return Err(miette::miette!(
                        "cranelift fast-jit assignment target must be a local variable"
                    ));
                };
                let existing = self.lookup(&target.name)?;
                let value = self.emit_expr(value)?.into_value()?;
                let value = self.cast(value, existing.ty);
                self.env.insert(target.name.clone(), value);
                value
            }
            ExprKind::Return(value) => {
                let value = value
                    .as_ref()
                    .ok_or_else(|| miette::miette!("cranelift fast-jit requires a return value"))?;
                let value = self.emit_expr(value)?.into_value()?;
                return Ok(EmitFlow::Return(value));
            }
            _ => {
                return Err(miette::miette!(
                    "unsupported expression in cranelift fast-jit mode"
                ));
            }
        };
        Ok(EmitFlow::Value(value))
    }

    fn lookup(&self, name: &str) -> Result<TypedValue> {
        self.env
            .get(name)
            .copied()
            .ok_or_else(|| miette::miette!("unknown variable `{name}` in fast-jit mode"))
    }

    fn emit_unary(&mut self, op: UnOp, operand: TypedValue) -> Result<TypedValue> {
        match op {
            UnOp::Plus if operand.ty.is_integer() => Ok(operand),
            UnOp::Neg if operand.ty.is_signed() => {
                let value = if self.trap_overflow {
                    let zero = self.builder.ins().iconst(operand.ty.clif_type(), 0);
                    let (value, overflow) = self.builder.ins().ssub_overflow(zero, operand.value);
                    self.builder
                        .ins()
                        .trapnz(overflow, TrapCode::IntegerOverflow);
                    value
                } else {
                    self.builder.ins().ineg(operand.value)
                };
                Ok(TypedValue {
                    value,
                    ty: operand.ty,
                })
            }
            UnOp::BitNot if operand.ty.is_integer() => Ok(TypedValue {
                value: self.builder.ins().bnot(operand.value),
                ty: operand.ty,
            }),
            UnOp::Not if operand.ty == PrimitiveType::Bool => Ok(TypedValue {
                value: self.builder.ins().icmp_imm(IntCC::Equal, operand.value, 0),
                ty: PrimitiveType::Bool,
            }),
            _ => Err(miette::miette!(
                "unsupported unary operator `{op}` for {} in fast-jit mode",
                operand.ty.label()
            )),
        }
    }

    fn emit_binary(
        &mut self,
        op: BinOp,
        left: TypedValue,
        right: TypedValue,
    ) -> Result<TypedValue> {
        if left.ty != right.ty {
            return Err(miette::miette!(
                "fast-jit operator `{op}` requires matching operand types, found {} and {}",
                left.ty.label(),
                right.ty.label()
            ));
        }

        if matches!(op, BinOp::And | BinOp::Or) {
            if left.ty != PrimitiveType::Bool {
                return Err(miette::miette!(
                    "logical operator `{op}` requires bool operands in fast-jit mode"
                ));
            }
            let value = match op {
                BinOp::And => self.builder.ins().band(left.value, right.value),
                BinOp::Or => self.builder.ins().bor(left.value, right.value),
                _ => unreachable!(),
            };
            return Ok(TypedValue {
                value,
                ty: PrimitiveType::Bool,
            });
        }

        if matches!(op, BinOp::Eq | BinOp::NotEq) {
            let condition = if op == BinOp::Eq {
                IntCC::Equal
            } else {
                IntCC::NotEqual
            };
            return Ok(TypedValue {
                value: self.builder.ins().icmp(condition, left.value, right.value),
                ty: PrimitiveType::Bool,
            });
        }

        if !left.ty.is_integer() {
            return Err(miette::miette!(
                "operator `{op}` requires integer operands in fast-jit mode"
            ));
        }

        if matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) {
            let condition = comparison_condition(op, left.ty.is_signed());
            return Ok(TypedValue {
                value: self.builder.ins().icmp(condition, left.value, right.value),
                ty: PrimitiveType::Bool,
            });
        }

        let value = match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul => {
                self.emit_overflowing_arithmetic(op, left, right)
            }
            BinOp::Div | BinOp::Mod => {
                self.builder
                    .ins()
                    .trapz(right.value, TrapCode::IntegerDivisionByZero);
                match (op, left.ty.is_signed()) {
                    (BinOp::Div, true) => self.builder.ins().sdiv(left.value, right.value),
                    (BinOp::Div, false) => self.builder.ins().udiv(left.value, right.value),
                    (BinOp::Mod, true) => self.builder.ins().srem(left.value, right.value),
                    (BinOp::Mod, false) => self.builder.ins().urem(left.value, right.value),
                    _ => unreachable!(),
                }
            }
            BinOp::BitAnd => self.builder.ins().band(left.value, right.value),
            BinOp::BitOr => self.builder.ins().bor(left.value, right.value),
            BinOp::BitXor => self.builder.ins().bxor(left.value, right.value),
            BinOp::Shl => self.builder.ins().ishl(left.value, right.value),
            BinOp::Shr if left.ty.is_signed() => self.builder.ins().sshr(left.value, right.value),
            BinOp::Shr => self.builder.ins().ushr(left.value, right.value),
            _ => {
                return Err(miette::miette!(
                    "unsupported binary operator `{op}` in fast-jit mode"
                ));
            }
        };
        Ok(TypedValue { value, ty: left.ty })
    }

    fn emit_overflowing_arithmetic(
        &mut self,
        op: BinOp,
        left: TypedValue,
        right: TypedValue,
    ) -> Value {
        if !self.trap_overflow {
            return match op {
                BinOp::Add => self.builder.ins().iadd(left.value, right.value),
                BinOp::Sub => self.builder.ins().isub(left.value, right.value),
                BinOp::Mul => self.builder.ins().imul(left.value, right.value),
                _ => unreachable!(),
            };
        }

        let (value, overflow) = match (op, left.ty.is_signed()) {
            (BinOp::Add, true) => self.builder.ins().sadd_overflow(left.value, right.value),
            (BinOp::Add, false) => self.builder.ins().uadd_overflow(left.value, right.value),
            (BinOp::Sub, true) => self.builder.ins().ssub_overflow(left.value, right.value),
            (BinOp::Sub, false) => self.builder.ins().usub_overflow(left.value, right.value),
            (BinOp::Mul, true) => self.builder.ins().smul_overflow(left.value, right.value),
            (BinOp::Mul, false) => self.builder.ins().umul_overflow(left.value, right.value),
            _ => unreachable!(),
        };
        self.builder
            .ins()
            .trapnz(overflow, TrapCode::IntegerOverflow);
        value
    }

    fn cast(&mut self, value: TypedValue, target: PrimitiveType) -> TypedValue {
        if value.ty == target {
            return value;
        }

        let result = match (value.ty, target) {
            (PrimitiveType::Bool, PrimitiveType::Integer { .. }) => {
                self.resize_integer(value.value, PrimitiveType::Bool, target)
            }
            (PrimitiveType::Integer { .. }, PrimitiveType::Bool) => {
                self.builder.ins().icmp_imm(IntCC::NotEqual, value.value, 0)
            }
            (PrimitiveType::Integer { .. }, PrimitiveType::Integer { .. }) => {
                self.resize_integer(value.value, value.ty, target)
            }
            (PrimitiveType::Bool, PrimitiveType::Bool) => value.value,
        };
        TypedValue {
            value: result,
            ty: target,
        }
    }

    fn resize_integer(
        &mut self,
        value: Value,
        source: PrimitiveType,
        target: PrimitiveType,
    ) -> Value {
        match target.bits().cmp(&source.bits()) {
            std::cmp::Ordering::Less => self.builder.ins().ireduce(target.clif_type(), value),
            std::cmp::Ordering::Greater if source.is_signed() => {
                self.builder.ins().sextend(target.clif_type(), value)
            }
            std::cmp::Ordering::Greater => self.builder.ins().uextend(target.clif_type(), value),
            std::cmp::Ordering::Equal => value,
        }
    }

    fn extend_to_abi(&mut self, value: TypedValue) -> Value {
        self.resize_integer(value.value, value.ty, PrimitiveType::I64)
    }
}

fn comparison_condition(op: BinOp, signed: bool) -> IntCC {
    match (op, signed) {
        (BinOp::Lt, true) => IntCC::SignedLessThan,
        (BinOp::Le, true) => IntCC::SignedLessThanOrEqual,
        (BinOp::Gt, true) => IntCC::SignedGreaterThan,
        (BinOp::Ge, true) => IntCC::SignedGreaterThanOrEqual,
        (BinOp::Lt, false) => IntCC::UnsignedLessThan,
        (BinOp::Le, false) => IntCC::UnsignedLessThanOrEqual,
        (BinOp::Gt, false) => IntCC::UnsignedGreaterThan,
        (BinOp::Ge, false) => IntCC::UnsignedGreaterThanOrEqual,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::{compile_and_run_with_cranelift_fast_jit, run_with_cranelift_fast_jit};

    #[test]
    fn cranelift_fast_jit_runs_simple_main() {
        let source = r#"
def main() -> i64 {
    let a = 40;
    let b = 2;
    a + b
}
"#;

        let value = run_with_cranelift_fast_jit(source, 2).expect("cranelift fast-jit should run");
        assert_eq!(value, 42);
    }

    #[test]
    fn cranelift_fast_jit_emits_primitive_numeric_ir() {
        let source = r#"
def main() -> i64 {
    let a: i8 = 7i8;
    let b: i8 = 3i8;
    let c: u16 = 21u16;
    let d: u16 = 4u16;
    let e: i32 = 9i32;
    let f: i32 = 2i32;
    let g: u64 = 12u64;
    let h: u64 = 5u64;
    let narrow = (a + b) as i64;
    let product = (c * d) as i64;
    let remainder = (e % f) as i64;
    let shifted = ((g & h) << 1u64) as i64;
    narrow + product + remainder + shifted
}
"#;

        let compiled = compile_and_run_with_cranelift_fast_jit(source, 2)
            .expect("primitive numeric program should compile through Cranelift");
        assert_eq!(compiled.value, 103);
        for instruction in ["iadd", "imul", "srem", "band", "ishl", "sextend", "uextend"] {
            assert!(
                compiled.ir.contains(instruction),
                "expected `{instruction}` in Cranelift IR:\n{}",
                compiled.ir
            );
        }
    }

    #[test]
    fn cranelift_fast_jit_supports_all_integer_widths_and_bool() {
        let source = r#"
def main() -> i64 {
    let a: i8 = 1i8;
    let b: i16 = 2i16;
    let c: i32 = 3i32;
    let d: i64 = 4i64;
    let e: u8 = 5u8;
    let f: u16 = 6u16;
    let g: u32 = 7u32;
    let h: u64 = 8u64;
    let i: isize = 9isize;
    let j: usize = 10usize;
    let flags: bool = true and not false;
    let ignored: bool = flags == true;
    (a + 1i8) as i64 + b as i64 + c as i64 + d + e as i64 + f as i64
        + g as i64 + h as i64 + i as i64 + j as i64
}
"#;

        let compiled = compile_and_run_with_cranelift_fast_jit(source, 2)
            .expect("all primitive integer widths should lower through Cranelift");
        assert_eq!(compiled.value, 56);
        assert!(compiled.ir.contains("band"), "{}", compiled.ir);
    }

    #[test]
    fn cranelift_fast_jit_uses_debug_overflow_traps_and_release_wrapping() {
        let source = r#"
def main() -> i64 {
    let a: i32 = 40i32;
    let b: i32 = 2i32;
    (a + b) as i64
}
"#;

        let debug = compile_and_run_with_cranelift_fast_jit(source, 0)
            .expect("debug program should compile");
        assert!(debug.ir.contains("sadd_overflow"), "{}", debug.ir);
        assert!(debug.ir.contains("trapnz"), "{}", debug.ir);

        let release = compile_and_run_with_cranelift_fast_jit(source, 2)
            .expect("release program should compile");
        assert!(release.ir.contains("iadd"), "{}", release.ir);
        assert!(!release.ir.contains("sadd_overflow"), "{}", release.ir);
    }

    #[test]
    fn cranelift_fast_jit_preserves_unsigned_comparison_and_division() {
        let source = r#"
def main() -> bool {
    let high: u64 = 18446744073709551615u64;
    let two: u64 = 2u64;
    let quotient: u64 = high / two;
    quotient > two
}
"#;

        let compiled = compile_and_run_with_cranelift_fast_jit(source, 2)
            .expect("unsigned program should compile through Cranelift");
        assert_eq!(compiled.value, 1);
        assert!(compiled.ir.contains("udiv"), "{}", compiled.ir);
        assert!(compiled.ir.contains("icmp ugt"), "{}", compiled.ir);
    }

    #[test]
    fn cranelift_fast_jit_rejects_non_primitive_runtime_calls() {
        let source = r#"
def make() -> i64 {
    7
}

def main() -> i64 {
    make()
}
"#;

        let error =
            run_with_cranelift_fast_jit(source, 2).expect_err("fast-jit should reject calls");
        assert!(
            error
                .to_string()
                .contains("unsupported expression in cranelift fast-jit mode"),
            "unexpected fast-jit error: {error}"
        );
    }
}
