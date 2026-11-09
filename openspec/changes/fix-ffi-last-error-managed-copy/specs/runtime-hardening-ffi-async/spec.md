## ADDED Requirements

### Requirement: Managed FFI diagnostic copying SHALL preserve its source error

The managed ffi_last_error_copy operation SHALL resolve and validate its
destination Buffer without clearing the diagnostic before its bytes are copied.

#### Scenario: A nonempty diagnostic is copied to a valid managed Buffer
- **WHEN** a registered destination has enough capacity and the current FFI diagnostic is nonempty
- **THEN** the wrapper SHALL return the exact copied byte count, preserve those diagnostic bytes and their error code, and set destination used_len to that count
- **AND** it SHALL NOT append a terminator beyond the copied byte extent or transfer destination ownership

#### Scenario: No diagnostic is present
- **WHEN** a valid destination is passed while the diagnostic is empty
- **THEN** the operation SHALL succeed with zero bytes and set used_len to zero

#### Scenario: The destination cannot receive the diagnostic
- **WHEN** the managed handle is invalid or closed, or its capacity is too small
- **THEN** the operation SHALL report the existing invalid-handle or buffer error through its result/status path
- **AND** a capacity failure SHALL preserve destination bytes and used_len

#### Scenario: The input-only runtime contract is verified
- **WHEN** ownership analysis relies on the new non-retaining handle input
- **THEN** the declaration SHALL be an actual external fixed-arity C function with the exact signed-i64 input/result prototype
- **AND** source-name shadows or incompatible prototypes SHALL NOT authorize that ownership exception
