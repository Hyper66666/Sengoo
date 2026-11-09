# Verification

The full thirteen-case native provider suite passes with exact resource
baselines on Windows x64, 2026-11-09. The application uses Sengoo, `sgllm`,
`sgschema` and generic immutable file snapshots.

- Compiler SHA256:
  `7f6343885839b8dfbcd9e03509a3da73739c1ee88d87224fd01fcb1ec5fca7d7`.
- Runtime C SHA256:
  `eaa900898e670775f8019a8ddc363c714861db6d9887f6c04d04cb97575995d5`.
- Executable SHA256:
  `b3f557923aa8ff12228520d3befefe004627a4adbcdd00fec132399b56846940`.
- Build/source report: `target/agent-completion/foundations/evidence.json`,
  SHA256 `025b673e26110a03e7eaac6d705f281e5468dc2fb30e8a24bcdebaf9fb0f7066`.
- Independent provider report:
  `target/agent-completion/foundations/audit-protocol.json`,
  SHA256 `52f7136a5b2790e640e9e0f3d5a35a27f542c21741b2a4d2288cfa16ad9e2a5b`.

The seven-step foundation gate includes formatting, schema native counters,
audit compilation and the independent provider driver. The driver covers two
concurrent connections, all three protocols, tool loops, immutable attributed
sources, rejected traversal/unselected/NUL tools, continuation, credential echo
redaction, malformed HTTP200, HTTP500, cancellation, conflicting IDs, oversized
files and invalid UTF-8. It removes development-tool PATH for the executable.
All failure cases also require exact String, Buffer, JsonDoc, opaque, network
handle/worker and request-reservation baselines after workers exit.

Historical `target/agent-foundations-20261109/` preserves the failed +6 String,
+94 Buffer result. The first v2 compiler removed the Buffer delta but retained
six Strings. The latter were fresh `&string_new()` referents at stream-finalization
calls. The generic compiler correction covers both Unary::Ref source lowering
and generated HIRExpr::Ref. No counters were weakened and no manual free was
added to this consumer. The smaller pre-fix/preflight reports retain their
original scope.

Reproduce the native gate with `scripts/verify_agent_foundations.py`. Locked
offline-registry consumption and copied operation after source removal remain
the separate `scripts/verify_agent_examples.py --example audit` delivery gate.
Local fixtures do not establish live-model quality/cost, clean-VM installation,
non-Windows availability or published release status. Package-local evidence
notes are updated after all shared package-input guards finish.
