# Explicit bounded JSON documents

The additive development APIs keep edition 2026 and runtime ABI 1. Existing
`json_parse`/`json_parse_buffer` and legacy destination `copy` retain their
1 MiB admission. Existing unconstrained builder behavior is not silently
redefined. A matching development stdlib/native bridge payload is required.

```text
json_parse_bounded(text: &str, max_bytes: i64) -> Result<JsonDoc,i64>
json_parse_buffer_bounded(buffer: &Buffer, input_len: i64, max_bytes: i64)
    -> Result<JsonDoc,i64>
json_doc_object_bounded(max_bytes: i64) -> Result<JsonDoc,i64>
JsonDoc.new_string_bytes(value: &str) -> Result<JsonValue,i64>
JsonDoc.new_string_owned(value: &String) -> Result<JsonValue,i64>
```

Choose `max_bytes` from 1 through 16 MiB. Invalid limits return status 2 and
oversize raw lengths return status 13 before reading their input address or
creating a document. Borrowed buffer parsing also requires `input_len` to fit
initialized `used_len`, not just allocated capacity. The selected limit remains
with the document and bounds destination copy and serialized output. Owned
payload is cumulative across live nodes, including disconnected values. Failed
deep copies remove new nodes and restore payload/link charges. Serialized
failure never updates the caller's output buffer.

Extended documents retain 64-depth/4,096-node limits and permit at most 4,096
container links across the document. Repeated references cannot grow arrays or
objects without bound. Serialization detects cycles and depth excess in these
documents, even when the graph was assembled by public builder operations.

Each extended document reserves `6 * max_bytes + 4 MiB` from an aggregate
512 MiB admission budget; at most 64 such documents may live. This conservative
reservation covers bounded internal payload/capacity/temporary expansion and
metadata, and is released on parse failure, close or Drop. It is admission,
not a measurement of committed process memory. Managed String/Buffer inputs
and returned outputs retain their separate runtime ownership; retaining an
arbitrary number of those values is not bounded by document admission. Legacy
documents do not consume this new reservation and are not claimed to have the
new document-scoped guarantees. JsonDoc/JsonValue remain neither Send nor Sync.

Use `new_string_owned(&value)` for exact owned text, including embedded NUL.
The existing borrowed `&str` view is C terminated: converting an owned string
to `as_str()` can lose the visible suffix after NUL. `new_string_bytes(&str)`
preserves the view's measured length, while the owned form passes the String's
stored length to the same length-aware native operation. Deep copy preserves
existing JSON strings and object keys without C-string truncation.

Local Windows x64 evidence (2026-11-04):

- `json_large_legacy_limit.sg` reproduces >1 MiB status-10 rejection before
  runtime changes; the legacy bound stays unchanged.
- `native_explicit_json_bounds_precede_reads_and_release_admission` covers
  exact 16 MiB input, rejected invalid addresses/lengths, initialized buffers,
  NUL, cumulative number-text accounting, copy rollback, unchanged failed
  outputs, cycles, link limits, aggregate exhaustion and complete release.
- `public_json_limits_large_unicode_copy_and_ownership` compiles the real SG
  wrappers against >1 MiB UTF-8/escaped-NUL synthetic input and checks copy,
  serialization, owned NUL and scope Drop.
- Existing Unicode and transactional copy regressions are retained.

Logs are under `target/file-media/json-*`. Source/compiler hashes are recorded
in the local evidence manifest; the source tree is dirty and no release SHA is
claimed. Windows AddressSanitizer exits zero but emits the same LLVM 19
`interception_win` warning as the file snapshot probe; clean sanitizer and
other-host qualification remain open. Provider wire-format/request admission
has separate application loopback evidence and is not inferred from JSON tests.
