# Agent knowledge server

A complete Sengoo MCP consumer using `sgmcp`, `sgschema` and `sgsqlite`.
The host selects one local directory and a persistent SQLite file. There is no
AI, embedding service, model-selected root or network connection.

Requires edition 2026 and the current Sengoo 0.2.1-dev source toolchain with
Windows FileSnapshot support. Windows local fixed disks are the validated
platform; FileSnapshot is explicitly unsupported elsewhere.

```text
sgpm update --manifest-path examples/realworld/agent-knowledge/Sengoo.toml
sgpm build --locked --manifest-path examples/realworld/agent-knowledge/Sengoo.toml
agent-knowledge.exe D:\documents D:\indexes\documents.sqlite
```

The protocol is newline-delimited MCP JSON-RPC on stdin/stdout. Logs go to
stderr. Configure the executable and the two arguments in an MCP client. The
root is trusted host configuration, never a tool argument. The database parent
must already exist. One server owns an index database at a time through an OS
lock; the database is bound to its canonical selected root and schema version.

| Tool | Arguments | Result |
| --- | --- | --- |
| `knowledge_status` | `{}` | Stored generation, indexed file count and byte count |
| `knowledge_reindex` | `{}` | Atomic full refresh; indexed/skipped/visited/bytes counters |
| `knowledge_search` | `{"query":"text"}` | At most 20 stored substring matches with path, preview, bytes and digest |
| `knowledge_read` | `{"path":"subdir/file.md"}` | Current owned UTF-8 text and SHA-256 |

Startup preserves the last index and does **not** silently rescan. Call
`knowledge_reindex` to create or refresh it. Search uses stored text, so a deleted
source can still match until refresh; read always validates the current file.
A failed database write or aggregate scan limit rolls the refresh back and
retains the prior complete index and generation.

Supported indexed extensions: `.txt`, `.md`, `.rst`, `.sg`, `.json`, `.csv`,
`.log`. Invalid UTF-8, NUL-bearing text, unreadable files, unsupported extensions,
reparse points and files above 64 KiB are skipped and counted. A read request
reports a tool error for these invalid inputs instead of truncating content.
Queries are literal substrings; SQLite's `lower()` folds ASCII, and Unicode
matches preserve case. No semantic retrieval or Unicode casefold is claimed.

The scan has explicit limits: depth 8, 1024 entries per directory, 4096 visited
entries, 512 indexed files and 16 MiB total indexed UTF-8 bytes. Exceeding a scan
limit returns a tool error and retains the prior index. Directory enumeration
is synchronous and its native API collects one directory before the count
check; no constant-memory or interactive-cancellation guarantee is claimed.
The MCP stdout adapter is synchronous: a consumer that stops reading stdout
can block it. The server core remains separate from this transport boundary.

Only relative forward-slash paths are accepted by `knowledge_read`. Empty,
absolute, drive/URI, `.`/`..`, backslash and NUL paths fail. FileSnapshot pins
every Windows ancestor and rejects reparse points before copying bytes; the
returned final canonical file path must share the selected root plus a real
separator boundary. ASCII case folding handles Windows drive/path casing;
non-ASCII casing is compared conservatively and may reject an equivalent alias.
It cannot grant access outside the root. This is scoped local file access,
not a promise that the server is a general OS sandbox.

Reproduce the real independent protocol fixture:

```text
python examples/realworld/agent-knowledge/tests/verify.py path/to/agent-knowledge.exe --report target/knowledge-protocol.json
```

It creates its own Unicode files, neighbor directory and junction, drives MCP
from Python, verifies schema rejection, traversal/reparse refusal, oversize and
encoding errors, SQL rollback and persistence through process restart, then
removes its exact temporary inventory. It reads no user files or credentials.
The integration owner separately records external locked/offline-registry
consumption, source removal and no-development-tool runtime evidence.
