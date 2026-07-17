# Editor setup (internal)

Guide for day-to-day editing with `sglsp`, formatting, and CLI diagnostic parity.

## Language server (`sglsp`)

1. Build the workspace: `cargo build --release`.
2. Point the VS Code Sengoo extension (see `vscode-sengoo/`) at `target/release/sglsp` if it is not already on `PATH`.
3. Open a package root containing `Sengoo.toml` so import resolution matches `sgpm`.

`sglsp` exposes:

- context-aware completion for general, receiver member (`.`), namespace
  (`::`), all four compiler-accepted import forms, and evidence-backed
  attributes after `#` / `#[`
- deterministic local/parameter/field/imported/project/stdlib/keyword ordering,
  UTF-16 replacement edits, and keyword snippets
- lazy Markdown completion documentation and revision-safe unique-origin
  auto-import; ambiguous origins remain separate choices and never guess an
  import
- nested-call and receiver-aware signature help, including overloads,
  documentation, Unicode source, and conservative unresolved-call handling
- revision-bound safe Code Actions for unique unresolved-symbol imports, exact
  unused-import removal, and complete missing-enum match arms; stale,
  ambiguous, wildcard, incomplete, or diagnostic-free cases intentionally
  produce no edit
- package-aware paths such as `sggame::snake_logic`, including workspace,
  dependency, and standard-library selective-export completion
- qualified receiver completion with conservative ambiguity handling and
  field/method/function-return chain inference
- completion and hover for `std::` imports
- go-to-definition for same-workspace modules
- diagnostics aligned with compiler import and type errors

The server builds a workspace/dependency index during initialization. Open
documents are versioned overlays, so normal completion and navigation do not
recursively reread the source tree. If completion appears stale, save or close
and reopen the affected document; watched-file notifications refresh only that
file. See `docs/lsp-compatibility.md` for the experimental completion metadata
contract.

Attribute completion is deliberately conservative. The catalog is limited to
capabilities with executable compiler or `sgc test` evidence: built-in derives,
`cfg`, `deprecated`, `test`, `case`, `export_name`, and extern-block `link`.
Targets are filtered before display; unsupported or externally configured
derive names are not advertised as built-ins.

### Troubleshooting completion and edits

- Confirm the editor opened the package root containing `Sengoo.toml`; module
  identity and dependency aliases are resolved from that manifest and lockfile.
- Inspect the initialize response for
  `experimental.sengoo.completionSchemaVersion = 1`. Older clients still get
  ordinary LSP completion, but may apply their legacy client-side filtering
  and cannot rely on schema-v1 resolve metadata.
- If a completion item no longer resolves or an offered Code Action disappears,
  request completion/diagnostics again. URI, integer document revision, content
  hash, range, diagnostic, and symbol facts are deliberately revalidated before
  edit-producing responses.
- `#` and `#[` candidates come only from `sglsp`; duplicate attribute candidates
  indicate an outdated client extension or another snippet provider.
- For a stale index after an external file change, save the file or close and
  reopen it and inspect watched-file notifications in the LSP trace. Open
  overlays always win over disk refreshes.
- Enable the editor's LSP trace and compare the request position and returned
  `textEdit` as UTF-16 coordinates. Unicode before the cursor is a common cause
  of apparent range errors in clients that count bytes or scalar values.

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
