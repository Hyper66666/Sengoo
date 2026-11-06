# Recheck packages after native foundation changes

Changes to `sgcli` or `sgsqlite` must travel through the official archives and
offline registry before a consumer lockfile is refreshed. Testing a path import
alone does not establish that the delivered package contains the change.

With Python 3.11+, built debug tools and clang available, run from the repository:

```powershell
python scripts/verify-official-foundations.py --bin target/debug `
    --output target/ecosystem-foundations
```

Use the path to a real Python executable if `python --version` is a Windows Store
alias rather than an installed interpreter. The script builds deterministic
official archives through `sgpm publish --dry-run`, verifies `SHA256SUMS`, checks
that packaged library/native implementations match current source, then tests
all three unpacked packages with fresh locks. A separate temporary consumer
outside the toolchain directory installs the local registry, refreshes its lock,
builds and reopens a persistent CSV index. Every subsequent operation uses
`--locked` and must preserve the lock bytes.

The output includes `foundations-evidence.json` and the refreshed
`consumer.Sengoo.lock`, alongside the package artifacts. These two ancillary
files are local evidence; the official five-artifact checksum manifest retains
its existing format. The working-tree flag remains true until a committed
candidate is rerun. No publishing, registry server or network download occurs.

For a separate source-development consumer using path dependencies:

```powershell
D:/Sengoo/target/debug/sgpm.exe --runtime-mode source-development update `
    --manifest-path C:/projects/example/Sengoo.toml
D:/Sengoo/target/debug/sgpm.exe --runtime-mode source-development build `
    --manifest-path C:/projects/example/Sengoo.toml --locked
```

Review the updated `Sengoo.lock` and commit it with the consumer. Replace an
offline registry with the newly verified bundle before updating a registry
consumer. Do not hand-edit source checksums to hide an implementation change.

The toolchain packager copies every `runtime*.c` into its runtime and stdlib
directories, including `runtime_system.c` and `runtime_terminal.c`. Installed
runtime validation and distribution fixtures list both explicitly. Run:

```powershell
cargo test --locked -p sgc --test args_unicode
cargo test --locked -p sgc --test runtime_distribution `
    installed_check_rejects_incomplete_runtime_bridge
```

On Windows x64, the 2026-11-04 dirty-worktree run passed all 18 package/consumer
checks, Unicode OS argument/directory/environment integration (including managed
Buffer initialized lengths) and the incomplete-runtime-bridge check.
The base was `d46b3f6cd8d6e770734f98edc57aa67cb931e26a`. This source-development
evidence does not qualify another host, a final committed SHA, an independent
user trial or a clean-machine installation.
