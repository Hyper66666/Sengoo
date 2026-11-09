# Explicit native SDK inputs for SG self-hosting

The approved self-hosting program needs reproducible native inputs after its SG
compiler and linker replace the Rust/LLVM build path. Native runtime bridge
declarations, provider identities and allowed platform/library components must
be explicit inputs. Portable logical layouts do not prove the native handle ABI.

This child owns native SDK manifests, compiler-consumable ABI declarations,
component provenance, packaging and compatibility qualification. M3 owns typed
language/runtime bridge facts; M4/M5 own native emission/linking. Separate SG
runtime ports own algorithm migration, and the umbrella owns final self-build,
release and performance gates.

Begin with x86_64 Windows native ABI 1 inventories for String views, sleep and
file-readiness futures. Explicitly identified reference runtimes may be used
for development; this does not qualify SG runtime algorithms or a release SDK.
Final ordinary self-building uses SG tools/sources and locked SDK binaries;
rebuilding permitted C/platform inputs is a separate development procedure.
