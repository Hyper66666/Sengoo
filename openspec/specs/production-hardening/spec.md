# production-hardening Specification

## Purpose
Define the retained safety, compatibility, performance, and installed-release
evidence required before Sengoo can claim its native toolchain is production
ready on a supported host.
## Requirements
### Requirement: Public input boundaries SHALL be fuzzed and regression-retained

The compiler, package manager, and runtime parsing boundaries SHALL have bounded
fuzz targets, and fixed crashes SHALL produce retained regression evidence.

#### Scenario: A fuzzer finds a compiler crash

- **WHEN** malformed source causes a panic, unbounded allocation, or invalid MIR
- **THEN** the issue is fixed
- **AND** a minimized corpus entry or deterministic regression test is retained
- **AND** the bounded per-commit fuzz gate exercises that input

### Requirement: Native and FFI boundaries SHALL pass safety and leak gates

Supported native hosts SHALL run sanitizer or equivalent memory-safety tests,
resource leak checks, and invalid pointer/length/handle boundary tests.

#### Scenario: A native handle is stale or released twice

- **WHEN** runtime or FFI code receives a stale generation handle or duplicate
  release
- **THEN** it returns the documented stable status without use-after-free,
  double free, panic across FFI, or leaked ownership

### Requirement: Compatibility and runtime ABI SHALL be versioned

The project SHALL publish source/edition compatibility, deprecation, manifest,
lockfile, diagnostic schema, and runtime ABI policies and SHALL diagnose
unsupported version combinations.

#### Scenario: A package uses an incompatible runtime ABI

- **WHEN** a compiled artifact or package requires an unsupported runtime ABI
- **THEN** build or launch fails before unsafe execution
- **AND** the diagnostic reports the required and available ABI versions

### Requirement: Performance and resource budgets SHALL be enforced

Committed benchmark scenarios SHALL have reviewed wall-time, peak-RSS, artifact-
size, startup, and representative runtime budgets.

#### Scenario: A compiler change exceeds a budget

- **WHEN** a reference scenario exceeds its allowed regression threshold or
  absolute resource ceiling
- **THEN** CI fails
- **AND** changing the budget requires recorded benchmark evidence and review

### Requirement: Release readiness SHALL be proven with installed artifacts

Realworld and official-package release gates SHALL use installed release
artifacts outside the source checkout on every supported host.

#### Scenario: A release candidate is evaluated

- **WHEN** a release candidate reaches the production gate
- **THEN** each supported host installs and verifies the artifact
- **AND** locked package check/test/doc/build/run succeeds without workspace path
  leakage
- **AND** skipped jobs outside the support matrix are documented rather than
  counted as success
