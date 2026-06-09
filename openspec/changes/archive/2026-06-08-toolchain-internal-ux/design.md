## Scope

Child change for `six-pillar-gap-closure` Pillar 6. Extends canonical
`tooling-mainstream-ecosystem`; does not create a parallel capability name.

## Assertion transport

Before launching each test, `sgc test` creates a unique runner-owned result path
and passes its absolute path through `SENGOO_ASSERT_REPORT`. Failed typed helpers
in `std::assert` write one bounded UTF-8 JSON line to that path before exiting
with status `1`. This works in capture and `--nocapture` modes on Windows and
POSIX without relying on inherited numeric file descriptors.

After a non-zero exit, `sgc test` reads at most 64 KiB from the result path,
validates envelope schema version `1`, maps it into text and JSON report fields,
and removes the file. A missing envelope preserves the ordinary non-assertion
failure path. A malformed or oversized envelope does not replace the test
failure; the runner reports an assertion-transport diagnostic. The runner never
parses panic stderr text as structured assertion data.

When `SENGOO_ASSERT_REPORT` is absent, such as a normal `sgc run`, assertion
helpers retain the existing non-zero panic/termination behavior without trying
to create an implicit report path.

Runtime/compiler work required:

- assert helpers populate `schema_version`, `kind`, `helper`, `message`, optional
  `file`/`line`, and optional string `expected`/`actual`
- compiler callsite plumbing supplies source location when available; unavailable
  optional fields are omitted rather than encoded with type-changing sentinels
- `sgc test` preserves existing JSON schema fields and adds optional `assertion`

## Real e2e

- `tools/sgpm/tests/realworld_e2e.rs` or feature `real-e2e`
- CI job `realworld-e2e` uses real binaries; hosts without toolchain skip explicitly

## Prerequisites

- Archive `sgc-test-manifest-tooling` before this child archives.

## Verification

- `cargo test -p sgc test`
- `cargo test -p sgpm realworld`
- realworld-e2e CI job
