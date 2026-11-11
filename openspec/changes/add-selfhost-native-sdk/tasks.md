## Native inventory and declarations

- [x] Inventory actual native x64 String, sleep and file-readiness exports with exact prototypes, family, source identities and language lifecycle obligations.
- [x] Implement a versioned native SDK manifest separate from the portable logical layout inventory.
- [x] Admit caller-pinned manifests and selfhost-owned SG ABI declarations with provider/source/artifact identity validation for the named development COFF scope.
- [x] Verify deterministic bytes and malformed, conflicting, missing-provider, wrong-target/ABI/hash rejection cases.

## Provider and lifecycle qualification

- [ ] Bind registered String view ownership/resource execution to the selected manifest and provider.
- [x] Execute file-readiness start/poll/result/cancel/drop cases with the explicitly selected development provider (`verify_native_readiness_provider.py`, qualified04; see readiness-checkpoint.md).
- [ ] Execute sleep start/poll/result/cancel/drop cases with the explicitly selected development provider.
- [ ] Preserve physical C signatures separately from language view/Future signatures in M3 integration.
- [ ] Inventory remaining SG runtime algorithm ports without treating development reference binaries as completed ports.

## Final SDK materialization

- [ ] Package only permitted reviewed shim/library components with immutable hashes and complete provenance.
- [ ] Verify source rebuilding separately from installed SDK use and preserve the existing override policy.
- [ ] Prove ordinary SG self-building does not invoke Rust/C compilers, assemblers or external linkers to materialize SDK inputs.
- [ ] Record complete Windows evidence and validate child/all specs strictly before release claims.


## Admission gate development correction

The first development/checkpoint runs used an incorrect argument index and
skipped external fixtures. Their original reports/logs remain preserved with
review-status.json invalidation notes under native-sdk/development-1 and
native-sdk/dispatch-gap-trial. The twenty internal resource/poison rounds did
execute; these reports do not qualify the forty-nine external catalogs or
provider objects. The corrected harness requires exactly one user argument at
index zero and an observed completion marker containing the fixture count.
The complete source-bound admission run below supersedes these invalid trials.

## Completed inventory and development admission checkpoint

The corrected source-bound report is
`target/selfhost/native-sdk/final-checkpoint/evidence.json`, SHA-256
`c9ddf42b4ceacbc838e6b291c5f3348ef0d893f10cb27a8a1a53fae00fd9b8ee`.
It records identical actual start/end revision
`a433e81a929897ab34f348ff1b267d05c261b822`, Windows 11 AMD64, ten successful
commands, 632 unchanged inputs and empty source/tool changes. All fifty-one
completed fixture IDs and both expected error codes appear individually in
the native output and are checked against the complete ordered unique input
list by Python. The original forty-nine cases are extended by malformed COFF
section-number and symbol-value cases; both fail with 5409.

Twenty native inventory/JSON/binding scopes return String, Buffer, opaque and
FileSnapshot handles and retained snapshot bytes exactly to their initial
counts. Native tests additionally corrupt all nine entry fields, C convention,
declaration/source certificates, selected artifact ordinal, artifact SHA text
and same-length in-memory object contents. Validators reject them and retain
the type-arena size. Argument owners are released before final resource checks.

The canonical manifest SHA-256 is
`f044410918f66b38c6059b5e4b29f94aa35ea08c748fcdd3d795a7ac4c055bbb`.
The actual runtime.c and ffi.sg fingerprints are respectively
`7f905b1935ad9d56bd36bd46a354211649ed8444f90afa10070b350648c61e7e`
and `ac6bf91064451dd05879c71a279faca5a2c533243a248d298c30971315d72a60`.
The actual runtime_string.c COFF object has SHA-256
`08b654eaf94d4bf6db0fec8753ee2e0c56c2793b8dbc5914b073360fd579c0f3`;
its dependency is the independently executed registered String report
`8fd3e9def047fe1fcb122ac399bb39b9138162cf9e52098d2a5b5b26070a3d7e`.
This gate verifies that unchanged provider source, object content, physical
target and actual defined String symbols; it does not rebuild that dependency
or repeat its resource execution. Reference components remain forbidden in
final mode, and the checked-in catalog lists no binary artifacts.

Child and all-spec strict validation passed (101 items). Evidence task text is
excluded from the build hash closure to avoid a self-referential report hash;
proposal, design, normative spec, complete compiler/runtime/tool Rust and Cargo
inputs and stdlib inputs remain guarded. Module-to-program binding mapping and
HIR integration, archive/static-library admission, native Future lifecycle
execution, final permitted components and ordinary self-building remain open.
This is explicitly development inventory admission, not a final SDK or complete
M3/M7/self-hosting qualification.

## Completed development file-readiness lifecycle checkpoint

`readiness-checkpoint.md` records the formal source-bound lifecycle run and its
limits. The named gate checks 1,600 measured resource scopes, sixteen ordered
case completions and the actual linked provider's eight defined exports. It
reuses the pinned admission report above for the identical catalog and source
bytes. Sleep, SG Future lowering and final SDK qualification remain open.
