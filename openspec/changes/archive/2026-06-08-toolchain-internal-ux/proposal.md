## Why

Internal teams still lack structured assertion failures in `sgc test`, real
realworld e2e proof, debugger guidance, and a documented internal release channel.
This child change extends the tooling surface first introduced by
`sgc-test-manifest-tooling`.

## What Changes

- Freeze a cross-platform assertion-failure envelope written to a runner-owned
  result file and extend `sgc test` JSON output.
- Add real `sgpm`/`sgc` realworld e2e tests and CI job.
- Add `docs/debugging-native.md`, `docs/editor-setup.md`, `docs/internal-release.md`.
- Narrow fake-`sgc` integration coverage superseded by real e2e.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `tooling-mainstream-ecosystem`: structured assertion reporting, realworld e2e,
  internal debugger/editor/release docs.

## Impact

- `tools/stdlib/assert.sg`, stdlib runtime assertion bridge, compiler callsite
  metadata, `tools/sgc/src/commands/test.rs`, `tools/sgpm/tests/`
- `docs/`, CI workflows, `examples/realworld` smoke tests
- Parent umbrella: `six-pillar-gap-closure` Pillar 6

## Prerequisites

- Archive `sgc-test-manifest-tooling` before archiving this child change so
  canonical `tooling-mainstream-ecosystem` exists and is not duplicated by a
  parallel capability name.
