# Explicitly bounded in-memory SHA256

## Problem

The native CLI's exact context-snapshot fence hashes a valid 90684-byte JSON
document. Existing `hash_sha256_str` rejects inputs above 65536 bytes with status
2, although the CLI's configured context ceiling is larger. Enlarging the old
limit silently would change a published resource boundary.

## Capabilities

Add opt-in `hash_sha256_bytes_bounded` and `hash_sha256_str_bounded` with an
explicit input maximum of 1 through 16777216 bytes. Preserve the original
functions and their 64-KiB maximum. Use the new API for exact CLI context fences.

## Non-Goals

No new hash algorithm, crypto protocol, streaming hash state or legacy ABI
change. No promotion of untested platforms or development APIs into v0.2
release evidence.

## Boundaries and ownership

Sengoo wrappers validate through a synchronous generic native hashing boundary.
The caller owns input and output memory; hashing retains no resources. The CLI
owns its context policy and selects a 786432-byte bound. The compaction
foundation owner owns this change and its dedicated regression.
