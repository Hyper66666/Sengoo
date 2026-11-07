# Preserve ownership through named question-mark operands

## Problem
The Sencode retry integration reproduced a successful response becoming an
invalid JSON handle after a named Result<JsonDoc, i64> was unwrapped with `?`.
MIR extracts the payload but leaves the named container's Drop obligation live.

## Capabilities
Correct ownership transfer for named Result/Option question-mark operands,
including failure propagation, without changing their tagged representation.
The integration owner owns the compiler change and native/CLI verification.

## Non-Goals
No new lifetime model, ABI, provider policy or user-data migration.

## Affected boundaries
MIR try lowering, move checking, ownership regressions and the adjacent CLI's
request/response continuation tests.
