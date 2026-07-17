# Migration: v0.1.0-rc.1 → v0.2

This guide records source and tool behavior changes between the retained
`v0.1.0-rc.1` compatibility fixture and the v0.2 mainstream core line.

## Still compatible without edits

- Edition remains `edition = "2026"`.
- Manifest `sengoo-schema` omitted or `1`.
- Lockfile v1/v2 readers; new graphs write v2.
- Existing one-shot file/io/string APIs and Buffer capacity/used-length contracts.
- Status categories `0`–`19` keep their numeric values.

## Additive v0.2 surfaces

| Surface | Notes |
| --- | --- |
| `STATUS_INVALID_UTF8` (`20`) | New category for known malformed UTF-8 |
| `string_from_utf8` | Strict constructor; `string_from_buffer` same strictness |
| `std::stream` | `Reader`/`Writer` traits, `Cursor`, `read_to_end` / `write_all` / `copy_stream` |
| `String.char_count()` / `char_codepoint` | Scalar count and migration helper |
| `chars()` item type | Projects `char`; use `char_codepoint` or `next_codepoint` for integer consumers |

## Deprecated / transitional

| Surface | Replacement | Earliest removal |
| --- | --- | --- |
| Treating `chars().next()` as raw i64 without migration | `char` + `char_codepoint` / `next_codepoint` | After v0.2.x window (not removed in v0.2.x) |
| Manual handle `.free()` / `.drop()` / `.close()` where Drop is automatic | Rely on automatic Drop; keep explicit release only for documented dual paths | Not removed in v0.2.x |

Deprecation diagnostics must name the replacement and earliest removal line.
Patch releases do not remove a deprecated interface.

## Tooling

- Unsupported explicit editions still reject with `unsupported Sengoo edition`.
- Unknown schema/ABI versions reject before version-dependent parsing.
- Unclassified public-input panics remain release blockers; prefer stable
  diagnostics/status codes.

## Residual (not a silent upgrade claim)

- Production HTTP handlers, keep-alive, response streaming, and TLS server stay
  under open owner `http-production-serving` until archived with proof.
- Portable WASM remains experimental scalar only; bytecode production remains
  NO-GO.
