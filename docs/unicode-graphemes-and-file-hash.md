# Unicode graphemes and bounded file hashing

These development stdlib additions preserve edition 2026 and runtime ABI 1.
Use the current `0.2.1-dev.0` toolchain containing these modules; published v0.2
projects remain compatible, but older compilers do not provide the new APIs.
Windows x64 is the verified execution host for the checks below.

## Pure Sengoo Unicode17

`import std::unicode;` provides extended grapheme boundaries using Unicode17
UAX #29 GB3 through GB999, including Hangul, Indic conjuncts, regional-indicator
flags, combining marks and pictographic ZWJ sequences. Algorithms and generated
property lookup trees are Sengoo source. The generator reads pinned local data;
ordinary compiler builds do not run Python or download Unicode files.

| API | Behavior |
| --- | --- |
| `unicode_grapheme_state()` | Fresh state for a scalar stream; retain it across transport chunks |
| `state.push(codepoint)` | `Result<bool,i64>`: true starts a grapheme before that scalar; invalid scalars fail without advancing |
| `unicode_next_grapheme(text: &String, cursor)` | Next byte boundary; the end remains the end |
| `unicode_previous_grapheme(text: &String, cursor)` | Previous byte boundary; zero remains zero |
| `unicode_display_width(text: &String)` | Sum of deterministic cell widths per extended grapheme |
| `unicode_scalar_cells(codepoint)` | Scalar policy for callers that already maintain cluster state |

Cursor operations require a byte offset that already names a grapheme boundary;
an offset inside UTF-8 or inside a composed grapheme returns invalid-argument.
Owned String length preserves embedded NUL. These convenience operations borrow
the text and release their temporary scalar iterator on success and failure.
For wrapping or streaming use the state interface to avoid rescanning a string
for every scalar. The existing String byte/scalar APIs keep their semantics.

The terminal policy treats East Asian W/F text and emoji presentation as two
cells, ambiguous-width text as one, and combining/default-ignorable marks as
zero. It combines a family emoji, a flag, skin-tone sequence or keycap into one
cluster width rather than summing every emoji scalar. VS15/VS16 select text/emoji
presentation where applicable. This is a fixed rendering policy, not a guarantee
about each terminal/font. It does not shape fonts, expand tabs, reorder bidi
text, implement line layout or sanitize control sequences. Controls have zero
width; sanitize untrusted text before writing it to a terminal.

`cargo test -p sgc --test unicode_grapheme` passed on Windows x64 on 2026-11-04:
all **766** official Unicode17 GraphemeBreakTest cases plus editing, byte-boundary,
Unicode/NUL, Hangul/Indic, flag/ZWJ/skin-tone/keycap and width regressions.
The official corpus verifies boundary rules; terminal widths are separately
tested policy examples and do not have a universal terminal/font oracle.

Run `python scripts/generate-unicode-sg.py --check` to verify each input SHA256
and the generated `unicode_tables.sg`. Source URLs, hashes and the Unicode data
license are [recorded with the inputs](../tools/stdlib/unicode-data/README.md).
The toolchain packager includes that complete directory. A local `-NoBuild`
packaging smoke verified the required source/data entries against manifest
SHA256 values, including the pinned license hash; its provenance remains
`prebuilt-unverified`. It is payload-layout evidence, not a tested release.

## File SHA256 without loading the whole artifact

`import std::hash;` adds
`hash_sha256_file(path: &str, max_bytes: i64) -> Result<String,i64>`. Success is
exactly 64 lowercase hexadecimal characters. The native bridge reads a regular
file in 64KiB chunks using the same SHA256 compression implementation as the
existing `hash_sha256_str` and buffer interfaces. Total file size is independent
of the managed Buffer capacity and does not increase native input buffering.

The explicit cap allows zero for an empty file. An over-cap file reports
overflow, invalid/negative or unrepresentable SHA256 length limits report
invalid-argument, and missing/unreadable inputs report a status error. Windows
paths use strict UTF-8 to wide API conversion. Directories, devices and reparse
targets are rejected; files are opened for shared read without write/delete
sharing while hashing. POSIX uses a nonblocking, close-on-exec regular-file
descriptor, rejects symlinks where O_NOFOLLOW exists and verifies observed
size/mtime after reading; this branch has no execution evidence from this host.
This API hashes an observed file and does not by itself bind a later launch or
establish an artifact's trust.

`cargo test -p sgc --test hash_file` passed 2/2 on the same host: a 129MiB+17
artifact with a Chinese/emoji path matches Rust SHA256, the cap rejects one byte
less, the existing string API remains usable, and an independent native probe
covers known empty/abc vectors, one-byte reads, injected read failures over 100
cycles, handle cleanup, invalid UTF-8/NUL, missing files and nonregular targets.
The installed-runtime suite passed 15/15 with a fresh external consumer that
calls both this file hash API and Unicode grapheme width, in addition to IPC and
terminal ownership. Its check/build/run/test commands do not invoke Cargo or
expose the compiler checkout as runtime identity.

The host baseline for these working-tree results is
`d46b3f6cd8d6e770734f98edc57aa67cb931e26a`, using clang 19.1.7 and local MSVC.
Candidate verification must rerun them at its committed revision; this is not
a published compatibility or non-Windows certification claim.
