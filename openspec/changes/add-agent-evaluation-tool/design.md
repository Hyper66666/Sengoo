# Design

The Sengoo core reads bounded UTF-8 files, validates complete raw event JSONL,
redacts case-insensitive credential keys and configured literal values, and
writes versioned sequence envelopes only after the complete input succeeds.
An explicit redactions file avoids secret values in argv. Error messages expose
only a numeric status and closed description, never input bodies or paths.

Replay validates schema and contiguous sequence IDs. Assertions cover exact
event count, ordered types, recursively equal values at object-key paths and
absent decoded strings. JSON integers retain their textual precision. Unknown
assertion fields fail closed. Reports have deterministic bytes, overall status
and individual check results. Existing output files are refused.

The tool imports no process/network/MCP/plugin dispatch API. Recorded tools are
inert data even if their arguments contain executable commands. It cannot
grant permissions, recover an Agent session or replay a side effect. Malformed
input creates no successful report. Assertion failure writes a failed report
and exits 1; malformed/I/O/capacity errors exit 2.

Bounds: 8 MiB file input, 1 MiB raw/2 MiB recorded line, 4096 events, 16 MiB
recording, 48 recursion depth, 65536 nodes/document, 128 literals (4096 bytes
each), and 1024 value checks. Bounds reject explicitly, never truncate.

NUL is unsupported. Input files are inspected through exact owned String byte
iteration before conversion to a C-compatible view. Every decoded JSON tree is
checked recursively, including object keys before lookup and string values
before redaction or assertion comparisons. Raw byte zero and escaped Unicode
NUL therefore fail explicitly instead of silently truncating a key or value.

Known credential fields are redacted recursively; free text and embedded JSON
strings require explicit literal redactions. This is documented as a recording
aid rather than an automatic privacy guarantee. Raw input/redactions stay out
of Git; tests use synthetic values and private temporary directories only.
