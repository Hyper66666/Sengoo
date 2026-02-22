use sengoo_compiler::compile_to_ir;
use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(input_path) = args.next() else {
        eprintln!("usage: cargo run -p sengoo-compiler --bin emit_ir -- <input.sg> <output.ll>");
        return ExitCode::from(2);
    };
    let Some(output_path) = args.next() else {
        eprintln!("usage: cargo run -p sengoo-compiler --bin emit_ir -- <input.sg> <output.ll>");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: too many arguments");
        return ExitCode::from(2);
    }

    let source = match fs::read_to_string(&input_path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("failed to read `{}`: {}", input_path, err);
            return ExitCode::from(1);
        }
    };

    let ir = match compile_to_ir(&source) {
        Ok(ir) => ir,
        Err(err) => {
            eprintln!("failed to compile `{}`: {}", input_path, err);
            return ExitCode::from(1);
        }
    };

    if let Err(err) = fs::write(&output_path, ir) {
        eprintln!("failed to write `{}`: {}", output_path, err);
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}
