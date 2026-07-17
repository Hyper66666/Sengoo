## Decisions

### D1: HTTP keeps its existing owner

`http-production-serving` defines and implements handlers, keep-alive, response
streaming, and TLS server behavior. M3 depends on its archive and proves those
APIs alongside stream/Unicode fixtures; it does not redefine them.

### D2: Freeze synchronous stream trait names and signatures

```sg
trait Reader {
    def read_into(&mut self, out: &mut Buffer) -> Result<i64, i64>;
}

trait Writer {
    def write_buffer(&mut self, data: &Buffer, used_len: i64) -> Result<i64, i64>;
    def flush(&mut self) -> Result<bool, i64>;
}
```

The stdlib also provides generic helpers:

```sg
def read_to_end<R: Reader>(reader: &mut R, out: &mut Buffer) -> Result<i64, i64>;
def write_all<W: Writer>(writer: &mut W, data: &Buffer, used_len: i64) -> Result<i64, i64>;
def copy_stream<R: Reader, W: Writer>(reader: &mut R, writer: &mut W, scratch: &mut Buffer) -> Result<i64, i64>;
```

Existing concrete methods remain compatibility wrappers.

### D3: Pin partial I/O and EOF semantics

- `read_into`: `Ok(0)` means EOF; `Ok(n>0)` means exactly `n` bytes were written
  at the start of `out`; errors write no trusted bytes.
- `write_buffer`: `Ok(n)` may be partial and `0 <= n <= used_len`;
  `write_all` loops until complete or error and rejects repeated zero progress.
- `flush`: succeeds only after buffered bytes are accepted by the adapter.
- `read_to_end` never grows `out`; Buffer capacity is the hard limit and
  insufficient capacity maps to `STATUS_BUFFER_TOO_SMALL`.
- `copy_stream` uses only caller-provided scratch capacity and returns total
  bytes copied with checked overflow.

### D4: Initial adapters are bounded and additive

M3 provides adapters for supported file handles, stdin/stdout/stderr or owned fd
wrappers, and `TcpStream`. Existing path one-shot functions and string-based
send/write calls remain. Timeout/cancellation stays adapter-specific and maps to
the existing `STATUS_TIMEOUT`/`STATUS_CANCELED` taxonomy.

### D5: Pin Unicode 17.0.0 and preserve byte clarity

The normative upstream version is the Unicode Consortium's
`https://www.unicode.org/versions/Unicode17.0.0/`; generated data records the
exact source-file checksums used by the build.

- `String.len()` and `str.len()` remain UTF-8 byte counts.
- `chars()` returns `Iterator<Item = char>`; numeric consumers migrate through
  `char_codepoint(value: char) -> u32`.
- `char_count()` returns Unicode scalar count, not grapheme count.
- `string_from_utf8(buffer: Buffer, used_len: i64)` validates strictly and
  returns `STATUS_INVALID_UTF8` on malformed input; existing
  `string_from_buffer` remains a compatibility wrapper with the same strict
  behavior.
- `to_lowercase_simple`, `to_uppercase_simple`, and `casefold` use Unicode
  17.0.0 locale-independent data and return owned String values.
- `char_is_alphabetic`, `char_is_whitespace`, and `char_is_numeric` use the same
  pinned data version.
- Normalization and grapheme/locale behavior are explicitly unavailable rather
  than approximated.

`STATUS_INVALID_UTF8` is the stable positive category `20`. Runtime raw errors
map to this category only when malformed UTF-8 is known; generic syntax/data
parsing continues to use `STATUS_PARSE` (`10`). `status_name_copy` and
`status_message_copy` must expose the new category.

### D6: Bound generated tables and operations

Unicode tables are generated deterministically from checked-in version metadata
or pinned source data with provenance. Case expansion and output growth use
checked lengths and managed Buffers; no operation allocates from untrusted size
without a documented ceiling.
