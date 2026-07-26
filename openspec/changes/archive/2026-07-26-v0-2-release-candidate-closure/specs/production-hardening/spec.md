## MODIFIED Requirements

### Requirement: Release readiness SHALL be proven with installed artifacts

Realworld and official-package release gates SHALL use installed release
artifacts outside the source checkout on Windows x64, Linux x64, macOS x64,
and macOS arm64. All required host jobs SHALL identify the same candidate
version and full commit SHA. Platform-specific capability skips SHALL match the
support matrix and SHALL NOT be counted as successful coverage.

#### Scenario: A release candidate is evaluated

- **WHEN** a release candidate reaches the production gate
- **THEN** each supported host checksum-verifies and installs its native archive
  outside the source checkout
- **AND** `sgc`, `sgpm`, `sgfmt`, and `sglsp` report one candidate version and
  revision
- **AND** locked package check/test/fmt/doc/build/run succeeds without workspace
  path leakage or silent manifest/lockfile rewriting
- **AND** host-specific TLS, debugger, reactor, safety, fuzz, and performance
  roles run as pinned by the release design
- **AND** required jobs that are skipped, mixed-SHA, or missing retained
  evidence fail the candidate gate

### Requirement: v0.2.0 SHALL require two consecutive release-candidate gates

Two release-shaped candidate commits SHALL each pass installed-artifact,
compatibility, safety, performance, realworld, and strict OpenSpec matrices on
every supported host before v0.2.0 is published. Candidate 2 SHALL prove upgrade
and rollback against retained candidate 1 artifacts and SHALL run candidate 1
Stable-surface fixtures without source or lockfile rewriting.

#### Scenario: Stable behavior changes after one passing candidate

- **WHEN** a P0/P1 fix changes a Stable source, stdlib, CLI, manifest, lockfile,
  diagnostic/protocol, or runtime ABI contract after candidate 1 passes
- **THEN** the consecutive-candidate count resets
- **AND** the changed candidate becomes the new candidate 1 baseline
- **AND** the reset reason and behavior-changing commit are retained

#### Scenario: Candidate 2 preserves candidate 1 contracts

- **WHEN** candidate 2 is proposed as the second consecutive candidate
- **THEN** it passes the complete matrix on its own immutable SHA
- **AND** an installed candidate 1 toolchain upgrades to candidate 2 through the
  documented path
- **AND** retained candidate 1 packages and lockfiles pass unchanged
- **AND** rollback to candidate 1 remains checksum-verified and non-destructive
