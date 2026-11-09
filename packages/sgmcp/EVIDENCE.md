# MCP SDK verification

Development host: Windows x64, DESKTOP-FUAQMFB, 2026-11-09.
Baseline Sengoo `ea803f1913f9d08f6393d412b0eb4b1de3055b3e` with working changes.
The package is experimental; no public release, live external MCP service or
Linux/macOS qualification is implied by these local checks.

## Frozen package verification

`tests/verify.py` copies `sgmcp` and its `sgschema` dependency to a temporary
directory outside the checkout, builds the real client/server/examples, and
drives the server using an independent Python client. Protocol fixtures do not
read user histories, credentials or configuration and make no network request.
The final passing record is
`target/sgmcp-sdk-20261109/evidence-nul-final-2.json`.
It records 23 successful steps: ten format checks, seven native builds and six
native/process runs, plus the independent Python client's assertions. Fourteen
SDK source inputs and 98 stdlib/runtime/Cargo inputs retain their initial hashes.

Final checks pass with compiler
`target/agent-unused-compiler/debug/sgc.exe` (SHA256
`f7ea03a89c27ccedf8e6f2f8c81fe7ab6008887dd028fd8848102774e2f5dde5`)
and formatter `target/agent-formatter/debug/sgfmt.exe` (SHA256
`d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`).
The library SHA256 is
`aab4faa797ae5c404437af7249f650b9bcc2c6d7e8b542273781e0d85f6bddcf`.

- Native framing/lifecycle includes byte-wise Unicode/CRLF, version negotiation,
  paged tools, schema mismatch refusal, output validation, opaque IDs and cancel.
- The native client consumes a Sengoo server over real process pipes, then
  exercises synthetic peers: 160 progress events with a 64-entry queue and
  deterministic full-queue backpressure, stderr draining/truncation, RPC error
  preservation, repeated failed-result document counts, oversized/invalid
  output, crash, cancellation, unsupported versions and deadlines.
- Process handle/resource counters return to zero after each client scope.
- An independent Python client has driven the Sengoo server through split
  Unicode input, paging, progress, structured output, errors and cancellation.
- NUL-suffixed methods, versions, names, peer identities, cursors and object
  keys are refused before C-string narrowing. Opaque string IDs and business
  values containing NUL survive JSON copies end to end, including both text and
  structured echo results. Missing/wrong-type/NUL accessor fallbacks return
  valid owned empty strings, with twenty iterations restoring the String count.

## Language defects discovered by real consumption

The initial resource check exposed borrowed `close(&mut self)` calls suppressing
the enclosing owner's scope cleanup. `tests/borrowed_close.sg` is an independent
minimal reproduction; the compiler owner fixed the release-method classification.
That regression now passes with the compiler pinned above. No manual resource
free was added to the SDK to hide that defect.

The progress stress also found a discarded successful owning Try value could
escape cleanup (`client.next_notification()?;`). The retained independent
reproduction is `tests/unused_owned.sg`. The compiler now scopes discarded
owning expression values and temporary borrow chains correctly; that native
regression passes in this same frozen run. SDK consumers also bind documents
when they need to inspect them. This was distinct from an HTTP/MCP error or
queue capacity policy.

An old local formatter binary changed `return self.method()` into a bare return.
`tests/formatter_return.sg` preserves that reproducer. The rebuilt current
formatter preserves the expression and every package SG file passes its check.

A NUL-rejection preflight exposed a package bug returning `String { handle: 0 }`
as a successful optional-string value. The helper now returns a real owned
empty String; all three malformed-input shapes and their resource baseline pass.
Zero handles remain confined to inactive error placeholders.

Failed and intermediate driver outputs remain under
`target/sgmcp-sdk-20261109/` for diagnosis and are not counted as final passes.

## Reproduce

```powershell
python packages/sgmcp/tests/verify.py `
  --compiler target/agent-unused-compiler/debug/sgc.exe `
  --clang C:/path/to/clang.exe `
  --formatter target/agent-formatter/debug/sgfmt.exe `
  --output target/sgmcp-sdk/evidence.json
```

The integration owner separately verifies the Sencode adapter, locked `sgpm`
consumption, offline registry distribution and installation. Those gates are
not established merely by copying package sources in this SDK test.
The historical run above used synchronous stdout glue. The current adapter uses
generic `StdWriter` and adds independent blocked-output cancellation, partial
progress with an unchanged deadline, and exact resource checks. Its new frozen
report is recorded separately below; older SDK evidence does not certify this
adapter or a later compiler/runtime.
