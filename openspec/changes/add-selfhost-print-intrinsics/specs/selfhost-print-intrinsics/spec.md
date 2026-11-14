## ADDED Requirements

### Requirement: Bounded SG print lowering SHALL preserve checked builtin identity and runtime ABI

The SG compiler SHALL lower resolved print, println and eprintln compiler
builtins for signed i64 and shared str through fixed external C runtime
declarations, while retaining a distinct language callable signature. Other
printable types SHALL remain explicitly unsupported until separately qualified.

#### Scenario: A checked scalar or borrowed-text builtin is lowered
- **WHEN** an exact compiler builtin identity, direct source syntax, one matching typed argument and unit result pass validation
- **THEN** lowering SHALL emit a typed runtime call to the correct stream with C convention and the original source span
- **AND** newline, integer, UTF-8 and embedded-NUL behavior SHALL match the frozen reference/runtime ABI

#### Scenario: A named function is not a compiler builtin
- **WHEN** a source function or method has a matching print spelling
- **THEN** that spelling alone SHALL NOT authorize builtin dispatch
- **AND** an incompatible or source-defined reserved runtime symbol SHALL fail rather than intercept the intrinsic

#### Scenario: Saved print authorization is corrupted
- **WHEN** builtin identity, flags, declaration provenance, source syntax, AST edges, call resolution or callable/argument/result types are changed
- **THEN** pure HIR validation SHALL reject the inconsistent facts before MIR emission
- **AND** clone and wire roundtrip SHALL preserve valid authorization without reconstructing semantic facts

#### Scenario: An imported unused body prints
- **WHEN** a concrete imported body contains a supported print call even if main does not call it
- **THEN** ordinary body checking and lowering SHALL preserve that call
- **AND** import qualification SHALL NOT skip unreachable functions to evade unsupported behavior
