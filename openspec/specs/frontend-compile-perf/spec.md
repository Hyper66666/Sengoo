# frontend-compile-perf Specification

## Purpose
Defines evidence-driven frontend compiler performance requirements, including phase timing observability, linear lowering behavior, and verification gates.
## Requirements
### Requirement: Per-phase frontend timing observability

The compiler driver SHALL provide opt-in, per-phase frontend timing output that attributes compile time to `parse`, `typeck`, `hir_lower`, `mir_lower`, `mir_opt`, and `codegen`, so optimization decisions are driven by measured data. Enabling the output MUST NOT change any compilation result.

#### Scenario: Phase timing enabled

- **WHEN** a build or run is invoked with the per-phase timing toggle enabled
- **THEN** the driver emits a per-phase breakdown (including the HIR-lower / MIR-lower / MIR-opt sub-split) to stderr without altering the produced artifact

#### Scenario: Phase timing disabled by default

- **WHEN** the toggle is not set
- **THEN** no timing output is emitted and compilation behavior is identical to before the instrumentation existed

### Requirement: Lowering scales linearly in function count

HIR→MIR lowering SHALL NOT duplicate program-global lowering tables (the set of known function names and the function-signature table) once per function. Per-function lowering work MUST stay proportional to that function's own body plus any instances it materializes, not to the size of the whole program.

#### Scenario: Lowering a many-function module

- **WHEN** a module containing N independent non-generic functions is lowered to MIR
- **THEN** lowering does not perform a per-function full copy of the program-global function tables, so total lowering cost grows about linearly in N rather than quadratically

#### Scenario: Materializing instances during lowering

- **WHEN** lowering a function materializes a generic, lambda, or async instance that must be registered in the lowering tables
- **THEN** the new entry is added without changing the observable MIR output relative to the pre-optimization lowering

### Requirement: Profile-first optimization acceptance gate

Each frontend compile-performance change SHALL be selected from measured phase data and accepted only when it keeps the verification baseline green and shows a bench-measured frontend effect (improvement, or no regression for enabling changes).

#### Scenario: Selecting the next optimization target

- **WHEN** a new frontend optimization is proposed
- **THEN** its target phase is identified from `SENGOO_PHASE_TIMINGS` measurements before implementation begins

#### Scenario: Accepting a performance change

- **WHEN** a frontend performance change is completed
- **THEN** the four verification suites (`sengoo-compiler --lib`, `sgc`, `sengoo-runtime --lib`, `sgpm`) pass and a before/after per-phase measurement is recorded

### Requirement: No source-language behavior change

Frontend compile-performance optimizations SHALL be internal representation/scheduling changes and MUST NOT change Sengoo source syntax, typing rules, runtime ABI, or generated program behavior.

#### Scenario: Compiling existing programs after an optimization

- **WHEN** existing Sengoo programs, examples, and tests are compiled after any frontend optimization in this program
- **THEN** accepted programs remain accepted, rejected programs remain rejected for the same user-facing reasons, and generated runtime behavior is unchanged
