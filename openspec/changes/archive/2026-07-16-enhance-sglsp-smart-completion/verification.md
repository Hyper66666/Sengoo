# Verification transcript

Date: 2026-07-13 (Asia/Shanghai)

## Server and protocol gates

- `cargo test -p sglsp -- --test-threads=1`: 167 passed, 0 failed.
  This includes checked-in protocol/golden fixtures, positive and negative safe
  Code Actions, schema-v1 completion data, warm completion p95, and warm
  signature-help p95.
- `cargo fmt --check --all`: passed.
- `cargo clippy -p sglsp --all-targets -- -D warnings`: passed.
- Attribute evidence: the cross-platform verifier passed the first nine exact
  compiler/derive/FFI/`sgc test` evidence entries. Its final Cargo invocation
  encountered a transient Windows `sgc.exe` file lock; rerunning the remaining
  exact test with
  `cargo test -p sgc discover_tests_expands_parameterized_cases_into_harnesses -- --nocapture`
  passed (1 passed, 0 failed). Thus every manifest evidence target executed and
  passed; the interrupted aggregate invocation is recorded rather than hidden.

## Fixed-binary Sencoder E2E

The test process was launched with:

```text
SGLSP_PATH=D:\Sengoo\target\debug\sglsp.exe
SGLSP_E2E_REQUIRED=1
PATH= (cleared for the spawned server)
```

Binary identity:

```text
path: D:\Sengoo\target\debug\sglsp.exe
size: 17076736 bytes
lastWriteTimeUtc: 2026-07-12T22:17:13.8986486Z
sha256: 2AEA1E87A80145094C057C732BDCB82F8497CAC763CB91373F8E1F5D50685ED5
rustc: 1.94.0 (4a4ef493e 2026-03-02), x86_64-pc-windows-msvc
cargo: 1.94.0 (85eff7c80 2026-01-15)
```

`node --test extensions/sengoo-lang/test/*.test.js` passed 64/64 with
zero skips. The initialize response advertised
`experimental.sengoo.completionSchemaVersion = 1`, completion resolve,
signature help, and Code Actions. The real RPC exercised context completion,
documentation resolve, safe auto-import, stale-revision refusal, non-BMP
UTF-16 replacement ranges, `#` attribute completion, signature help, a positive
unresolved-symbol import action, and a tampered-diagnostic negative action.
Required mode also rejected missing, relative, nonexistent, directory, and
non-executable paths without falling back to `PATH`.

Observed local performance in the final run:

- deterministic structural batch p95: 0.52 ms (gate: below 20 ms)
- real `sglsp` completion RPC p95: 2.4 ms (gate: below 80 ms)

## Remaining gate

Strict OpenSpec CLI validation was not run because the CLI is not installed or
available on `PATH`. The change artifacts were checked manually for the
required proposal/design/spec/tasks structure; task 7.8 remains open.
