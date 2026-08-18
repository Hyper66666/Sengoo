# Senline HTTP Dogfood

`senline-http-dogfood` is a development-only localhost harness for synthetic
Senline V1 facts. It depends on the sibling `senline_domain_worker` package and
calls its strict decoder, typed request model, pure planner, and normalized
plan/error encoders directly. It does not carry a second protocol codec.

The executable has no bind configuration. It permits only `127.0.0.1:0`, asks
the OS for an ephemeral port, prints one machine-readable
`READY 127.0.0.1:<port>` line, and handles one request before closing. The only
accepted transport shape is `POST /v1/submit-envelope HTTP/1.1` with no query,
exact `Content-Type: application/json`, no `Transfer-Encoding`, and a body of
1..32768 bytes. Headers are capped at 4096 bytes and normalized output at 8192
bytes. Only `execution_mode=fixture` reaches the planner through this harness.

HTTP response bodies preserve the worker payload exactly, including its final
LF byte. Plan and normalized worker error envelopes use status 200 so transport
status cannot rewrite their frozen bytes. HTTP method, path, header, and body
policy failures are bounded transport-level 400 responses.

The retained server subset is serial and plaintext with `Connection: close`.
It does not support TLS, keep-alive, streaming, callback handlers, general task
cancellation, production or internal-alpha ingress, deployment manifests, or
client endpoints. Sandbox, supervisor, deadlines, admission, final validation,
mutation, and rollback remain owned by Senline Rust.

Run the locked source-development loop with:

```powershell
sgpm --runtime-mode source-development check --locked
sgpm --runtime-mode source-development test --locked
sgpm fmt --check --locked
sgpm --runtime-mode source-development doc --locked
sgpm --runtime-mode source-development build --locked --release
```
