# Offline Agent evaluation tool

## Why
Agent failures need reproducible protocol evidence without API credentials,
paid calls or accidentally replayed file/command side effects.

## Capabilities
Add experimental `tools/sgeval` 0.1.0: Sengoo JSONL event recording, recursive
credential/literal redaction, sequence-validated data-only replay, deterministic
assertions and JSON reports. The evaluation-tool owner owns these files only.

## Non-Goals
No model invocation, tool dispatch, arbitrary assertion code, claims of automatic
PII detection, statistical model grading or unverified platform support.
