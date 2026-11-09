## Text algorithms

- [x] Implement bounded allocation-free UTF-8 validation and validated boundaries in SG.
- [x] Keep the C adapter limited to a raw byte load without text policy.
- [x] Emit with the qualified SG compiler and independently execute valid/invalid scalars, truncations and byte sequences.
- [x] Verify no-read zero/negative/oversized/range-rejection paths and protected-page extents.
- [x] Record source/tool/host/revision evidence and strict decoder comparisons.
- [x] Independently verify C SDK metadata conversion and the actual large-frame stack-probe dependency without changing SG objects.
- [x] Reject wrong source, SG compiler binary and SG formatter binary pins before executing a tool.
- [x] Validate this child and all specs strictly, keeping full runtime/SDK milestones open.

## Later runtime integration

- [ ] Bind storage and unsafe input lifetime contracts to the selected SDK.
- [ ] Integrate owned String construction/mutation with unchanged status and ownership behavior.
- [ ] Qualify packaging, Rust-free self-building, concurrency and performance before replacing the C implementation.

## Executed local checkpoint

`target/selfhost/runtime-text-final-checkpoint/evidence.json` has SHA256
`ba8af7fba5a6025f3016ecac88fc5cb3bc41d1ced4bb3cfd539219e4363c808b`.
It records identical start/end revision
`736b80fc71862e777c09232d74f8cac0ab7999ea` on
`Windows-11-10.0.26300-SP0`, 55 completed commands, 1,126 source inputs,
126 external inputs (including actual C headers and the loaded NTDLL provider)
and 24 tool identities. Source, external-input, tool and checkout guards are
all empty. The report records the actual concurrent dirty checkout rather than
claiming a clean committed rebuild. The root independently rehashed every
consumed manifest entry before committing this phase.

The strict Python decoder and per-character re-encoding checked both generated
corpora completely. Each has 1,221,242 records: 1,150,662 valid and 70,580 invalid,
including every Unicode scalar and all one/two-byte combinations. The identical
9,374,470-byte corpora have SHA256
`5ba19a59d8b565bd44b0d1e7715ddbb593ee8e651570b91ff64c2df047f6bc47`.
The instrumented caller performed 62,091,698 checked reads and verified
2,443,718 no-read calls, complete valid-input coverage and unchanged input.
The separate 64 MiB read-only/guard-page run performed 201,327,322 checked
reads, including all-byte coverage for validation and zero/end boundaries.
The production adapter ran separately with `instrumented = false` and
`memory_reads = null`; no instrumented read-count claim is made for that mode.
All three modes checked 60 head/tail guard cases. Changing the first empty
input's expected validity produced actual exit one at `validation-oracle`.

Two fresh directories with Unicode, spaces and ampersands produced identical
unmodified SG objects, normalized C SDK objects and SG-linked PE bytes. The
raw C objects are retained separately; their timestamp bytes are explicitly
normalized, so raw C-object byte equality is not claimed. Twelve deliberate
code/data/address/relocation corruptions were rejected by the independent
metadata comparator. Product SG emission, SG linking and execution used an
empty PATH. Clang/objcopy/objdump/dlltool were explicit development SDK or
independent verification dependencies; the actual SG algorithm was never
compiled or linked through a reference fallback.

The development stack-probe library binds the actual loaded
`C:\Windows\System32\ntdll.dll` export `__chkstk`, ordinal 2347, RVA 1467408,
with DLL SHA256
`651ac9041af8e6b5ebd331d53163244c0a27d36d1433f0f15cac1f28c5f2e82c`.
The explicit DEF has SHA256
`9d197356f00c1000c3d5b6c84051c653061a3b8eb0d8902936f8339292abaff6`,
and its generated AMD64 import library has SHA256
`a84c01a227d7ba417c2188a378854e0bba0a9d25988e6c8af1fa98cfc00bd55f`.
Independent PE inspection verified the real import and 4,096-byte initial stack
commitment. A real SG function with a 131,312-byte frame used the emitted
RAX-size/call/sub-RSP probe sequence and cold-started with exit 42. Both text
programs also cold-started with exit 42 and empty stdout/stderr. This selected
local dependency is not a public Windows SDK API or final SDK qualification.

Independent qualification rejection evidence is
`target/selfhost/m7-runtime-qualification-negative-controls/evidence.json`,
SHA256 `22e153659e098547ec6f1fe954aaeedb0f7e93ea6f48b777e334dae5dc226334`.
All three wrong-pin controls failed with the expected identity error and zero
tool commands. Its runner identity equals the passed checkpoint's frozen
`verify_runtime_text.py` SHA256
`c7ceb18322ecd0523e4adf45a142e61a4f456e71faa379c9ae7e8d257b97f04e`.

Failed trials 01 through 03 retain the Windows dependency-parser mistake,
unsupported ordinary C COFF metadata and missing real stack-probe provider.
Trial 04 passed the functional gate; the final checkpoint additionally records
actual start/end Git status. The locked Rust formatter still rejects these
unsafe C export sources; this gate explicitly uses the qualified SG formatter
preview. Owned String storage, final SDK, performance and full M7/M8 remain open.

The child and all-spec strict validation passed on this Windows host after the
ledger update: 105 items passed, zero failed. The owned staged diff check also
passed before the implementation commit.
