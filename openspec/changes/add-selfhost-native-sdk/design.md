# Native SDK identity and declarations

Use a versioned manifest with a native bridge family, target triple, pointer
width, ABI version, component class and artifact/content hashes. Keep native
handle-based ABI 1 separate from portable logical-layout schema 1. Each export
records its real symbol, calling convention, physical parameter/result ABI,
source revision/path/hash and provider component. A caller supplies the pinned
manifest hash and expected target/ABI; a manifest does not authorize itself.

Declarations are generated or checked against the inventory. Missing,
conflicting or changed providers fail instead of becoming guessed bindings.
Source snapshots and provider artifacts must remain unchanged through admission.
Language signatures and physical C signatures stay distinct. String views
retain receiver layout and lifetime facts in addition to an i64 pointer-bits
result. Future families enumerate start/poll/result/cancel/drop prototypes,
handle origin, language outcome and lifecycle obligations. M3 validates HIR/MIR
bridge facts; SDK data alone does not authorize an integer-to-reference cast.

Reference-runtime components remain development-only. Final components are
reviewed OS/memory shims and permitted precompiled libraries with provenance.
Runtime algorithms migrate to SG under separate owners. Packaging must not
hide non-migrated algorithms or compiler logic behind a component label.
Existing installed-mode source-override rejection remains intact; rebuilding
uses explicit source-development mode and verified tools.

Qualification compares declarations to actual providers, checks deterministic
manifest/prelude bytes and wrong-target/ABI/hash failures, and independently
executes lifecycle/resource effects. Reports retain source/tool/provider hashes
and full revision intervals. Declaration admission does not close M7.
