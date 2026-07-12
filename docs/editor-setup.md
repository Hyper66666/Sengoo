# Editor setup (internal)

Guide for day-to-day editing with `sglsp`, formatting, and CLI diagnostic parity.

## Language server (`sglsp`)

1. Build the workspace: `cargo build --release`.
2. Point the VS Code Sengoo extension (see `vscode-sengoo/`) at `target/release/sglsp` if it is not already on `PATH`.
3. Open a package root containing `Sengoo.toml` so import resolution matches `sgpm`.

`sglsp` exposes:

- completion and hover for `std::` imports
- go-to-definition for same-workspace modules
- diagnostics aligned with compiler import and type errors

## Format on save

Enable the Sengoo formatter (`sgfmt`) in the extension or run manually:

```powershell
sgfmt --write src/main.sg
sgpm fmt --manifest-path Sengoo.toml
```

Locked CI and local verification use `sgpm fmt --check --locked`.

## JSON diagnostics parity with `sgc`

CLI JSON diagnostics use:

```powershell
sgc --error-format json check src/main.sg
```

`sglsp` maps the same diagnostic payloads to LSP ranges (see `tools/sglsp/src/diagnostics.rs`). When investigating mismatches, compare:

1. `sgc --error-format json check …` stderr
2. the LSP `textDocument/publishDiagnostics` payload in the editor log

## Realworld fixtures

The `examples/realworld/*` packages are the canonical smoke targets for IDE + CLI parity. Start with `cli-json-audit` when validating JSON-related tooling.

## Debug from VS Code

The `vscode-sengoo` extension contributes a `type: "sengoo"` debug entry. Press
F5 on a `.sg` file, or add a launch configuration with:

```json
{
  "type": "sengoo",
  "request": "launch",
  "name": "Debug Sengoo file",
  "program": "${file}",
  "mode": "run",
  "cwd": "${workspaceFolder}"
}
```

That path is a lightweight `sgc run` / build-and-run DAP wrapper. For native
source breakpoints, stepping, and local-variable inspection, use the
`cppvsdbg`/`lldb` launch configurations in `docs/debugging-native.md`; those
build with `sgc build -O 0 --debug-info` before attaching the native debugger.
