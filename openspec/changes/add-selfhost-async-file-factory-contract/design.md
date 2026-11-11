# Bounded AsyncFile factory authorization

This is the implementation contract, not a claim of completed support.

## Establish authority before using it

The module loader must supply validated physical source ownership before method
body checking. Today `sh_modules_check` calls `sh_check` before it fills
`node_modules` and `binding_modules`; adding a name comparison inside
`sh_s_check_function` cannot establish source authority. Split collection/body
checking, or pass a validated origin context with the original AST nodes.

Select the standard file module through its actual resolved loader edge and
stable snapshot identity, and bind its reviewed source digest and contract
version through the selected compiler/SDK source profile. Accept relocation of
the same validated source snapshot; reject changed bytes, cross-file mappings,
user same-name types, shadow providers, and an arbitrary path merely named
`file.sg`. The standard-module bit or filename alone grants no capability.
Standalone string-check APIs get no factory privilege unless their caller
explicitly supplies and validates the same source contract. Do not hard-code a
machine-specific absolute path.

Physical validation must re-read bytes, digest, metadata, identity and path
through the retained OS source handle. Comparing mutable graph text and a
stored digest to each other is insufficient: both can be changed together.
The factory stage consumes the corrected compiler snapshot validation API and
must exercise that same coherent-mutation rejection before final qualification.

The selected `stdlib/file.sg` path must agree with the existing retained
source path. A source request must also be proved by parsing the importing
retained source, checking its exact import span and its mapping to the joined
compiler text. Direct and group `std::file` imports remain valid. The loader's
single implicit `std::stream` to `std::file` edge requires a real selected stream
module and a proved standard stream import. Changing `standard_ids`, requested
names, request rows and unit flags together cannot grant a local same-byte copy
the standard factory privilege.

Path normalization does not reopen the source. Exact canonical path spelling
is accepted; a spelling differing only by Windows ordinal case requires every
already-retained ancestor directory to report case-insensitive lookup. The
platform helpers perform UTF-8/UTF-16 ordinal string comparison and query
`FileCaseSensitiveInfo` through the guard's existing directory handles. A
case-sensitive directory or an unknown/unsupported policy rejects case-folded
matching. ASCII and Unicode case variants, relative paths, trailing separators
and relocated selected libraries need positive controls; sensitive/unknown
responses need negative controls. A host that cannot enable case-sensitive
directories cannot claim an actual sensitive-filesystem execution pass.

## Prove the exact initial shape

Resolve all names to binding IDs, never just spelling. The initial version has:

- The selected nominal `AsyncFile`, with exactly one signed-i64 `handle` field;
  the selected nominal `FileReadinessOutcome`, with ordered bool `is_ok`, bool
  `value`, signed-i64 `error` fields. Windows x64 result layout is size 16,
  alignment 8, offsets 0, 1, 8. Do not confuse the source output type with an
  unrelated same-layout nominal.
- One synchronous, nongeneric, fixed-arity inherent method with a shared
  `&AsyncFile` receiver, one i64 timeout, and the builtin structural
  `Future<that FileReadinessOutcome>` result. No trait/dyn receiver,
  arbitrary Future output, extra parameters or variadic convention.
- A resolved external C declaration, no SG body, fixed signed-i64/signed-i64
  inputs and signed-i64 result, matched to the selected provider contract
  `sengoo_async_file_wait_readable__start`. A local function with that spelling
  or a mismatched C declaration cannot be substituted.
- The complete canonical method body is a single tail call of that declaration
  with field-zero `self.handle` and the actual timeout parameter in that order.
  Check both expressions normally and retain their source/binding identities.
  No extra statements, alternate returns, arithmetic, captures, forged literal,
  hidden second start or retaining call is admitted by this initial proof.
  Supporting equivalent alternative syntax later requires a separate complete
  dataflow proof rather than weakening this exact canonical-body check.

Only after these facts validate may this method return its factory-created
Future and may this exact final raw C result cross to the language Future.
The exception is per declaration and producer expression, not per result type,
method name, module, or all implementation bodies. The raw provider call still
has its actual i64 ABI. No general implicit i64-to-Future conversion is added.
Ordinary user Future returns, nested escapes, foreign Future signatures and
unrelated i64/Future mismatches retain their existing restrictions.

## Preserve facts and fail closed

Own and serialize the factory declaration ID, wrapper body/call site,
receiver/output nominal IDs and layouts, physical source/profile identity,
language function type, exact C declaration and type, and a closed lifecycle
origin identifying entries 11..15. Use a versioned bounded fact table and pure
validation. Do not silently append meanings to the six-word String intrinsic
record. If the HIR wire shape changes, bump its schema and reject old data
explicitly. Clone/roundtrip must not rediscover authorization from names.

The initial factory record has 24 words, with one owned source/profile witness.
HIR schema seven bounds, copies, compares and encodes both. Its structural
source anchor is an integrity check, not an unforgeable physical certificate.
Coherently rewriting the anchor and serialized module identities can preserve
structural consistency; the module consumer must rebuild selection and compare
against the actual retained graph before using the facts. Ordinary HIR lowering
continues to reject module-witness-bearing input. Neither a self-consistent
wire payload nor a copied standard-module bit grants native authority.

Maintain two capabilities: a proved source factory may be semantically valid;
native execution additionally requires a selected verified provider and native
consumer implementation. Missing native support produces a located unsupported
error for a reachable call. It must never erase a Future to i64, omit cleanup,
or attach the reference's arbitrary string origin. All ordinary imported bodies
remain semantically checked regardless of code-generation reachability.

When implemented, the native factory owns one opaque handle with this origin:
start creates; poll observes; result OR cancel OR drop consumes, exactly once.
The actual Rust provider's result/cancel/drop each take and free the state, so
calling drop after result is invalid. The result has an explicit output pointer
plus i64 handle and returns void; the target LLVM spelling uses sret align 8.
Preserve that physical ABI separately from language `Future<Outcome>`.
The provider registers a duplicated file resource; awaiting readiness does not
transfer or retain a raw pointer to the source AsyncFile struct. Preserve the
receiver for later read/close and qualify clone registration/release behavior.

Existing documented rejection of unsupported owned async cancellation remains
in force; this child does not authorize those paths. A real native extension
needs execution and exact-resource tests for ready, pending/wakeup, result,
cancel, unused-Future drop, invalid argument/handle and source-file close while
the duplicated readiness interest exists, all bound to the actual provider.

## Observed next boundaries

Removing only diagnostic 5124 is insufficient: the same small std::file
program also reports 5104 at 6258..6321 and 6248..6327 (provider expression and
body are i64 where Future is expected). A sealed producer bridge addresses
both without suppressing body checks.

An ordinary `async worker -> i64; async main { await worker() }` already passes
SG semantics but native compilation stops in lowering with 5003. Native Future
drop currently rejects kind 16 with 6502. The development Rust provider's
file-readiness lifecycle is independently qualified by
`target/selfhost/native-readiness-provider/qualified04/evidence.json`; sleep,
SG Future consumption and the final SDK remain open. The private factory path
advances a real `std::file` import through semantics, ownership and HIR; its
subsequent print-call lowering failure belongs to `add-selfhost-print-intrinsics`.
The next full compiler self-input result must still be measured and recorded.
Existing source-flow reachability may avoid an unused factory
in the compiler closure, but it must not become a semantic skip mechanism.
