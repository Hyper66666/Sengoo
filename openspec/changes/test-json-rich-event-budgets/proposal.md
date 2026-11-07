# Verify explicit JSON budgets for rich event metadata

## Capabilities
An independent Sengoo native regression verifies >4096-node metadata round
trips with explicit JSON limits, stable legacy parse behavior, bounded overflow
status, malformed input and exact reservation cleanup over repeated calls.

## Non-Goals
No new ABI, default-limit or status-code change. The CLI uses the existing
`json_parse_limits` interface instead of expanding the legacy API globally.

## Ownership
Sengoo integration owns this stdlib regression and reference guidance. Provider
event parsing remains in Sencode CLI's `fix-provider-completion-metadata-budget`.
