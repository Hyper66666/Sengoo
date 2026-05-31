## Context

Native Sengoo artifacts link `tools/stdlib/runtime.c`. The source is discovered
at runtime and recorded in run/build metadata as a path. That is not enough:
the linked behavior can change while the path stays identical.

There are two independent cache layers:

1. Run/build metadata can return a completed cached executable before any
   runtime object work occurs.
2. `ensure_runtime_object` caches compiled runtime objects under a temporary
   filename derived from canonical path, file length, second-resolution mtime,
   optimization level, and target.

Both layers need content identity. Fixing only the object cache still leaves
the early executable cache hit stale. Fixing only metadata still leaves a
same-length, same-second object-cache collision during relinking.

## Goals / Non-Goals

**Goals:**

- Make native artifact reuse sensitive to runtime C byte changes.
- Keep unchanged runtime source cacheable.
- Preserve current incremental behavior for Sengoo source artifacts.
- Emit a clear runtime-source cache-miss reason.
- Keep old metadata backward-compatible for deserialization but stale for
  reuse when a current runtime source has a fingerprint.

**Non-Goals:**

- No cryptographic digest requirement.
- No cache directory migration or eager cleanup.
- No compiler-version, clang-version, linker-version, or environment
  fingerprinting in this slice.
- No change to source/module interface fingerprints.

## Design

Add `runtime_c_fingerprint: Option<u64>` to `RunCacheMetadata`,
`BuildCacheMetadata`, `RunCacheKey`, and `BuildCacheKey`. Metadata fields use
`#[serde(default)]`, so existing JSON still loads. Commands compute the current
runtime fingerprint by streaming `runtime.c` bytes through the existing
`file_fingerprint` helper. When no runtime source is present, the value remains
`None`.

Run/build metadata matches require both runtime path and runtime byte
fingerprint equality. Mismatch diagnostics distinguish `runtime path changed`
from `runtime source changed`.

Replace runtime object-cache path identity based on length and second-level
mtime with canonical path plus `file_fingerprint(runtime_c)`, optimization
level, and target. This preserves deterministic reuse for unchanged content
and invalidates same-size rapid edits.

The main Sengoo object can still be reused when only runtime bytes changed.
Native linking already appends the runtime object and relinks after a metadata
miss, so no broad source workset invalidation is needed.

## Risks / Trade-offs

- **Risk:** Hashing `runtime.c` adds work to cached runs and builds.
  **Mitigation:** stream one small runtime source file; avoid hashing unrelated
  toolchain files.
- **Risk:** Old metadata starts missing after upgrade.
  **Mitigation:** accept a one-time rebuild; stale native reuse is worse.
- **Risk:** `DefaultHasher` is not a cryptographic digest.
  **Mitigation:** this is a local invalidation key, not an adversarial integrity
  boundary.
- **Risk:** Other toolchain changes can still leave stale cache entries.
  **Mitigation:** keep this slice scoped to the reproduced runtime-source bug
  and specify further toolchain fingerprints separately if needed.

## Verification

- Run-cache metadata misses when only the runtime byte fingerprint changes.
- Build-cache metadata misses when only the runtime byte fingerprint changes.
- Cache-miss diagnostics mention runtime-source changes.
- Runtime object-cache paths differ for two byte payloads with equal lengths.
- Existing `sgc` tests, formatting, clippy, OpenSpec validation, and diff checks
  remain green.
