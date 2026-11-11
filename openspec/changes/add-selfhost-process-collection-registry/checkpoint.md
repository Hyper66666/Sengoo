# Process-profile registry checkpoint

Qualified on Windows-11-10.0.26300-SP0, 2026-11-11, at unchanged revision
`e3e087a80ec0a55c2621a7d0466ad9d9e417126e`.
`target/selfhost/process-registry-formal-04/evidence.json` has SHA-256
`6d64004889e54defc6f5824828205b546fc143346eb7c38a886291e4e9f3043d`.
All 153 commands have their expected exit and retained stdout/stderr digests.
The report binds 1,746 source entries, 219 external inputs, 82 tool identities
and 1,326 artifacts; all source, external, tool and checkout guards are empty.

## Actual process profile

SG owns initialization, slot selection, generation retirement, kind checks,
RawVec mapping, word-block values and ownership cleanup. C owns two static zero
storage addresses and SRWLOCK operations. The existing qualified SG RawVec and
explicit-context registry algorithms are reused byte for byte.

Both the directly linked storage object and the actual storage DLL execute the
same independent C resource probe: 100 scopes, 1,201 owning Drop callbacks which
each reenter the registry, four OS threads with 8,000 independent metadata
transactions, 1,833 actual allocations/releases, zero final payload bytes and a
3,104-byte peak payload. Every release is checked with VirtualQuery/MEM_FREE.
The probe covers all 128 slots, publication failure cleanup, three allocation
failures, positive context/reference versus negative process token rejection,
wrong-kind and stale tokens, invalid integer ranges and maximum-generation
retirement. Its ten completion IDs are checked exactly.

The final retirement control uses explicit test-only metadata setup after all
live entries are closed. The independent C formula gives the last valid token
as `-(2^62 + 16777215*256 + 128) = -4611686022722355072`. A retired slot cannot
be republished, and the immediately smaller integer is rejected. Production
code exposes no reset operation. This verifies the boundary without claiming
16 million real creation cycles.

## Actual development DLL and products

Two fresh directories independently emit five SG objects, compile the memory
and callback leaves, compile real storage `.bss`/auxiliary/unwind records and
produce the DLL/import library. Only the storage COFF timestamp is zeroed;
its state, code, relocations and unwind are preserved. Existing memory/callback
leaf metadata normalization is independently checked as in the prior core gate.
The source-bound historical SG linker emits both final executables. All eleven
corresponding SG/C/DLL/import/PE products are byte equal; both executables return
42 without output with an empty PATH.

The actual loaded DLL path and SHA are read from Windows. Both static roots are
zero before SG initialization. RtlLookupFunctionEntry confirms registered unwind
for both nonleaf lock functions. Actual forwarded lock/thread/memory provider
paths, function RVAs and DLL hashes are bound independently.

The storage DLL SHA is
`85edf34da775235d55ffb73141f2d27ee8bd9ad0bb84dc959b4d8d8d3b6ddbbd`;
its import library SHA is
`05036af7ff30d585638a71b108971e688815346657b05bcc2df23647fb5e7aee`;
the SG-linked PE SHA is
`74b33792b60ce77b6b2e87fbe14875133f8e2b36fed9c15ffd070bf2681ae25f`.

## Explicit OS requalification

The host's System32 ntdll.dll and kernel32.dll changed since the historical
text/core/linker qualifications. Those reports remain unchanged. They declare
no retained copy of the old DLL bytes, so their complete historical OS closure
cannot be reverified against the current machine.

`--os-rehost` explicitly records both historical and current hashes and replays
all 55 text and 45 core commands in new output directories. Every producer's
output is absent before that command runs; intermediate outputs, actual command
receipts and new products are recorded. Original arguments/flags are preserved,
with only the output-root path relocated. Normalized C timestamps follow the
original verifier. Twelve SG/normalized-leaf/SG-PE products from each parent
are byte equal to their historical products. Raw C probe timestamps carry no
reproducibility claim.

The fresh text execution covers 1,221,242 corpus records, guard pages, the real
64 MiB limit, production memory adapters, the negative empty-oracle control and
a cold large SG stack using the actual NTDLL import. The fresh core execution
covers the complete 50,000-operation/five-family differential and 100 actual OS
resource scopes with 20,100 owning drops. New working-set observations remain
observations, not a fixed RSS equality claim. New process/DLL gates then run
against the same current OS providers.

Current ntdll.dll SHA:
`a0f81bf5c610c0ff0a550adc020a58446f214790c748057bb575042e972e6a34`.
Current kernel32.dll SHA:
`970e5e15eed7d53c8b88e06ddc71884c31804f1cb5286abf01fbce39b7f0e29f`.
Any other historical input drift is a hard failure. Requalification does not
retroactively assert that the old OS DLL bytes were retained or that all old
linker profiles were rerun on this host.

Failed formal attempts remain available: 01 rejects the OS identity change;
02 completes text replay but catches an overly broad raw-C timestamp equality
assertion; 03 completes both replays and catches the copied retirement-boundary
constant error. The final 04 gate includes the corrected independent boundary
calculation and the actual DLL resource probe.

## Remaining integration

This is an unsafe, explicit process/DLL-lifetime profile with 128 slots.
Metadata locking does not protect unpinned payload use against concurrent close
or mutation. Duplicate DLL instances or unloading/reloading a DLL do not give
globally unique capabilities. The 16,512 state bytes exclude OS lock/runtime
storage and are not a whole-process RSS result.

Namespace2, existing provider/source-lowering contracts, public runtime handle
ABIs and stdlib remain unchanged. String, Buffer, maps, Rc/Arc, asynchronous
objects, scheduler TLS and other global domains still require migration.
Installed SDK deployment, complete SG self-build, stage convergence, corpus and
performance/release/rollback gates remain open.
