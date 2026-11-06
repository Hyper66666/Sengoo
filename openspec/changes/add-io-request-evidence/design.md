# Design

The worker stores a release-ordered atomic flag immediately before its first
application write. DNS, TCP and TLS handshake leave the flag false. A query uses
acquire ordering and returns 0/1, or -3 for an invalid/closed handle. It never
resets after cancellation. False is useful for retry only after terminal failure;
a running worker may write immediately after a query. True does not prove that
the server received or acted on the request. Close removes the handle normally.

Windows console records retain at most one repeat payload (four UTF-8 bytes or
one special-key code plus modifiers). Each subsequent read delivers one repeat
before consuming the next OS record. UTF-16 surrogate halves are combined before
retaining a repeat payload. `discard_input` clears native and retained repeats,
preserving the fresh-input approval barrier. State and memory remain bounded.

The runtime does not interpret HTTP status, approvals, commands or agent state.
