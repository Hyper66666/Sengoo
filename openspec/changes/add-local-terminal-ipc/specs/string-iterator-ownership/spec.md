## ADDED Requirements

### Requirement: String snapshot iterators SHALL release owned resources on scope exit
String byte, scalar and split iterators SHALL own their snapshots independently
of the originating String and release their native allocation through Drop,
including early returns and exhausted iterators. Explicit free SHALL remain
source-compatible and SHALL safely compose with Drop. Existing iterator item
types, one-handle layout, C symbol signatures and runtime ABI 1 SHALL remain.

#### Scenario: A function returns before exhausting its iterators
- **WHEN** a function creates all three iterator types and returns early
- **THEN** live native iterator ownership returns to its preceding count
- **AND** repeating the function does not retain one allocation per call

#### Scenario: A caller explicitly frees an iterator twice
- **WHEN** a live iterator is explicitly freed twice before its slot is reused
- **THEN** both frees succeed and next reports exhaustion
- **AND** later scope exit does not release the native allocation again

### Requirement: Iterator identities SHALL reject foreign and stale tokens
Native String iterator entry points SHALL validate opaque generation and domain
before dereferencing storage. Closed metadata slots SHALL be reusable; metadata
growth SHALL follow peak concurrent ownership rather than completed iterations.

#### Scenario: An old iterator slot is reused
- **WHEN** an iterator closes and a new owner reuses its metadata slot
- **THEN** calls with the old token cannot inspect or free the new owner
- **AND** cross-domain and foreign integer tokens are rejected safely
