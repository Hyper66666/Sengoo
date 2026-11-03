## ADDED Requirements

### Requirement: Generated ownership coverage SHALL retain reproducible failures

The validation tooling SHALL generate a bounded deterministic set of supported
ownership/control-flow combinations and separately asserted unsupported cases.
Successful native cases SHALL restore live resource counts and destroy each
observed owner exactly once. Failures SHALL retain seed, source and available IR.

#### Scenario: A generated case leaks or releases twice

- **WHEN** a seeded supported case fails an ownership oracle or sanitizer
- **THEN** the gate exits nonzero and retains enough inputs to replay the failure.

### Requirement: Incremental index imports SHALL be atomic and bounded

An unchanged validated source SHALL avoid entry rewrites. A changed source SHALL
replace its rows and source metadata atomically. A malformed, over-limit or
failed database import SHALL preserve the previously committed index.

#### Scenario: A previously indexed file becomes malformed

- **WHEN** the user imports malformed replacement CSV at the same source path
- **THEN** the command fails with an actionable diagnostic and prior rows remain queryable.

### Requirement: Python qualification SHALL exercise independent batch consumers

The consumer SHALL validate real batches, Unicode, explicit errors and conversion
failure cleanup. Repeated calls SHALL use native resource oracles and record
reproducible cost observations without asserting machine-independent timings.
Only repaired wheels SHALL reach clean offline consumers on supported hosts.

#### Scenario: A repaired wheel changes its platform tag

- **WHEN** platform repair produces a new wheel filename
- **THEN** clean consumption selects the repaired artifact and excludes raw wheels.

### Requirement: Candidate evidence SHALL distinguish preparation from execution

A candidate SHALL identify a committed revision. Its evidence SHALL reject
wrong revisions, missing platforms, failed or skipped required jobs and expired
required artifacts. Stable release evidence requirements SHALL remain unchanged.

#### Scenario: A required platform has no completed run

- **WHEN** candidate qualification is evaluated
- **THEN** the report remains incomplete and SHALL NOT claim that platform passed.

### Requirement: Independent pilot observations SHALL come from actual participants

The pilot kit SHALL provide verified artifact identities, tasks, environment
capture and a blocker log. Pilot completion SHALL require actual observations
from three to five independent people over two weeks and zero open blockers.

#### Scenario: Only internal acceptance has passed

- **WHEN** no independent participant observations have been collected
- **THEN** the pilot remains not started even if all internal checks pass.
