## ADDED Requirements

### Requirement: A fresh source entry SHALL connect canonical constructors to actual runtime allocation

The experimental development source-object entry SHALL reconstruct checked
module, provider, ownership and owning HIR6 authority from a source path and
independent pins before lowering canonical constructor functions. It SHALL
reject unsupported profiles and SHALL NOT accept saved MIR or serialized
candidates as source authority. This bounded checkpoint does not complete
general collection lowering or compiler self-building.

#### Scenario: Ordinary source constructs a real owning vector

- **WHEN** a caller selects actual i64, bool and owning-element constructor
  functions in a source importing only `std::collections`
- **THEN** the compiler loads the separately authenticated physical provider
  declarations without requiring an implementation import in that source
- **AND** the bounded profile requires parameterless concrete SG functions with
  a zero-argument canonical call and nonzero T up to one MiB, alignment at most
  eight; other source shapes and layouts remain explicit unsupported failures
- **AND** the object calls the actual allocator with T's target layout and
  verified move/drop callbacks, returning the unchanged two-field Vec layout
- **AND** independently compiled native code observes source clearing, preserved
  element values, exactly-once owning Drop and complete resource restoration

#### Scenario: Source shapes cannot borrow constructor permission

- **WHEN** selection names are absent, duplicated or resolve to a parameterized,
  generic, manual-aggregate, forwarding, local-storage or branching function
- **THEN** this bounded entry rejects the selection without producing an object
- **AND** an unrelated function named like an intrinsic gains no authority
- **AND** owned names containing NUL reject rather than selecting a truncated
  prefix, and class/enum elements or owned nested class/enum fields reject
  until their complete inherited/variant Drop chains are proved

#### Scenario: Lowered jobs must have checked concrete bodies

- **WHEN** constructor callback generation discovers an SG Drop job
- **THEN** its actual nominal target, declaration and callable type must match
  the concrete element or owned field, its checked body state must be one and
  its concrete signature must equal its resolved declaration signature
- **AND** deferred trait-default bodies and unproved generic jobs remain rejected

#### Scenario: Independent authority is mandatory

- **WHEN** the provider manifest/object pin or collections/option source pin is
  wrong, development selection is false, or input/output limits are invalid
- **THEN** the fresh entry rejects before publishing an object
- **AND** every failure restores all String, Buffer, opaque and FileSnapshot
  handles and snapshot bytes to their first-call values

#### Scenario: Existing lowering retains its independent boundary

- **WHEN** callers use the existing ordinary HIR/MIR/source lowering entries
- **THEN** the constructor-enabled path is disabled and collection candidate
  authority is still required rather than inferred from a new module import
- **AND** the existing MIR, projection, disarm and source/native regression gates
  preserve their accepted, rejected and separately named reference-gap results

#### Scenario: Deep HIR ownership preserves original inputs

- **WHEN** lowering interns concrete internal types in its owned HIR arena
- **THEN** the original module/ownership witness including the ownership suffix
  and the original independently replayed provider/ownership facts still agree
- **AND** comparing only the module prefix cannot substitute for that full
  witness or silently bypass a failed validation
