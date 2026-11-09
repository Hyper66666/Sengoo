# Windows local consumer evidence

Host DESKTOP-FUAQMFB, Windows x64, 2026-11-09. Sengoo baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes.

- Compiler `target/agent-final-compiler/debug/sgc.exe` SHA256
  `6b40efa12c374adbc48dd7f543ddc83820e00140fa87689b3cb3a35479039c7a`.
- Formatter `target/agent-formatter/debug/sgfmt.exe` SHA256
  `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
- sgpm SHA256 `0e8a3d7bd9579582c2da334a4c80240a28e35e67db60185b5879ebeddab9b06a`.
- Complete application SHA256
  `54612c6b91e6c8e29c5d2f62c5e2789e73e0a983484ce290f0a68240a231b5cb`;
  retained at `target/agent-knowledge/agent-knowledge.exe`.
- Exact consumer source/lock hashes and commands:
  `target/agent-knowledge/external-evidence.json`.
- Independent-client results: `target/agent-knowledge/protocol.json`.

The external runner copied the actual application and its three package sources
outside the checkout. Real sgpm `build --locked` passed and left its relative
lockfile unchanged. It deleted the complete copied source and dependency tree
before running the executable. System32 was the only PATH entry and all
`SENGOO_`/`SGPM_` overrides were removed. The Python driver was invoked by its
absolute interpreter path; the application needed no compiler or source path.

All six real MCP protocol groups pass:

1. Negotiation, public tool list/schemas and an explicit empty initial index.
2. Wrong types and model-supplied root overrides fail validation.
3. Atomic refresh, stored keyword search, Chinese/emoji and digest-correct read.
4. Traversal, sibling root prefix, junction/reparse path, NUL, invalid UTF-8 and
   files above 64 KiB are rejected or explicitly counted as skipped.
5. Injected SQLite insert failure after deletion rolls the entire refresh back,
   retaining the old generation and complete old search results.
6. A process restart retains index/generation after source deletion; live read
   fails and explicit refresh removes the stale stored record.

The fixture creates only synthetic files in a private temporary inventory. Its
junction targets its own neighboring directory. All owned server processes
exit or are explicitly reaped, the junction is removed first, and exact files
and then empty directories are removed; no recursive deletion touches a user
folder. The SG source passes the current formatter and the dedicated OpenSpec
change passes strict validation.

Reproduce the complete external proof using explicit tool paths:

```text
python examples/realworld/agent-knowledge/tests/external.py --compiler path/to/sgc.exe --clang path/to/clang.exe --formatter path/to/sgfmt.exe --sgpm path/to/sgpm.exe --output target/knowledge-external/evidence.json
```

This verifies Windows source-development local path dependencies and no-tool
runtime behavior. It does not claim a public registry bundle, release artifact,
fresh installer VM, Linux/macOS, live MCP service or semantic RAG capability.
The integration owner may add this consumer to the offline registry profile.
The synchronous stdout/directory and Windows FileSnapshot boundaries in README
remain in effect.
