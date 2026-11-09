# sgllm 0.1.0 — experimental native model SDK

Requires Sengoo **0.2.1-dev.0** with bounded JSON/network APIs, edition 2026,
native backend. Agent state, protocol parsing, HTTP framing and SSE decoding are
Sengoo code. DNS/TLS and byte movement use the general native network runtime.
The package does not require Sencode or read its database, configuration or keys.

```toml
[dependencies]
sgllm = { path = "../sgllm" }
```

Run `sgpm update`, retain `Sengoo.lock`, then use `sgpm check/build/run --locked`.
The integration distribution supplies exact-version local registry bundles; do
not treat this development checkout as a published remote registry.

## Request API

```sengoo
import sgllm;

def start(base: &str, key: &str) -> Result<ProviderJob, i64> {
    let config = llm_request_config(base, provider_protocol("responses"), "configured-model")?;
    let history = json_parse("{\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}")?;
    let tools = json_parse("{\"tools\":[]}")?;
    llm_request_start(&config, key, &history, &tools)
}
```

See `examples/stream.sg` for the complete polling loop. Supply credentials through
your own credential store; its command-line fixture input is not a recommendation
to put production secrets in process arguments. A config owns endpoint, protocol,
model, effort, opaque connection scope and explicit image capability, but no key.

`provider_protocol` accepts `chatCompletions`, `responses`, `messages` and the
retained documented protocol aliases; unknown values produce `-1` and request
validation rejects them. Model identity is application-supplied, not inferred.
`effort` defaults to empty (provider default), and uses the existing protocol
mapping when explicitly set. `images_allowed` defaults to false; enabling it is
a caller assertion about that exact model, never a guarantee from its name.

`ProviderJob.poll()` returns immediately with available owned text, including an
empty value when no bytes are ready. Continue polling until `is_finished()`.
`activity()` distinguishes reasoning (`1`) from other activity (`0`) without
exposing private reasoning text. `completed()` means terminal with a provider
completion, not merely HTTP200. `result()` before HTTP framing finishes returns
status12 without interrupting the active request. Final `result()` returns a
canonical owned `JsonDoc` with `role`, `content`, `tool_calls`, input/output tokens
and protocol continuation metadata. Check `usage_reported()` before treating
token totals as measured. Missing usage is not exact accounting.

Tool calls are `{id,name,arguments}` with complete JSON arguments and stable IDs.
The SDK does **not** execute them. Applications must validate their own tool
Schema, permissions, deduplication and durable admission. Feed accepted assistant
results back into `messages`, then tool messages with `tool_call_id` and `content`.
Responses opaque items remain scoped by endpoint, model and `config.scope`.
Changing scope strips provider-private items while preserving public text/tools.

Ordered text/image `content_parts` accept the retained validated inline PNG/JPEG
base64 shape. Files, image authorization and immutable file capture belong to
the application; no path is opened by the SDK. Borrowed string views cannot
represent embedded NUL; owned protocol strings with NUL are rejected before
being narrowed. No audio, PDF, video, remote image fetching or arbitrary output
tool types are promised by this first package.

## Errors, cancellation and limits

After a failed poll, call `llm_request_failure(&job, code)`. It returns the numeric
code, HTTP status, closed parser stage, safe reason, Retry-After and request-start/
stream-observed uncertainty. HTTP500 is distinct from a malformed HTTP200 SSE
stream. No arbitrary server error body or key is included. Parsing and admission
do not print to stdout/stderr; applications own presentation and logging.

Failure payloads contain zero handles rather than partially owned JSON, buffers,
endpoint strings or event vectors. As with the existing `Result` contract,
inspect `.value` only after `is_ok()`; use the job's failure accessor for error
details. The repeated failure regression checks String, Buffer, JSON and opaque
handle counts after each of twenty rounds, including invalid endpoints, malformed
events, completion rejection, SSE overflow and invalid/oversized HTTP slices.

A poll failure latches the terminal error and cancels the byte worker. Explicit
`cancel()` returns the job to terminal status19 and never reports a successful
answer. Drop closes the network handle; applications do not manually free SDK
strings, JSON or parser buffers. Cancellation interrupts I/O immediately, while
native worker reservations are reclaimed when the worker actually exits. This
package does not implement structured async cancellation or transferable Agent
state; keep job ownership on one thread.

The SDK performs no automatic retries, provider fallback, tool replay or request
count limiting. The host decides whether to retry a model request and must keep
tool side effects out of that retry loop. Request-start is uncertainty metadata,
not proof that a server accepted or rejected the request.

`llm_default_limits()` provides 30s connect, 90s idle, 256KiB queued response,
1MiB complete request and 1MiB body. Set `config.limits` explicitly to change:

| Input | Valid range |
| --- | --- |
| connect timeout | 1–60000 ms |
| idle timeout | 1–600000 ms |
| native response queue | 4096–1048576 bytes |
| complete HTTP request | 1–16777216 bytes |
| serialized body | 1 through complete request bound |

The complete request limit includes headers, key and escaped JSON. It is checked
before starting a worker. General native admission also limits eight workers/
handles and 64MiB total live request reservations. SSE event limit is 1MiB,
HTTP headers 64KiB, decoder pending bytes 2MiB, tool arguments 1MiB, provider
event JSON 1MiB/65536 nodes, and retained Responses metadata has its own bounds.
These are resource limits, not context-token budget guarantees. The application
supplies context/max-output token settings and enforces its own total budget.

The original `provider_*`, `http_*`, `sse_*` and `context_*` entry points are kept
for thin Sencode adapters. New consumers should use `llm_request_config/start`
and the job methods. All these APIs are experimental until a stable release.

## Verification

```text
python tests/run.py --compiler /path/to/sgc --sengoo /path/to/Sengoo --clang /path/to/clang --sgpm /path/to/sgpm --formatter /path/to/sgfmt --evidence target/acceptance.json
```

The runner copies this package outside its checkout, freezes input hashes,
compiles protocol/framing/media/config/network consumers, then exercises actual
native requests against synthetic peers. A separate repeated-resource consumer
checks owning failure paths. Coverage includes Unicode, byte/chunk
fragmentation, three protocols, tool arguments, usage, opaque target scopes,
HTTP500 versus invalid/incomplete HTTP200, timeout and four concurrent cancelled
jobs returning handles/workers/reservations to baseline. An independent manifest
consumer locks dependencies, checks and runs using sgpm. No paid API or user
credential store is accessed.

Current host results and exact source/compiler hashes are recorded in this
change's verification note and the named evidence output. A passing fixture does
not prove a live provider, four-platform runtime, clean-machine installation or
sanitizer qualification. Supported platforms are listed only after their real
candidate checks pass.
