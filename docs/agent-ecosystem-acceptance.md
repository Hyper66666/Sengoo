# Agent ecosystem Windows development acceptance

This report covers the first local Windows preview of the approved Agent SDK
roadmap. It is not a published toolchain release or a four-platform support
claim. The implementation base is Sengoo
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus source-hashed working changes;
Sencode's base is `c603b84022bb3afeec4f3fb217dd016abd48f028` plus its shared SDK
adapters. The generated JSON reports record the exercised source identities.

## Fixed toolchain and runtime

| Input | SHA256 |
| --- | --- |
| `target/agent-conditional-compiler/bin/sgc.exe` | `9b1055c5a5cb7f09c016228a44f7427b7519b6a92e69cad5993004831c3aa1e5` |
| `target/agent-formatter/debug/sgfmt.exe` | `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814` |
| Clang 19.1.7 | `56ef1abd0df11a3b1b77822193e927c6345078c24511567ae10b3afd803d03a3` |
| `tools/stdlib/runtime.c` | `eaa900898e670775f8019a8ddc363c714861db6d9887f6c04d04cb97575995d5` |
| `tools/stdlib/io.sg` | `7cc532211d6dabf47eb78bba51680d3270fd8cc61b82bbc7bb6c212fa80ab88f` |

Compiler library tests pass 1,178/1,178. The compiler now releases fresh owning
values borrowed as `&make_owner()` through both Unary::Ref and generated
HIRExpr::Ref. The twenty-round minimal reproduction and complete thirteen-case
two-model audit return String, Buffer, JSON and other tracked resources to
baseline. Anonymous owners behind borrowed branch/block results also remain live
until the outer consumer. The unchanged original native TUI suite passes five
recorded repetitions with that correction; no frame assertion was weakened.
The SDK does not compensate with manual frees.

## Recorded gates

| Requirement | Machine-readable report | Result |
| --- | --- | --- |
| Exact owning temporary, discarded value and native borrow regressions | `target/agent-completion-v2/ownership/evidence.json` | 19 steps passed, source guard passed |
| Schema and complete two-model audit | `target/agent-completion-v2/foundations/evidence.json` | 7 steps and 13 audit scenarios passed |
| Generic pollable stdout | `target/agent-completion-v2/stdout/evidence.json` | 12 steps; native and Sengoo gates passed |
| Model SDK | `target/agent-completion-v2/llm/evidence.json` | 28 steps; three protocols, streams, failures, resources and locked consumption |
| MCP independent peers and blocked stdout | `target/agent-completion-v2/mcp/evidence.json` | 25 steps; 6 blocked-output cancellation/deadline cases |
| Agent core and real maintenance host | `target/agent-completion-v2/core/evidence.json` | 26 steps; six native suites, normal/crash and descendant-process deadline cases |
| SQLite ledger | `target/agent-completion-v2/ledger/evidence.json` | 9 records; actual locks, transactions, fencing, rollback and exact resources |
| Three external registry consumers | `target/agent-completion-v2/examples/evidence.json` | 15 steps; exact versions, lock unchanged, source/registry removed before runtime |
| Tutorial and checked reference | `target/agent-completion-v2/tutorial/evidence.json` | 23 steps including diagnostics/fix, stream/cancel and SQLite restart |
| Python tool wheel | `target/agent-completion-v2/python/evidence.json` | External locked build and fresh offline CPython 3.12 environment; 200 resource rounds |
| Offline evaluation and removed-source runtime | `target/agent-completion-v2/evaluation/evidence.json` | 7 steps; native resource suite and seven offline groups |
| Compile/startup/four-stream workload | `target/agent-completion-v2/benchmark/evidence.json` | 5 stream trials, 30 SDK startup trials and 60 loopback requests; exact output and 15 resource baselines |
| CLI shared policy/protocol/TUI native gates | Sencode `target/agent-sdk-20261109/candidate-6/native-evidence.json` | 9 native suites passed |
| Same-binary product and real-console gates | Sencode `target/agent-sdk-20261109/candidate-6/production-evidence.json` | 11 suites passed, including 25 fullscreen checks |
| Packaging, upgrade/rollback and local installation | Sencode `target/agent-sdk-20261109/candidate-6/{install-smoke,local-install-evidence,final-gates}.json` | Passed; 15 payload files and 6 entrypoints; PATH preserved |

The installed `sen`/`sencode` executable SHA256 is
`0288b51b0626aca61b7d2e4b50ad08b767278eae55b6d62270a0764eb35ce7c2`.
The local ZIP is Sencode
`target/packages-agent-sdk-20261109-v3/sencode-cli-0.1.0-dev.0-windows-x64.zip`,
SHA256 `19439583db8395e108d5301954d7fc17d02cc2d9d530dbc02c662eb93e920b8a`.
The installer verified the pre-existing rollback package, installed the same
tested bytes and preserved PATH. This remains an internal source-development
candidate. It is not a remotely published release or fresh-VM qualification.

The exact five-package local registry is
`target/agent-completion/distribution/sengoo-agent-preview-registry-0.1.0.tar.gz`,
SHA256 `5a122925a04105834116419a89289c7b2b569b3f83f77e7dcabc4170b15dc281`.
The Python wheel is
`target/agent-completion-v2/python/sengoo_agent_tools_example-0.1.0-py3-none-win_amd64.whl`,
SHA256 `0f3d21bf9eb6fd69827e07dda330c6cb2f5ac43d4c36fadcaa3189bf1e7aba11`.
These are local development artifacts, not remotely published release assets.

The measured project-cache cold/hot/body-edit builds were 25.381/10.776/19.482
seconds, with frontend module reuse 0/3, 3/0 and 2/1. OS and shared native caches
were not flushed; other host workloads were not controlled. Four-stream runtime
sampled peak RSS was 18,001,920 bytes. The workload includes four paced streams,
four 180,224-byte delayed-consumer streams and cancellation after a no-poll
window, repeated five times. Configured 4 KiB native queue capacity is not a
measured queue high-water mark. The report explicitly leaves that metric null.
SDK ready+exit p50/p95 was 39.475/41.633 ms over 30 fresh processes. SDK startup includes creation, ready marker, clean exit and process reaping;
it makes no CLI/TUI interactive-startup or competitor comparison claim.

The StdWriter gate covers forty lifecycle rounds, eight constructor threads
each attempting forty create/close cycles, an independent resource observer,
OS handle/thread baselines, cancellation before/during native writes,
constructor rollback, stale generation tokens and exact binary fidelity.
Its worker and ring remain owned until actual join. The MCP gate also permits
one complete native write before blocking again, proving that progress does
not reset the original output deadline. Example-host shutdown policy is
separate from per-call cancellation in the server core.

The maintenance sample separately waits for actual Windows Job-tree quiescence
after root exit. A deterministic real descendant stays alive after its parent;
normal completion waits for its marker, while the original ten-second deadline
terminates the tree and preserves an unknown outcome that restart cannot replay.
The former single-snapshot implementation fails this regression. This is a host
correction; the kernel's unknown-side-effect policy remains unchanged.

## Scope boundaries

- The new output backend supports local Windows byte pipes. Console, file,
  message/remote pipe and POSIX backends explicitly return unsupported.
- Provider behavior is verified with independent synthetic loopback peers.
  No paid-provider availability, model quality or cost result is implied.
- A source-removed executable and a fresh virtual environment are useful
  independent-consumption checks, but do not constitute fresh-VM qualification.
- Linux/macOS, CPython 3.11/3.13/3.14, independent human trials and public release
  remain separate gates. This host has no installed WSL and only CPython 3.12
  within the supported binding range.
- Child-agent strategies, consultation, workflows, memory and remote MCP remain
  follow-up ecosystem capabilities. Existing Sencode behavior and pure policy
  helpers do not establish independent public SDK implementations for them.

Earlier failed/preflight artifacts are retained. A failed source guard is not
promoted because behavior assertions passed; final runs were repeated after
the compiler and runtime stopped changing. Documentation additions after a
run do not change the native implementation identified by its report.
