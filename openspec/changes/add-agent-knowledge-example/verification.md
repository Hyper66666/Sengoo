# Verification

Windows x64, DESKTOP-FUAQMFB, 2026-11-09; baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes.
See `examples/realworld/agent-knowledge/EVIDENCE.md` and
`target/agent-knowledge/external-evidence.json` for exact compiler, consumer
source/lock and executable hashes and command outputs. The pinned compiler is
`6b40efa12c374adbc48dd7f543ddc83820e00140fa87689b3cb3a35479039c7a`; native
executable SHA256 is
`54612c6b91e6c8e29c5d2f62c5e2789e73e0a983484ce290f0a68240a231b5cb`.

Real repository-external locked build passed. All copied sources and packages
were removed before execution with System32-only PATH and no source-runtime
overrides. The independent Python MCP client passes all six protocol groups,
including reparse traversal refusal, malformed text/path/schema, transaction
rollback after a SQL failure and persistent index restart/explicit refresh.
Current formatting and dedicated strict OpenSpec validation pass.

This closes the independent local-path consumer gate. It does not establish
offline registry publication, installer VM, other platform or live-service
qualification. No model, embedding, credential or user-file fixture is used.
