## Purpose

Define owned UTF-8 `String` semantics, explicit conversions to `Buffer` and `&str`
workflows, and safe runtime handle lifetimes for stdlib text APIs.

## Requirements

### Requirement: Owned String values SHALL have explicit UTF-8 ownership semantics

Sengoo SHALL provide a first-class canonical stdlib `String` value type (`struct
String { handle: i64 }`) whose contents are owned UTF-8 bytes in runtime-managed
storage, with explicit move (for that canonical type), clone, drop, equality, and
byte-length semantics.

#### Scenario: A program owns text and inspects byte length

- **WHEN** a program constructs a `String` from a string literal via `string_from_str`
- **THEN** the resulting value owns a copy of the literal bytes
- **AND** `len()` reports the number of UTF-8 bytes
- **AND** `is_empty()` reflects whether that byte length is zero

#### Scenario: Moving transfers ownership

- **WHEN** a canonical stdlib `String` value is moved into another variable or function argument
- **THEN** the destination owns the handle and bytes
- **AND** subsequent use of the moved-from binding is rejected with a source-range diagnostic

#### Scenario: Cloning allocates a separate copy

- **WHEN** a program clones a `String`
- **THEN** mutations to the clone do not mutate the original
- **AND** allocation failure returns `Result` with a stable negative status category

#### Scenario: Owned text is not exposed as &str

- **WHEN** a program needs the bytes of an owned `String`
- **THEN** it uses `len()` and/or `copy_to_buffer` into a managed `Buffer`
- **AND** the language does not provide `String::as_str()` in v1

### Requirement: String, str, and Buffer conversions SHALL be explicit and compatible

Conversions between `String`, `&str`, and managed `Buffer` SHALL be explicit so
callers can see allocation, copying, and capacity effects.

#### Scenario: A program copies a String into a Buffer

- **WHEN** a program copies a `String` into a managed `Buffer` via `copy_to_buffer`
- **THEN** success returns the number of bytes written
- **AND** a too-small buffer returns `STATUS_BUFFER_TOO_SMALL`
- **AND** `Buffer.len()` capacity semantics remain unchanged

#### Scenario: A program builds a String from Buffer bytes

- **WHEN** a program builds a `String` from a `Buffer` and a used byte length
- **THEN** only the requested byte range is copied
- **AND** invalid UTF-8 returns `STATUS_INVALID_ARGUMENT`
- **AND** the source `Buffer` remains valid

### Requirement: Runtime string handles SHALL be safe to free once

The runtime SHALL track string handles with generation counters so double-free
and use-after-free through stale handles fail with `INVALID_HANDLE` rather than
heap corruption.

#### Scenario: A program drops a String explicitly

- **WHEN** a program calls `drop` on a `String`
- **THEN** the runtime releases the slot
- **AND** a second `drop` on the same logical value reports failure without corrupting memory

### Requirement: Borrowed &str and Buffer baselines SHALL remain compatible

Existing `&str` literal helpers and managed `Buffer` workflows SHALL continue to
compile and pass their baseline tests while owned `String` is available.

#### Scenario: Legacy buffer examples still build

- **WHEN** existing stdlib examples that use `Buffer` and `&str` are checked and built
- **THEN** they succeed without requiring migration to owned `String`
