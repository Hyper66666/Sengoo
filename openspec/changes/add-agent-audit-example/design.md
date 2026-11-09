# Design

The application owns two pollable ProviderJobs and independent histories. The
same main loop polls each without waiting for a complete response. A monotonic
deadline and optional cancellation-file request cancel both jobs. HTTP success
does not admit tools: only a completed, validated provider turn does.

A trusted config selects at most 32 relative paths. Existing generic file
snapshots reject reparse traversal and pin the final local files; canonical
root boundaries are checked before copying text. Each file is at most 32 KiB;
the complete snapshot is at most 128 KiB. Tools read immutable captured text,
not current filesystem state. Tool definitions are validated by sgschema.

Tool identities are scoped per reviewer. Repeated identical calls reuse the
previous read-only result; conflicting reuse is rejected. Bounded retained
history and tool receipts are memory admission, not a model request-count budget.
Credentials come from named environment variables and never enter output.
