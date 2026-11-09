# Design

The integration owner owns packages/sgschema. Public validation returns an owned
structured verdict; runtime failures retain Result numeric errors. Definitions
are checked before instances so absent optional fields cannot hide unsupported
keywords. Pointer paths escape ~ and /, lengths count Unicode scalars, i64 enum
and bounds avoid f64 rounding. Depth and visit limits are explicit and independent
of document admission. Reject NUL keys before lookup. Raw duplicate keys require
wire-level parser validation and are not recoverable from an overwritten tree.

No refs, remote resolution, mutation, implicit coercion, callbacks or effects.
SDK consumers pass owned source documents by borrowed values for one call.
Tests cover schema and data failures separately, including capacity exhaustion.
