# Add an independent two-connection repository audit consumer

Implement a read-only Sengoo application consuming sgllm and sgschema. The host
selects a local root, an explicit file allowlist, and two labelled model targets.
Both reviewers operate on the same bounded immutable UTF-8 snapshots. Model tool
calls can read or search those snapshots; they cannot open arbitrary paths or
execute commands. Output is attributed JSONL, not an unattributed merged answer.

Use synthetic local providers for repeatable verification; real paid requests
require a caller's explicit configuration. Windows-first snapshot support is
reported honestly. No Sencode private configuration or credential access.
