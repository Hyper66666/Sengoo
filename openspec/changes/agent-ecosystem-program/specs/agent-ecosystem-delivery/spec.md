## ADDED Requirements

### Requirement: Program evidence must cover the approved roadmap
The program SHALL keep every approved roadmap deliverable traceable to a single
owner, implementation and explicit consumer/platform evidence.

#### Scenario: A package is extracted
- **WHEN** an SDK is claimed complete
- **THEN** both Sencode and an independently locked external consumer use its shared implementation
- **AND** protocol, error, cancellation, lifecycle and recovery tests accompany it

#### Scenario: A milestone is incomplete
- **WHEN** a required consumer, platform or behavior has not passed its relevant check
- **THEN** the program retains that requirement as incomplete with its next action
- **AND** does not substitute a narrow fixture or declared CI workflow for that proof
