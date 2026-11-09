# Verification

Windows x64, DESKTOP-FUAQMFB, 2026-11-09; source baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus recorded working changes.

The final SDK driver passes 23 steps and an independent Python client:
`target/sgmcp-sdk-20261109/evidence-nul-final-2.json`.
The library SHA256 is
`aab4faa797ae5c404437af7249f650b9bcc2c6d7e8b542273781e0d85f6bddcf`.
Compiler2 is pinned to SHA256
`f7ea03a89c27ccedf8e6f2f8c81fe7ab6008887dd028fd8848102774e2f5dde5`;
the current formatter to
`d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.

The driver copies packages outside the checkout and records 14 package and
98 foundation source hashes unchanged across ten format checks, seven builds,
six native/process executions and independent Python protocol assertions.
Coverage and exact reproductions are documented in `packages/sgmcp/EVIDENCE.md`.

Native protocol coverage includes split UTF-8/CRLF, initialization, version
negotiation, paged tools, unknown schema rejection, validated dispatch/output,
tool error semantics, progress identity, unknown/matching cancellation and
unsupported methods. The client uses real process pipes; hostile peers cover
stderr truncation while draining, oversized/malformed output, RPC errors,
deadlines, explicit cancellation and process exit. A full 64-entry queue
applies backpressure, and 160 notifications are consumed without loss. Failed
results retain diagnostics through a successful accessor while carrying no
owning inactive value; document and process counters return to their baselines.

NUL negatives cover control methods, versions, peer names, tool names,
pagination cursors and recursive object keys in both directions. Independent
opaque IDs and business values preserve NUL as JSON, without prefix matching.
The independent client verifies both echo representations retain A-NUL-B.
Missing/wrong-type/NUL optional fields return real empty Strings and twenty
iterations restore the managed String count.

The SDK exposed borrowed-close and discarded-owning-Try compiler cleanup bugs.
Their independent native programs, plus a formatter-return program, pass with
the same compiler/formatter. Failed intermediate records are retained and do
not count as passes.

This evidence establishes local source-runtime and synthetic peer behavior;
it does not establish public publication, live-service interoperability,
fresh-VM installation, Linux/macOS or the integration owner's separate locked
registry and Sencode adapter gates. The optional server stdout glue is
synchronous; generic pollable stdout remains a separately tracked foundation
requirement. No tool policy, side-effect retry or optional unsupported MCP
capability is inferred from these protocol checks.
