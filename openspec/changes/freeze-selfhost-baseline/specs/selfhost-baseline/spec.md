## ADDED Requirements

### Requirement: The Windows self-hosting corpus SHALL retain authoritative compatibility inputs

The baseline SHALL inventory edition 2026 and Windows Stable/Supported-subset
language, runtime, package, formatter, LSP, realworld and distribution gates from
the current reference/support documents. Each expanded input SHALL have a
recorded content identity. Experimental and unavailable-host work SHALL remain
separately classified. An inventory SHALL NOT count as successful execution.

#### Scenario: A corpus baseline is recorded

- **WHEN** the runner expands the frozen manifest
- **THEN** its report identifies authoritative documents, actual retained input files and source/tool revision information
- **AND** later changes, additions or removals invalidate that measured input identity

### Requirement: Daily measurements SHALL use fixed behavior and comparable sampling

Each frozen startup/check/full-build/incremental-build/format/runtime/package/LSP
workload SHALL warm up then retain at least ten raw timing and root-memory
samples. Native programs SHALL use the recorded optimization level. Candidate
comparison SHALL alternate reference/candidate order with separate project
caches and identical input/options. Shared/native and OS cache scope SHALL be
explicit.

#### Scenario: Reference and candidate workloads run

- **WHEN** both toolchains are measured
- **THEN** each workload has at least ten measured samples after warmup and raw commands/status/output
- **AND** median candidate time <=110% and median root/tree peak memory <=115% of reference are required independently
- **AND** existing compiler-scale/resource/artifact/compatibility gates remain additional requirements

#### Scenario: Short native processes are measured

- **WHEN** a process exits before a periodic memory sample
- **THEN** its held native process handle still supplies the OS lifetime root working-set peak
- **AND** missing memory data or sampler failure cannot become a zero-memory successful sample

### Requirement: LSP measurements SHALL assert a real diagnostic lifecycle

The recorded embedded-analysis LSP workload SHALL initialize, open valid source,
observe clean diagnostics, edit to a known invalid program, observe the expected
error, and gracefully shutdown/exit over stdio JSON-RPC. It SHALL exclude random
host compiler discovery and explicitly distinguish external compiler integration.

#### Scenario: A document becomes invalid

- **WHEN** a known variable reference is changed to the frozen undefined identifier
- **THEN** the actual LSP process reports that identifier as an error and edit-to-diagnostic latency is retained
- **AND** identical isolated LSP bytes, lifecycle responses and memory identity are recorded

### Requirement: Release qualification SHALL bind to measured tools without relabeling provisional evidence

Unqualified tools SHALL produce only provisional baseline evidence. A qualified
release result SHALL require successful explicit release qualification for the
same four tool hashes and source revision. Later qualification SHALL preserve
the original raw report and bind it by digest after unchanged input/tool checks.
Development/concurrent-load measurement SHALL remain provisional and SHALL NOT
be promoted when tools later qualify; a fresh quiet-host measurement is required.

#### Scenario: Release qualification arrives after measurement

- **WHEN** matching successful release evidence is supplied for an unchanged completed provisional run
- **THEN** a separate attachment identifies the original report digest and exact qualified tool hashes
- **AND** it does not claim new samples or qualify failed/changed measurement
