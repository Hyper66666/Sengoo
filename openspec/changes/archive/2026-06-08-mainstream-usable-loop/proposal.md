## Why

Sengoo already has many pieces that mainstream users expect from a usable
language: `sgc`, `sgpm`, `sgfmt`, `sglsp`, source-level stdlib imports,
lockfiles, docs, tests, reflection, and a growing standard library. The gap is
now integration proof. A new user should be able to create or inspect a real
package, run the normal project loop, and see supported and unsupported
behavior through stable diagnostics rather than repository lore.

This change defines the "mainstream usable loop" as a concrete acceptance
target: real examples, package workflow, locked CI commands, LSP diagnostics,
stdlib coverage, and a documented gaps matrix.

## Proposal

- Add `examples/realworld` with at least three end-to-end package examples that
  exercise the project workflow and mainstream stdlib modules.
- Make those examples runnable through `sgpm check`, `sgpm test`, `sgpm fmt
  --check`, `sgpm doc`, and `sgpm build` in locked mode after lockfile update.
- Add tests proving the realworld examples are covered by `sgc`, `sgpm`,
  `sgfmt`, `sglsp`, and stdlib/runtime behavior where relevant.
- Document the current support matrix for async IO, task cancellation,
  deferred stdlib features, unsupported platform behavior, package/test/doc
  diagnostics, and LSP coverage.
- Update README and quickstart documentation so users can discover the
  realworld workflow without reverse-engineering fixtures.

## Impact

- Adds examples under `examples/realworld`.
- Updates OpenSpec, README/quickstart docs, and examples documentation.
- Adds integration coverage in `tools/sgc`, `tools/sgpm`, `tools/sglsp`, and
  stdlib/compiler test suites as needed.
- May harden diagnostics or explicitly reject unsupported behavior, but should
  not opportunistically add broad new async IO, shell, or runtime features
  without updating this change.

## Non-Goals

- No public package registry launch.
- No implicit shell execution or package script execution.
- No new async IO runtime model unless the spec is updated with wakeup,
  lifecycle, portability, and test semantics.
- No claim that every stdlib module is production-complete; deferred or
  unsupported behavior must be documented and tested instead.
- No breaking source-language syntax change.
