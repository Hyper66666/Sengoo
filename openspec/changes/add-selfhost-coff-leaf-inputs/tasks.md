## Private prototype
- [x] Snapshot current linker and verify real source-locked C leaf inputs and tool hashes.
- [x] Implement bounded BSS/section-file-aux/addrsig input processing with relocation-index preservation.
- [x] Execute SG-linked and independent LLD products proving zero initialization, writes, initialized data and relocated BSS pointers.
- [x] Reject malformed aux/addrsig/BSS, auxiliary relocation targets and unsupported COMDAT/weak/common/directive forms with preserved output.
- [x] Record resource recovery, raw provider rejection and exact private evidence; production promotion and complete M5 remain open.

## Private evidence only

- `target/selfhost/m5-linker-sdk-leaf-prototype/final-evidence.json`: SHA256 `610210fe8b708259626a409aad095e6a097c86525e0a6cb0d7e3b5f36cd4427f`, 50 commands, four real C input groups each executed through SG and independent LLD with exit42; 24 exact malformed/unsupported controls preserve previous output. Five first-call scopes cover all positives and 24 rejection paths with exact String/Buffer/opaque/FileSnapshot/bytes recovery. All candidate inputs unchanged; full revision `60892e6b74d770ba7612519737be9d1d05281e66` through `60892e6b74d770ba7612519737be9d1d05281e66`.
- `target/selfhost/m5-linker-sdk-leaf-prototype/loader-evidence.json`: SHA256 `6ff56c2acffe950e04ea45c4104b2d56a386b49d59762af9763ee89ce0463463`, three commands. The actual Windows loader maps both SG/LLD products away from their preferred image base; BSS8193 bytes are zero, the initialized pointer equals the rebased BSS address, C execution returns42 and both endpoint writes are20. The SG BSS has virtual8193/raw0 and the uninitialized-size header agrees.
- `exact-file-evidence.json`: SHA256 `411db870c0d32f8f8b15bf17235dcfcca5d810bf4d25b0ba38a944176a5276a5`, actual clang one-record .file name of exactly18 bytes without NUL is accepted; long multi-record actual .file is part of the four-group evidence.
- `actual-unsupported.json`: SHA256 `cd4377ffde5acee4f42c3602c0745c8d37c819f8677fc9d69e5d943c45e3cd46`, actual clang .drectve, common (`-fcommon`) and COMDAT selectany objects all fail7203 and preserve output. The pinned actual runtime.obj/runtime_string.obj remain rejected7203; this is not a full CRT success.
- `target/selfhost/m5-selfbuild-link-inputs/evidence.json` inventories the existing pinned C provider and actual archive trace: raw objects have BSS, section/.file auxiliaries/addrsig; runtime has ANY/ASSOCIATIVE COMDAT and directives. The default existing static CRT link extracts311 members, exceeding the unchanged128-object bound, and has .CRT initialization/load-config needs. Direct input common/weak/TLS sections and final TLS/resource/export directories are absent in that concrete branch.

## Production promotion

- [x] Review exact ABI/profile and primary-format rules, promote only authorized producer/parser changes with dedicated provenance.
- [x] Run existing complete linker/archive/import and PDB source/step/unwind/multiunit/mixed regressions with qualified fresh producer snapshots.
- [ ] Qualify final SDK packaging and necessary COMDAT/directive/CRT work separately; do not increase the object cap to mask unsupported selection/startup semantics.

Production evidence: `target/selfhost/m5-coff-leaf-qualified4/evidence.json`,
SHA256 `c87172eaae81da7029b8c76d23dd15a33cf7122ce4c8a2ada09b8478e39be768`.
It passes107 parent commands and504 commands across eight fresh child reports.
See `production-checkpoint.md` for the actual host/revisions, exact child counts,
diagnostic migration and independent terminal-audit limits. All source,
snapshot, tool, configuration and artifact guards are empty. Complete M5 and
selfhosting remain open.

Primary format references: [Microsoft PE/COFF](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format), [LLVM address significance tables](https://llvm.org/docs/Extensions.html).
