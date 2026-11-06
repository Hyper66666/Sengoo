## ADDED Requirements

### Requirement: Larger in-memory hashes require an explicit bound

The standard library SHALL provide additive bounded byte and borrowed-string
SHA256 functions. The accepted explicit maximum SHALL be 1 through 16777216
bytes, and the result SHALL be the ordinary 64-byte lowercase hexadecimal
SHA256 digest. The function SHALL retain no input or output resources.

#### Scenario: A valid context exceeds the old limit

- **WHEN** the caller hashes more than 65536 bytes within its explicit bound
- **THEN** the bounded API SHALL return the exact SHA256 digest
- **AND** a context fingerprint SHALL remain a digest of the complete exact
  serialized source rather than a truncated prefix or custom chunk checksum

#### Scenario: The input or declared bound is invalid

- **WHEN** input length exceeds the explicit maximum
- **THEN** the API SHALL fail with overflow status before reading input
- **AND** a zero, negative or greater-than-16-MiB maximum SHALL fail as invalid
- **AND** output capacity below 64 bytes SHALL fail without an output overrun

#### Scenario: Raw input contains NUL

- **WHEN** raw byte input includes an embedded zero byte
- **THEN** the API SHALL hash all supplied bytes, including bytes after the zero

### Requirement: Legacy hashing retains its compatibility boundary

The existing SHA256 entry points SHALL retain their symbols, signatures and
65536-byte input maximum.

#### Scenario: A legacy caller uses the former boundary

- **WHEN** a legacy caller supplies exactly 65536 bytes
- **THEN** hashing SHALL continue to succeed
- **AND** a legacy caller supplying 65537 bytes SHALL continue to receive its
  existing invalid-argument result
