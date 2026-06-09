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

### Requirement: Production compile-scale gates SHALL close absolute 1000k budgets

Sengoo SHALL treat the synthetic 1000k LOC workload on the pinned reference CI
host as a production gate: median peak RSS at most 1.8× the C++ baseline and
median frontend phase share at most 65% of compile-stage time.

#### Scenario: 1000k absolute gate passes on the reference host

- **WHEN** `advanced_pipeline_bench.py` runs the 1000k workload in default pipeline
  mode on the pinned host using the median of three runs
- **THEN** peak RSS is at most 1.8× the checked-in C++ baseline snapshot
- **AND** frontend phase time is at most 65% of total compile-stage time
- **AND** end-to-end compile time remains faster than the C++ baseline unless
  explicitly superseded by a later change

#### Scenario: Relative regression gate remains mandatory

- **WHEN** a pull request regresses peak RSS by more than 10%, frontend share by
  more than 5 percentage points, or end-to-end time by more than 10% against the
  checked-in snapshot
- **THEN** the perf gate job fails before merge
- **AND** snapshot updates require checked-in before/after evidence in the change
  `INVENTORY.md`

### Requirement: Ladder workloads SHALL report compile-scale progress before 1000k closes

Sengoo SHALL publish 100k and optional 2500k ladder measurements on the same
host profile so optimization work is trackable before the 1000k absolute gate passes.

#### Scenario: 100k ladder stays within its budget

- **WHEN** the 100k workload runs on the pinned host with the median of three runs
- **THEN** peak RSS is at most 1.5× the C++ 100k baseline
- **AND** frontend share is at most 70%

#### Scenario: 2500k stretch workload is reported without blocking archive

- **WHEN** the 2500k workload is runnable on the reference host
- **THEN** CI or `INVENTORY.md` records RSS ratio and frontend share
- **AND** failing the 2500k stretch targets does not block archive once 1000k
  absolute targets pass
