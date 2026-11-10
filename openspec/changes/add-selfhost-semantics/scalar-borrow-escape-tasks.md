# Primitive borrow escape focused checkpoint

- [x] Compare exact baseline and minimal target patch on actual typed positive and preserved negative source cases.
- [x] Promote only the proved kind1/2/3/4 path-escape guard to production borrow.sg after the baseline barrier.
- [x] Run focused first-call resources and actual owning primitive-result execution with exactly-once Drop.
- [x] Run existing ownership corpus, raw MIR/owned HIR6, projection22 and disarm/actions8/9/10 regressions against immutable sources.
- [x] Verify separate reference/Owner move/invalid backing/false-Some rejection boundaries, source/tool guards and strict specification checks.
- [x] Freeze the focused owned files with complete report, artifact and qualification identities for root review.

Full collection provider/NLL/MIR27 integration and the M0-M8 self-hosting plan
remain independently open.

## Qualified evidence

Final report: target/selfhost/m3-scalar-borrow-escape/checkpoint/evidence.json,
SHA256 9517fc21002e146f8a2e7664c9db4911c3963368ef43e713dc741f0abb5b2c68.
Actual full revision ed3aaa4ea7f778603712cf21a49aa8661ac1634b at start and end,
Windows11 10.0.26300-SP0/Python3.12.14. Actual UTC
2026-11-10T02:53:07.192073+00:00 through
2026-11-10T03:07:16.478021+00:00. All25 top-level commands pass, including the
actual documented exit42 programs/negative guards. All592 source/dependency
inputs,226 producer origins,271 immutable snapshots/artifacts, five selected
tools and Cargo configuration candidates remain unchanged. All source,
snapshot, producer-checkout, tool and configuration change lists are empty.
No production HIR/MIR/provider/runtime source is modified by this phase.

The runner reconstructs the exact old ownership bytes from the unique minimal
new condition and requires SHA990fb05324a641ecd2ebee7cd696d3224e55f8992df5bdd4118718969bdff45c.
Its baseline executable and the candidate executable run the exact same19
actual typed source cases fifty times each with String/Buffer/opaque resource
baselines captured before the first call. Six primitive implicit field-return
false positives change from6105 to0. Two existing successes (local scalar
dereference and explicit return) remain0. Eleven Ref/reference-ADT/rawptr/
nonprimitive-ADT/array/tuple/generic-field/Owner-move/live-borrow-owner-move/
legacy-handle cases retain their actual old diagnostics. Typed checking must
succeed before each ownership assertion. The live Owner move keeps6104;
no invented diagnostic code or broad !carriesBorrow exemption is used.

The production delta is one path condition in sh_b_escape, SHA256
827539b4f828a0575c8945517be79cb63da03cdb6638466e5c7ec2fcd5d90dd2.
Earlier walk/move/Drop/legacy-handle checks, generic type identities and
ShOwnership22 remain unchanged. No diagnostic list or old error is cleared.

The unchanged ownership corpus and raw MIR checkpoint pass their four-command
child gate (including50 ownership resource rounds,10 HIR/lowering/cleanup and30
CFG/SSA/ABI resource rounds), report SHA
e1ee555d67289fc5ba410d1f7a35a58fdc2e435b907dd3f860511b1557f75d06.
Projection22 commands pass, report SHA
5c5fcb5b9ca3c4235c5cf1d48eadc673a241f820ad7f3ae7d007ea9c0aace650.
Disarm19 commands pass with its explicitly preserved reference zero-free Drop
policy difference, report SHA
e11eeb89ac5f37f6391f4851be4827081dc316ce547c906699ac52440c551cd7.
That difference is the preexisting source Drop/free(0) call policy, not a leak
or a new parity claim. Disarm action10, observed/projected actions8/9 and actual
native100-transfer cleanup are retained.

Owned HIR6 ordinary first-call resources and the complete actual standard graph
three-scope lift/modulevalidate/encode/decode/clone gate pass. Old wire5
rejection and all22 ownership fields remain intact. The independent optional
physical false-Some modes3/4 each reject5007 in five resource-checked scopes;
ordinary module/provider candidate permission remains separately gated.

A new ordinary source Option<&Owner> guarded primitive-read program is emitted
through the SG native compiler and independently through the pinned frozen
reference LLVM path. Both linked executables return42 for100 scopes. The
independent C probe requires exactly one acquisition and release per scope and
zero live owners, including the original Owner's real Drop body. It implements
only counters, not ownership or return-type inference. This is ordinary source
borrow/copy cleanup proof, not collection provider admission or new SafeRef
permission. Snapshot child reports, binaries/objects/IR and the consumed input
hashes remain bound to this report.

Wrong-seed stale-pass replacement control exits1 with zero executed steps;
failed report SHA48b3c6c7946ba34102f8934a6bb5f3252e6ec92726de3e1a45f151ef754ad15b.
Only this tasks evidence ledger is excluded from the focused producer guard to
avoid a report-hash cycle. Design/spec and owned source bytes are guarded.
Child/all OpenSpec strict108/0, format and diff checks pass. Full library
ownership/NLL, generic jobs, actual compiler self-build and M0-M8 remain open.
