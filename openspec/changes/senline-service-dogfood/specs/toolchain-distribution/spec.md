## ADDED Requirements

### Requirement: Installed distributions SHALL include target-native runtime artifacts

Windows x64 and Linux x64 installed toolchain archives SHALL include the native runtime library and every declared runtime dependency required to compile, link, and run supported native Sengoo packages without a Sengoo source checkout.

#### Scenario: An installed native package builds outside the checkout

- **WHEN** a release-shaped archive is installed in a fresh path and `sgc` builds a supported native package from a different path
- **THEN** `sgc` resolves the target-native runtime and standard-library dependencies from the installed distribution
- **AND** no source checkout, Cargo target directory, `SENGOO_ROOT`, or local absolute repository path is required

#### Scenario: Required runtime content is missing or incompatible

- **WHEN** the installed runtime library is absent, has the wrong target or ABI, fails its hash, or lacks a declared dependency
- **THEN** native build fails with a stable diagnostic naming the incompatible distribution component
- **AND** `sgc` does not silently build or select a different runtime

### Requirement: Native runtime manifests SHALL be complete and verifiable

Each target distribution manifest SHALL record the runtime ABI version, target triple, source revision, tool versions, ordered link arguments, dynamic dependency identities, build-manifest identifier, and SHA-256 hash for every shipped payload file needed by native compilation and execution.

#### Scenario: A consumer verifies an installed bundle

- **WHEN** a consumer validates the distribution against its reviewed manifest
- **THEN** every executable, native runtime library, standard-library/runtime bridge file, and declared dynamic dependency has an expected identity and hash
- **AND** the consumer can reject a partial, mixed-target, or tampered installation before execution

#### Scenario: Absolute development paths enter metadata

- **WHEN** packaging inspects manifests, link arguments, and generated package metadata
- **THEN** mutable checkout, user-profile, Cargo target, and build-runner absolute paths are absent from consumer-facing resolution data

### Requirement: Installed runtime discovery SHALL not hide a Cargo fallback

Normal installed `sgc` native build/run/test SHALL prefer and require the installed manifest-selected runtime. Building the runtime through Cargo SHALL be available only through an explicit Sengoo-source development mode and SHALL never be an implicit recovery path for an installed distribution.

#### Scenario: Cargo is unavailable during an installed smoke

- **WHEN** an installed toolchain runs native worker and HTTP package smokes with a deliberately failing fake `cargo` first on PATH
- **THEN** check, test, and release build succeed using only installed runtime artifacts
- **AND** the fake Cargo executable is never invoked

#### Scenario: A developer explicitly selects source-runtime mode

- **WHEN** a contributor working inside a Sengoo source checkout opts into the documented source-runtime development mode
- **THEN** Cargo runtime construction may run with a diagnostic identifying the non-release mode
- **AND** artifacts from that mode are not accepted as installed-distribution or Senline pin evidence

### Requirement: Installed application smokes SHALL cover real consumer packages

Distribution dry-runs and release gates SHALL install the archive outside the source checkout and run the locked `senline-domain-worker` stdio/strict-JSON package loop plus the `senline-http-dogfood` native localhost smoke on Windows x64 and Linux x64.

#### Scenario: A consumer package fails on one target

- **WHEN** either installed package cannot check, test, format-check, document, release-build, or perform its required real execution smoke on a target
- **THEN** that target's distribution is ineligible for Senline pinning and publication evidence
- **AND** the failing target, command, artifact manifest, and diagnostic are retained

#### Scenario: Independent builds reproduce payload identity

- **WHEN** the same clean revision is built twice independently for one target
- **THEN** normalized manifests require identical payload hashes, runtime ABI, link arguments, and dynamic dependency identities
- **AND** only documented timestamps, runner metadata, and provenance-signature differences may be excluded from the comparison
