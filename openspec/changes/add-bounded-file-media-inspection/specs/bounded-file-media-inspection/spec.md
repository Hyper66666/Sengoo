## ADDED Requirements

### Requirement: File snapshots SHALL bind bytes and metadata to one owned read
A bounded snapshot SHALL use one regular-file handle for identity, metadata and
the complete read, own the returned copy and hash that exact copy. It SHALL
refuse observed changes and oversize input instead of returning prefix success.
It SHALL state the source-consistency limits of the host filesystem.

#### Scenario: The file is exactly at or beyond the cap
- **WHEN** an isolated regular file is read with an explicit byte cap
- **THEN** exactly-at-cap content succeeds only after EOF is proven
- **AND** one additional byte causes a bounded oversize error with no snapshot.

#### Scenario: An acquired pathname is targeted for replacement
- **WHEN** an isolated fixture attempts to replace the file or rename an ancestor before read completion
- **THEN** the retained Windows guards block that replacement or the read fails
- **AND** no path reopen mixes the new object's digest with the old bytes.

#### Scenario: Input cannot satisfy a requested local-file policy
- **WHEN** a device, directory, invalid UTF-8/NUL path or disallowed reparse path is supplied
- **THEN** the operation refuses or reports unsupported without a partial success
- **AND** all handles and partial allocations are released.

### Requirement: Media inspection SHALL distinguish bounded header facts from full validity
Pure Sengoo parsers SHALL classify bytes and inspect bounded PNG/JPEG/PDF header
facts with checked arithmetic. Unknown, malformed, unsupported and scan-limit
outcomes SHALL remain distinct. Header success SHALL NOT imply full decode.

#### Scenario: An image declares unsafe dimensions or segment sizes
- **WHEN** dimensions overflow a configured pixel limit or a segment exceeds input/scan bounds
- **THEN** inspection returns the corresponding bounded diagnostic
- **AND** no allocation follows an unchecked untrusted dimension.

#### Scenario: A PDF only contains misleading page markers
- **WHEN** a recognized PDF header is followed by arbitrary `/Type /Page` strings
- **THEN** the header version is available but page count and validity remain unknown
- **AND** the inspector does not infer encryption or execute PDF content.

### Requirement: PDF capability reporting SHALL describe locally verified functions
PDF capability reporting SHALL separately describe header metadata, page count,
encryption queries, text extraction and rendering. Absent engines SHALL degrade
explicitly without network downloads, automatic installation or arbitrary PATH
program execution.

#### Scenario: No native PDF engine is packaged
- **WHEN** an application requests PDF capabilities
- **THEN** header-only support is distinguishable from parser/render support
- **AND** the query changes neither the environment nor installed dependencies.
