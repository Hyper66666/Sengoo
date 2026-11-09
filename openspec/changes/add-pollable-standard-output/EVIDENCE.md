# Windows development evidence

Executed 2026-11-09 on Windows 11 x64 (`10.0.26300`). Base Git revision:
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e`, with the current uncommitted
implementation identified by full source hashes below. These are development
results, not a released toolchain or a new four-host support claim.

`target/std-writer/evidence.json` records a passing source-guarded native and
generated-Sengoo gate. Reproduce from the checkout with:

```powershell
& 'C:/Users/admin/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe' -X utf8 tools/stdlib/tests/verify_std_writer.py --compiler target/agent-parameter-compiler-v2/debug/sgc.exe --clang 'C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe' --formatter target/agent-formatter/debug/sgfmt.exe --evidence target/std-writer/evidence.json
```

Pinned identities:

| Input/artifact | SHA256 |
| --- | --- |
| Development sgc | `7f6343885839b8dfbcd9e03509a3da73739c1ee88d87224fd01fcb1ec5fca7d7` |
| Clang 19.1.7 | `56ef1abd0df11a3b1b77822193e927c6345078c24511567ae10b3afd803d03a3` |
| Current-parser sgfmt | `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814` |
| tools/stdlib/runtime.c | `eaa900898e670775f8019a8ddc363c714861db6d9887f6c04d04cb97575995d5` |
| tools/stdlib/io.sg | `7cc532211d6dabf47eb78bba51680d3270fd8cc61b82bbc7bb6c212fa80ab88f` |
| Native probe executable | `3ea73eefc4534eff9a3a378a45a9da85ec6b830669bd6a3446d493826db28e5c` |
| Generated Sengoo executable | `902d188036cc26b3b52c1ff78926b251079e72ec133bb9606a3abf042390b377` |

The native gate passes forty repeated constructor/queue/cancellation/Drop
rounds. Before each reuse, the original standard handle and pipe mode remain
valid, internal owner/worker/handle/queued/in-flight/capacity counts return to
zero, and actual process handle/thread counts equal their original baselines.
It injects constructor failures at allocation/admission, duplicate-handle and
pre-thread creation stages; rejects invalid capacities, lengths, uninitialized
source regions, duplicate endpoint owners, malformed tokens and stale reuse.
Canceled state retains in-flight storage and capacity until actual join.

Eight independent constructor threads each complete forty create/close cycles
against the same standard endpoint. A ninth observer continuously inspects live
counts. Endpoint exclusivity, registry accounting and exact final OS resources
pass; the global registry uses an SRW lock. That lock is never held while
waiting for a worker or performing a native byte write.

The independent parent does not read the child stdout pipe. The child verifies
`GetThreadIOPendingFlag` before notifying readiness, receives cancellation over
stdin, cancels the blocked WriteFile and joins before its completion marker.
The retained `GetTickCount64` cancellation sample is 0 ms at that clock's coarse
tick resolution, not a zero-latency claim. A deterministic hook also cancels
after selecting a write but before issuing it; cleanup must preserve the owner
through its initial unsuccessful cancellation request and retry until join.

Native stdout, stderr and slow-reader cases each preserve the same 8,192 raw
bytes including NUL and arbitrary split UTF-8. A broken reader records failure
and returns resources to baseline. The generated Sengoo case independently
checks partial queue admission, a duplicate constructor, finish/drain, atomic
status, exact bytes and zero writer resources after Drop. The old synchronous
stream helper retains exact binary behavior. Formatter check, source-development
type check and O0 native build/run pass with the pinned tools.

The implementation rejects unsupported endpoint kinds before starting workers;
the native gate checks a device/file endpoint and a Windows message pipe.
POSIX remains explicitly unsupported and has not been executed on a POSIX host.
No arbitrary-device cancellation guarantee, real-provider API claim or MCP
distribution claim follows from this gate. MCP input/cancel/backpressure and
locked-consumer tests belong to the integration owner.

`openspec validate add-pollable-standard-output --strict` passed. This verifies
the proposal format independently of the executable Windows evidence.
