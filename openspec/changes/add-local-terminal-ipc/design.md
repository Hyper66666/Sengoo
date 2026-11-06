# Design

TerminalEvent retains its existing fields and numeric kinds 0 through 5.
Mouse is appended as kind 6, with a separate TerminalMouseEvent snapshot
accessor on Terminal. Coordinates are zero-based viewport cells; actions and
button identifiers are documented constants. Fullscreen preserves prior raw
and mouse ownership, is idempotent and is unwound by restore, Drop and normal
process exit. Native code handles console state and byte/key events only.

Named pipes expose owning listeners and streams, bounded nonblocking byte I/O,
poll, EOF and close. Handles are generation checked and non-Send/non-Sync.
Windows endpoints are locally restricted, DACL-limited to the current logon SID
and validate their peer's user and logon identity. The runtime does not parse
application messages or negotiate their versions. The public module and exact
verification are recorded with implementation.

StringBytesIter, StringCharsIter and StringSplitIter keep their one-i64 layout
and existing C entry points. Their i64 is now an opaque generation token, checked
against a reusable slot registry before accessing storage. Byte/scalar and split
tokens occupy distinct domains. Each Sengoo wrapper implements Drop. Explicit
free borrows the wrapper; another free before slot reuse is successful, while a
stale token after reuse cannot free the new owner. The representation stays
runtime ABI 1; code must never treat an iterator token as a native pointer.
