## Scope

`six-pillar-gap-closure` is an umbrella lane that closes the six structural gaps
identified after `mainstream-usable-loop`. It consumes and extends:

- `stdlib-mainstream-usability`, `owned-string-text`
- `runtime-hardening-ffi-async` and archived `async-phase-2-features`
- `sgc-test-manifest-tooling`, `mainstream-usable-loop`
- `frontend-build-performance`, `frontend-compile-perf`

It must not silently redefine existing `STATUS_*` meanings, lockfile `version = 1`
headers, or `sgc test` JSON schema without explicit MODIFIED requirements.

## Program shape

```text
Phase 0  Inventory + cross-pillar dependency map + matrix baseline
Phase 1  Pillar 6 quick wins (assertions, real e2e) — unblocks all other lanes
Phase 2  Pillar 1 + Pillar 4 (stdlib + surface) in parallel
Phase 3  Pillar 3 (package graph)
Phase 4  Pillar 2 (async/reactor) — longest pole
Phase 5  Pillar 5 (1000k perf gates)
Phase 6  Integration, matrix refresh, archive gate
```

Parallel lanes are allowed inside phases, but **Phase 6** is the only integration
gate that can mark the umbrella change done.

## Upstream archive prerequisites

Before a child change edits a capability owned by an active upstream change, the
upstream change must be archived first. In particular,
`mainstream-usable-loop`, `runtime-hardening-ffi-async`,
`sgc-test-manifest-tooling`, `stdlib-breadth-mainstream`, and
`stdlib-next-usability-wave` must either be archived in dependency order or be
listed as explicit blockers in the child proposal. Two active changes must not
claim the same canonical requirement.

## Archive strategy

This change is archiveable only after every required pillar-scoped child change
has been implemented, validated, and archived into its owned canonical
capability. Accepted platform skips may exist for tests that cannot run on a
host, but they cannot replace a pillar implementation.

Do not archive this umbrella first and leave the canonical specs unchanged.
This umbrella carries cross-pillar integration requirements only; capability
requirements belong to the child changes listed in `proposal.md`.

## Pillar 1 — Stdlib production surface

### Design principles

- Public stdlib wrappers remain the only documented user surface.
- Text output gains additive `Result<String, i64>` helpers. Existing
  caller-supplied `Buffer` APIs keep their current names and behavior throughout
  this program. Removing them requires a later breaking OpenSpec change.
- Handle types (`JsonDoc`, `ProcessOutput`) stay until a later owned-wrapper
  migration is spec'd; this lane focuses on ergonomics at the stdlib layer.

### Deliverables

| Area | Target |
| --- | --- |
| Owned string ABI | Add `_string` helpers and owned-value methods returning `String` or `Result<String, _>` without changing current Buffer-output names |
| String collections | `Vec<String>`, `StringMapString`, iteration by `String` |
| JSON | Raise input cap (target: 1 MiB default, configurable test hook); keep existing `json_parse(&str)` / `json_parse_buffer` inputs and add `json_value_as_string` returning `String` |
| Recursive IO | `dir_walk`, `dir_copy_tree`, `dir_remove_tree` with depth/count limits and stable `STATUS_*` |
| Process pipes | `ProcessCommand.pipe_stdout_to(child)` with explicit argv-safe pipe setup |
| Background process | `ProcessHandle` with `wait/kill/exit_code/close` and generation-checked ownership |
| Sync fd IO | `std::io` read/write on owned fds where platform supports; no async until Pillar 2 |

### Pinned public API names

Implementation agents MUST update this table before changing any public names.

| API family | Public names | Result/error contract |
| --- | --- | --- |
| Path and directory text | `path_join_string`, `path_normalize_string`, `path_parent_string`, `path_file_name_string`, `path_stem_string`, `path_extension_string`, `dir_entry_name_string` | `Result<String, i64>`; invalid UTF-8 or host path failure maps to `STATUS_INVALID_ARGUMENT` or `STATUS_IO`; existing Buffer APIs keep their names |
| JSON string reads | `JsonValue.string_value()`, `json_value_as_string(value)` | `Result<String, i64>`; wrong kind returns `STATUS_INVALID_ARGUMENT`; oversize/copy failures return stable status |
| Text vectors | `vec_new_string() -> Vec<String>`, `Vec<String>.push/get/remove/iter` | `push(value: String)` moves ownership into the vector; `get` and iteration return clones; `remove` transfers the stored value out; invalid handles return `STATUS_INVALID_HANDLE` |
| String maps | `string_map_string_new() -> StringMapString`, `insert/get/remove/contains/iter_keys` | `insert(key: &str, value: String)` copies the key and moves the value; `get` returns a clone; `remove` transfers the value out; key iteration returns owned key copies; keys compare by UTF-8 byte ordering without Unicode normalization |
| Recursive IO | `dir_walk`, `dir_copy_tree`, `dir_remove_tree` | Defaults: max depth 64, max entries 100000 unless caller supplies stricter limits; symlinks are not followed by default |
| Process pipes | `ProcessCommand.pipe_stdout_to(child) -> Result<ProcessCommand, i64>` | Shell-free; success consumes both inputs and returns the final command owning the pipeline chain; `run()` returns the final stage `ProcessOutput`; upstream spawn/setup failures are errors, while upstream nonzero exit does not replace the final exit code |
| Background process | `ProcessCommand.spawn() -> Result<ProcessHandle, i64>` | Required on Windows and POSIX CI hosts; `wait(timeout_ms) -> Result<i64, i64>` returns the exit code or `STATUS_TIMEOUT`; `kill() -> Result<bool, i64>`; `exit_code() -> Result<i64, i64>` is valid only after completion; `close()` releases the handle |
| Sync fd IO | `io_fd_read(fd, buffer)`, `io_fd_write(fd, data)` | Only documented owned fds; stdin/stdout/stderr remain separate helpers |

### Runtime notes

- Recursive and pipe helpers live in split C bridges (`runtime_process.c`,
  `runtime_breadth.c` or new `runtime_walk.c`) with generation-slot handles.
- Resource limits must be centralized in `runtime_shared.h`.
- Recursive walk and copy do not follow symlinks by default. Tree removal unlinks
  a symlink itself and never recursively removes the symlink target.

## Pillar 2 — Mainstream async runtime

### Architecture

```text
sgc async lowering
    -> native poll dispatch (existing)
        -> CoroutineScheduler (existing)
            -> NEW: Reactor (timers + sockets + fds)
                -> host: epoll / WSAEventSelect / etc.
```

### Semantic contracts

| Topic | Decision |
| --- | --- |
| User `Future` | Trait `Future<T>` with `poll(&mut self, ctx) -> Poll<T>`; polling exclusively borrows rather than consumes the future, and compiler lowers `async fn` to an internal implementation |
| Future flow | Allow local binding, param passing, struct fields when `'static` or scoped to async frame; keep rejecting unsound cross-thread escape |
| `select` | Homogeneous variadic `select(f0, ..., fn)` for 2..8 futures; each select instance rotates its first-polled operand between polls; losers are not canceled and are dropped through normal future cleanup |
| IO wakeups | Reactor registers interest; scheduler skips until readiness or deadline |
| Timeouts | Existing `timeout(future, ms)` keeps its non-canceling readiness semantics; new `timeout_cancel(future, ms) -> Result<T, i64>` consumes the future and returns `STATUS_TIMEOUT` after cancel/drop cleanup |
| TLS | Out of scope unless Pillar 1 HTTP work proves host support; remain `STATUS_UNSUPPORTED` |

### Future contract draft

The source-level shape is intentionally small until the type system grows
lifetime syntax:

```sengoo
enum Poll<T> {
    Ready(T),
    Pending,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T>;
}
```

Rules:

- `await value` accepts compiler futures and user types implementing
  `Future<T>`.
- The `&mut self` receiver is an exclusive borrow for one call. `Pending`
  preserves the same future for a later poll; `Ready` is terminal, transfers the
  output, and the remaining future state is dropped exactly once.
- Concurrent, reentrant, or post-`Ready` polling is rejected statically where
  possible and otherwise fails with a stable runtime error.
- `AsyncContext` is an opaque compiler/runtime-provided value. User code cannot
  construct, store, return, or compare it, and it is valid only during the
  current poll call.
- `poll` must not block the scheduler thread; blocking host IO belongs in
  reactor-backed stdlib futures.
- Returning `Poll.Pending` requires the implementation to register a wakeup or
  deadline through the provided context during that poll. A pending result
  without a wakeup registration is a stable runtime/diagnostic error rather than
  an unconditional busy-poll request.
- `Future` values may flow through locals, parameters, returns, and struct fields
  only when the compiler can keep ownership inside one async frame or prove a
  static runtime handle.
- Cross-thread escape, captured stack references inside returned futures, and
  storing non-static futures in global state remain rejected.
- `select` operands must have the same result type. When multiple operands are
  ready in one poll, the first ready operand in the current rotating poll order
  wins.
- Existing `timeout(future, ms)` does not consume or cancel the inner future.
  `timeout_cancel` is the explicit consuming cancel-on-timeout operation.

### Migration

- Existing `async def` / `spawn` / `join` / 2-way `select` tests must keep passing.
- New tests live in `compiler/src/tests/async_*`, `runtime/src/async_runtime/`,
  `tools/sgc/src/tests.rs` native section.

## Pillar 3 — Package graph maturity

### Resolver changes

- Dependency key MAY differ from `[package].name` only when the dependency table
  includes `package = "actual_name"`.
- Lockfile package-node ids include `(name, version, source)`. Dependency aliases
  are edge metadata and MUST NOT change package identity.
- Multi-version support bumps the lockfile schema to `version = 2`.
- Readers continue to accept `version = 1` only for graphs that do not require
  aliases or multiple versions. `sgpm update` performs the deterministic v1 to
  v2 rewrite. Locked/check/build/test commands never rewrite a lockfile; when a
  v1 lockfile cannot represent the selected graph they fail with an actionable
  `sgpm update` diagnostic.
- `sgpm metadata --format json` exposes resolved alias → package mapping.

### Real e2e testing

- Replace fake `sgc` stubs in realworld locked-loop tests with real tool
  invocation behind `#[ignore]` or feature `real-e2e` when clang/msvc absent.
- Minimum: `sgpm test --locked` on `cli-json-audit` compiles and runs smoke
  tests on CI Windows/Linux agents with toolchain installed.

## Pillar 4 — Language surface expansion

### Attributes

- Phase 4a uses this declaration matrix:

| Attribute | struct | enum | class | trait | impl | fn/method | const |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `#[derive(...)]` | yes | yes | yes | no | no | no | no |
| `#[cfg(target_os = "...")]` | yes | yes | yes | yes | yes | yes | yes |
| `#[deprecated]` / `#[deprecated("message")]` | yes | yes | yes | yes | no | yes | yes |

- Phase 4a `cfg` accepts only `target_os` values supported by the compiler target
  model. Feature expressions and arbitrary predicates require a follow-up spec.
- A false `cfg(target_os = "...")` removes the declaration before type checking.
- `deprecated` accepts an optional message. Use of a deprecated declaration emits
  a stable warning that `sgc` and `sglsp` both expose.
- Reject unsupported attributes with stable diagnostic codes, not generic parse
  errors.

### Class header traits

- Parse `class Child: Base, TraitA, TraitB` into `extends: Option<Path>` and
  `implements: Vec<Path>`. If the first resolved path is a class it is the sole
  base; if it is a trait the class has no base and every listed path is an
  implemented trait. A class path after a trait or a second class path is an
  error.
- Typeck validates trait refs; codegen uses existing vtable / trait dispatch
  paths where possible.

### FFI widening

- Extend the existing dynamic native i64 call ABI from arity `0..=4` to
  `0..=8`. Struct, aggregate, owned `String`, and callback signature expansion
  remain out of scope unless the child change adds an explicit ABI table.
- Keep `STATUS_UNSUPPORTED` for dynamic signatures beyond the supported set.

### Async frame limits

- Remove diagnostics that exist only for "phase-1" restrictions once Pillar 2
  reactor + frame layout supports the shape; each removal needs a regression test.

## Pillar 5 — Large-scale compile performance

### Targets (1000k LOC synthetic workload, default pipeline unless noted)

| Metric | Current (README Feb 2026) | Target |
| --- | --- | --- |
| Peak RSS vs C++ | 3.14x | ≤ 1.8x |
| Frontend time share | 86.93% | ≤ 65% |
| E2E compile time vs C++ | 0.37x (faster) | stay faster while meeting RSS target |

The reference CI host profile, compiler revisions, generator seed, and C++
baseline command must be recorded in `INVENTORY.md`. Results use the median of
three runs. A pull request fails the permanent relative regression gate if peak
RSS regresses by more than 10%, frontend share regresses by more than 5
percentage points, or end-to-end time regresses by more than 10% against the
checked-in reference snapshot. This gate remains active after the absolute
targets are met, and snapshot updates require checked-in before/after evidence.

Missing the absolute RSS/frontend target does not complete Pillar 5. The support
matrix may record the measured value and mitigation, but the child change stays
open until the target is met or this umbrella is explicitly superseded by a new
approved OpenSpec.

### Approach

- Continue MIR/type interning improvements (`interned-types` spec).
- Default-on frontend memory strategy where incremental correctness allows.
- Expand `advanced_pipeline_bench.py` CI snapshot with regression thresholds.
- Do not trade away runtime cache fingerprint correctness from
  `frontend-build-performance`.

## Pillar 6 — Default toolchain experience

### Assertions

- Extend the existing `std::assert` module rather than creating a competing
  assertion namespace. `std::error` remains a compatibility import.
- Existing typed helpers (`assert_eq_i64`, `assert_eq_bool`, `assert_eq_str`,
  `assert_eq_f64`, and `assert_ne_*`) gain readable failure messages.
- `sgc test` passes a unique runner-owned result path through
  `SENGOO_ASSERT_REPORT`; assertion helpers write one bounded schema-version-1
  JSON line there before exiting non-zero. The runner removes the file after
  validation. This transport works in capture and `--nocapture` modes on Windows
  and POSIX and does not parse panic stderr.
- Outside `sgc test`, absence of `SENGOO_ASSERT_REPORT` preserves the current
  non-zero assertion panic path and does not create an implicit report file.
- `sgc test` captures assertion messages in text output and adds an optional
  JSON `assertion` object with `schema_version`, `helper`, `message`, optional
  source location, and optional string `expected`/`actual` fields. Existing JSON
  fields remain backward compatible.

### Real e2e

- CI job `realworld-e2e`: build `sgc`/`sgpm`, run locked loop on all three
  fixtures, require exit 0.

### Debugger

- Minimum: document lldb workflow for native `sgc build` artifacts on Windows/Linux.
- Stretch: `sglsp` textDocument/definition + breakpoint mapping spike.

### Internal release

- Versioned GitHub/internal artifact for `sgc`, `sgpm`, `sgfmt`, `sglsp`.
- Smoke: `cargo test` subsets + realworld e2e + `openspec validate --strict`.

## Cross-pillar dependencies

```text
P6 assertions ──► all pillars (tests)
P1 owned String ──► P4 FFI/string returns
P2 reactor ──► P1 async fd IO (if any)
P3 real e2e ──► P6 CI release
P5 perf gates ──► independent, but runs in Phase 6
P4 class traits ──► independent of P2
```

## Support matrix

Update `examples/realworld/SUPPORT_MATRIX.md` after each pillar phase. Rows
that move from Deferred → Supported must cite new tests. Rows that remain
Deferred must cite this change and the accepting internal policy.

## Done Definition

A new internal developer can:

1. Follow README → realworld → locked loop with **real** tools in CI.
2. Write tests with **assertions**, not only exit codes.
3. Use **String/JSON/collections/process** APIs without raw `ffi_buffer_*` in
   application code.
4. Resolve **renamed and multi-version** deps in a workspace.
5. Opt into **async IO** for internal services with documented semantics.
6. Compile **1000k LOC** workloads within published RSS/time budgets.
7. Attach a **debugger** using documented steps.

## Risk register

| Risk | Mitigation |
| --- | --- |
| Pillar 2 schedule dominates | Time-box reactor to internal TCP/HTTP/timer fds first |
| Stdlib breaking changes | Deprecation aliases + `sgfmt`/`sglsp` signature updates |
| Multi-version resolver bugs | Lockfile golden tests + property tests on small graphs |
| Perf targets not met | Keep P5 open, publish measured evidence, and continue behind the documented `--low-memory` mitigation |
| Debugger stretch slips | Minimum lldb doc still satisfies P6 Done Definition |
