# Explicit diagnostic UTF-8 decoding and monotonic time

## Why
Real command output can contain malformed or legacy-encoded bytes. Discarding
an entire stderr hides actionable diagnostics. UI durations must not jump when
the wall clock changes.

## What Changes
- Add pure Sengoo `string_from_utf8_lossy(Buffer, used_len)` as explicit opt-in.
- Add `time_monotonic_ms()` backed by a host monotonic clock, with no wall fallback.
- Preserve strict constructors, existing time APIs, numeric statuses and ABI 1.
- Add standalone and independent differential regression evidence.

## Impact
Only additive standard-library APIs. No encoding detection, Unicode normalization,
automatic locale conversion, scheduler redesign or persisted monotonic timestamps.
