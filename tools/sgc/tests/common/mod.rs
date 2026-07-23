use std::process::Command;

pub(crate) fn source_sgc_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sgc"));
    command.args(["--runtime-mode", "source-development"]);
    command
}
