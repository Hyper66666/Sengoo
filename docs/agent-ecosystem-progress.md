# Agent ecosystem implementation ledger

Updated 2026-11-09. The first Windows development preview is separate from the wider roadmap and public release. Current tool identities, verification and limits are recorded in [Windows acceptance](agent-ecosystem-acceptance.md).

Committed baselines authored by Hyper66666 <2247081184@qq.com>:

- Sengoo `ea803f1913f9d08f6393d412b0eb4b1de3055b3e`.
- Sencode CLI `c603b84022bb3afeec4f3fb217dd016abd48f028`.

Implementation remains in both working trees. Those baseline commits do not contain or certify subsequent SDK/ownership changes; reports record source hashes without claiming a clean released revision.

| Requirement | Implementation | Current state |
| --- | --- | --- |
| Three model protocols, streams, tools, usage and cancellation | `packages/sgllm` | Shared SDK and CLI modules; complete 28-step gate repeated with corrected compiler |
| MCP client/server, tools, pagination, progress and cancellation | `packages/sgmcp` | Independent peers and hostile-server regressions; bounded pollable stdout passes blocked-reader cancellation and original-deadline checks |
| Agent action kernel and host policies | `packages/sgagent` | Tool contracts, permissions, explicit budgets/unlimited defaults, scheduler, retries and durable recovery; six native suites and maintenance host |
| Durable ledger | `packages/sgagent/src/sqlite_store.sg` | OS lock, fencing, atomic/idempotent commit, rollback, unknown effects and twenty-round resources; actual SQLite/multiprocess gate |
| Tool Schema subset | `packages/sgschema` | Exact i64, Unicode, NUL, unsupported keywords and resources |
| Offline evaluation | `tools/sgeval` | External locked build, removed-source execution, seven offline groups and native resources; recordings never execute tools |
| Dual-model audit | `examples/realworld/agent-audit` | All 13 cases with String/Buffer/JSON/network baselines; external exact-version registry consumption |
| Knowledge server | `examples/realworld/agent-knowledge` | Independent client, SQLite persistence, directory policy and rollback; source-removed executable |
| Maintenance Agent | `packages/sgagent/examples/maintenance.sg` | Real read/patch/command/checkpoint/restart; crash blocks replay; waits for actual process-tree quiescence using the original deadline |
| Pollable stdout | `tools/stdlib/io.sg`, `runtime.c` | Windows local byte pipes, bounded FIFO, status, cancellation and actual join; synchronized constructor admission and exact native/OS resources |
| Five-package registry | `scripts/package_ecosystem.py` | Checksums, exact versions, corruption negatives and external consumers; local artifacts, not published release assets |
| Python | `examples/realworld/python-agent-tools` | Current-compiler external wheel build, source removal and fresh offline venv; CPython 3.12 Windows x64; Unicode/i64/errors/NUL and 200 resource rounds |
| Tutorial/reference | `examples/agent-learning`, `docs/agent-language-reference.md` | 23 checked steps, JSON diagnostic → correction, streaming/cancel and SQLite restart |
| Compile/startup/RSS/streams | `scripts/measure_agent_workload.py` | Project-cache cold/hot/body-edit builds, SDK ready+exit, four concurrent paced/burst streams and cancel; host-local observations |
| CLI final candidate | Sencode `target/agent-sdk-20261109/candidate-6` | 9 native and 11 product suites pass; same-binary package/upgrade/rollback verified and local sen/sencode updated |
| Linux/macOS and other Python versions | External host gates | Unverified for this preview; no installed WSL or supported local CPython other than 3.12 |
| Child agents, consultation, workflows, memory, remote MCP | Future individual changes | Deferred roadmap; existing Sencode features do not prove independent public SDK implementations |

The former +94 Buffer/+6 String audit failure is closed by general compiler corrections, without manual frees or weaker expectations. Fresh `&make_owner()` passes through both HIR reference representations. The original TUI native suite also exposed borrowed `String.as_str()` results surviving a branch after its owner had dropped. Hidden owning producers now reach the outer consumer; function escape stays rejected. All 1,178 compiler tests and five repeated original TUI suites pass.

Current compiler: `target/agent-conditional-compiler/bin/sgc.exe`, SHA256 `9b1055c5a5cb7f09c016228a44f7427b7519b6a92e69cad5993004831c3aa1e5`. Current integration reports are under `target/agent-completion-v2/`. Earlier 7f634388/preflight reports under `target/agent-completion/` retain their original scope. Changed-source runs, narrow probes and configured workflows do not certify another artifact.

Maintenance repetition found root-process exit and Windows Job-tree quiescence can appear at different times. The host now polls within its original deadline; a real descendant-process regression covers eventual completion and a deadline leaving an unknown outcome. The kernel's prohibition on replaying unknown side effects remains intact.

StdWriter constructor admission uses an SRW lock: eight threads each race forty create/close attempts while an independent observer reads resources. Native and OS worker/handle/byte baselines recover; no registry lock spans native output or join. POSIX and unsupported endpoints return unsupported. No arbitrary-device latency promise is made.

No real credentials or paid providers were used by the fixtures. Independent human trials, clean-machine installation, additional hosts and public release remain separate gates. Stable v0.2 release evidence does not certify these `0.2.1-dev.0` additions.
