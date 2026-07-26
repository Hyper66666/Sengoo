# Sengoo compatibility and release support policy

This policy describes the supported pre-1.0 toolchain line. It is a contract
for release decisions, not a claim that every operating-system or dependency
version works.

**v0.2 Stable-surface and fixture evidence SHA:**
`6f9475dd956e63c886c8868278bc233a7044806b` ([PR #55](https://github.com/Hyper66666/Sengoo/pull/55)).
Surface classes, edition `2026` rejection, migration notes, public-input panic
policy, and retained fixtures (`examples/compat/v0.1.0-rc.1`,
`examples/compat/v0.2.0-rc.1`) are documented against that release candidate.
Two consecutive four-host candidates passed; `v0.2.0-rc.2` release run
[`30188454330`](https://github.com/Hyper66666/Sengoo/actions/runs/30188454330)
retains the RC1 upgrade, compatibility, and checksum-verified rollback proof.

## Surface stability classes

| Class | Meaning |
| --- | --- |
| **Stable** | Documented contract; changes require deprecation window or security/soundness exception |
| **Supported subset** | Worked proof exists; remaining edges may be deferred without removing the proven subset |
| **Experimental** | Available for evaluation; may break without deprecation |
| **Deprecated** | Still functional for the named window; warning names replacement and earliest removal |

Authoritative per-surface classes live in `docs/language-reference.md` and
`examples/realworld/SUPPORT_MATRIX.md`. Experimental surfaces (for example
scalar WASM) do not inherit Stable guarantees.

## Source and edition policy

Sengoo currently has one source edition, spelled `edition = "2026"` in
`Sengoo.toml`. A missing edition retains the current compatibility behavior;
any explicit value other than `2026` is rejected with an
`unsupported Sengoo edition` diagnostic before package compilation.

Before 1.0, source compatibility is promised within one published patch or
release-candidate line unless a security or soundness defect requires an
exception. A 0.x minor release may make documented source changes. Such a
change must include release notes, migration guidance, and retained
compatibility fixtures for every behavior that remains supported.

v0.2.x patch lines keep Stable and Supported-subset source/tool/stdlib surfaces
compatible with the v0.2 release-candidate baseline unless a security or
soundness exception is published.

A future edition requires its own OpenSpec change. New editions must not
silently reinterpret a manifest that explicitly selects `2026`.

## Deprecation window

A supported public source or toolchain interface is deprecated for at least one minor release
before removal. The warning must name the replacement and the
earliest removal line. Patch and release-candidate updates do not remove a
deprecated interface.

Deprecation metadata (when emitted) includes: stable warning code, replacement
text, earliest removal version, and documented suppression policy for tools that
support it.

Stable source deprecations use structured metadata:

```sg
#[deprecated(replacement = "new_api", removal = "v0.3.0", note = "use the fallible API")]
```

The compiler retains the legacy `#[deprecated("message")]` form for source
compatibility, but that form does not satisfy the metadata requirement for a
new Stable-surface deprecation. `sgc` JSON emits `replacement` and `removal` as
top-level warning fields; sglsp carries the same values in Diagnostic `data`.

An immediate change is allowed only to correct a security or soundness defect,
prevent data loss, or reject behavior that was never in the documented support
surface. The release notes must identify the exception and its migration path.

Migration notes for v0.1 → v0.2 live in `docs/migration-v0-1-to-v0-2.md`.

## Runtime and data schemas

Source compatibility and binary/data compatibility are versioned separately.
The current repository has these explicit formats:

| Surface | Current accepted version |
| --- | --- |
| Native runtime bundle ABI | `1` |
| Generic collections descriptor ABI | `1` |
| `Sengoo.toml` `sengoo-schema` | omitted legacy form or `1` |
| `Sengoo.lock` | `1` and `2`; new graphs write `2` |
| Test report and assertion JSON | `1` |
| Compiler diagnostic JSON | `1` |
| Package metadata JSON | `2` |
| Publish metadata JSON | `1` |
| Reflection metadata | `1` |

Readers reject unknown explicit versions before consuming version-dependent
fields. Before compiling or linking a selected native runtime bundle, `sgc`
compares its required whole-runtime ABI with
`SENGOO_RUNTIME_ABI_VERSION` from that bundle's shared header and reports both
versions on mismatch. The runtime also exports
`sengoo_runtime_abi_version()` for native probes.

## Supported release hosts

Release candidates are built and installed on native CI runners for:

| Host | Target |
| --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` |
| Linux x64 | `x86_64-unknown-linux-gnu` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple Silicon | `aarch64-apple-darwin` |

The supported host is the target plus the runner/toolchain generation recorded
in the release manifest and provenance. A target not listed here is
experimental until a native build, install, and outside-checkout smoke gate is
retained. Native compilation requires clang/LLVM 15 or newer; the core
conformance reference job pins LLVM 19.

## Release support window

Before 1.0, the project supports the latest stable 0.x line. The immediately
previous release candidate is retained as a compatibility and rollback input,
but normally receives no fixes after stable publication. Security or soundness
advisories state whether an older artifact must be withdrawn.

Every candidate must publish checksums and provenance, install into a clean
prefix, report one coherent tool version/hash, and pass the release smoke
matrix. Unsupported or unavailable host jobs are release blockers rather than
successful skips.

The retained project under `examples/compat/v0.1.0-rc.1` is run outside the
checkout by `.github/workflows/compatibility.yml` with both that published
toolchain and the current toolchain. Its transcript is retained on every run.

The v0.2 candidate fixture under `examples/compat/v0.2.0-rc.1` freezes the
v0.2 source surface (edition 2026, stream/Unicode baseline imports allowed only
when retained as additive). Two consecutive release-candidate matrices on the
same host set are required before tagging `v0.2.0`; a P0/P1 Stable-behavior fix
restarts the candidate sequence. Candidate runs `30184545506` and
`30188454330` satisfy that gate without a Stable-behavior change between them.

Release notes for this line live in `docs/release-notes-v0.2.0.md`.

## Public-input panic policy

Public toolchain and runtime entry points that accept untrusted source,
manifest, lockfile, archive, JSON protocol, handle, or portable artifact input
must fail with stable diagnostics or status codes. An unclassified panic on
such input is a release blocker. Internal assertions may remain, but raw panic
text is not a valid user diagnostic. FFI boundaries must not unwind across
language ABIs.
