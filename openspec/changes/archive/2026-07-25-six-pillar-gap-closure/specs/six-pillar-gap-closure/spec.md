## ADDED Requirements

### Requirement: Umbrella closure SHALL be delivered through independently archived child changes

The six-pillar program SHALL use one required child change per pillar so
canonical capability deltas remain independently reviewable, revertible, and
archiveable.

#### Scenario: A pillar begins implementation

- **WHEN** implementation work begins for one of the six pillars
- **THEN** the pillar has the child change id listed in `proposal.md`
- **AND** any active upstream change owning the same capability is archived first
  or recorded as an explicit blocker
- **AND** that child change owns its capability deltas, design decisions, tasks,
  tests, and archive gate
- **AND** the umbrella does not substitute its aggregate requirements for the
  child capability delta

#### Scenario: The umbrella is archived

- **WHEN** `six-pillar-gap-closure` is proposed for archive
- **THEN** all six required child changes have already passed strict validation
  and been archived
- **AND** platform-specific test skips include evidence and do not stand in for
  an unimplemented pillar

### Requirement: Stdlib SHALL expose production-grade text, data, filesystem, and process APIs

Sengoo SHALL close the stdlib MVP gap so internal application code can avoid raw
`ffi_buffer_*` and handle-only workflows for mainstream scripting tasks.

#### Scenario: Text-producing helpers return owned String values

- **WHEN** a program calls specified owned-text helpers such as
  `path_join_string`, `dir_entry_name_string`, or `json_value_as_string`
- **THEN** the helpers return `Result<String, i64>` or owned `String` per the
  migrated API table in `design.md`
- **AND** callers are not required to allocate a `Buffer` first for those helpers
- **AND** existing Buffer-based helper names remain source-compatible throughout
  this program

#### Scenario: String collections support internal data processing

- **WHEN** a program imports `std::collections`
- **THEN** it can create and iterate `Vec<String>` and use string-key maps with
  string values through safe stdlib wrappers
- **AND** insertion moves owned values into the collection, reads return clones,
  and removal transfers the stored value out
- **AND** invalid handle or allocation failures return stable `STATUS_*` categories

#### Scenario: JSON utilities support larger internal payloads

- **WHEN** a program parses JSON within the new default input cap (at least 1 MiB)
- **THEN** parsing succeeds for valid documents under the cap
- **AND** oversize input returns a stable oversize status without crashing
- **AND** scalar string reads can produce owned `String` values

#### Scenario: Directory tree operations are explicit and bounded

- **WHEN** a program calls recursive walk or tree copy/remove helpers
- **THEN** the helpers traverse with documented depth and entry-count limits
- **AND** symlinks are not followed by default
- **AND** symlink policy matches `docs/runtime-platform-behavior.md`
- **AND** failures map to stable `STATUS_*` values

#### Scenario: Process pipe setup remains shell-free

- **WHEN** a program chains two `ProcessCommand` values through a pipe helper
- **THEN** argv arrays remain literal and shell-metacharacter safe
- **AND** successful pipe setup consumes both inputs and returns the final
  `ProcessCommand` owning the pipeline chain
- **AND** `run()` reports the final stage output while pipe setup/spawn failures
  return a stable status
- **AND** stdout/stderr capture and timeout helpers continue to work
- **AND** background processes use a generation-checked `ProcessHandle` with
  `wait(timeout_ms)` returning an exit code or `STATUS_TIMEOUT`, plus kill,
  exit-code, and close operations
- **AND** this background lifecycle is verified on Windows and POSIX CI hosts

#### Scenario: Synchronous fd IO is available for CLI workflows

- **WHEN** a program imports `std::io` for fd read/write on supported platforms
- **THEN** the synchronous subset documented in `design.md` is available
- **AND** async fd readiness remains gated on the reactor requirement below

### Requirement: Async runtime SHALL support IO wakeups and mainstream future flow

Sengoo SHALL move beyond the cooperative sleep/spawn subset to a documented
reactor-backed async model suitable for internal services and concurrent IO.

#### Scenario: Reactor wakeups unblock socket and timer futures

- **WHEN** an async program awaits a timer, TCP read, or specified fd-readiness
  future
- **THEN** the scheduler registers interest with the reactor
- **AND** the task resumes only after readiness or deadline
- **AND** the behavior is covered by compiler and native runtime tests

#### Scenario: User-defined awaitables use a trait-based Future contract

- **WHEN** a type implements the specified `Future` trait and defines a poll
  contract
- **THEN** `await` on that type is accepted by the compiler
- **AND** polling uses an exclusive mutable borrow that preserves the future
  after `Pending` rather than consuming it
- **AND** `Ready` is terminal, while concurrent, reentrant, and post-completion
  polls are rejected or fail with a stable runtime error
- **AND** the opaque async context cannot be constructed, stored, or returned by
  user code
- **AND** returning `Pending` without registering a wakeup or deadline produces
  a stable diagnostic or runtime failure instead of silent busy polling
- **AND** unsound implementations are rejected with stable diagnostics

#### Scenario: Futures flow through locals, parameters, and returns where sound

- **WHEN** a program stores an awaited or spawned future in a local, passes it
  to a function, or returns it from an `async fn` where the type system permits
- **THEN** compilation succeeds
- **AND** programs that would allow unsound cross-frame or cross-thread escape
  are still rejected with explicit diagnostics

#### Scenario: N-ary select is supported with documented loser policy

- **WHEN** a program uses `select` with more than two awaitables
- **THEN** homogeneous variadic select accepts between two and eight operands
- **AND** polling rotates the first-polled operand between pending polls
- **AND** the first ready branch in the current poll order wins
- **AND** losing branches are not canceled and are dropped through normal future
  cleanup
- **AND** native tests cover at least three-branch select

#### Scenario: Timeout and cancellation semantics are testable

- **WHEN** a program uses existing `timeout(future, ms)`
- **THEN** timeout readiness does not consume or cancel the inner future
- **AND** when a program uses `timeout_cancel(future, ms)`, the operation consumes
  the future and returns `STATUS_TIMEOUT` after cancel/drop cleanup
- **AND** `cancel_task` behavior for pending tasks remains stable and tested

### Requirement: Package resolution SHALL support aliases and multiple versions

`sgpm` SHALL resolve internal monorepos with renamed dependencies and multiple
versions of the same package name without ambiguous lockfiles.

#### Scenario: Renamed dependency keys resolve to the correct package

- **WHEN** `Sengoo.toml` contains `[dependencies.my_alias]` with
  `package = "actual_name"` and a supported source such as `path = "../actual_name"`
- **THEN** `sgpm update` resolves `my_alias` to `actual_name`
- **AND** `sgpm check` and `sgpm build` compile against the correct sources
- **AND** diagnostics do not require dependency keys to equal package names

#### Scenario: Multiple versions of one package coexist in a graph

- **WHEN** a workspace requires `foo 1.0.0` and `foo 2.0.0` through distinct
  dependency paths allowed by the resolver
- **THEN** `sgpm update` writes a `version = 2` lockfile with distinct package
  nodes identified by `(name, version, source)`
- **AND** dependency aliases are recorded on dependency edges rather than package
  identity
- **AND** `sgpm metadata --format json` lists both versions
- **AND** conflicting source requirements for the same version still fail with an
  actionable diagnostic

#### Scenario: An existing version 1 lockfile is encountered

- **WHEN** a version 1 lockfile still represents a single-version graph without
  dependency aliases
- **THEN** locked commands may read it without rewriting it
- **AND** `sgpm update` upgrades it deterministically to version 2
- **AND** a version 1 lockfile that cannot represent aliases or multiple versions
  fails with an actionable `sgpm update` diagnostic

#### Scenario: Realworld locked loop runs with real sgc and sgpm

- **WHEN** CI executes the `realworld-e2e` verification job on a host with the
  native toolchain available
- **THEN** `sgpm update`, `sgpm check --locked`, `sgpm test --locked`,
  `sgpm fmt --check --locked`, `sgpm doc --locked`, and `sgpm build --locked`
  succeed for all `examples/realworld/*` fixtures using real `sgc` and `sgpm`
  binaries
- **AND** locked commands do not rewrite `Sengoo.lock` content

### Requirement: Language surface limits SHALL expand for internal OOP and FFI

The compiler SHALL remove avoidable artificial restrictions that block internal
libraries while keeping unsound features rejected with stable diagnostics.

#### Scenario: Supported attributes parse on major declaration kinds

- **WHEN** a program uses allowed attributes from the Pillar 4 attribute table on
  struct, enum, class, trait, impl, function, or const declarations
- **THEN** parsing and lowering succeed
- **AND** unsupported attributes produce diagnostics naming the attribute and site
- **AND** `derive` remains limited to struct, enum, and class declarations
- **AND** the initial `cfg` form accepts only supported `target_os` predicates
- **AND** deprecated declarations produce a stable warning in both `sgc` and
  `sglsp`

#### Scenario: Class header trait lists are supported

- **WHEN** a declaration uses `class Child: Base, TraitA, TraitB`
- **THEN** the parser records `extends` and `implements`
- **AND** a first class path becomes the sole base, while a first trait path
  means the declaration has no base and all paths are traits
- **AND** type checking rejects a class path after a trait or more than one class
  base
- **AND** method dispatch succeeds in tests covering trait calls on class types

#### Scenario: FFI surface widens with hardening intact

- **WHEN** a program uses the dynamic native i64 call ABI with arity `0..=8`
- **THEN** compilation and native linking succeed on supported hosts
- **AND** out-of-range arity, aggregate values, owned `String`, or other
  unsupported signatures return `STATUS_UNSUPPORTED` or compile-time errors per
  the FFI table
- **AND** existing `runtime-hardening-ffi-async` negative tests still pass

#### Scenario: Obsolete async frame restrictions are removed only with tests

- **WHEN** a program uses an async shape previously rejected only for
  implementation-phase limits
- **THEN** the compiler accepts it if Pillar 2 semantics support it
- **AND** each removed restriction has a regression test in `async_tests.rs` or
  native async tests

### Requirement: Large-scale compile workloads SHALL meet published budgets

Sengoo SHALL publish and enforce compile memory and frontend time budgets for
100k and 1000k LOC workloads used in internal monorepos.

#### Scenario: 1000k peak RSS meets the umbrella target

- **WHEN** the `advanced_pipeline_bench.py` 1000k workload runs in default mode
  on the reference CI host
- **THEN** peak RSS is at most 1.8x the C++ baseline for that workload
- **AND** the result is the median of three runs using the pinned host profile,
  generator seed, compiler revisions, and baseline command

#### Scenario: Frontend time share decreases at 1000k

- **WHEN** the same 1000k benchmark runs
- **THEN** frontend phase time is at most 65% of total compile-stage time
- **AND** a regression below this target blocks umbrella archive or requires a
  follow-up perf change with new evidence

#### Scenario: Performance regressions fail CI

- **WHEN** a pull request regresses peak RSS by more than 10%, frontend share by
  more than 5 percentage points, or end-to-end time by more than 10% against the
  checked-in reference snapshot
- **THEN** the perf gate job fails with the before/after snapshot paths

### Requirement: Toolchain SHALL provide default internal-developer experience

Sengoo SHALL ship assertions, real e2e verification, debugger guidance, and an
internal release channel so teams can adopt the language without repository
archaeology.

#### Scenario: Existing typed assert helpers report readable failures

- **WHEN** a package test imports `std::assert` and a typed helper such as
  `assert_eq_i64(expected, actual)` fails
- **THEN** the test process exits non-zero
- **AND** a bounded schema-version-1 assertion envelope is written to the unique
  runner-owned path provided through `SENGOO_ASSERT_REPORT`
- **AND** `sgc test` text output includes the assertion message
- **AND** JSON output includes an `assertion` object with `helper`, `message`, and
  optional source location plus string `expected` and `actual` fields for
  assertion failures
- **AND** the transport works in capture and `--nocapture` modes on Windows and
  POSIX without parsing panic stderr
- **AND** assertions outside `sgc test` preserve non-zero termination when the
  report environment variable is absent
- **AND** existing JSON report fields and `std::error` compatibility imports
  remain valid

#### Scenario: Realworld e2e job uses real tools

- **WHEN** CI runs the `realworld-e2e` job
- **THEN** it does not substitute fake `sgc` or `sgfmt` executables
- **AND** failures print the delegated command output

#### Scenario: Debugger quickstart exists

- **WHEN** a developer opens `docs/debugging-native.md`
- **THEN** it explains how to build with debug symbols and attach `lldb` or the
  documented Windows debugger to a `sgc build` artifact
- **AND** the steps are validated on at least one host in CI or a manual checklist
  linked from `tasks.md`

#### Scenario: Internal release channel is documented

- **WHEN** a developer reads `docs/internal-release.md`
- **THEN** it explains how to obtain versioned `sgc`, `sgpm`, `sgfmt`, and `sglsp`
  binaries, the smoke tests run before tagging, and the rollback procedure

#### Scenario: Editor setup matches CLI diagnostics

- **WHEN** a developer opens the documented `sglsp` workspace configuration
- **THEN** realworld imports receive completion, hover, diagnostics, formatting,
  and definition behavior consistent with `sgc check` for the same sources

### Requirement: Support matrix SHALL reflect pillar closure status

`examples/realworld/SUPPORT_MATRIX.md` SHALL be updated as each pillar completes
so internal users have a single current facts source.

#### Scenario: Matrix rows move only with proof

- **WHEN** a capability moves from Deferred to Supported
- **THEN** the matrix row cites a test, example, or benchmark path introduced by
  this change
- **AND** README links remain pointed at the matrix rather than duplicating claims
