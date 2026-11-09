## ADDED Requirements

### Requirement: Native binary encoding SHALL be checked and deterministic

The SG backend SHALL generate instruction/object bytes directly with checked
operands, bounds, lengths and fixups. Invalid widths/registers/indices or
capacity overflow SHALL fail without publishing an apparently valid object.

#### Scenario: An encoding or fixup is emitted
- **WHEN** supported operands and labels are encoded repeatedly
- **THEN** their emitted instruction bytes and relocations are deterministic and match the target architecture
- **AND** all displacements and output offsets are checked before use

#### Scenario: A binary operation is invalid
- **WHEN** a writer, instruction or fixup receives an invalid operand or exhausted limit
- **THEN** it reports a stable failure without unchecked access or partial successful output
- **AND** owned resource counts recover after repeated failed/successful scopes

### Requirement: Windows native code SHALL preserve typed MIR and calling conventions

Validated MIR SHALL lower to native code with the documented numeric, control,
aggregate, ownership, FFI and async behavior. Windows ABI, alignment, preserved
registers, stack probing and unwind records SHALL describe the emitted code.

#### Scenario: A concrete MIR type needs target storage
- **WHEN** a supported concrete type with nominal definitions is laid out for Windows x64
- **THEN** its size, alignment, array stride and field offsets match the documented reference representation
- **AND** invalid physical claims, recursive values and bounded-size overflow fail before code emission
- **AND** unresolved types fail explicitly rather than receiving guessed storage

#### Scenario: A supported program executes natively
- **WHEN** typed MIR includes calls, aggregates, branches, loops or cleanup
- **THEN** its native output and side effects match the supported reference behavior
- **AND** C interoperability and stack unwinding succeed on the recorded Windows host

### Requirement: Object emission SHALL provide independently consumable native artifacts

The backend SHALL emit checked COFF data, code, symbols, relocations, unwind
and CodeView inputs needed by the SG linker. Normal native self-build emission
SHALL not call an external compiler or assembler or silently fall back to LLVM.

#### Scenario: A native object is produced
- **WHEN** a supported validated module is compiled with the SG native backend
- **THEN** the resulting object is independently inspectable and linkable with its declared target/ABI
- **AND** final SG self-build process evidence contains no external compiler or assembler

#### Scenario: An adapter requests native address facts before serialization
- **WHEN** a fresh native builder optionally collects debug addresses into an empty object
- **THEN** each emitted instruction, terminator, prologue and return epilogue has a bounded machine-code range
- **AND** enabling address collection preserves the object's emitted bytes before debug records are attached
- **AND** reusing populated construction state fails rather than appending conflicting addresses
