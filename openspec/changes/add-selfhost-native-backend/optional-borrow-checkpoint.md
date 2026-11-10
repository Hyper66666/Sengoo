# Optional borrow physical ABI qualification

This checkpoint qualifies the internal physical MIR27 consumer, concrete push
MIR builder, aggregate Drop layout initialization and their bounded tests.
It does not qualify a public source-to-MIR27 compiler, safe-reference minting,
collection effect refinements, constructors or full M3/M4 self-bootstrap.
Ordinary `sh_mir_validate`, instruction typecheck and native entrypoints reject
27 with 5008. A fresh source compiler must independently discharge provider,
typed source, backing, ownership and active-payload obligations before calling
the internal physical path. The MIR schema remains one with a new opcode
extension; exact old consumers explicitly reject it rather than reinterpret it.

The optional operation consumes the actual fixed-C next return and that call's
exact handle SSA value. It emits the whole concrete bool/reference Option,
forming None without a standalone zero reference. Direct SSA and a unique
local initializer retain the same-tag true-branch association; the false
successor cannot reach payload extraction without retesting. Whole optional
transfer, hidden carrier aliases and escaping payload references reject.
Scalar fields of a borrowed non-Copy owner remain address-based reads. The
backend initializes the actual supported aggregate Drop layout before walking
fields, after preserving unsupported glue/type rejection precedence.

Formal physical report:
`target/selfhost/m4-optional-borrow-physical-qualified2/evidence.json`, SHA-256
`975c7fce8cdadf7db1506d5de256d363e571effd487665964cac90dd8cbdc682`.
Windows-11-10.0.26300-SP0, full revision
`2d3d2b87c34f6d13799a49faf7b83ee03d9178fa` through
`7ae89c92d6844fc069ff5c3cfc40461cc5f2d293`; concurrent commits did not change
consumed inputs. Fifty-eight commands pass their expected exits, with 675
input hashes, 195 original checkout hashes, ten tools and eighteen main
artifacts. All source/tool/configuration/checkout/artifact guards are empty.
The runner explicitly selects the development runtime, forces source rebuilds,
pins real cargo/rustc paths and disables wrapper overrides, observes their
actual invocations, and guards present/absent ancestor Cargo configurations.

The exact command is Python `selfhost/verify_optional_borrow.py --seed
D:/Sengoo/target/agent-conditional-compiler/bin/sgc.exe --clang
C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe
--formatter D:/Sengoo/target/agent-formatter/debug/sgfmt.exe --output
target/selfhost/m4-optional-borrow-physical-qualified2`. The report records
actual resolved tool paths/full hashes and UTC start/end times.

One hundred first successful scopes and nineteen negative cases with five
first rejected scopes each preserve exact resources. Every MIR scalar,
vector, storage identity, type/symbol limit and symbol String handle/byte is
compared before and after validation. Two intentional assertion failures
exit one. Objects emitted in two fresh directories are byte-identical and
independently inspected/disassembled with LLVM. Actual qualified C provider
objects exercise i64, bool, non-Copy Owner16, scalar-helper branches and
repeated None for 1,000 scopes. Owned elements remain live until vector Drop.
Typed push also exercises real 1MiB owning values and actual OOM in a separate
task-owned process under a 48MiB Windows Job process-memory cap, with a 32MiB
stack reserve for that ABI. Status zero consumes by move, fourteen by callee
Drop and two leaves the value for exactly one typed caller cleanup. The OOM
case records seventeen creations/releases and twenty-eight moves, including
twelve reallocations of already-stored elements rather than new creations.

The linked provider is the fixed historical namespace-two qualification
`target/selfhost/m3-collection-provider-ast-checkpoint/evidence.json`, SHA-256
`a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3`.
Its actually consumed immutable source copies, tool hashes and exact three C
objects are revalidated. It is not a newly self-recorded hash of a changed
current provider. C probes only observe ABI/resources or apply OS memory
pressure; they do not implement compiler semantic policy. Each C header
dependency closure, actual lld-selected library and loaded kernel32,
KernelBase and ntdll path/hash is retained. C links explicitly select lld.
These host observations do not claim to capture the OS kernel implementation
or establish the final no-C/no-Rust bootstrap SDK.

The existing native regression executes all thirty-eight commands. Its actual
recursive import closure now preserves all eighteen baseline sources and adds
`mir_optional`; missing imports explicitly fail instead of escaping the guard.
Legacy nested clang links remain behavior regressions and do not establish
complete installed SDK provenance.

Focused old-consumer/cache report:
`target/selfhost/m4-optional-borrow-controls-qualified2/evidence.json`, SHA-256
`dc8bd7e3c31ddecfb9d9b8d99895c1879cf09d52c23b26634e63cc5d565eb1fe`,
full start/end revision `7ae89c92d6844fc069ff5c3cfc40461cc5f2d293`, same host.
Thirteen commands pass, with 667 input hashes, 187 original checkout hashes,
eight tools and six artifacts; all guards are empty. Exact Git sources from
the pre-extension revision are rebuilt. They reject opcode27 at the builder
with 5002 and at instruction typecheck with 5003. The pre-fix native consumer
rejects the pointer-only aggregate Drop fixture with 6301 and preserves its
output sentinel. The fixed consumer emits it successfully without any earlier
owner-field read. Independent C executes 1,000 actual typed resource Drops
with exit42. An empty destructor emits successfully but exits78 on the
independent resource check, so layout rejection cannot mask failed cleanup.
The command uses `selfhost/verify_optional_controls.py` with the same three
tool arguments and `--output target/selfhost/m4-optional-borrow-controls-qualified2`.

Independent flat audits preserve all declared source/tool/report/artifact and
receipt hashes, plus additional emitted negative-control outputs:
physical `audit.json` SHA-256
`c05c09d928dc06eded7284add7805ae5c0b81774982d12f7df73a44b9f309408`
(1,848 records, 230 receipts), focused `audit.json` SHA-256
`ffadadeb1dd5ee586458dc79ad8e6c2ec1a30f432da04f06d47ece952d9648fc`
(1,540 records, 64 receipts). Historical original provider documentation is
matched to its consumed immutable copies; unchanged template inputs retain
their actual original hash. The old native regression retains receipt files
without embedding their hashes in its report schema; the audit records their
actual retained hashes without rewriting that report.

Failed trials remain intact: the first physical runner omitted one fixture
snapshot helper and failed compilation; the first focused C link lacked an
unused foreign-declaration trap stub. Fresh complete runs above contain the
repairs. Neither failed report is relabeled as passed. Source effects and
their outer ownership consumer remain a separate pending qualification; no
source Some/push integration or compiler milestone is checked off here.
