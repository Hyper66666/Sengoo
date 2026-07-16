## Why

Sengoo ships `sgc`, `sgpm`, `sgfmt`, `sglsp`, a VS Code extension, structured
diagnostics, tests, docs, and native debugger setup. The pieces work, but v0.2
needs one compatibility-tested developer loop rather than independent tools.
In-progress smart-completion work already supplies a workspace index, completion
schema, context ranking, safe edits, and protocol E2E evidence; M2 must integrate
that work without redefining compiler syntax or duplicating `native-debug-info`.

## What Changes

- Integrate or supersede `enhance-sglsp-smart-completion` with all protocol,
  performance, and cross-repository evidence retained.
- Require completion, navigation, rename, signature help, diagnostics, safe code
  actions, and formatting to share one indexed workspace/document revision.
- Make `sgfmt` idempotence and parser compatibility release gates.
- Consume the `native-debug-info` archive and expose its documented CDB/LLDB
  launch path from the editor without inventing a new debugger protocol.
- Prove edit -> check -> format -> test -> debug -> doc on an installed package.

## Capabilities

### Modified Capabilities

- `tooling-mainstream-ecosystem`: add a versioned, performance-bounded,
  installed-toolchain developer-loop contract.
- `debug-and-test-tooling`: require editor launch integration with the retained
  native debug owner and test-result/diagnostic navigation.

## Impact

- `tools/sglsp`, `tools/sgfmt`, `tools/sgc`, `tools/sgpm`, `vscode-sengoo`,
  editor/debug docs, protocol fixtures, and installed release tests.
- `native-debug-info` remains the implementation owner for debug metadata.
- Compiler/parser and `sgfmt` fixtures remain syntax authorities.

## Non-Goals

- AI completion, postfix completion, broad refactoring, inlay hints, or call/type
  hierarchy.
- A custom DAP implementation.
- An editor-only parser or type checker.
- Making a separate editor repository the sole source of verification truth.
