# Official package acceptance

Each package is versioned independently at 0.1.0 and requires toolchain
0.2.1-dev.0. Package support must be demonstrated for the release candidate on
Windows x64, Linux x64, macOS x64, and macOS arm64. A local Windows pass is not
four-platform release evidence.

There are two complementary test layers:

```console
python packages/test_native_packages.py --clang clang --sanitize --output target/native-packages.json
sgpm test --manifest-path packages/sgcli/Sengoo.toml
sgpm test --manifest-path packages/sgdata/Sengoo.toml
sgpm test --manifest-path packages/sgsqlite/Sengoo.toml
```

Source developers add `--runtime-mode source-development` to sgpm commands.
Installed-toolchain validation uses the default installed mode. Do not mix
an old installed runtime with a new source standard library.

The native harnesses exercise the actual package C implementation; only String
and Buffer allocation are supplied by a small test host. On Windows the runner
uses AddressSanitizer, on Unix AddressSanitizer and UndefinedBehaviorSanitizer.
Every compiler/test failure produces a failing exit code. Windows LLVM sanitizer
interceptor warnings, if any, remain in the JSON evidence and must be considered
when assessing coverage. LeakSanitizer is not claimed on Windows.

The `.sg` tests then exercise the real compiler, native runtime, owned return
values, and automatic Drop. Each restores the String runtime live-handle count
and all of its package handle counts to the starting state. SQLite additionally
checks a statement outliving its connection and rollback after a `?` error.
sgdata verifies exact JSON integers above 2^53 and at both i64 boundaries.

Build and verify outside-checkout consumers from the checksum-verified local
registry bundle using `scripts/package_ecosystem.py` and
`scripts/verify_daily_development.py`. The latter runs update, locked check,
format, tests, docs, and build on extracted package sources; consumes the three
packages together from an independent manifest; indexes CSV in one process;
queries persisted data in another; and reruns the delivered executable without
development tools on PATH. See `examples/realworld/file-csv-index/README.md`.

The focused incremental-application acceptance also verifies unchanged imports
with SQL audit triggers and exact database-byte comparison, atomic replacement,
old-schema migration, UTF-8 and byte limits, damaged-input recovery, and rollback
after a deliberately injected SQLite write error following a partial insertion:

```console
python scripts/verify-index-incremental.py --bin-dir target/release --distribution target/ecosystem-dist --runtime-mode source-development --report target/index-incremental.json
```

Use `--runtime-mode installed` with the candidate installation's `bin/` directory
for release-host proof. All behavior assertions run after deleting the consumer
source/registry and removing development tools from PATH. The Python acceptance
driver's sqlite3 module acts as an independent database oracle; the delivered
Sengoo executable continues to use its vendored SQLite 3.54.1.

These checks do not replace independent user trials or authorize a public release.
