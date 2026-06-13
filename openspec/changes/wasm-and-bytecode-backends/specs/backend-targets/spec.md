## ADDED Requirements

### Requirement: The compiler SHALL emit WebAssembly modules

`sgc` SHALL compile programs to `.wasm` and define a WASI-based host-interface
subset for the standard library.

#### Scenario: Run a program under a WASM runtime

- **WHEN** a representative program is built with `--target wasm` and executed
  under a WASM runtime
- **THEN** a valid `.wasm` module is produced and runs to the expected result
- **AND** stdlib calls outside the supported WASI subset are rejected with a
  documented diagnostic rather than miscompiling

### Requirement: The compiler SHALL provide a portable bytecode VM

`sgc` SHALL define a portable bytecode format and an interpreter so programs can
run without a native toolchain (clang/LLVM).

#### Scenario: Clang-free execution on the VM

- **WHEN** a program is run with the bytecode target on a machine without
  clang/LLVM installed
- **THEN** it executes on the interpreter and produces the same result as the
  native build for the core conformance suite

### Requirement: Build targets SHALL be selectable with a documented capability matrix

`sgc build --target {native,wasm,bytecode}` SHALL select the backend, and a
per-target capability matrix SHALL document supported stdlib areas.

#### Scenario: Selecting a target

- **WHEN** a user passes `--target wasm` or `--target bytecode`
- **THEN** `sgc` produces the corresponding artifact
- **AND** the capability matrix documents which stdlib areas are supported on
  that target
