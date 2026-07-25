# mainstream-adoption-gap-closure Specification

## Purpose
TBD - created by archiving change mainstream-adoption-gap-closure. Update Purpose after archive.
## Requirements
### Requirement: Adoption closure SHALL be delivered through independently archived child changes

The four-pillar adoption program SHALL use one required child change per
pillar so canonical capability deltas remain independently reviewable,
revertible, and archiveable.

#### Scenario: A pillar begins implementation

- **WHEN** implementation work begins for one of the four pillars
- **THEN** the pillar has the child change id listed in `proposal.md`
- **AND** any active upstream change owning the same capability is archived
  first or recorded as an explicit blocker
- **AND** that child change owns its capability deltas, design decisions,
  tasks, tests, and archive gate
- **AND** the umbrella does not substitute its aggregate requirements for
  the child capability delta

#### Scenario: Debug-info work starts before the correctness gate is closed

- **WHEN** `native-debug-info` proposes merging codegen-affecting work while
  `codegen-ir-correctness-and-gate` is not archived
- **THEN** the dependency is recorded as an explicit blocker in the child
  proposal
- **AND** the debug-info codegen merge waits until the conformance gate
  drives the real `sgc` CLI on the pinned toolchain

#### Scenario: The umbrella is archived

- **WHEN** `mainstream-adoption-gap-closure` is proposed for archive
- **THEN** all four required child changes have already passed strict
  validation and been archived
- **AND** platform-specific test skips include evidence and do not stand in
  for an unimplemented pillar

### Requirement: Native builds SHALL support source-level debugging

Sengoo SHALL emit standard debug metadata so developers can set breakpoints
on Sengoo source lines and step through Sengoo source in a native debugger
on Windows and Linux.

#### Scenario: A developer debugs at source-line level

- **WHEN** a program is built with the debug-info flag defined in `design.md`
  decision D1
- **THEN** the emitted IR carries compile-unit, subprogram, and statement
  location metadata for Sengoo sources
- **AND** a breakpoint set on a Sengoo source file and line binds and hits
  in the documented debugger on that host
- **AND** stepping follows Sengoo source lines rather than assembly only

#### Scenario: Debug builds do not change semantics or default performance

- **WHEN** the same program is built with and without the debug-info flag
- **THEN** both builds produce the same program results
- **AND** debug and non-debug artifacts use distinct cache fingerprints
- **AND** the default (non-debug) pipeline's perf-gate numbers are not
  regressed by debug-info support

### Requirement: Async and process work SHALL be cancellable with documented semantics

Sengoo SHALL provide cooperative cancellation for spawned tasks, a consuming
select variant that cancels losers, and a cancellation-capable process wait,
each with stable status mapping.

#### Scenario: A spawned task is canceled

- **WHEN** a program cancels a spawned task that has not completed
- **THEN** the task stops at its next await point without running subsequent
  user code
- **AND** its status reaches the canceled terminal state
- **AND** the behavior is covered by compiler and native runtime tests

#### Scenario: A consuming select cancels its losers

- **WHEN** a program uses the consuming select variant with two to eight
  homogeneous operands
- **THEN** the first ready branch wins and is returned
- **AND** losing branches are canceled and dropped deterministically with no
  dangling reactor interest registrations
- **AND** the existing non-canceling `select` behavior remains unchanged

#### Scenario: A process wait is canceled

- **WHEN** a program kills or cancels a background process it is waiting on
  through the cancellation-capable wait
- **THEN** the wait resolves promptly with the pinned cancellation status
- **AND** handle cleanup follows the existing generation-checked lifecycle

### Requirement: HTTP serving SHALL support production connection and handler semantics

The stdlib HTTP server SHALL support handler-callback routing, bounded
opt-in keep-alive, streaming response bodies, and a TLS server subset on the
existing platform TLS stacks.

#### Scenario: Requests are routed to registered handlers

- **WHEN** an application registers per-route handlers and a request arrives
- **THEN** the matching handler produces the response without the
  application hand-pulling the request loop
- **AND** unmatched routes produce the documented status response

#### Scenario: A connection is reused under keep-alive bounds

- **WHEN** keep-alive is enabled with pinned max-request and idle-timeout
  bounds
- **THEN** sequential requests within bounds reuse one connection
- **AND** exceeding a bound closes the connection with the documented
  behavior
- **AND** the default behavior without opting in remains `Connection: close`

#### Scenario: A response body streams in bounded chunks

- **WHEN** a handler streams a response body
- **THEN** chunks are written within the pinned bounds until completion or
  client disconnect
- **AND** disconnect and timeout paths map to stable statuses

#### Scenario: A TLS server completes a real handshake

- **WHEN** the TLS server subset is enabled with a test certificate
  authority on a supported host
- **THEN** a real TLS handshake completes through the platform stack used by
  the TLS client rows
- **AND** no plaintext fallback or disabled verification is reported as TLS
  success

### Requirement: The toolchain SHALL be installable from versioned prebuilt artifacts

Sengoo SHALL publish versioned, checksummed toolchain archives for Windows
x64 and Linux x64 with install scripts, gated on the documented smoke
matrix, so adopters do not build from source.

#### Scenario: A fresh host installs the toolchain

- **WHEN** a developer runs the documented install script for a pinned
  version on a supported host
- **THEN** the script verifies the archive checksum before installing
- **AND** `sgc`, `sgpm`, `sgfmt`, and `sglsp` are available on PATH
- **AND** `sgc run examples/01_hello.sg` succeeds with the documented result

#### Scenario: A release is gated on the smoke matrix

- **WHEN** a release tag triggers the packaging workflow
- **THEN** the `docs/internal-release.md` smoke matrix runs first
- **AND** a failed smoke blocks publication of the archives

#### Scenario: Tool versions are coherent

- **WHEN** a developer queries the version of any released tool
- **THEN** all four tools report the same version string sourced from the
  workspace version
- **AND** the version identifies the built revision

### Requirement: Support matrix SHALL reflect adoption closure status

`examples/realworld/SUPPORT_MATRIX.md` SHALL be updated as each pillar
completes so adopters have a single current facts source for debugging,
cancellation, serving, and distribution claims.

#### Scenario: Matrix rows move only with proof

- **WHEN** a capability covered by this program moves from Deferred or
  absent to a supported claim
- **THEN** the matrix row cites a test, example, transcript, or workflow
  path introduced by the owning child change
- **AND** README links point at the matrix rather than duplicating claims

