# Design verification

Host: Windows x64, `D:/Sengoo`, 2026-11-09.

This change contains design documents only. No generic I/O source, native
runtime source or frozen MCP SDK source was changed for it. Implementation,
native execution, blocked-reader timing and platform verification remain open
in `tasks.md`; a successful specification check does not close those gates.

Read-only inspection covered `tools/stdlib/io.sg`, stdout/stderr and fd writes
in `tools/stdlib/runtime.c`, owned nonblocking child-stdin writes and resource
counts in `tools/stdlib/runtime_process.c`, `docs/process-terminal-runtime.md`,
and canonical concurrency/I/O and cancellation specs. The design records the
inherited-handle distinction and actual-join requirement rather than reusing an
MCP-specific native writer or changing shared endpoint flags.

Validation command:

```text
openspec validate add-pollable-standard-output --strict
```

Result: passed. The implementation candidate/compiler SHA is intentionally not
specified: this change introduces no executable candidate. The completed MCP
SDK's separate provenance remains in `packages/sgmcp/EVIDENCE.md` and its own
change verification file, without conflating this pending output capability.
