## ADDED Requirements

### Requirement: Stage0 SHALL validate explicit seed and dependency identities

Bootstrap inputs SHALL record exact compiler/source/target and relevant native
tool/runtime identities. The stage0 runner SHALL validate required identities
before compiler execution. A mismatched or absent seed SHALL fail with a clear
error rather than selecting an unchecked PATH executable. Bootstrap lock schema
and source-byte normalization SHALL be explicit and separate from published
package/runtime formats.

#### Scenario: A matching seed is supplied

- **WHEN** the caller supplies the locked seed and native inputs for the recorded target
- **THEN** the runner verifies their identities and executes the named seed
- **AND** source-runtime overrides select `--runtime-mode source-development` before the subcommand

#### Scenario: Seed contents differ at the same path

- **WHEN** a supplied compiler is missing or has a different content hash
- **THEN** the runner fails before invoking it
- **AND** no PATH lookup or cached result substitutes for the rejected seed

### Requirement: The stage0 entry SHALL expose honest foundation behavior

An SG entry SHALL expose deterministic foundation probe/self-test output and
stable failure for malformed invocation. Successful stage0 execution SHALL be
identified as reference-seed execution, not native self-hosting. Host-language
bootstrap support SHALL NOT implement the source/span/symbol/arena behavior on
behalf of the SG entry.

#### Scenario: The SG foundation entry executes

- **WHEN** the validated seed builds and runs the foundation probe/self-test
- **THEN** its output and exit status identify the exercised foundation behavior
- **AND** the result does not claim the SG compiler, native backend, minimal SDK or full self-hosting is complete

#### Scenario: Entry invocation is malformed

- **WHEN** the entry receives an unknown operation or missing required argument
- **THEN** it reports usage/error with a nonzero status
- **AND** no success marker is emitted for the rejected operation

### Requirement: Source storage SHALL preserve file identity and byte spans

Owned source storage SHALL assign file identities, retain source bytes and
validate ordered byte spans against the associated file length. Read errors
SHALL remain errors. Empty input and UTF-8 input SHALL not change byte-offset
meaning or permit invalid source/file references.

#### Scenario: UTF-8 and empty sources are stored

- **WHEN** empty input or input containing multibyte UTF-8 is added
- **THEN** source byte length and valid boundary spans resolve correctly
- **AND** source contents remain owned and unchanged until the store is destroyed

#### Scenario: A source or span is invalid

- **WHEN** a source read fails, a file identity is absent, or span offsets are negative, reversed or beyond source length
- **THEN** the foundation reports failure without manufacturing source contents or an unchecked span

### Requirement: Symbol identities SHALL be deterministic and checked

Interning SHALL map byte-identical names to the same symbol identity and distinct
names to distinct identities. Identity assignment SHALL follow explicit insertion
order. Resolving an invalid identity SHALL fail rather than reading a fallback
symbol as if it were valid.

#### Scenario: Symbols repeat and differ

- **WHEN** a sequence interns repeated and distinct names
- **THEN** repeated names resolve to the original ID and bytes
- **AND** a repeated run of that sequence yields identical IDs

#### Scenario: A symbol ID is outside the store

- **WHEN** lookup receives an invalid or out-of-range symbol identity
- **THEN** lookup reports failure without returning another valid symbol

### Requirement: Node arenas SHALL preserve checked identities through bounded growth

Append-only node storage SHALL use checked integer identities, preserve existing
node resolution across growth, and reject invalid indices and capacity/size
overflow before unchecked access. The supported limit SHALL be documented and
tested without weakening unrelated runtime limits.

#### Scenario: An arena grows

- **WHEN** appended nodes require storage growth within the supported bound
- **THEN** all earlier IDs still resolve to their original node values
- **AND** new nodes receive distinct deterministic IDs

#### Scenario: An index or size is invalid

- **WHEN** lookup uses an invalid ID or append exceeds the supported bound
- **THEN** the operation reports failure
- **AND** existing nodes and resource ownership remain valid

### Requirement: Foundation evidence SHALL bind results to actual native inputs

Verification SHALL record source revision and fingerprints, host, compiler and
dependency hashes, commands, exit statuses, output and limits. Repeated owned
foundation lifecycles SHALL recover tracked resources to baseline. Ignored
native outputs and historical reports SHALL NOT qualify different source or
tool inputs.

#### Scenario: A foundation gate passes

- **WHEN** the locked seed builds and runs the positive, negative, boundary and lifecycle checks
- **THEN** its report identifies the exact native/source inputs and asserted results
- **AND** repeated lifecycle checks return tracked resource counts to baseline
- **AND** later native self-hosting milestones remain incomplete
