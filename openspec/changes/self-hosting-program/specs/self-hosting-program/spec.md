## ADDED Requirements

### Requirement: Self-hosting integration SHALL use explicit milestones and unique owners

The self-hosting program SHALL track M0-M8 and one implementation owner for
each behavioral requirement. It SHALL retain incomplete compatibility work after
the first subset self-build and SHALL distinguish stage0 execution from native
self-hosting. Windows is the first completion target; other-host and experimental
backend claims require separate evidence.

#### Scenario: The first SG compiler compiles its own subset

- **WHEN** a minimal SG compiler produces another executable compiler
- **THEN** the program records the applicable intermediate milestone and remaining gaps
- **AND** it does not mark all four tools, compatibility or default cutover complete

#### Scenario: Foundation implementation is delivered

- **WHEN** the seed lock, stage0 entry and SG source/span/symbol/node arenas are implemented
- **THEN** `add-selfhost-foundations` owns their behavioral requirements
- **AND** this umbrella records their verified integration result without duplicating the requirements

### Requirement: Default cutover SHALL require complete native self-build evidence

Default Windows toolchain replacement SHALL require the full frozen supported
corpus, all four SG tools, SG runtime, SG native backend/linker, real native
debugging and identical S2/S3 unsigned artifacts. Final self-build inputs SHALL
be SG tools, SG sources and locked native SDK inputs; no external compiler,
assembler or linker may execute on that path. SDK source rebuilding remains a
separate procedure.

#### Scenario: A complete native self-build is verified

- **WHEN** S1 builds S2 and S2 builds S3 from identical source, SDK, target and options
- **THEN** the four tools, linker, SG runtime objects and PDBs compare byte for byte
- **AND** different-directory and empty-cache repetitions preserve the result
- **AND** subprocess evidence demonstrates that no Rust, clang/LLVM, external assembler or external linker executed

#### Scenario: A required compatibility or debugging check is absent

- **WHEN** supported behavior, a required host check or actual Windows debugger verification is missing, skipped or failing
- **THEN** default cutover remains incomplete
- **AND** metadata inspection or an older passing artifact does not substitute for the missing result

### Requirement: Cutover SHALL preserve measurable development performance and release boundaries

The same-host frozen representative workloads SHALL retain candidate median
latency at most 110% and peak RSS at most 115% of reference while existing
performance/resource gates remain enforced. Two consecutive complete candidates
and a tested rollback SHALL precede default cutover. Windows self-hosting preview
results SHALL NOT replace existing four-host stable publication requirements.

#### Scenario: Candidate meets correctness but exceeds a performance limit

- **WHEN** a candidate passes correctness but a frozen workload exceeds its time, memory or existing resource gate
- **THEN** default cutover remains blocked on that gate
- **AND** the raw result is retained without silently changing its workload or baseline

#### Scenario: A Windows preview is packaged

- **WHEN** Windows preview artifacts are produced before other self-hosting ports
- **THEN** they retain explicit Windows preview scope
- **AND** Linux/macOS and stable multi-host publication are not reported as passed
