# Native ABI-one declaration inventory

This inventory describes development reference providers for Windows x64. It
contains declarations and checked metadata, not implementations or a final SDK.
String is supplied by the existing C runtime. Sleep and file-readiness futures
are supplied by the existing Rust native bridge. All remain explicitly classified
as reference-runtime components; the ordinary inventory lists no binary artifacts.

The compiler caller must pin manifest SHA-256, target and ABI independently.
Selecting an artifact additionally requires a caller-pinned artifact hash and
an explicit development selection. A manifest cannot authorize itself. Native
handle ABI one is separate from the portable logical layout registry.

The SG admission modules are `native_sdk_json`, `native_sdk_contracts`,
`native_sdk`, `native_sdk_bind` and `native_sdk_artifact`. Inventory loading
retains immutable manifest/prelude/provider snapshots. Binding certificates
carry actual prelude AST/type IDs and physical source identities; those IDs are
in the SDK namespace and require explicit mapping into a program's checked
bindings. They do not enable native Future lowering or general aggregate FFI.

`prelude.sg` contains the exact readiness output pointer. Ordinary stage0 FFI
rejects that aggregate pointer; SDK inventory parsing keeps this restriction
separate from the admitted declaration's physical layout instead of replacing
the pointer with an integer.

Run the development admission gate from the repository root:

```text
python selfhost/verify_native_sdk.py --output target/selfhost/native-sdk/checkpoint
```

The gate builds and executes the SG implementation, verifies immutable input
hashes and exact resource reclamation, and records actual revision intervals.
It uses a separately verified registered-String COFF artifact to test content
and symbol selection. That dependency is not rebuilt or relabeled as a final SDK.

After a reviewed provider source change, refresh fingerprints explicitly:

```text
python selfhost/sdk/update_native_inventory.py --output target/updated-native-manifest.json
```

Review the result before replacing the checked-in manifest and its caller pin.
Changed physical prototypes fail this operation and require an ABI review.
The source fingerprints cover exact checkout bytes, including line endings.
A checkout with different source bytes must fail admission until those bytes
have been explicitly reviewed and the development inventory regenerated. Git
preserves the manifest itself byte for byte; the inventory is not a claim of
cross-host source rebuilding or a clean-machine SDK qualification.
Final SDK components, complete runtime migration, sleep lifecycle, native SG
Future lowering and ordinary SG self-building remain separate open requirements.

The development file-readiness lifecycle has a separate execution gate:

```text
python selfhost/verify_native_readiness_provider.py --clang <verified-clang.exe> --admission-report target/selfhost/native-sdk/final-checkpoint/evidence.json --admission-pin c9ddf42b4ceacbc838e6b291c5f3348ef0d893f10cb27a8a1a53fae00fd9b8ee --output target/selfhost/native-readiness-provider/new-run
```

This requires Windows x64, the locked development compiler tools and the retained
admission artifacts. It rebuilds unchanged Rust reference-runtime sources in a
separate test workspace and calls their actual exports from an independent C
consumer. Sixteen cases run one hundred measured rounds each, including real
named-pipe Pending to Ready and source-close independence; OS handles and Rust
allocation counts/bytes recover in every measured scope. Result uses an explicit
alignment-eight output pointer and consumes the owned state, as do cancel and
drop. No readiness algorithm is implemented in the test consumer.

The successful source-bound report is
`target/selfhost/native-readiness-provider/qualified04/evidence.json`, SHA-256
`77c51f9c587dc8b72959c4b8b6e9cfa286d115a0500e2fffd3bc83dc6a76ab9e`.
See `openspec/changes/add-selfhost-native-sdk/readiness-checkpoint.md` for the
exact inputs, lifecycle cases and remaining provenance boundaries. This report
does not add a binary to the ordinary catalog or change its final-mode rejection.
