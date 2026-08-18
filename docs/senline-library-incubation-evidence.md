# Senline Library Incubation Evidence

Recorded: 2026-07-14

This record covers the first source-development loop for the reusable packages
incubated by `senline-domain-worker`. It is development evidence only. It is
not installed-toolchain, cross-platform, stable-release, or stdlib-graduation
evidence.

## sgframing 0.1.0

- Scope: bounded `u32` big-endian framing over exact binary standard I/O.
- Product names/defaults in API: none; Senline limits remain in the worker.
- Lock: package-local `Sengoo.lock`, current.
- Package tests: `2 passed`.
- Real binary-pipe harness: valid/control-byte/max-boundary frames echo exactly;
  clean EOF, every four-byte-prefix split, every four-byte-payload truncation,
  zero length, and over-limit length produce the expected bounded outcome with
  empty rejected stdout/stderr.
- Source-development gates: check, fmt-check, doc, release build, and publish
  dry run passed.
- Consumer-discovered runtime regression: `SGDOG-2026-006` makes empty Buffer
  free/drop a no-op so clean EOF cannot pollute FFI error state.
- Dry-run archive SHA-256:
  `2de8fd4203b015b15bb0f13172e961cd3adef2bf49bbcab8280d30cc12595151`.
- Known gaps: broken-pipe/flush-failure injection, installed Windows/Linux,
  package license metadata/files, registry publication, and
  independent-consumer evidence remain absent.
- Stability: incubating; no `1.0` or `std::` claim.

## sgjson_contract 0.1.0

- Scope: exact closed-object composition, required typed getters, integer
  bounds, ASCII/hex, closed enums, and sorted-unique string arrays over strict
  `std::json` documents.
- Product names/defaults in API: none; V1 DTOs remain in product packages.
- Lock: package-local `Sengoo.lock`, current.
- Package tests: `7 passed`, including negative object, scalar, nested, array,
  and stale-runtime-handle classification cases.
- Source-development gates: check, fmt-check, doc, release build, and publish
  dry run passed.
- Dry-run archive SHA-256:
  `f8e35a4ac52826d7df35e1e0e06bbbe9cedd1c69ec04a610cbb962b95681436c`.
- Consumer-discovered runtime regression: `SGDOG-2026-007` covers container
  corruption when parsed JSON grows beyond the initial 16-node allocation.
- Protocol/runtime regressions: `SGDOG-2026-009` adds stable strict-parser
  error kinds, and `SGDOG-2026-010` adds checked length-aware owned-String
  building with embedded-NUL preservation.
- Runtime lifecycle regression: 64 permissive-plus-strict parse/close cycles
  restore the JSON document live-handle count after every round.
- Known gaps: parser provenance cannot be recovered from `JsonValue`; callers
  must supply a live strict document. Runtime fault injection, malformed
  corpus, long soak, installed Windows/Linux, package license metadata/files,
  registry publication, and independent-consumer evidence remain absent.
- Stability: incubating; no `1.0` or `std::` claim.

## Consumer Integration

The worker root lock contains both packages. Its source-development package
loop passes two worker tests plus three product-planner tests. Real
parent/child execution emits the frozen handshake, processes all five V1
fixtures with byte-exact frames, classifies duplicate/invalid-Unicode/trailing
parser errors, recovers after parser and schema rejections, preserves an
embedded NUL and suffix in an echoed identifier, and shuts down cleanly on EOF.
The complete realworld harness passes 13 tests. The reusable packages own
framing and contract primitives; Senline DTOs and policy remain in the product
package. The same consumer path minimized and fixed compiler regressions
`SGDOG-2026-011` (early-return move-state reachability) and
`SGDOG-2026-012` (nested field references producing invalid LLVM). The worker
uses the direct nested immutable borrow, so no compiler workaround remains.
Installed Windows/Linux loops and immutable build-info injection remain
pending.

Required gates are never converted to green when skipped. Archive hashes above
identify local dry-run contents only and are not Senline pin evidence.
