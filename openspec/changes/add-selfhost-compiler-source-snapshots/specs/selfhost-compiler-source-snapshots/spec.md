## ADDED Requirements

### Requirement: Compiler-owned retained source graph

The compiler SHALL own source snapshots in SG with explicit unique-module and
byte budgets, retained physical file/ancestor guards, SG hash/UTF-8 validation,
complete reads, EOF and metadata checks. Public FileSnapshot capacity SHALL stay
unchanged. Duplicate identity probes SHALL remain possible at full capacity.

#### Scenario: Full compiler source graph

- **WHEN** the SG compiler loads its actual complete source import closure within
  the configured compiler budgets
- **THEN** source admission reaches the real next compiler phase while preserving
  every accepted physical witness and reports any later rejection explicitly.

#### Scenario: Capacity and duplicate import

- **WHEN** the accepted unique-module limit is full
- **THEN** a matching duplicate identity is reused after a bounded temporary
  probe, a new identity is rejected, and accepted guards and budgets do not change.

#### Scenario: Caller graph limit and retained-source resource limit

- **WHEN** a valid caller graph limit exceeds 1,024 but the actual graph is small
- **THEN** loading remains permitted, the caller graph limit is retained, and the
  independent 1,024-owner resource bound rejects only a new excess identity with
  the existing module resource diagnostic.

#### Scenario: Failed or changed source

- **WHEN** a read is incomplete, EOF/metadata/UTF-8 validation fails, a path is
  disallowed or a budget is exceeded
- **THEN** no source is admitted and all partial native and SG resources close.

#### Scenario: Source consumers preserve ownership

- **WHEN** semantic, origin, liveness, HIR or MIR code reads source witnesses
- **THEN** it uses the owning compiler arena and never interprets an arena index
  as a public FileSnapshot handle or substitutes a reopened path for a live guard.

#### Scenario: Copied witness fields are changed

- **WHEN** copied source bytes, digest, identity, metadata or either stored path
  is changed, including changing source bytes and their digest together
- **THEN** source-authority validation compares against the same retained file
  handle and rejects the changed witness before granting semantic/HIR authority.

#### Scenario: Invalid source buffer shape

- **WHEN** a copied source has a negative length, insufficient capacity or a
  used length different from the recorded source length
- **THEN** copying rejects the invalid source before any raw read or destination
  write, and restoring the valid source permits normal copying again.

#### Scenario: Current adapter qualification

- **WHEN** the compiler source snapshot gate reports success
- **THEN** the independent OS probe executes a DLL built from the same captured
  adapter sources used by that compiler build and all input identities are checked.
