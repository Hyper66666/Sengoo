# Two-connection repository audit

Experimental Sengoo-only host using `sgllm` and `sgschema`, edition 2026. Windows
first, with the Agent preview toolchain's owned-parameter/temporary fixes. It
does not depend on Sencode, TUI state or a private configuration database.

The host captures explicitly selected UTF-8 files once. Two labelled reviewers
start concurrently and use read/search tools against the same immutable data.
Each keeps its own model, connection, history, tool identities and usage. JSONL
events are `delta`, `tool`, `report`, `usage`, `error` or `cancelled`; there is no
automatic merge that obscures which model produced a finding.

```json
{
  "question": "Find correctness problems; cite paths and explain evidence",
  "files": ["src/main.sg", "README.md"],
  "deadline_ms": 120000,
  "targets": [
    {"label":"reviewer-a","base":"https://your-endpoint/v1","protocol":"responses","model":"your-model-a","key_env":"AUDIT_KEY_A"},
    {"label":"reviewer-b","base":"https://your-other-endpoint/v1","protocol":"chatCompletions","model":"your-model-b","key_env":"AUDIT_KEY_B"}
  ]
}
```

Set the named credential environment variables in the invoking process. Do not
put secrets in this config or command arguments. Supported protocols are
`responses`, `chatCompletions` and `messages`. Targets are explicit: the host
does not discover accounts, fall back to another provider or automatically retry.

```text
sgpm update
sgpm --runtime-mode source-development check --locked
sgpm --runtime-mode source-development build --locked
agent-audit.exe D:\project audit.json
agent-audit.exe D:\project audit.json D:\temporary\cancel-audit
```

Create the optional cancellation file to cancel both requests. The example also
enforces a configurable monotonic deadline (100 ms–10 min, default two minutes).
It has no request-count budget. Deadline, complete-history bytes and receipt
capacity bound the loop. A failed provider stops the audit; partial streams are
provisional and a report is emitted only after valid completion. Responses
`end_turn=false` continues within the same reviewer and cumulative usage.

Read/search tool arguments pass the shared Schema subset and NUL checks. Paths
must be explicitly selected, relative, forward-slash paths without `..` or drive
prefixes. Existing Windows file snapshots pin file/path ancestry and reject
reparse traversal. No shell, arbitrary disk read, write, network tool or MCP is
offered to models. Source text is untrusted; prompts do not grant permissions.

Limits: 32 files, 32 KiB each, 128 KiB combined input, 128 retained unique tool
receipts per reviewer, 768 KiB serialized history and the SDK's independently
bounded request/event/stream memory. Fully serialized/escaped output may reach
its bound before raw source bytes do. These failures are reported, never prefix
success. Host file capture is synchronous; no UI latency guarantee is made.

The configured credential value is redacted from that reviewer's textual output,
including a value split across streamed fragments. This is exact-value redaction,
not a general detector for credentials in chosen source files or transformed
secret values. Do not select secret files. Provider bodies are not logged on
errors. Usage is labelled as measured only when every request supplied it; token
counts are checked for negative values and overflow.

`tests/verify.py` runs native requests against independent synthetic peers with
fragmented bytes, three protocols, rejected paths, NUL arguments, explicit
Responses continuation, credential echo, HTTP500, malformed HTTP200, cancellation
and conflicting tool IDs. Native counters verify reclamation after success and
failure. Live API quality/cost and non-Windows support need separate evidence.
The offline-registry integration runner is `scripts/verify_agent_examples.py`.
