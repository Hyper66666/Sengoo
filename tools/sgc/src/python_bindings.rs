//! Typed Python bridge generation. Reflection strings are deliberately not an ABI input.
use miette::{IntoDiagnostic, Result};
use sengoo_compiler::codegen::common::{mir_type_to_llvm_str, mir_value_type_to_llvm_str};
use sengoo_compiler::mir::{MIRType, MirFunction};
use sengoo_compiler::typeck::ty::{FloatKind, IntKind, Ty, TyKind};
use sengoo_compiler::{DeclKind, Parser, TypeChecker};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PythonType {
    I64,
    F64,
    Bool,
    Str,
    String,
    VecI64,
    VecF64,
    Unit,
}

impl PythonType {
    fn owner_kind(&self) -> i64 {
        match self {
            Self::String => 1,
            Self::VecI64 | Self::VecF64 => 2,
            _ => 0,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct Parameter {
    pub name: String,
    pub ty: PythonType,
}

#[derive(Debug, Serialize)]
pub(crate) struct PythonFunction {
    pub name: String,
    pub symbol: String,
    pub params: Vec<Parameter>,
    pub returns: PythonType,
    pub result: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct Descriptor {
    pub schema_version: u32,
    pub binding_abi_version: u32,
    pub identity: String,
    pub toolchain_version: String,
    pub target: String,
    pub functions: Vec<PythonFunction>,
}

#[derive(Debug)]
pub(crate) struct Bridge {
    pub descriptor: Descriptor,
    pub llvm: String,
    pub c_source: String,
    pub exports: Vec<String>,
}

fn python_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
        && ![
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "try", "while", "with", "yield",
        ]
        .contains(&name)
}

fn python_type(ty: &Ty, output: bool) -> Result<PythonType> {
    Ok(match &ty.kind {
        TyKind::Int(IntKind::I64) => PythonType::I64,
        TyKind::Float(FloatKind::F64) => PythonType::F64,
        TyKind::Bool => PythonType::Bool,
        TyKind::Unit => PythonType::Unit,
        TyKind::Tuple(types) if types.is_empty() => PythonType::Unit,
        TyKind::Ref(false, base) if !output && matches!(base.kind, TyKind::Str) => PythonType::Str,
        TyKind::Adt { name, args } if name == "String" && args.is_empty() => PythonType::String,
        TyKind::Adt { name, args } if name == "Vec" && args.len() == 1 => match &args[0].kind {
            TyKind::Int(IntKind::I64) => PythonType::VecI64,
            TyKind::Float(FloatKind::F64) => PythonType::VecF64,
            _ => miette::bail!("Python bindings support only Vec<i64> and Vec<f64>"),
        },
        _ => miette::bail!("unsupported Python {} type `{ty}`; borrowed returns, objects, callbacks, async and generics are not supported", if output { "return" } else { "parameter" }),
    })
}

/// Validate using the checked symbol environment, then use MIR layouts for code generation.
pub(crate) fn generate(
    source: &str,
    names: &[String],
    functions: &[MirFunction],
) -> Result<Bridge> {
    let ast = Parser::parse(source).map_err(|e| miette::miette!("{e}"))?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&ast)
        .map_err(|e| miette::miette!("{e}"))?;
    let env = checker.into_env();
    let mut descriptors = Vec::new();
    let mut llvm = String::from("\n; Typed Python binding ABI v1\n");
    llvm.push_str("declare i64 @__sgpy_bits(ptr, i64)\ndeclare ptr @__sgpy_ptr(ptr, i64)\ndeclare i64 @__sgpy_make(ptr, i64, i64)\ndeclare void @__sgpy_discard(i64, i64)\ndeclare void @__sgpy_reset(ptr)\ndeclare void @__sgpy_error(ptr, i64, i64)\ndeclare void @__sgpy_scalar(ptr, i64)\ndeclare void @__sgpy_owned(ptr, i64, i64)\n");
    let mut unique = BTreeSet::new();
    for name in names {
        if !unique.insert(name.clone()) {
            miette::bail!("duplicate Python export `{name}`");
        }
        let declaration = ast
            .decls
            .iter()
            .find_map(|decl| match &decl.kind {
                DeclKind::Function(function) if function.name.name == *name => Some(function),
                _ => None,
            })
            .ok_or_else(|| {
                miette::miette!("Python export `{name}` is not an existing free function")
            })?;
        if declaration.is_async || declaration.is_unsafe || !declaration.type_params.is_empty() {
            miette::bail!("Python export `{name}` must be synchronous, safe and non-generic");
        }
        if !python_identifier(name) || name == "SengooError" || name.starts_with("__") {
            miette::bail!("Python export names must be non-reserved ASCII Python identifiers");
        }
        let checked = env
            .lookup(name)
            .and_then(|symbol| symbol.get_ty())
            .ok_or_else(|| miette::miette!("missing checked type for `{name}`"))?;
        let TyKind::Fn { params, ret, .. } = &checked.kind else {
            miette::bail!("`{name}` is not a checked function");
        };
        let (return_ty, result) = match &ret.kind {
            TyKind::Adt { name, args } if name == "Result" && args.len() == 2 => {
                if !matches!(args[1].kind, TyKind::Int(IntKind::I64)) {
                    miette::bail!("Python Result error type must be i64");
                }
                (&args[0], true)
            }
            _ => (ret.as_ref(), false),
        };
        let descriptor = PythonFunction {
            name: name.clone(),
            symbol: format!("__sgpy_call_{}", descriptors.len()),
            params: params
                .iter()
                .enumerate()
                .map(|(index, ty)| {
                    Ok(Parameter {
                        name: if python_identifier(&declaration.params[index].name.name) {
                            declaration.params[index].name.name.clone()
                        } else {
                            format!("arg{index}")
                        },
                        ty: python_type(ty, false)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            returns: python_type(return_ty, true)?,
            result,
        };
        let mir = functions
            .iter()
            .find(|function| function.name == *name)
            .ok_or_else(|| miette::miette!("missing MIR for Python export `{name}`"))?;
        emit_function(&mut llvm, &descriptor, mir)?;
        descriptors.push(descriptor);
    }
    let encoded = serde_json::to_vec(&descriptors).into_diagnostic()?;
    let identity = format!("{:x}", Sha256::digest(&encoded));
    let mut exports = descriptors
        .iter()
        .map(|function| function.symbol.clone())
        .collect::<Vec<_>>();
    exports.extend(["__sgpy_abi_v1".to_string(), "__sgpy_release_v1".to_string()]);
    let c_source = format!(
        "{}\nSGPY_EXPORT const char* __sgpy_abi_v1(void) {{ return \"{}\"; }}\n",
        include_str!("python_bridge.c"),
        identity
    );
    Ok(Bridge {
        descriptor: Descriptor {
            schema_version: 1,
            binding_abi_version: 1,
            identity,
            toolchain_version: crate::cli::SGC_VERSION.to_string(),
            target: crate::cross_compile::host_triple().to_string(),
            functions: descriptors,
        },
        llvm,
        c_source,
        exports,
    })
}

fn validate_layout(ty: &PythonType, mir: &MIRType) -> Result<()> {
    let valid = match (ty, mir) {
        (PythonType::Unit, MIRType::Tuple(types)) if types.is_empty() => true,
        (PythonType::I64, MIRType::Int(64))
        | (PythonType::F64, MIRType::Float(64))
        | (PythonType::Bool, MIRType::Bool)
        | (PythonType::Unit, MIRType::Unit) => true,
        (PythonType::Str, MIRType::Ptr(base)) => matches!(base.as_ref(), MIRType::Int(8)),
        (PythonType::String, MIRType::Struct { name, fields }) => {
            name == "String" && fields.len() == 1 && fields[0].1 == MIRType::Int(64)
        }
        (PythonType::VecI64 | PythonType::VecF64, MIRType::Struct { name, fields }) => {
            name.starts_with("Vec_")
                && fields.len() == 2
                && fields.iter().all(|field| field.1 == MIRType::Int(64))
        }
        _ => false,
    };
    if !valid {
        miette::bail!("unsupported compiler layout for Python type {ty:?}: {mir:?}");
    }
    Ok(())
}

fn emit_function(ir: &mut String, descriptor: &PythonFunction, mir: &MirFunction) -> Result<()> {
    ir.push_str(&format!(
        "define void @{}(ptr %args, ptr %out) {{\nentry:\n  call void @__sgpy_reset(ptr %out)\n",
        descriptor.symbol
    ));
    let mut arguments = Vec::new();
    let mut owners = Vec::new();
    for (index, (parameter, ty)) in descriptor.params.iter().zip(&mir.params).enumerate() {
        validate_layout(&parameter.ty, ty)?;
        let llvm_ty = mir_value_type_to_llvm_str(ty);
        if parameter.ty == PythonType::Unit {
            arguments.push(format!("{llvm_ty} zeroinitializer"));
            continue;
        }
        let kind = parameter.ty.owner_kind();
        if kind != 0 {
            ir.push_str(&format!("  %h{index} = call i64 @__sgpy_make(ptr %args, i64 {index}, i64 {kind})\n  %ok{index} = icmp sgt i64 %h{index}, 0\n  br i1 %ok{index}, label %ready{index}, label %fail{index}\nfail{index}:\n"));
            for (prior, prior_kind) in &owners {
                ir.push_str(&format!(
                    "  call void @__sgpy_discard(i64 %h{prior}, i64 {prior_kind})\n"
                ));
            }
            ir.push_str(&format!("  call void @__sgpy_error(ptr %out, i64 -1, i64 -1)\n  ret void\nready{index}:\n  %a{index} = insertvalue {llvm_ty} zeroinitializer, i64 %h{index}, 0\n"));
            owners.push((index, kind));
        } else if parameter.ty == PythonType::Str {
            ir.push_str(&format!(
                "  %a{index} = call ptr @__sgpy_ptr(ptr %args, i64 {index})\n"
            ));
        } else {
            ir.push_str(&format!(
                "  %b{index} = call i64 @__sgpy_bits(ptr %args, i64 {index})\n"
            ));
            match parameter.ty {
                PythonType::F64 => {
                    ir.push_str(&format!("  %a{index} = bitcast i64 %b{index} to double\n"))
                }
                PythonType::Bool => {
                    ir.push_str(&format!("  %a{index} = trunc i64 %b{index} to i1\n"))
                }
                _ => ir.push_str(&format!("  %a{index} = add i64 %b{index}, 0\n")),
            }
        }
        arguments.push(format!("{llvm_ty} %a{index}"));
    }
    let ret_ty = mir_type_to_llvm_str(&mir.return_type);
    if ret_ty == "void" {
        ir.push_str(&format!(
            "  call void @{}({})\n",
            mir.name,
            arguments.join(", ")
        ));
        emit_input_cleanup(ir, &owners, None, "unit");
        ir.push_str("  ret void\n}\n");
        return Ok(());
    }
    ir.push_str(&format!(
        "  %ret = call {ret_ty} @{}({})\n",
        mir.name,
        arguments.join(", ")
    ));
    let (payload, payload_ty) = if descriptor.result {
        let MIRType::Struct { fields, .. } = &mir.return_type else {
            miette::bail!("Python Result must use the supported struct layout");
        };
        if fields.len() != 3 || fields[0].1 != MIRType::Bool || fields[2].1 != MIRType::Int(64) {
            miette::bail!("unsupported Result layout");
        }
        ir.push_str(&format!("  %success = extractvalue {ret_ty} %ret, 0\n  %value = extractvalue {ret_ty} %ret, 1\n  br i1 %success, label %success_path, label %error_path\nerror_path:\n  %code = extractvalue {ret_ty} %ret, 2\n  call void @__sgpy_error(ptr %out, i64 1, i64 %code)\n"));
        let kind = descriptor.returns.owner_kind();
        if kind != 0 {
            let payload_llvm = mir_type_to_llvm_str(&fields[1].1);
            ir.push_str(&format!(
                "  %placeholder = extractvalue {payload_llvm} %value, 0\n"
            ));
            emit_input_cleanup(ir, &owners, Some(("%placeholder", kind)), "error");
            ir.push_str(&format!(
                "  call void @__sgpy_discard(i64 %placeholder, i64 {kind})\n"
            ));
        } else {
            emit_input_cleanup(ir, &owners, None, "error");
        }
        ir.push_str("  ret void\nsuccess_path:\n");
        ("%value", &fields[1].1)
    } else {
        ("%ret", &mir.return_type)
    };
    validate_layout(&descriptor.returns, payload_ty)?;
    let payload_llvm = mir_type_to_llvm_str(payload_ty);
    let kind = descriptor.returns.owner_kind();
    if kind != 0 {
        ir.push_str(&format!(
            "  %owner = extractvalue {payload_llvm} {payload}, 0\n"
        ));
        emit_input_cleanup(ir, &owners, Some(("%owner", kind)), "success");
        ir.push_str(&format!(
            "  call void @__sgpy_owned(ptr %out, i64 %owner, i64 {kind})\n"
        ));
    } else {
        emit_input_cleanup(ir, &owners, None, "success");
        if descriptor.returns != PythonType::Unit {
            match descriptor.returns {
                PythonType::F64 => {
                    ir.push_str(&format!("  %bits = bitcast double {payload} to i64\n"))
                }
                PythonType::Bool => ir.push_str(&format!("  %bits = zext i1 {payload} to i64\n")),
                _ => ir.push_str(&format!("  %bits = add i64 {payload}, 0\n")),
            }
            ir.push_str("  call void @__sgpy_scalar(ptr %out, i64 %bits)\n");
        }
    }
    ir.push_str("  ret void\n}\n");
    Ok(())
}

/// The bridge creates legacy String/numeric-Vec handles; by-value parameters
/// deliberately do not have compiler-inserted Drop for these handle types.
/// Release those inputs after the call, preserving a returned alias until the
/// Python output copy and release. A callee may already consume an input, in
/// which case the runtime's generation-checked release is idempotent. This rule
/// does not apply to custom owning objects, which this ABI rejects.
fn emit_input_cleanup(
    ir: &mut String,
    owners: &[(usize, i64)],
    retained: Option<(&str, i64)>,
    path: &str,
) {
    for (index, kind) in owners.iter().rev() {
        if let Some((owner, retained_kind)) =
            retained.filter(|(_, retained_kind)| retained_kind == kind)
        {
            debug_assert_eq!(retained_kind, *kind);
            ir.push_str(&format!("  %keep_{path}_{index} = icmp eq i64 %h{index}, {owner}\n  %discard_{path}_{index} = select i1 %keep_{path}_{index}, i64 0, i64 %h{index}\n  call void @__sgpy_discard(i64 %discard_{path}_{index}, i64 {kind})\n"));
        } else {
            ir.push_str(&format!(
                "  call void @__sgpy_discard(i64 %h{index}, i64 {kind})\n"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bridge(source: &str, name: &str) -> Result<Bridge> {
        let mir = sengoo_compiler::compile_to_mir(source).map_err(|e| miette::miette!("{e}"))?;
        generate(source, &[name.into()], &mir)
    }
    #[test]
    fn typed_metadata_and_scalar_ir() {
        let generated = bridge(
            "def add(a: i64, b: f64) -> bool { a > 0 && b > 0.0 }",
            "add",
        )
        .unwrap();
        assert_eq!(generated.descriptor.functions[0].returns, PythonType::Bool);
        assert!(generated.llvm.contains("call i1 @add(i64 %a0, double %a1)"));
        assert_eq!(generated.descriptor.identity.len(), 64);
    }
    #[test]
    fn rejects_generic_async_and_borrowed_return() {
        assert!(bridge("def borrowed(x: &str) -> &str { x }", "borrowed")
            .unwrap_err()
            .to_string()
            .contains("unsupported Python return"));
        assert!(bridge("def wide(x: i32) -> i32 { x }", "wide").is_err());
    }

    #[test]
    fn explicit_unit_return_and_generic_rejection() {
        assert_eq!(
            bridge("def complete() -> () { () }", "complete")
                .unwrap()
                .descriptor
                .functions[0]
                .returns,
            PythonType::Unit
        );
        assert!(generate(
            "def identity<T>(value: T) -> T { value }",
            &["identity".into()],
            &[]
        )
        .unwrap_err()
        .to_string()
        .contains("non-generic"));
    }

    #[test]
    fn owned_inputs_cleanup_covers_void_scalar_and_aliased_result_paths() {
        let prefix = "struct String { handle: i64 }\n";
        let unit = bridge(
            &format!("{prefix}def ignored(value: String) {{ }}"),
            "ignored",
        )
        .unwrap();
        assert!(unit
            .llvm
            .contains("call void @__sgpy_discard(i64 %h0, i64 1)\n  ret void"));
        let scalar = bridge(
            &format!("{prefix}def length(value: String) -> i64 {{ value.handle }}"),
            "length",
        )
        .unwrap();
        assert!(scalar
            .llvm
            .contains("call void @__sgpy_discard(i64 %h0, i64 1)"));
        let owned = bridge(
            &format!("{prefix}def echo(value: String) -> String {{ value }}"),
            "echo",
        )
        .unwrap();
        assert!(owned
            .llvm
            .contains("%keep_success_0 = icmp eq i64 %h0, %owner"));
        let result = bridge(&format!("{prefix}struct Result<T, E> {{ is_ok: bool, value: T, error: E }}\ndef checked(value: String, fail: bool) -> Result<String, i64> {{ Result {{ is_ok: !fail, value: value, error: 22 }} }}"), "checked").unwrap();
        assert!(result
            .llvm
            .contains("%keep_error_0 = icmp eq i64 %h0, %placeholder"));
        assert!(result
            .llvm
            .contains("%keep_success_0 = icmp eq i64 %h0, %owner"));
    }

    #[test]
    fn nested_vectors_and_custom_owning_objects_are_rejected() {
        let nested = "struct Vec<T> { handle: i64, marker: T }\ndef nested(values: Vec<Vec<i64>>) -> i64 { 0 }";
        assert!(generate(nested, &["nested".into()], &[])
            .unwrap_err()
            .to_string()
            .contains("only Vec<i64> and Vec<f64>"));
        let object =
            "struct Owner { handle: i64 }\ndef object(value: Owner) -> i64 { value.handle }";
        assert!(generate(object, &["object".into()], &[])
            .unwrap_err()
            .to_string()
            .contains("unsupported Python parameter"));
    }

    #[test]
    fn native_bridge_cleans_failed_conversions() {
        use std::fs;
        use std::process::Command;
        let directory =
            std::env::temp_dir().join(format!("sengoo-bridge-probe-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../sengoo-python/tests/bridge_failure_probe.c");
        let target = crate::NativeBuildTarget::host();
        let object = directory.join(format!("probe.{}", target.object_extension()));
        let executable = directory.join(target.default_output_basename("probe"));
        let clang =
            crate::find_clang().expect("clang is required for binding allocation failure tests");
        crate::native_toolchain::compile_c_to_object_for_shared(
            &clang,
            &source,
            &object,
            0,
            Some(&target),
        )
        .unwrap();
        crate::native_toolchain::link_native_binary_from_objects(
            &clang,
            &[object],
            &executable,
            Some(&target),
            None,
        )
        .unwrap();
        assert!(Command::new(&executable).status().unwrap().success());
        fs::remove_dir_all(directory).unwrap();
    }
}
