# Official package catalog

The first CLI/data package set is maintained under `packages/` and versioned
independently of the standard library. These development packages require
**Sengoo 0.2.1-dev.0 or newer with the matching new features**. Published v0.2.0
remains the compatibility baseline. Planned native hosts are Windows x64,
Linux x64, macOS x64 and macOS arm64; actual current-candidate proof is recorded
in [development-evidence.md](development-evidence.md).

| Package | Version | Purpose | Maintenance | Executable example |
| --- | --- | --- | --- | --- |
| `sgcli` | 0.1.0 | Options, flags, positional values, one level of subcommands, help/version | Official, development | `file-csv-index` |
| `sgdata` | 0.1.0 | Bounded UTF-8 I/O, typed JSON fields, CSV reading/writing | Official, development | `file-csv-index` |
| `sgsqlite` | 0.1.0 | SQLite 3.54.1, statements, binding, owned rows, rollback transactions | Official, development | `file-csv-index` |

`packages/CATALOG.toml` is the machine-readable catalog. Package README files
define their exact supported API. This set does not add GUI, a hosted registry,
an ORM, a connection pool, asynchronous SQLite, shell completion, YAML or a
general serialization derive mechanism.

The Agent development preview adds four experimental `0.1.0` packages:
`sgllm` (shared native model protocols/HTTP/SSE), `sgmcp` (stdio client/server),
`sgschema` (explicit parsed tool-schema subset), and `sgagent` (durable action
kernel and host policies). Its five-package registry also carries the existing
`sgsqlite`, required by the Agent ledger. Their platform evidence is
currently Windows x64; they do not inherit the foundation set's planned host
matrix. See [Agent SDK quickstart](agent-sdk-quickstart.md) and the
[implementation ledger](agent-ecosystem-progress.md) for exact APIs and gates.
Use the explicit `--profile agents` on both ecosystem build and verify commands
to produce the independent preview artifact set. The default foundation profile
and its three-package bundle remain compatible.

The server protocol core and MCP client are pollable. The stdio adapter uses
the generic bounded `StdWriter` for Windows local byte pipes and keeps polling
input and cancellation under output backpressure. Other output endpoints and
platform backends remain explicitly unsupported by this adapter. The CLI consumes
shared model/MCP implementations; policy and standalone Agent gates are recorded
individually.

Build the offline artifacts from a verified candidate:

```text
python scripts/package_ecosystem.py build --sgpm target/release/sgpm --output target/ecosystem-dist
python scripts/package_ecosystem.py verify target/ecosystem-dist
```

On Windows, select `target/release/sgpm.exe`. The build produces three
deterministic source archives, `ecosystem-manifest.json`, `SHA256SUMS` and
`sengoo-official-registry-0.1.0.tar.gz`. The latter extracts to a `registry/`
tree directly usable by `sgpm`. Build and extraction need no registry server.
Verify `SHA256SUMS` against trusted release bytes before extraction. A checksum
proves byte identity, not the identity of an untrusted publisher.

Place `registry/` next to the consumer's manifest, then configure relative
paths and exact versions:

```toml
[registries.official]
path = "registry"

[dependencies]
sgcli = { version = "=0.1.0", registry = "official" }
sgdata = { version = "=0.1.0", registry = "official" }
sgsqlite = { version = "=0.1.0", registry = "official" }
```

Run `sgpm update` once, commit the resulting lockfile and use `--locked`
afterwards. The file/CSV index application and Python batch module live in
`examples/realworld/file-csv-index` and `examples/realworld/python-batch`.
Their acceptance runs copy the projects outside the compiler checkout.

The integrated native acceptance entry point is:

```text
python scripts/verify_daily_development.py --bin-dir <installed-bin> --distribution target/ecosystem-dist --report target/daily-development-evidence.json
```

This exercises doctor, bin/lib new/init templates, library consumption,
retained compatibility projects, all three packages and the index app outside
the checkout. Python wheel installation and two-module isolation are exercised
by the Python workflow. Publishing artifacts and inviting pilot users are
separate delivery steps after current-candidate evidence is reviewed.
