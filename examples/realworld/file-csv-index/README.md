# Persistent file / CSV index

Requires toolchain **0.2.1-dev.0** or later containing the ecosystem additions,
edition 2026, clang, and the matching native runtime. Dependencies are pinned
to official package `0.1.0`; SQLite 3.54.1 is built from the package's vendored
source, without a system SQLite installation or network access during builds.

Create the offline official registry bundle from the repository root:

```console
python scripts/package_ecosystem.py build --sgpm target/release/sgpm --output dist/official
```

On Windows, append `.exe` to the tool path. Copy this project outside the
toolchain checkout, extract the bundle, and put its `registry/` beside
`Sengoo.toml`. The manifest uses a relative
registry path and exact versions. Do not point it at a checkout-specific path.

```console
sgpm update
sgpm check --locked
sgpm fmt --check
sgpm build --locked
sgpm run --locked -- --db index.db index --csv sample.csv
sgpm run --locked -- --db index.db index --csv sample.csv
sgpm run --locked -- --db index.db query --name Alice
sgpm run --locked -- --db index.db query --name 世界
sgpm run --locked -- --db index.db index --csv sample.csv --max-bytes 1048576
```

The database persists between separate processes. The first import prints
`Indexed rows:` and the number of data records; repeating the same path with
identical bytes prints `Unchanged rows:` and the previous count, without deleting
or inserting rows or writing source metadata. Changing any input byte causes a
validated replacement of that source's rows and metadata in one transaction.
Rows belonging to other sources remain untouched. Header-only input removes
that source's data records. A missing, malformed, oversized or unreadable file
keeps the last successful import queryable; a database write failure also rolls
back the DELETE, any partial INSERTs, and metadata together.

Source identity is the exact `--csv` argument, relative to the working directory
for file access. Use a consistent working directory and path spelling: `a.csv`,
`./a.csv` and an absolute path are separate sources. Files are read once per
command; bytes successfully read are the imported snapshot. Reimport after a
concurrent external edit to get its later contents. There is no background
watcher, path canonicalization or mtime-only shortcut. Exact input text is stored
in the `index_sources` table to avoid hash collisions and timestamp ambiguity;
it increases database size by approximately the imported input size. Editing the
database directly is outside the synchronization contract.

Existing databases containing only the original `entries` table are preserved.
Opening one adds `index_sources`; querying old rows works immediately. The first
reimport replaces the selected source and seeds its metadata; subsequent exact
reimports can skip writes. Keep the database together with its SQLite sidecars
when moving it while another process is using it.

Input must be UTF-8 without a byte-order mark, with the exact two-column header
`name,note` and consistent columns. Quoted newlines, escaped quotes, LF and CRLF
are accepted. The default and maximum input cap is **16,777,216 bytes** (16 MiB),
enforced during reading, including a file that grows. `index --max-bytes N` can
lower the cap from 1 to 16,777,216; invalid limits are usage errors. The cap is an
input-byte limit, not a total-memory budget: input, parsed copied fields and the
SQLite snapshot are held in memory. This application deliberately uses bounded
whole-file parsing, so split larger data into separate source files rather than
raising the limit. UTF-8 sizes count bytes, not characters.

Queries bind parameters, including names containing SQL punctuation. Output
lists each matching source, one-based data-record number, name and note, followed
by the total. Quoted CSV multiline fields remain multiline in the output.

`--help` and `--version` return 0. Invalid CLI arguments return 2; input, database,
and I/O errors return 1. Applications own printing and exit decisions. Connections,
prepared statements, transactions, and CSV tables clean up automatically; early
return rolls an uncommitted import back. Input failures report the source path
and reason on stderr; malformed CSV includes the logical record/field and byte
offset, or the expected/actual column count. A logical record may contain quoted
newlines. Database failures include its path, SQLite message/code and the
existing Sengoo status. The original status/count stdout output remains available.

Repeat the automated real-consumer acceptance from the toolchain checkout:

```console
python scripts/verify-index-incremental.py --bin-dir target/release --distribution dist/official --runtime-mode source-development --report target/index-acceptance.json
```

For an installed toolchain, use its `bin/` directory and
`--runtime-mode installed`. The script verifies the archive checksums, copies this
project outside the checkout, consumes all three pinned registry packages through
the locked check/format/test/doc/build loop, then deletes consumer sources and
runs only the delivered executable with development tools removed from PATH.
It checks exact unchanged imports with zero database-byte changes and SQL audit
triggers; changed imports and cross-process persistence; Unicode, quoted records
and SQL parameters; invalid input and byte limits; a real SQLite trigger failure
after a partial insert; recovery; and migration from the original schema. It
returns nonzero and writes a JSON failure report if any assertion fails.

For final artifact validation, copy the built executable into a clean directory
with `sample.csv`, remove the source tree and build tools from PATH, then run an
index process and a separate query process. This native application requires no
Sengoo compiler at runtime. Four-platform release validation and independent
user trials are separate release gates.
