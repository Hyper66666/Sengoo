# large-file-splits Capability

## ADDED Requirements

### Requirement: Public Rust API preservation

The split MUST preserve every public item that crossed the original module boundary, including names, signatures, generic parameters, lifetime parameters, visibility, and (for `#[no_mangle] extern "C"` items) ABI symbol identity.

#### Scenario: Extern C ABI preservation

- **GIVEN** a single-file module declares `#[no_mangle] pub extern "C" fn foo(x: i64) -> i64`
- **WHEN** the module is split into a directory module
- **THEN** an `nm`-equivalent inspection of the produced staticlib MUST still
  list `foo` with the same signature and the same `#[no_mangle]` attribute
- **AND** any external `.sg` or test source that referenced `foo` by name
  MUST continue to link without edit.

#### Scenario: Pure Rust public surface preservation

- **GIVEN** a single-file module declares `pub const FOO: i32 = 7;` and
  `pub fn bar() -> i32 { FOO }`
- **WHEN** the module is split
- **THEN** consumers calling `mymodule::FOO` and `mymodule::bar()` MUST keep
  compiling unchanged
- **AND** the items MAY be re-exported via `pub use submodule::*;` from the
  module root rather than redefined there.

### Requirement: Observable behavior preservation

The split MUST preserve runtime behavior, error messages, ordering of side effects, and test outcomes; it is a pure structural refactor.

#### Scenario: Pre-existing tests pass unchanged

- **GIVEN** the original module had N passing `#[test]` functions
- **WHEN** the split is complete
- **THEN** the same N test functions MUST still pass with the same assertions
- **AND** no test assertion text or fixture value MAY be modified to make a
  test pass.

#### Scenario: Error string stability

- **GIVEN** the original module emitted error strings via helpers like
  `set_error(code, "message")`
- **WHEN** those helpers are relocated to a submodule
- **THEN** the exact byte sequence of every emitted error message MUST be
  preserved.

### Requirement: Largest file size reduction

The split MUST produce a layout in which the largest single resulting file is strictly smaller than the original pre-split file, advancing the roadmap target of no non-test source file exceeding approximately 25 KB (~1000 LoC) without a documented reason.

#### Scenario: Size reduction is real

- **GIVEN** the original file was 978 LoC
- **WHEN** the split is complete
- **THEN** every file in the resulting directory module MUST be ≤ 978 LoC
- **AND** at least one resulting file MUST be ≤ 50% of the original size
  (evidence that meaningful decomposition occurred, not a token move).

### Requirement: Minimal visibility widening

The split MUST NOT widen any item to `pub` unless that item was already part of the public surface; private items MAY be promoted to `pub(super)` or `pub(crate)` only to the extent required by the new submodule boundaries.

#### Scenario: Promoting private helper to pub(super)

- **GIVEN** the original file had `fn helper() {}` (private) used elsewhere
  in the same file
- **WHEN** `helper` is moved to a submodule and the caller stays in the
  module root
- **THEN** `helper` SHOULD become `pub(super) fn helper() {}` to satisfy the
  module boundary without leaking outside the directory module.

#### Scenario: Pub-widening is forbidden

- **GIVEN** the original file had `fn internal_only() {}` (private) used only
  within the same file
- **WHEN** the file is split but `internal_only` ends up in the same
  submodule as its caller
- **THEN** `internal_only` MUST remain private (not `pub(super)`).

### Requirement: Incremental verified slices

The split MUST be performed in a series of slices, each independently buildable, independently testable, and ending with the full verification baseline (compiler, sgc, runtime, sgpm) green.

#### Scenario: Slice 0 is always a mechanical rename

- **GIVEN** an OpenSpec change proposes splitting `foo.rs` into `foo/`
- **WHEN** the first slice lands
- **THEN** that slice MUST be a byte-identical rename of `foo.rs` to
  `foo/mod.rs` with no content edit
- **AND** the verification baseline MUST be green before the next slice begins.

#### Scenario: Each subsequent slice extracts one focused submodule

- **GIVEN** the directory module exists after Slice 0
- **WHEN** a subsequent slice lands
- **THEN** that slice MUST extract exactly one logical concern into one new
  submodule
- **AND** the verification baseline MUST be green before the next slice begins.

### Requirement: Reusable SOP capture

The split, when it is the first instance of a broader track, MUST capture the applicable Standard Operating Procedure at the end of `tasks.md` so that subsequent splits can apply it verbatim.

#### Scenario: SOP is captured in tasks.md

- **GIVEN** a Large File Splits change is the first of its track
- **WHEN** the change is complete
- **THEN** `tasks.md` MUST contain a numbered SOP section describing slice
  ordering, visibility rules, test relocation rules, and the verification
  baseline command set
- **AND** the SOP section MUST be copyable verbatim by the next Large File
  Splits change.
