# Design

The client's single state owner exposes begin/poll/result. Poll writes at most
8 KiB and reads bounded stdout/stderr chunks; it never reads the host's stdin.
Each operation retains one monotonic deadline through send/backpressure and
progress. A complete matching result or RPC error ends it. Errors retain the
original JSON envelope, and no tool is resent automatically.

MCP line framing decodes UTF-8 only after newline, with 1 MiB default message and
32,768-node bounds. Retained stderr is capped at 64 KiB but continues draining.
Notification queues are bounded; the client applies cooperative backpressure
until the host drains them, without silently losing progress or resetting the
deadline. Buffered complete lines are drained before reading more stdout.
Cancellation notifies only a completely sent
active non-initialize request, then terminates/reaps its process tree. Explicit
close and Drop own process cleanup. Initialization is never cancelled by
notifications/cancelled. Numeric statuses preserve existing runtime conventions.

The server's accept method does not execute handlers. It returns a validated
dispatch envelope while the caller can continue polling input and observing
cancellation. Registration freezes at initialization; listChanged is not
advertised. Tools use the shared sgschema subset before dispatch and for
successful structured output. Tool failures use isError, protocol failures use
JSON-RPC errors. Opaque string/number IDs and progress tokens retain identity.
Cancellation affects only matching active calls; completion after cancellation
cleans up without sending a response. SDK policy cannot grant host permissions.

Version negotiation is pinned to 2025-11-25 and legacy 2024-11-05. The latter
keeps existing Sencode compatibility; unsupported client versions receive a
supported version, while clients reject unsupported replies. No optional
capability is advertised without implementation. Public native examples prove
both client and server against independent peers, outside Sencode state.

The optional SgMcpStdio uses the generic std::terminal non-TTY input polling
primitive without raw/fullscreen mode. Its protocol core also accepts host-fed
JsonDocs, so other frontends are not tied to terminal input. Output is NDJSON;
application logs must use stderr. No new native code is added.
