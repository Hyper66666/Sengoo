## ADDED Requirements

### Requirement: 1000k compile workloads SHALL meet published memory and frontend-share budgets

Sengoo SHALL publish and enforce compile-stage budgets for the synthetic 1000k LOC
workload on a pinned reference CI host profile.

#### Scenario: Peak RSS meets the 1.8x target on the reference host

- **WHEN** `advanced_pipeline_bench.py` runs the 1000k workload in default pipeline
  mode on the pinned CI host profile using the median of three runs
- **THEN** peak RSS is at most 1.8x the C++ baseline recorded in the reference snapshot
- **AND** the host profile, generator seed, compiler revisions, and baseline command
  are recorded in this change's `INVENTORY.md`

#### Scenario: Frontend time share meets the 65% target

- **WHEN** the same 1000k benchmark runs on the same host profile
- **THEN** frontend phase time is at most 65% of total compile-stage time
- **AND** end-to-end compile time remains faster than the C++ baseline unless this
  change is explicitly superseded

#### Scenario: Performance regressions always fail CI

- **WHEN** a pull request regresses peak RSS by more than 10%, frontend share by
  more than 5 percentage points, or end-to-end compile time by more than 10%
  against the checked-in reference snapshot
- **THEN** the perf gate job fails with before/after snapshot paths
- **AND** this relative regression gate remains active before and after the
  absolute RSS and frontend-share targets are met
- **AND** updating the reference snapshot requires checked-in before/after evidence

### Requirement: Type interning and frontend memory reductions SHALL support the 1000k gate

Frontend compile-performance work for this change SHALL include measured memory
reductions in type-checking and lowering hot paths without changing source
semantics.

#### Scenario: Interning or pruning reduces frontend RSS on 1000k

- **WHEN** the 1000k workload is measured before and after an accepted frontend
  memory optimization from this change
- **THEN** peak RSS decreases or remains within the regression gate
- **AND** `cargo test -p sengoo-compiler --lib` and `cargo test -p sgc` remain green

#### Scenario: Phase timing remains available for target selection

- **WHEN** engineers investigate frontend share on the 1000k workload
- **THEN** per-phase timing output remains available through the existing
  `frontend-compile-perf` observability requirement
- **AND** optimization targets are chosen from measured phase data
