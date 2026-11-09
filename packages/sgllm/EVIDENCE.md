# Windows development evidence

Experimental package source, 2026-11-09, host DESKTOP-FUAQMFB, Windows x64.
Only the native backend and local synthetic peers were executed.

- Library `src/lib.sg` SHA256:
  `b0a5bf45e6b907ba33cfcf2bd543d20df958f8418643229a40bfd3a09cb8f736`.
- Verified compiler SHA256:
  `deb91ea7223b5cf6c41d9fed585db4af73f5a555183470e0c42c9210ee30432d`.
- Verified formatter SHA256:
  `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
- Full 28-step local report: `target/acceptance.json`, SHA256
  `a874ddad1857baed0cf7c2c5d3a2053814527b52eff9ec0c27fa60e048d9aca6`.

The report passes nine formatter checks; seven test builds and their native or
loopback runs; a complete streaming example build/run; and a second external
consumer's sgpm update, locked check and locked run. All compilation and
execution happen after copying the SDK outside the repositories. No Sencode
module, user connection configuration or actual API key is consumed.

Coverage includes three protocols, four concurrent byte streams, Unicode/chunk
splits, complete tool arguments, usage, opaque target scopes, HTTP500 versus
malformed/incomplete HTTP200, actual HTTP truncation, partial SSE, timeout and
cancel. Twenty repeated invalid/overflow/failed completion cases return String,
Buffer, bounded JSON and opaque handle counts to baseline. Cancellation tests
also verify worker, network handle and request-byte reclamation.

For reproducible commands use `tests/run.py` as described in README. Full
provenance and superseded formatter/compiler evidence are recorded in the
Sengoo change `openspec/changes/add-agent-llm-sdk/verification.md`. A later
toolchain candidate must rerun the gate; a compiler binary's successful test does
not prove a different compiler binary. Catalog/distribution, Sencode adapters,
live services, clean machines, sanitizers and Linux/macOS remain independent
integration qualifications.

The sample's later raw-stdout change received a separate targeted 11-step
format/build/loopback run using compiler SHA256
`69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`.
`target/exact-stream-69a-acceptance.json` SHA256 is
`e08109b5807f277ef225f0ee4de0ceeea2111aa860986a7102a27902dd056c5a`;
stdout is exactly `世界😀\n17\n8\n`. That focused sample result is not a
28-step resource qualification for that newer compiler. `--example-only` in
the runner selects this focused scope; the default retains all tests.
