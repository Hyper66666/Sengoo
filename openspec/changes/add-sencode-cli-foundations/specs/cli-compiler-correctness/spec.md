## ADDED Requirements

### Requirement: Reference receivers preserve their storage and destructor ABI
The compiler SHALL pass ordinary borrowed method receivers by reference, retain
the destructor calling convention, and reject mutation through shared references.

#### Scenario: Mutable receiver updates nested storage
- **WHEN** a method modifies a local, nested field or reborrowed parameter
- **THEN** the original storage changes and owned resources release once.

#### Scenario: Mutable argument is temporarily read
- **WHEN** an `&mut T` argument is passed to a function taking `&T`
- **THEN** the call succeeds; passing `&T` to `&mut T` remains an error.

### Requirement: Real application syntax retains its control flow
The compiler SHALL preserve explicit `return self` expressions, asynchronous
function modifiers during attribute expansion, and logical short circuiting.

#### Scenario: Imported asynchronous helper and URL fragment
- **WHEN** source imports asynchronous helpers and includes a `#` character
- **THEN** helpers retain their asynchronous contexts after attribute expansion.

#### Scenario: Guard prevents absent JSON field access
- **WHEN** a false left operand guards a fallible right operand with `and`
- **THEN** the right operand is not evaluated; `or` similarly skips on true.

### Requirement: Method argument loans SHALL follow resolved result types

The borrow checker SHALL end temporary argument loans after calls returning
non-borrowing scalars or aggregates. It SHALL retain loans for reference payloads,
reference fields, guards and iterators, and still reject overlapping arguments
within one call. Unknown result shapes SHALL remain conservative.

#### Scenario: Stream read is followed by a borrowed byte copy

- **WHEN** a loop reads into `&mut Buffer` and receives `Result<i64, i64>`
- **THEN** a later call can borrow that Buffer through `&Buffer`
- **AND** numeric results do not retain a mutable alias into the Buffer

#### Scenario: Returned reference is hidden in an aggregate

- **WHEN** a method result contains a reference in generic arguments or fields
- **THEN** an overlapping mutable/shared borrow is still rejected

### Requirement: Empty struct construction SHALL preserve aggregate value ABI

Zero-field struct values SHALL have their declared aggregate value type in
stores, returns, parameters and borrowed receiver storage. Construction SHALL
not substitute an allocation pointer for the aggregate value.

#### Scenario: Empty reader mutates a separate borrowed buffer

- **WHEN** an empty struct is created, returned, passed or used as a receiver
- **THEN** native IR compiles and each receiver reaches the original storage
- **AND** a nested empty field retains its declared layout
