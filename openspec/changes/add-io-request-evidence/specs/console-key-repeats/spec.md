## ADDED Requirements

### Requirement: Windows key repeats SHALL survive console record coalescing
The terminal SHALL deliver each key-down repeat with its original modifiers,
without changing event kinds or unbounded allocation. Input discard SHALL also
discard pending native repeats.

#### Scenario: Navigation and Unicode text arrive in repeated console records
- **WHEN** a key-down record reports three Down keys or three Chinese characters
- **THEN** three corresponding key events are delivered in order
- **AND** no extra event is emitted after the reported repeats
- **AND** an explicit input barrier removes remaining repeats before approval.
