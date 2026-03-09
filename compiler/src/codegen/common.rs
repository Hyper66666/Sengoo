//! Shared codegen utility functions
//!
//! This module contains utility functions shared between `Codegen` (mod.rs)
//! and `JITCodegen` (jit.rs) to eliminate code duplication.
//! Both codegen backends can call these free functions instead of
//! maintaining their own copies of the same logic.

use crate::mir::{Local, LocalKind, MIRType, MirBinOp, MirConstant};

/// Convert a MIR type to its LLVM IR type string representation.
///
/// This is the comprehensive version that handles all MIR type variants,
/// including recursive types like pointers, arrays, tuples, and function types.
pub fn mir_type_to_llvm_str(ty: &MIRType) -> String {
    match ty {
        MIRType::Unit => "void".to_string(),
        MIRType::Never => "void".to_string(),
        MIRType::Bool => "i1".to_string(),
        MIRType::Int(n) => format!("i{}", n),
        MIRType::Float(n) => match n {
            32 => "float".to_string(),
            64 => "double".to_string(),
            _ => "double".to_string(),
        },
        MIRType::Ref(inner) | MIRType::Ptr(inner) => {
            format!("{}*", mir_type_to_llvm_str(inner))
        }
        MIRType::Array(elem, n) => {
            format!("[{} x {}]", n, mir_type_to_llvm_str(elem))
        }
        MIRType::Tuple(types) if types.is_empty() => "void".to_string(),
        MIRType::Tuple(types) => {
            let field_types: Vec<String> = types.iter().map(|t| mir_type_to_llvm_str(t)).collect();
            format!("{{{}}}", field_types.join(", "))
        }
        MIRType::Fn { params, ret } => {
            let param_str: Vec<String> = params.iter().map(|p| mir_type_to_llvm_str(p)).collect();
            format!("{} ({})*", mir_type_to_llvm_str(ret), param_str.join(", "))
        }
        MIRType::Struct { name, .. } => {
            format!("%{}", name)
        }
        MIRType::Enum { .. } => {
            // Enums are represented as { discriminant, payload }
            "{ i64, i64 }".to_string()
        }
        MIRType::Future(_) => {
            // Future<T> is an opaque i64 handle at runtime
            "i64".to_string()
        }
    }
}

/// Generate a local variable name from a `Local` based on its `LocalKind`.
///
/// This uses the kind-based naming convention from the main codegen:
/// - `Param`  → `%l_{id}`
/// - `Temp`   → `%t_{id}`
/// - `User`   → `%u_{id}`
/// - `Return` → `%ret_{id}`
pub fn local_name(local: Local) -> String {
    match local.kind {
        LocalKind::Param => format!("%l_{}", local.id),
        LocalKind::Temp => format!("%t_{}", local.id),
        LocalKind::User => format!("%u_{}", local.id),
        LocalKind::Return => format!("%ret_{}", local.id),
    }
}

/// Append indentation (2 spaces per level) to the given IR buffer.
pub fn emit_indent(ir: &mut String, indent: usize) {
    for _ in 0..indent {
        ir.push_str("  ");
    }
}

/// Map a `MirBinOp` to the corresponding LLVM IR instruction name,
/// taking into account whether the operand type is floating-point or integer.
///
/// Returns the LLVM instruction mnemonic (e.g., `"add"`, `"fadd"`, `"icmp eq"`, `"fcmp oeq"`).
pub fn binary_op_to_llvm(op: MirBinOp, ty: &MIRType) -> &'static str {
    let is_float = matches!(ty, MIRType::Float(_));
    match op {
        MirBinOp::Add => {
            if is_float {
                "fadd"
            } else {
                "add"
            }
        }
        MirBinOp::Sub => {
            if is_float {
                "fsub"
            } else {
                "sub"
            }
        }
        MirBinOp::Mul => {
            if is_float {
                "fmul"
            } else {
                "mul"
            }
        }
        MirBinOp::Div => {
            if is_float {
                "fdiv"
            } else {
                "sdiv"
            }
        }
        MirBinOp::Rem => {
            if is_float {
                "frem"
            } else {
                "srem"
            }
        }
        MirBinOp::Eq => {
            if is_float {
                "fcmp oeq"
            } else {
                "icmp eq"
            }
        }
        MirBinOp::Ne => {
            if is_float {
                "fcmp one"
            } else {
                "icmp ne"
            }
        }
        MirBinOp::Lt => {
            if is_float {
                "fcmp olt"
            } else {
                "icmp slt"
            }
        }
        MirBinOp::Gt => {
            if is_float {
                "fcmp ogt"
            } else {
                "icmp sgt"
            }
        }
        MirBinOp::Le => {
            if is_float {
                "fcmp ole"
            } else {
                "icmp sle"
            }
        }
        MirBinOp::Ge => {
            if is_float {
                "fcmp oge"
            } else {
                "icmp sge"
            }
        }
        MirBinOp::BitAnd => "and",
        MirBinOp::BitOr => "or",
        MirBinOp::BitXor => "xor",
        MirBinOp::Shl => "shl",
        MirBinOp::Shr => "ashr",
        MirBinOp::LogAnd => "and",
        MirBinOp::LogOr => "or",
    }
}

/// Convert a `MirConstant` to its LLVM IR literal string representation.
///
/// This handles all constant variants and produces the value portion
/// (without the type prefix) suitable for use in LLVM IR instructions.
pub fn mir_constant_to_llvm_str(constant: &MirConstant) -> String {
    match constant {
        MirConstant::Unit => "0".to_string(),
        MirConstant::Bool(b) => (if *b { 1 } else { 0 }).to_string(),
        MirConstant::Int(n) => n.to_string(),
        MirConstant::Uint(n) => n.to_string(),
        MirConstant::Float(f) => f.to_string(),
        MirConstant::Char(c) => (*c as u32).to_string(),
        // String constants are emitted as globals by codegen backends; direct
        // literal fallback uses null pointer.
        MirConstant::String(_) => "null".to_string(),
        MirConstant::Bytes(bytes) => {
            if bytes.is_empty() {
                "zeroinitializer".to_string()
            } else {
                let elems: Vec<String> = bytes.iter().map(|b| format!("i8 {}", b)).collect();
                format!("[{}]", elems.join(", "))
            }
        }
        MirConstant::GlobalRef(name) => format!("@{}", name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── mir_type_to_llvm_str tests ──────────────────────────────────────

    #[test]
    fn test_mir_type_to_llvm_str_unit() {
        assert_eq!(mir_type_to_llvm_str(&MIRType::Unit), "void");
    }

    #[test]
    fn test_mir_type_to_llvm_str_never() {
        assert_eq!(mir_type_to_llvm_str(&MIRType::Never), "void");
    }

    #[test]
    fn test_mir_type_to_llvm_str_bool() {
        assert_eq!(mir_type_to_llvm_str(&MIRType::Bool), "i1");
    }

    #[test]
    fn test_mir_type_to_llvm_str_int_variants() {
        assert_eq!(mir_type_to_llvm_str(&MIRType::Int(8)), "i8");
        assert_eq!(mir_type_to_llvm_str(&MIRType::Int(16)), "i16");
        assert_eq!(mir_type_to_llvm_str(&MIRType::Int(32)), "i32");
        assert_eq!(mir_type_to_llvm_str(&MIRType::Int(64)), "i64");
    }

    #[test]
    fn test_mir_type_to_llvm_str_float_variants() {
        assert_eq!(mir_type_to_llvm_str(&MIRType::Float(32)), "float");
        assert_eq!(mir_type_to_llvm_str(&MIRType::Float(64)), "double");
    }

    #[test]
    fn test_mir_type_to_llvm_str_pointer() {
        let ptr_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
        assert_eq!(mir_type_to_llvm_str(&ptr_ty), "i8*");
    }

    #[test]
    fn test_mir_type_to_llvm_str_ref() {
        let ref_ty = MIRType::Ref(Box::new(MIRType::Int(64)));
        assert_eq!(mir_type_to_llvm_str(&ref_ty), "i64*");
    }

    #[test]
    fn test_mir_type_to_llvm_str_array() {
        let arr_ty = MIRType::Array(Box::new(MIRType::Int(32)), 10);
        assert_eq!(mir_type_to_llvm_str(&arr_ty), "[10 x i32]");
    }

    #[test]
    fn test_mir_type_to_llvm_str_tuple() {
        let tuple_ty = MIRType::Tuple(vec![MIRType::Int(64), MIRType::Bool]);
        assert_eq!(mir_type_to_llvm_str(&tuple_ty), "{i64, i1}");
    }

    #[test]
    fn test_mir_type_to_llvm_str_empty_tuple() {
        let empty_tuple = MIRType::Tuple(vec![]);
        assert_eq!(mir_type_to_llvm_str(&empty_tuple), "void");
    }

    #[test]
    fn test_mir_type_to_llvm_str_fn() {
        let fn_ty = MIRType::Fn {
            params: vec![MIRType::Int(64), MIRType::Bool],
            ret: Box::new(MIRType::Int(64)),
        };
        assert_eq!(mir_type_to_llvm_str(&fn_ty), "i64 (i64, i1)*");
    }

    #[test]
    fn test_mir_type_to_llvm_str_enum() {
        let enum_ty = MIRType::Enum {
            discr_type: Box::new(MIRType::Int(64)),
            variants: vec![(0, None), (1, Some(MIRType::Int(64)))],
        };
        assert_eq!(mir_type_to_llvm_str(&enum_ty), "{ i64, i64 }");
    }

    // ── local_name tests ────────────────────────────────────────────────

    #[test]
    fn test_local_name_param() {
        let local = Local::new(1, LocalKind::Param);
        assert_eq!(local_name(local), "%l_1");
    }

    #[test]
    fn test_local_name_temp() {
        let local = Local::new(5, LocalKind::Temp);
        assert_eq!(local_name(local), "%t_5");
    }

    #[test]
    fn test_local_name_user() {
        let local = Local::new(3, LocalKind::User);
        assert_eq!(local_name(local), "%u_3");
    }

    #[test]
    fn test_local_name_return() {
        let local = Local::new(0, LocalKind::Return);
        assert_eq!(local_name(local), "%ret_0");
    }

    // ── emit_indent tests ───────────────────────────────────────────────

    #[test]
    fn test_emit_indent_zero() {
        let mut buf = String::new();
        emit_indent(&mut buf, 0);
        assert_eq!(buf, "");
    }

    #[test]
    fn test_emit_indent_one() {
        let mut buf = String::new();
        emit_indent(&mut buf, 1);
        assert_eq!(buf, "  ");
    }

    #[test]
    fn test_emit_indent_three() {
        let mut buf = String::new();
        emit_indent(&mut buf, 3);
        assert_eq!(buf, "      ");
    }

    // ── binary_op_to_llvm tests ─────────────────────────────────────────

    #[test]
    fn test_binary_op_int_arithmetic() {
        let int_ty = MIRType::Int(64);
        assert_eq!(binary_op_to_llvm(MirBinOp::Add, &int_ty), "add");
        assert_eq!(binary_op_to_llvm(MirBinOp::Sub, &int_ty), "sub");
        assert_eq!(binary_op_to_llvm(MirBinOp::Mul, &int_ty), "mul");
        assert_eq!(binary_op_to_llvm(MirBinOp::Div, &int_ty), "sdiv");
        assert_eq!(binary_op_to_llvm(MirBinOp::Rem, &int_ty), "srem");
    }

    #[test]
    fn test_binary_op_float_arithmetic() {
        let float_ty = MIRType::Float(64);
        assert_eq!(binary_op_to_llvm(MirBinOp::Add, &float_ty), "fadd");
        assert_eq!(binary_op_to_llvm(MirBinOp::Sub, &float_ty), "fsub");
        assert_eq!(binary_op_to_llvm(MirBinOp::Mul, &float_ty), "fmul");
        assert_eq!(binary_op_to_llvm(MirBinOp::Div, &float_ty), "fdiv");
        assert_eq!(binary_op_to_llvm(MirBinOp::Rem, &float_ty), "frem");
    }

    #[test]
    fn test_binary_op_int_comparison() {
        let int_ty = MIRType::Int(64);
        assert_eq!(binary_op_to_llvm(MirBinOp::Eq, &int_ty), "icmp eq");
        assert_eq!(binary_op_to_llvm(MirBinOp::Ne, &int_ty), "icmp ne");
        assert_eq!(binary_op_to_llvm(MirBinOp::Lt, &int_ty), "icmp slt");
        assert_eq!(binary_op_to_llvm(MirBinOp::Gt, &int_ty), "icmp sgt");
        assert_eq!(binary_op_to_llvm(MirBinOp::Le, &int_ty), "icmp sle");
        assert_eq!(binary_op_to_llvm(MirBinOp::Ge, &int_ty), "icmp sge");
    }

    #[test]
    fn test_binary_op_float_comparison() {
        let float_ty = MIRType::Float(64);
        assert_eq!(binary_op_to_llvm(MirBinOp::Eq, &float_ty), "fcmp oeq");
        assert_eq!(binary_op_to_llvm(MirBinOp::Ne, &float_ty), "fcmp one");
        assert_eq!(binary_op_to_llvm(MirBinOp::Lt, &float_ty), "fcmp olt");
        assert_eq!(binary_op_to_llvm(MirBinOp::Gt, &float_ty), "fcmp ogt");
        assert_eq!(binary_op_to_llvm(MirBinOp::Le, &float_ty), "fcmp ole");
        assert_eq!(binary_op_to_llvm(MirBinOp::Ge, &float_ty), "fcmp oge");
    }

    #[test]
    fn test_binary_op_bitwise() {
        let int_ty = MIRType::Int(64);
        assert_eq!(binary_op_to_llvm(MirBinOp::BitAnd, &int_ty), "and");
        assert_eq!(binary_op_to_llvm(MirBinOp::BitOr, &int_ty), "or");
        assert_eq!(binary_op_to_llvm(MirBinOp::BitXor, &int_ty), "xor");
        assert_eq!(binary_op_to_llvm(MirBinOp::Shl, &int_ty), "shl");
        assert_eq!(binary_op_to_llvm(MirBinOp::Shr, &int_ty), "ashr");
    }

    #[test]
    fn test_binary_op_logical() {
        let bool_ty = MIRType::Bool;
        assert_eq!(binary_op_to_llvm(MirBinOp::LogAnd, &bool_ty), "and");
        assert_eq!(binary_op_to_llvm(MirBinOp::LogOr, &bool_ty), "or");
    }

    // ── mir_constant_to_llvm_str tests ──────────────────────────────────

    #[test]
    fn test_mir_constant_unit() {
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Unit), "0");
    }

    #[test]
    fn test_mir_constant_bool() {
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Bool(true)), "1");
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Bool(false)), "0");
    }

    #[test]
    fn test_mir_constant_int() {
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Int(42)), "42");
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Int(-1)), "-1");
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Int(0)), "0");
    }

    #[test]
    fn test_mir_constant_uint() {
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Uint(100)), "100");
    }

    #[test]
    fn test_mir_constant_float() {
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Float(3.14)), "3.14");
    }

    #[test]
    fn test_mir_constant_char() {
        assert_eq!(mir_constant_to_llvm_str(&MirConstant::Char('A')), "65");
    }

    #[test]
    fn test_mir_constant_string() {
        assert_eq!(
            mir_constant_to_llvm_str(&MirConstant::String("hello".to_string())),
            "null"
        );
    }

    #[test]
    fn test_mir_constant_bytes() {
        assert_eq!(
            mir_constant_to_llvm_str(&MirConstant::Bytes(vec![1, 2, 3])),
            "[i8 1, i8 2, i8 3]"
        );
        assert_eq!(
            mir_constant_to_llvm_str(&MirConstant::Bytes(vec![])),
            "zeroinitializer"
        );
    }

    #[test]
    fn test_mir_constant_global_ref() {
        assert_eq!(
            mir_constant_to_llvm_str(&MirConstant::GlobalRef("my_func".to_string())),
            "@my_func"
        );
    }
}
