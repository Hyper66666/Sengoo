# Verification

Windows x64, DESKTOP-FUAQMFB, 2026-11-09; repository baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes.

The independent `packages/sgmcp/tests/borrowed_close.sg` uses a custom owning
struct with a `JsonDoc` field and `close(&mut self)`. It checks that bounded
live-document counts return to baseline after scope exit. It passes with
compiler `deb91ea7223b5cf6c41d9fed585db4af73f5a555183470e0c42c9210ee30432d`;
exact source/build/executable hashes and output are preserved in
`target/agent-temporary-compiler/native-evidence.json`.

The real MCP client preflight uses the same fixed compiler and actual process
pipes. Its lifecycle tests verify process live handles/resources return to zero
after close and scope exit, along with bounded progress queues, stderr drain,
cancellation and failed-result JSON cleanup. See `packages/sgmcp/EVIDENCE.md`
and retained preflight output under `target/sgmcp-sdk-20261109/`. This records
local synthetic-peer preflight; the MCP owner's final candidate run and
external distribution remain separate integration evidence.

Final compiler SHA256
`f7ea03a89c27ccedf8e6f2f8c81fe7ab6008887dd028fd8848102774e2f5dde5`
retains this correction. `target/agent-unused-compiler/final-language-evidence.json`
records all 18 try tests and 77 Drop-selected compiler tests passing, including
`borrowed_close_and_free_names_preserve_caller_ownership`, the consuming-receiver
regression and legacy explicit-handle cleanup tests. Dedicated strict validation
passes. No manual field release was added to the MCP package to evade the bug.

The old `explicit_field_drop_free_and_close_suppress_automatic_field_cleanup`
expectation is unchanged. Its isolated native comparison prints
`833,833,832,832,831,831` with both the old pinned compiler and the earlier fixed
compiler; these raw results are recorded as an observation, not a passing broad
ownership suite. Marker 831 represents an unresolved older nested explicit
destructor duplication. The borrowed free/close methods for 832/833 only print
markers and do not release fields; a name alone cannot now suppress caller
cleanup. An API that releases owned fields must communicate that transfer or
empty/reset its handles consistently with its receiver contract. Expectations
were not changed to disguise this semantic distinction.
