## ADDED Requirements

### Requirement: Synchronous AsyncFile buffer observation

The selfhost ownership checker SHALL permit a proved Buffer pointer projection
in argument one of the exact fixed C declaration
`sengoo_async_file_read_into(i64, i64, i64) -> i64`, using the existing native
pointer-as-i64 source bridge contract. It SHALL preserve ownership of the Buffer
through this nonretaining synchronous call.

#### Scenario: Standard read method

- **WHEN** the checked standard AsyncFile method passes `buffer.ptr()` followed
  by `buffer.len()` to the qualified synchronous read function
- **THEN** the Buffer remains available during and after the call and retains
  its normal exactly-once cleanup obligation.

#### Scenario: Unqualified call or declaration

- **WHEN** the declaration changes convention, arity, integer width, return
  type or name, or the pointer is passed to a retaining wrapper
- **THEN** the new observer contract does not apply and ordinary ownership and
  escape rules remain in force.

#### Scenario: Other argument positions

- **WHEN** ownership analysis examines the file-handle or capacity argument
- **THEN** this correction grants no new observation permission to that position.
