# Source-qualified native COFF leaf inputs

Production qualification uses the current `link_coff.sg` and `link_pe.sg` with
the unchanged bounded linker model. BSS is zero-initialized by Windows, has no
raw image payload, and can be the target of initialized pointers. Validated
standard section/file auxiliaries preserve physical primary-symbol indices.
Strict address-significance metadata has no emitted runtime allocation.

Five real C input groups cover ordinary/no-addrsig metadata, long and exact18-byte
file records, and the actual memory_read.c shim. Each SG PE pair has identical
bytes and returns42, as does an independently linked LLD image. The Windows
loader test forces both images away from their preferred bases and verifies all
8193 BSS bytes, the relocated address and endpoint writes. Twenty-seven poisoned
inputs retain causal diagnostics and previous output; five resource scopes check
String, Buffer, opaque and FileSnapshot counts and retained snapshot bytes.
Three real clang common/COMDAT/directive inputs remain rejected7203.

The mixed-debug raw-clang fixture retains its original bytes. Standard aux
admission now reaches the existing absolute-symbol public-catalog rejection7333
for `@feat.00`; the ordinary COFF/PE control must succeed for exactly that object.
PDB absolute symbols have not been added. Earlier7203 expectation failure is
retained under `m5-coff-leaf-qualified2`, alongside successful positive and
debugger observations, and is not counted as a passed parent.

The first harness run failed because clang's driver trace omitted the `.exe`
suffix on its selected linker path. The third run hit a transient Windows share
conflict replacing the progress report. Both failed reports remain retained;
the runner now resolves the observed executable and retries atomic replacement
for at most two seconds. No failed report is promoted by rewriting its result.

The old linker/archive/SDK, all CodeView profiles, typed source/unwind, native,
mixed, independent-unit, TPI hash and multi-source debugger gates must all pass
against the same producer source identities before this checkpoint is closed.
Current host DLL identities are recorded separately from historical evidence.
Final release SDK packaging, COMDAT selection, directives/CRT startup, complete
compiler self-build and M6–M8 remain open.

## Terminal evidence

`target/selfhost/m5-coff-leaf-qualified4/evidence.json` has SHA256
`c87172eaae81da7029b8c76d23dd15a33cf7122ce4c8a2ada09b8478e39be768` and result
`passed-coff-leaf-inputs`. It records107 parent commands,980 bound source and
external input records,265 checkout origins and265 retained producer snapshots,
16 tools/platform DLLs and60 direct artifacts. Every changed-source, checkout,
snapshot, tool, configuration and artifact guard is empty.

The actual host is `Windows-11-10.0.26300-SP0`. The producer started at full
revision `e3e087a80ec0a55c2621a7d0466ad9d9e417126e` and finished at
`09b88a6f4216e04558263719601062f245bfad7e`; the parallel process-registry commit
did not change any consumed producer input. Production `link_coff.sg` is
`9d694592edb1805fe653e25b86d58e4302beed4633a529f618d2b83bd27d1bd3` and
`link_pe.sg` is
`a7862b4570053f2fd77842c1f603ca897c58a0e45e45b6956f0597cb4692d704`.

Fresh regression command counts are69 for linker/archive/SDK,93 for all
CodeView profiles,12 for the PDB hash parent,32 for typed source/step/unwind,
79 for native typed objects,66 for mixed objects,109 for independent SG units,
and44 for multiple physical source files. These are504 child commands and611
total command records including parent dispatches. They are not unique case
counts. The complete native/typed/mixed/unit inputs were freshly rebuilt;
earlier passing children are not reused to mask changed linker inputs.

The mixed control's original raw COFF SHA256 is
`d97467f58909ff1c56d23c730cabfbdec259c2deee6928b8b0def2f54801d9b5`.
Plain COFF/PE linking exits0 for those exact bytes; the complete mixed gate
checks7333 and preserved prior PE/PDB. Actual DbgHelp loads the current fixed
PDB, matches its path/hash/GUID/age and resolves all four requested symbols.
The old hashless control still fails with `0xC0000005`. Current DbgHelp SHA256
is `547a1a6b57ca6c3d77bf8a1ab1096377c11e7c35d28622463729daf28841e578`;
ntdll is `a0f81bf5c610c0ff0a550adc020a58446f214790c748057bb575042e972e6a34`
and kernel32 is
`970e5e15eed7d53c8b88e06ddc71884c31804f1cb5286abf01fbce39b7f0e29f`.
No historical host identity was rewritten.

An independent terminal audit verifies nine reports,2689 unique hashed
source/tool/artifact/receipt records and all611 command outcomes. There are238
original per-step hashed stdout/stderr receipts and978 older-schema logs
observed at terminal audit. Older child schemas lack per-step stream hashes;
these978 observations are explicitly not represented as original receipts.
The final freeze also binds the current reviewed files and strict specification
checks. `openspec validate --all --strict` reports112 passed and0 failed.
