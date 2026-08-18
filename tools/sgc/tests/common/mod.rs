use std::process::Command;

pub(crate) fn source_sgc_command() -> Command {
    if let Some(installed_sgc) = std::env::var_os("SENGOO_TEST_INSTALLED_SGC") {
        return Command::new(installed_sgc);
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_sgc"));
    command.args(["--runtime-mode", "source-development"]);
    command
}
