## ADDED Requirements

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
