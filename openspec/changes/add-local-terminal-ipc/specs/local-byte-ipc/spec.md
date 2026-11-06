## ADDED Requirements

### Requirement: Local byte IPC SHALL bind owning endpoints to one Windows logon
Named-pipe listeners and streams SHALL be owned, non-Send/non-Sync values with
generation checked handles. Connections SHALL be local and restricted to the
current Windows user and logon SID. Application framing and version negotiation
SHALL remain outside the native bridge.

#### Scenario: A listener exchanges bytes with its local peer
- **WHEN** a client in the same user and logon connects
- **THEN** bounded nonblocking I/O transfers bytes without protocol parsing
- **AND** a zero-byte read is distinguished from EOF

### Requirement: IPC cancellation and Drop SHALL release pending OS operations
Close and Drop SHALL cancel outstanding native operations, release all handles
and leave stale generation handles invalid. Poll and I/O SHALL not accumulate
unbounded native output.

#### Scenario: A pending listener or pressured stream is dropped
- **WHEN** accept or I/O has not completed
- **THEN** Drop cancels it and releases its native resources
- **AND** future operations on the stale handle report invalid-handle
