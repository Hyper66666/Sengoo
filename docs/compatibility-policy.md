# Sengoo compatibility and release support policy

This policy describes the supported pre-1.0 toolchain line. It is a contract
for release decisions, not a claim that every operating-system or dependency
version works.

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

A future edition requires its own OpenSpec change. New editions must not
silently reinterpret a manifest that explicitly selects `2026`.

## Deprecation window

A supported public source or toolchain interface is deprecated for at least one minor release
before removal. The warning must name the replacement and the
earliest removal line. Patch and release-candidate updates do not remove a
deprecated interface.

An immediate change is allowed only to correct a security or soundness defect,
prevent data loss, or reject behavior that was never in the documented support
surface. The release notes must identify the exception and its migration path.

## Runtime and data schemas

Source compatibility and binary/data compatibility are versioned separately.
The current repository has these explicit formats:

| Surface | Current accepted version |
| --- | --- |
| Generic collections descriptor ABI | `1` |
| `Sengoo.toml` `sengoo-schema` | omitted legacy form or `1` |
| `Sengoo.lock` | `1` and `2`; new graphs write `2` |
| Test report and assertion JSON | `1` |
| Package metadata JSON | `2` |
| Publish metadata JSON | `1` |
| Reflection metadata | `1` |

Readers reject unknown explicit versions before consuming version-dependent
fields. The production-hardening runtime ABI task remains open until native
artifacts also carry and validate one whole-runtime ABI version at link or
launch time.

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

Before 1.0, the project supports the latest prerelease line. The immediately
previous prerelease is retained as a compatibility-test input, but normally
receives no fixes after its successor is published. Security or soundness
advisories state whether an older artifact must be withdrawn.

Every candidate must publish checksums and provenance, install into a clean
prefix, report one coherent tool version/hash, and pass the release smoke
matrix. Unsupported or unavailable host jobs are release blockers rather than
successful skips.
