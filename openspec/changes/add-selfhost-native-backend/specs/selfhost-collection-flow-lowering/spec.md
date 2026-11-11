## ADDED Requirements

### Requirement: Internal collection flow lowering SHALL preserve ordinary lowering behavior

The internal flow port SHALL consume only a fresh source entry's concrete
site/job configuration and SHALL NOT authenticate saved candidate or MIR input.
Ordinary and constructor-only wrappers SHALL keep their existing behavior;
only the separate flow mode MAY emit physically validated optional borrowing.

#### Scenario: An ordinary source is lowered after adding the flow port
- **WHEN** an ordinary or constructor-only entry invokes its existing wrapper
- **THEN** the flow configuration SHALL be absent and existing validation SHALL remain strict
- **AND** typed MIR, projection, disarm and the complete existing source-codegen regression corpus SHALL retain their recorded behavior and explicit reference gaps

#### Scenario: A flow configuration exceeds its bounded shape
- **WHEN** role/provider counts, factory/hook widths or budgets, root budgets or mode-specific fields are invalid
- **THEN** lowering SHALL reject before cloning the flow vectors or emitting any function/instruction
- **AND** all actual checked input, type/symbol state and first-call resource counts SHALL remain unchanged

### Requirement: A proved internal flow SHALL retain typed cleanup and physical payload validation

The flow port SHALL lower only its explicit concrete jobs and source sites,
generate typed callbacks/cleanup for the actual T, transfer sink ownership, and
validate the complete emitted MIR without weakening ordinary validation.

#### Scenario: An admitted primitive collection flow is emitted
- **WHEN** the fresh source entry supplies exact constructor, factory, provider and nine-word helper/push/next site rows for i64 or bool
- **THEN** lowering SHALL retain true/false edge identity, exact whole-Option payload guards and concrete typed cleanup
- **AND** generated callback and cleanup bodies SHALL be independently revalidated even when the complete module contains opcode27
- **AND** any additional executable source job, unsupported call, unresolved type, malformed site or missing physical payload proof SHALL reject

#### Scenario: A push sink receives an already owned source temporary
- **WHEN** the actual physical sink takes responsibility for the by-value argument
- **THEN** lowering SHALL disarm that exact original temporary and skip only its proved whole-root cleanup actions
- **AND** unknown child paths SHALL reject rather than skip cleanup
- **AND** field zeroing SHALL NOT substitute for an initialization or ownership proof
