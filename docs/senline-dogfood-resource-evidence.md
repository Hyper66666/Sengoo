# Senline Domain Worker Resource Soak Evidence (task 8.3)

This file is the **durable, checked-in** publication record for resource soak
claims. Large JSONL sample series stay gitignored under
`target/senline-resource/`; green claims must cite a summary SHA-256 and source
revision here (see `docs/senline-dogfood-resource-methodology.md` publication
rules).

## Tip-era schema v2 1M soak (Windows x64)

| Field | Value |
| --- | --- |
| Source revision | `2c6f2cce7` (branch tip when soak was produced; re-verify after rebase) |
| Summary path (local) | `target/senline-resource/soak-soak-1m-windows-x86_64-1784302594.summary.json` |
| Summary SHA-256 | `408c61c129c79041602ae7a34f01f71e6ed67ea88ff96776f9caf97ed2cef087` |
| Summary size (bytes) | 2978 |
| JSONL companion | `soak-soak-1m-windows-x86_64-1784302594.jsonl` (gitignored; full series) |
| cases_requested | 1_000_000 |
| cases_completed | 1_000_000 |
| memory.metric | `private_bytes` (Windows `PrivateUsage`) |
| OLS regression slope (B/case) | ≈ −0.03985 |
| endpoint growth (B/case) | ≈ 0.05736 |
| max 10k-window delta (bytes) | 163_840 |
| handles.within_plateau | true |
| process_count (summary field) | 1 *(pre-fix summaries hardcoded this; sampler now measures worker process tree)* |
| zero_failures | true |
| oracle | independent Rust decision/reason match for reviewed-boundary corpus |

### How to re-verify the published hash

```powershell
Get-FileHash -Algorithm SHA256 `
  target/senline-resource/soak-soak-1m-windows-x86_64-1784302594.summary.json
```

A re-soak under the measured process-tree sampler should replace this table
(with a new summary name + SHA-256) before claiming 8.3 green again.

## Sampler process-count methodology (post-fix)

- Windows: `CreateToolhelp32Snapshot` / `Process32FirstW` — count the worker PID
  plus processes whose parent PID equals the worker.
- Linux: scan `/proc/*/stat` for `ppid == worker`.
- Gate: every sample must observe `process_tree_count == 1` (no unexpected children).
