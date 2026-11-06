## ADDED Requirements

### Requirement: File SHA256 SHALL stream regular files in bounded memory
`hash_sha256_file(path, max_bytes)` SHALL return an owned 64-character lowercase
hex digest or an explicit positive status error. Reads SHALL use at most 64KiB
of native input buffering and reuse the existing SHA256 compression core.
The existing buffer/str interfaces and runtime ABI 1 SHALL remain compatible.

#### Scenario: A packaged runtime exceeds the managed buffer limit
- **WHEN** a regular artifact above 128MiB is hashed within the supplied cap
- **THEN** the result matches an independently computed SHA256 digest
- **AND** total file contents are never loaded into a managed Buffer

#### Scenario: A fragmented read or read failure occurs
- **WHEN** input arrives in short reads or reading fails partway through
- **THEN** complete short reads produce the correct digest
- **AND** a failure returns an error and closes the file handle

### Requirement: File hashing SHALL validate limits and filesystem input
File hashing SHALL reject negative or overflowing limits, nonregular/reparse
targets and files above the supplied limit, and report open/read failures.
Windows paths SHALL convert strict UTF-8 to the OS wide path API.

#### Scenario: An artifact violates the supplied cap or path contract
- **WHEN** a caller supplies an over-cap file, directory, device or invalid path
- **THEN** hashing returns an explicit status without a partial success digest
- **AND** an empty regular file with a zero-byte cap succeeds
