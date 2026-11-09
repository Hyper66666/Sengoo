# Knowledge consumer design

`examples/realworld/agent-knowledge` owns its manifest, relative lock, pure
Sengoo binary, independent client fixture and documentation. It depends on
sgmcp (which depends on sgschema) and sgsqlite. It does not import Sencode.

The two argv values are trusted root and database paths. The root is canonical,
the database stores exact schema version and its bound root, and a canonical
OS file lock prevents simultaneous refresh writers. MCP cannot override root.
All exposed tools are registered through the public server/schema API.

Reindex is an explicit synchronous full refresh inside one SQLite transaction.
It enumerates bounded directory levels without following reparse entries,
reads allowed extensions through FileSnapshot, validates strict UTF-8 and NUL,
and stores relative path, copied content, exact byte count and SHA-256. Bad
individual files are counted as skipped. SQL failures and aggregate limits
abort the transaction, preserving the prior complete index and generation.
Startup uses the stored index unchanged, so restart does not erase knowledge.

Read rejects absolute/drive/URI/backslash/NUL paths and dot components.
FileSnapshot pins every Windows ancestor without reparse traversal. Its owned
immutable bytes are decoded only after its final canonical path matches the
root plus separator, using conservative ASCII folding on Windows paths.
Non-ASCII casing is not approximated as Unicode casefold; a false denial is
preferable to accepting a different boundary. Search returns at most 20 rows,
with 200-character previews; a read returns at most 64 KiB. Limits reject or
count skips explicitly and never truncate file contents.

The first version has no embedding, model, remote fetch or AI-generated SQL.
Search is literal SQLite substring matching with ASCII case folding. The
server uses the package's synchronous stdout glue and synchronous directory
counts; these are documented boundaries, not claimed pollable cancellation.
FileSnapshot's Windows-local support determines the supported host platform.

The independent Python client creates only synthetic data, including a sibling
root and a directory junction pointing outside the chosen root. It performs
MCP negotiation/tool listing/schema failure/index/search/read, malformed file
and path probes, transaction failure injection, process restart, stale-index
observation and explicit refresh. External locked build and clean-runtime
qualification are separate integration evidence, not inferred from a native
compiler check.
