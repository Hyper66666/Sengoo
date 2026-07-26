# Sengoo v0.2.0

Sengoo v0.2.0 is the first stable native toolchain line built from the v0.2
mainstream program. It packages `sgc`, `sgpm`, `sgfmt`, `sglsp`, the standard
library, and the native runtime for Windows x64, Linux x64, macOS x64, and
macOS arm64.

## Highlights

- Automatic Drop, move checking, drop flags, aggregate cleanup, and dynamic
  trait dispatch on the supported native path.
- Generic traits and collections, owned strings, numeric widths/conversions,
  formatting, Unicode scalar iteration, and stream composition.
- Cooperative async, user futures, cancellation, structured task scopes,
  reactor-backed IO, generic channels, locks, and opt-in worker execution.
- Deterministic package graphs with aliases and multiple versions, locked
  realworld loops, structured diagnostics, native source debugging, and
  self-contained release archives.
- Production-shaped HTTP routing, bounded keep-alive and response streaming,
  plus verified TLS composition using Schannel on Windows and rustls on POSIX.

## Compatibility

- Source edition remains `2026`.
- Runtime ABI remains `1`; generic collections descriptor ABI remains `1`.
- `Sengoo.lock` readers accept schemas 1 and 2; new dependency graphs write 2.
- Stable v0.2 surfaces follow the deprecation and security/soundness exception
  rules in `docs/compatibility-policy.md`.
- Migration details are in `docs/migration-v0-1-to-v0-2.md`.

## Supported Hosts

| Host | Target |
| --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` |
| Linux x64 | `x86_64-unknown-linux-gnu` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple Silicon | `aarch64-apple-darwin` |

Each published archive includes a SHA-256 sidecar and GitHub build-provenance
attestation. The release workflow installs archives outside the checkout and
runs compatibility, package, TLS, debugger/reactor, upgrade, and rollback
smokes before publication.

## Release Evidence

- Candidate 1: `f5a09c4baa83f539c5d7e889c9fce3d23e2b4289`, run
  [`30184545506`](https://github.com/Hyper66666/Sengoo/actions/runs/30184545506).
- Candidate 2: `6f9475dd956e63c886c8868278bc233a7044806b`, run
  [`30188454330`](https://github.com/Hyper66666/Sengoo/actions/runs/30188454330).
- Candidate 2 passed six same-SHA main workflows, four package jobs, and four
  RC1-to-RC2-to-RC1 transition jobs. Its public archives and evidence manifest
  are retained at the [`v0.2.0-rc.2` release](https://github.com/Hyper66666/Sengoo/releases/tag/v0.2.0-rc.2).
- Stable: `92c8f399f61b73d63990581c637da68572b6e133`, run
  [`30191226253`](https://github.com/Hyper66666/Sengoo/actions/runs/30191226253),
  provenance attestation [`37141681`](https://github.com/Hyper66666/Sengoo/attestations/37141681).

| Target | SHA-256 |
| --- | --- |
| `aarch64-apple-darwin` | `e25b10df03f7aa49c5da6a31ebcbece1fd264fb031d162ec0496853ba2894842` |
| `x86_64-apple-darwin` | `f46140b369e36e06e8834cb9bd81bcaca003e9eb29bca3978ecf657036e99562` |
| `x86_64-pc-windows-msvc` | `11baa4fc0819e4062d3fa134e928a6c244a30c4bb3d0a68007232ed12e0d31c3` |
| `x86_64-unknown-linux-gnu` | `3c041ddb8430c6d3d014e3a4aa93430985a3b8c74f9edf780c6e78cb02d52170` |

The stable evidence manifest records the six successful main-push workflows,
four package jobs, four RC2-to-stable-to-RC2 transition jobs, frozen fixture
hashes, and no platform skip.

## Experimental or Deferred

- WASM remains an experimental scalar backend; bytecode is not production GO.
- Cranelift is an opt-in primitive fast-JIT, not the release-reference backend.
- HTTP/2, WebSocket-over-TLS, request-body streaming, async middleware, broad
  database/web frameworks, and a hosted public package registry are not v0.2
  Supported claims.

## Install

```powershell
.\scripts\install.ps1 -Version 0.2.0
```

```sh
sh scripts/install.sh --version 0.2.0
```
