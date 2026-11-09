## ADDED Requirements

### Requirement: Read-only attributed two-model audit
The example SHALL independently poll two explicitly configured model targets and
SHALL attribute streams, errors, usage and final reports to their labels/models.

#### Scenario: Two concurrent tool loops
- **WHEN** two synthetic providers request read/search and return different findings
- **THEN** each receives its own matching results and emits an attributed final report

### Requirement: Explicit immutable source boundary
The host SHALL capture only selected relative UTF-8 files under its chosen local
root, using bounded snapshots and rejecting traversal, NUL and reparse escape.

#### Scenario: Unselected path and source changes
- **WHEN** a model requests an unselected path or a captured file changes
- **THEN** the tool rejects the unselected path and reads the original snapshot

### Requirement: Cancelable bounded standalone consumption
The application SHALL honor its monotonic deadline/cancel request, reject invalid
provider turns before tools, and run from an externally locked package build.

#### Scenario: Stream failure and cancel
- **WHEN** a provider returns malformed HTTP200, HTTP500 or exceeds the deadline
- **THEN** output records the safe stage/status and the other job is cancelled on shutdown
