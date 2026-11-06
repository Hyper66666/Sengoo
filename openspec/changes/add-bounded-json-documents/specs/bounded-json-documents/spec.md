## ADDED Requirements

### Requirement: Extended JSON limits SHALL require explicit document admission
New bounded parsers and constructors SHALL require a limit from 1 to 16 MiB,
retain it through copy and serialization, and bound aggregate extended-document
ownership. Legacy parsers SHALL retain the 1 MiB limit.

#### Scenario: Rejected length uses an invalid input address
- **WHEN** a raw length exceeds its explicit limit
- **THEN** the call refuses before reading the address or allocating a document.

#### Scenario: An admitted document is copied or serialized
- **WHEN** its encoded value exceeds the destination document bound
- **THEN** copy rolls back or serialization refuses without a partial success
- **AND** release restores the extended-document admission budget.
