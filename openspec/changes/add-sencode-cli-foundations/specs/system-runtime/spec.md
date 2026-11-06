## ADDED Requirements

### Requirement: Native file locks SHALL preserve a persistent lock identity

The runtime SHALL expose an owning non-Send/non-Sync `FileLock` with nonblocking
exclusive and shared acquisition. Lock contention SHALL return status 11.
Explicit close and Drop SHALL release the OS lock without removing its file.
Windows SHALL deny file deletion while a lock owns the file. Applications SHALL
never unlink a POSIX lock file while using it as a coordination identity.

#### Scenario: Two processes acquire the same lock
- **WHEN** one process owns an exclusive lock and another attempts acquisition
- **THEN** the second process immediately receives status 11
- **AND** acquisition succeeds after the owner closes without unlinking the file

#### Scenario: Shared lock ownership
- **WHEN** two owners acquire a shared lock on the same path
- **THEN** both acquisitions succeed and exclusive acquisition fails
- **AND** stale or foreign-domain handle close never accesses foreign storage

### Requirement: Path canonicalization SHALL resolve the actual existing target

`canonical_path_string` SHALL return an independently owned UTF-8 absolute path
for an existing target, resolving symlinks/junctions and Windows path case through
the OS. It SHALL reject embedded NUL, invalid UTF-8 and nonexistent targets.
It SHALL preserve case-sensitive POSIX path identities.

#### Scenario: Windows junction and path case alias
- **WHEN** a directory is opened through a junction and a differently cased path
- **THEN** canonicalization returns the same normalized target path
- **AND** the returned String has normal Sengoo ownership

### Requirement: Generic Windows credentials SHALL use the operating-system store

The generic credential read/write/delete helpers SHALL use the current user's
Windows Generic Credential store, with a nonempty UTF-8 target of at most 1024
bytes and a UTF-8 value of at most 2560 bytes. These helpers SHALL never enumerate
or log credentials. Temporary native value copies SHALL be zeroed before free.
POSIX SHALL report unsupported. Missing read SHALL report not-found; missing
delete SHALL successfully report false.

#### Scenario: Disposable credential lifecycle
- **WHEN** a unique test record is written, read and deleted
- **THEN** its Unicode value round-trips and subsequent read reports not-found
- **AND** no preexisting credential record is accessed

#### Scenario: Unsupported platform
- **WHEN** a credential helper is called on a POSIX host
- **THEN** it reports status 8 without reading or creating credential files
