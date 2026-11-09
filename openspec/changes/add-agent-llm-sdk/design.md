# Design

The package is the single shared implementation extracted from Sencode's pure
protocol modules. It imports only standard String/JSON/Buffer/collections/net.
Application connection selection, credentials, image authorization and files,
UI events, persistence, retry policy and tool admission remain outside the SDK.
Compatibility names allow Sencode to replace its implementations with imports.

`LlmRequestConfig` owns endpoint, protocol, model, effort, opaque connection
scope and explicit image capability. Credentials are supplied per start and are
not retained in configuration or diagnostic records. `LlmLimits` validates
timeouts, bounded byte queues and complete serialized request limits before a
network job starts. The native byte worker only moves bytes; HTTP/SSE and model
events remain Sengoo-owned on the state owner thread.

`ProviderJob.poll` returns available text without blocking and latches failures
as terminal, cancels its worker and retains closed diagnostic categories.
`result` refuses incomplete HTTP bodies even if a model completion was observed.
Only a complete, validated turn exposes canonical tool calls. Callers still
perform schema checks, durable admission and permission decisions before tools.
`LlmFailure` records numeric error, HTTP status, parser stage, safe reason,
Retry-After, request-start uncertainty and whether stream bytes were observed.
It never returns an arbitrary error body or secret key. The SDK never retries,
swaps models or replays tools; the application decides how to recover.

Cancellation is explicit, terminal and non-successful. Drop releases the owning
network handle, parser buffers and provider state. Tests wait for native workers
and request reservations to return to baseline. Owned API objects do not cross
threads; the underlying NetByteStream forbids Send/Sync.

The current Result representation drops its payload only on success. Failed
results therefore carry zero handles, including the empty typed Vec literal
(the general vec_new intrinsic can allocate a handle). Local owners remain local
on early return so their Drop runs. Twenty repeated malformed/invalid/overflow
paths compare String, Buffer, bounded JSON and opaque runtime counters against
the initial baseline. Public byte-slice inputs and addition bounds are checked
before allocating a decoder output buffer.

Bounds preserve the tested native contracts: connect 1..60000 ms, idle
1..600000 ms, response byte queue 4096..1048576, request 1..16777216 bytes.
Protocol event, tool, history and text bounds remain documented and deterministic;
limits are resource admission, not a model request-count budget.

Verification copies the package outside the checkout. Native protocol/framing/
media/config checks and real four-way HTTP/SSE jobs use only the copied source.
Loopback peers split UTF-8 and tool JSON across byte/chunk boundaries, validate
headers and count jobs. Error peers distinguish HTTP500, invalid/incomplete
HTTP200, idle timeout and cancellation. A second manifest-backed consumer uses
sgpm update and locked check/run. No fixture or workflow definition counts as
live-service or four-platform proof.
