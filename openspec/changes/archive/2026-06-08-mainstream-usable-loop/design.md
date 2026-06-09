## Scope

This change is an integration lane over existing Sengoo capabilities. It should
not duplicate the lower-level `stdlib-next-usability-wave`,
`runtime-hardening-ffi-async`, or `sgc-test-manifest-tooling` work. Instead, it
turns those capabilities into a user-facing loop that can be validated from
fresh project state.

`std::http`, `std::log`, HTTP/TLS unsupported behavior, and broad stdlib
runtime status semantics are consumed from `stdlib-breadth-mainstream` and
`stdlib-next-usability-wave`. Async/task lifecycle and FFI/platform unsupported
semantics are consumed from `runtime-hardening-ffi-async`. Manifest, lockfile,
test, formatter, doc, LSP, and bench protocol behavior is consumed from
`sgc-test-manifest-tooling`. This change must not redefine those API names,
status meanings, or unsupported semantics; if an upstream surface is still
active, this change may only use the current documented public surface or
record an accepted unsupported/deferred path.

## Realworld Examples

`examples/realworld` should contain committed repository fixtures, not
test-generated scratch packages. Each example must be small enough to run in CI
but realistic enough to prove a workflow rather than a single helper. The
examples should collectively use:

- `std::args`
- `std::file`
- `std::dir`
- `std::json`
- `std::process`
- `std::http`
- `std::log`
- `std::status`
- `std::collections`

Required example shapes:

1. `cli-json-audit`: a single-package CLI/data tool with `Sengoo.toml`,
   `src/main.sg`, `tests/**/*.sg`, package docs, and sample input/output files.
   It must cover `std::args`, `std::file`, `std::dir`, `std::json`,
   `std::log`, `std::status`, and at least one `std::collections` shape.
2. `http-client-status`: a single-package HTTP/status example with
   `Sengoo.toml`, `src/main.sg`, `tests/**/*.sg`, package docs, and a stable
   local or unsupported-path test. It must import `std::http` and `std::log`
   through public wrappers, use JSON/status handling, and document TLS/HTTPS or
   host-specific unsupported behavior through `examples/realworld/SUPPORT_MATRIX.md`.
   It must not use legacy `std::net` HTTP compatibility names as the primary
   public example surface.
3. `workspace-doc-loop`: a workspace or dual-target package example with a
   root `Sengoo.toml`, at least one `[lib]` entry, tests, docs, format checks,
   lockfile validation, and process invocation through `std::process`. It must
   prove package/workspace selection behavior without generating the fixture at
   test time.

Implementation may adjust behavior inside each named example if tests prove the
same acceptance surface, but changing the required example names or replacing
committed fixtures with generated scratch packages requires updating this
OpenSpec change first.

## Project Loop

Each realworld package must support this lifecycle:

```text
cd examples/realworld/<example>
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```

README and quickstart docs must show exact commands from the repository root to
each example directory. After `sgpm update`, the remaining locked commands must
not rewrite `Sengoo.lock`; tests should compare the lockfile content or
timestamp where practical.

If a command cannot support a given example because the runtime capability is
intentionally unsupported on the host, the example must expose that as a stable
status/diagnostic and still participate in the documented skip or unsupported
path.

## Diagnostics And LSP

Realworld examples should be used as fixtures for editor and CLI consistency:

- `sgc --error-format json` must produce machine-readable diagnostics for
  representative package/source failures.
- `sgpm` diagnostics should identify the selected manifest/package, stale
  lockfiles, unsupported features, and remediation commands.
- `sglsp` should expose imported stdlib symbols, signatures, hover,
  diagnostics, formatting, and definition behavior for the examples or
  reduced fixtures derived from them.

The representative failure matrix must include at least:

| Failure | Tools | Required alignment |
| --- | --- | --- |
| Stale lockfile | `sgpm update --check`, locked package commands | Manifest/package context plus remediation command. |
| Missing or malformed import | `sgc --error-format json`, `sglsp` diagnostics | Source location, import name, and compatible user-facing diagnostic. |
| Unsupported runtime capability | `sgpm`/`sgc` example flow, docs, support matrix | Stable `STATUS_UNSUPPORTED`, compiler diagnostic, or accepted platform skip with support category. |

`sglsp` coverage may continue using reduced fixtures in
`tools/sglsp/src/stdlib.rs` when full package harnessing would duplicate
existing tests. Those fixtures must be derived from realworld imports and named
so reviewers can trace them back to the example.

## Gaps Matrix

`examples/realworld/SUPPORT_MATRIX.md` is the single user-facing fact source
for this lane's support/gaps matrix. README, README.zh-CN,
`docs/sgpm-quickstart.md`, and `examples/README.md` should link to it instead
of duplicating support semantics.

The matrix must use this table shape:

```text
Capability | Status | Host scope | Proof example/test | Stable diagnostic/status | Upstream spec/change
```

The matrix must distinguish:

- supported behavior that is verified by examples/tests;
- unsupported behavior that returns `STATUS_UNSUPPORTED` or a stable compiler
  diagnostic;
- deferred behavior that requires a future OpenSpec before implementation;
- platform-specific behavior that is accepted only with documented tests or
  skips.

The initial matrix must cover async IO, task cancellation boundaries, select
limitations, process cancellation/background execution, stdlib compression,
TLS/HTTP limitations, dynamic FFI availability, package/test/doc diagnostics,
and LSP coverage.

## Done Definition

The lane is done when a new user can follow README or quickstart instructions
to inspect a realworld Sengoo package, run the locked project loop, understand
which runtime gaps are supported or intentionally unsupported, and receive
consistent CLI/LSP diagnostics.
