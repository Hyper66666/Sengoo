## ADDED Requirements

### Requirement: Async Phase-2 change artifacts SHALL describe the shipped core surface on `main`
The change artifacts SHALL record the already-shipped async Phase-2 surface
instead of presenting it as entirely future work.

#### Scenario: Compiler evidence exists for the core surface
- **WHEN** maintainers inspect async Phase-2 evidence
- **THEN** the artifacts reference compiler tests covering async blocks, timer-related builtins, spawn/task-lifecycle builtins, join, and select lowering

#### Scenario: Native/runtime evidence exists for the core surface
- **WHEN** maintainers inspect async Phase-2 evidence
- **THEN** the artifacts reference `sgc` runtime tests covering async block execution, timer behavior, spawn/task lifecycle behavior, and join/select execution on the currently supported shapes

### Requirement: Async Phase-2 change artifacts SHALL document current remaining boundaries
The change SHALL distinguish shipped Phase-2 functionality from still-open follow-up work.

#### Scenario: Remaining boundaries are explicit
- **WHEN** maintainers read the change artifacts
- **THEN** the artifacts state that cyclic async CFG, richer frame types such as payload-carrying enums across `await`, and the final generalized select surface remain follow-up work
