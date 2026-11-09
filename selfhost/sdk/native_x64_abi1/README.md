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
Final SDK components, complete runtime migration, native async lifecycle and
ordinary SG self-building remain separate open requirements.
