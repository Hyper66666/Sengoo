# sgmcp 0.1.0 (experimental)

Sengoo-native MCP over stdio: an owned, pollable client and an application-driven
server. Requires **Sengoo 0.2.1-dev.0 with the current process/JSON APIs**, native
backend, edition 2026. `sgschema` is a sibling package dependency. No native
code, GUI, credential store, Sencode database or Agent state is embedded here.

The byte framing and process-draining design are extracted from Sencode CLI's
`mcp.sg`. CLI policy, user input and permission decisions remain in its adapter;
SDK consumers supply their own. Nothing automatically retries a tool call.

## Installation

For a source checkout:

```toml
[dependencies]
sgmcp = { path = "../sgmcp" }
```

The release integrator adds `sgmcp` and its `sgschema` dependency to the official
offline registry bundle; **this source package does not itself establish a
published registry version**. Run `sgpm update`, retain `Sengoo.lock`, and build
with `--locked` after the bundle is available. Examples use `import sgmcp;`.

## Client

```sengoo
let args = json_parse("[]")?;
let mut client = sgmcp_start(server_executable, args.root(), workspace)?;
client.begin_initialize("2025-11-25", "my-agent", 5000)?;
loop { if client.poll()? { break; }; time_sleep_ms(1); };
client.result()?;
client.finish_initialize()?;
let params = json_parse("{\"name\":\"echo\",\"arguments\":{\"message\":\"Hello\"}}")?;
client.begin("tools/call", params.root(), 30000)?;
loop {
    // The host can process input/other jobs here, or call client.cancel().
    if client.poll()? { break; };
    time_sleep_ms(1);
};
let response = client.result()?;
client.close()?;
```

- One request is active per client. `begin` rejects overlap. Use independent
  clients for independent servers. Deadlines use a monotonic clock and cover
  sending, waiting and receiving; progress does not reset them.
- `poll` does bounded writes and drains stdout and stderr without reading the
  application's stdin. `true` means terminal; `result()` returns the outcome.
- A matching JSON-RPC error gives numeric status **9** while preserving the
  complete error envelope through `rpc_response()`. Failed Results carry no
  owned value; the explicit accessor returns an owned successful Result. A tool result's `isError` remains
  an application result, separate from transport/RPC failure.
- `next_notification()` returns one queued notification, or `{}` if empty.
  Drain these while polling. A full notification queue applies backpressure:
  `poll()` returns false until the owner drains it; progress is not discarded
  and does not turn a long-running tool into a capacity error. The original
  deadline still applies while the application is not consuming output.
  It never executes sampling, root access or permission requests. Unsupported
  server-initiated requests receive `-32601`.
- `finish_initialize` verifies the negotiated version and queues
  `notifications/initialized` before any subsequent request. Supported versions
  are **2025-11-25** and **2024-11-05**; unsupported replies are rejected.
- `cancel()` attempts a cancellation notification only for a completely sent,
  active non-initialize request, then terminates and reaps the process tree.
  Initialization timeout closes the transport without an illegal cancellation
  notification. `close()` and Drop also reclaim owned child processes.
- Transport/protocol errors never cause automatic resending. A caller must
  distinguish a known unexecuted operation from an uncertain side effect.

`examples/echo-client.sg` is a complete working client, including pagination.

## Server

`sgmcp_server(name, version, page_size)` creates an in-process protocol state
owner. Register tools before negotiation using `server.tool(name, description,
input_schema, output_schema)`. Both schemas use `sgschema`'s documented explicit
subset; unknown schema keywords are rejected rather than ignored.

`server.accept(request)` handles initialization, initialized notifications,
ping, paged tools/list, cancellation and unsupported methods. For a validated
tools/call it returns:

```json
{"kind":"call","id":"opaque-id","name":"echo","arguments":{"message":"Hello"}}
```

The application applies its policy and executes that operation. It completes
with `server.complete(id, result)` or `server.fail_tool(id, message)`. Success
requires `content` plus `structuredContent` matching the registered output
schema; an invalid result leaves the call active so the application can correct
it or report a tool error. `fail_tool` returns a normal MCP `isError` result.

`server.progress(id, progress, total, message)` emits the original progressToken
only when the client supplied one. `server.cancelled(id)` lets a cooperative
handler stop work. Completing a cancelled call removes it without responding.
Unknown and malformed cancellation IDs do not cancel another request.

The server does not call application handlers inline, so the owner can keep
feeding incoming cancellation while work proceeds. Drain `next_output()` into
the host's transport; `sgmcp_stdio()` provides optional stdin/stdout glue using
the generic non-TTY terminal polling primitive, without entering raw mode.
`examples/echo-server.sg` demonstrates text, structured output and cancellation.
Protocol output must go to stdout; logs go to stderr.
The echo application's `slow` cancellation deliberately shuts down its stdio
host after quiescent cleanup. The server core itself only cancels the matching
call; other hosts may keep the connection and other calls active.

## Bounds and status

| Input/resource | Default limit |
| --- | --- |
| Frame and outgoing pipe queue | 1 MiB |
| JSON document nodes | 32,768 |
| Retained stderr prefix | 64 KiB, always drained afterwards |
| Pending client notifications / server outputs | 64 each |
| Registered server tools / page size | 256 / caller-selected 1–256 |
| Concurrent server calls | 64 |
| One client deadline | caller-selected 1–3,600,000 ms |

The framer separately accepts an explicit 128-byte–16-MiB capacity. Oversized
input fails with **13**; invalid lifecycle/closed handle **3**, bad argument
**2**, malformed protocol **10** (strict UTF-8 keeps its runtime code), timeout
**11**, cancellation **19**, and an unconfirmed process-tree cleanup **12**.
These are resource bounds, not model-request budgets.

Stdout must contain newline-delimited JSON-RPC only. UTF-8 scalars can cross
reads; decoding happens after a complete line. CRLF and LF are accepted.
Truncated EOF is rejected. The server supports tools only: resources, prompts,
remote HTTP, OAuth, sampling, elicitation and task extensions are not advertised.
The application still owns tool sandboxing and authorization.

Protocol control strings and all object keys reject embedded NUL before any
C-string comparison. Opaque IDs retain their original serialized JSON identity,
including NUL; business values remain JSON and can preserve NUL by copying them.
The echo server demonstrates this without narrowing payload text to `&str`.
For configuration-owned strings prefer `sgmcp_start_owned`, `begin_owned`,
`sgmcp_server_owned` and `tool_owned`, which validate full owned lengths before
narrowing command/path/method/identity strings. Existing `&str` entry points
remain for already validated strings and literals.

The optional stdio adapter uses Sengoo's generic `StdWriter` on supported Windows
local byte pipes. `flush(&server)` submits at most 64 KiB per call and returns
false while output remains pending; keep calling `next()` to process input and
cancellation between flushes. Its single pending frame is bounded to 1 MiB plus
newline, and queue plus in-flight writer storage is bounded to 64 KiB. The core's
64-document outbox remains separately bounded; applications must pace producers.
`send(&document)` enqueues one frame and rejects another while that frame remains
pending. Acceptance is not peer delivery, and failed/unknown output is never
replayed automatically.

`sgmcp_stdio_with_timeout(ms)` selects an output deadline from 1 to 600,000 ms;
the default is 30,000 ms. Partial progress and repeated flushes do not restart a
pending output deadline. At normal shutdown use `finish(&server, timeout_ms)` to
drain and join; a false result retains the owner and is not successful cleanup.
For cancellation use `cancel(timeout_ms)` and check its quiescence result. Drop
also cancels and joins, so returning early cannot leave a detached writer.
Console, file, message-pipe and unimplemented platform endpoints return explicit
unsupported status. Applications can use the pure server core with another
transport on those endpoints.

## Verification

- `tests/protocol.sg`: byte-wise framing, Unicode, lifecycle, version negotiation,
  pagination, IDs, schema validation, structured results, progress, cancellation.
- `tests/client.sg` with `tests/hostile_server.py`: real process transport,
  output bounds, large stderr, deadlines, server exit, explicit cancellation,
  JSON-RPC errors and resource-count baseline checks.
- `tests/verify.py`: builds/runs the package and examples and drives the Sengoo
  server from an independent Python client. Reports label fixture evidence.
- `tests/backpressure.py`: independently stops reading a real server stdout
  pipe, then verifies concurrent input cancellation and the original deadline,
  followed by exact protocol/writer resource baselines.

No API key, user configuration, shell authorization or network service is used
by these tests. Current host evidence belongs in `EVIDENCE.md`; unrun platforms
and configured CI jobs must not be called tested.

Protocol references: [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle),
[tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools),
[cancellation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation).
