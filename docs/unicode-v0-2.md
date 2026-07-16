# Unicode v0.2 baseline

## Pinned version

- **Unicode**: 17.0.0 (`https://www.unicode.org/versions/Unicode17.0.0/`)
- **Provenance string** (runtime): `unicode_version_copy` / `unicode_provenance_copy`
  report `17.0.0` and a short provenance note.

v0.2 implements a **strict UTF-8 + scalar** foundation, not a full Unicode
property database:

| Surface | Status |
| --- | --- |
| Strict UTF-8 validation on owned `String` construction | Supported |
| `STATUS_INVALID_UTF8` (`20`) | Supported |
| `String.len()` / `str.len()` as **UTF-8 byte** counts | Supported (stable) |
| `String.chars()` as `Iterator<Item = char>` | Supported |
| `char_codepoint(char) -> Result<i64, i64>` migration helper | Supported |
| `String.char_count()` scalar count | Supported |
| Simple ASCII upper/lower (`str_to_ascii_*`) | Supported |
| Full property tables, simple upper/lower, casefold | Deferred |
| Normalization, graphemes, collation, locales | Deferred (explicit) |

## Constructors

- `string_from_utf8(buffer, used_len)` — strict; malformed input → `STATUS_INVALID_UTF8`.
- `string_from_buffer(buffer, used_len)` — compatibility wrapper; **same** strict UTF-8
  behavior (no silent lossy acceptance).

## Indexing clarity

- Byte offsets are valid only on UTF-8 scalar boundaries for slice/`get` APIs.
- Scalar count (`char_count`) is **not** grapheme count.

## Status taxonomy

- `STATUS_PARSE` (`10`) remains for generic syntax/data parse failures.
- `STATUS_INVALID_UTF8` (`20`) is used only when malformed UTF-8 is known.
