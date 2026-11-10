# Fresh source constructor checkpoint

`collection_source.sg` owns the fresh source-file entry. `mir_lower.sg` has an
additive constructor-enabled internal mode; existing lowering selects its
disabled mode. The public source entry takes independent filesystem/provider
pins and function names and does all checking itself. No saved MIR or caller
candidate facts are accepted.

The bounded source profile admits concrete parameterless SG functions containing
the canonical `vec_new` call. It discovers actual concrete element/owned-field
Drop implementations using the checker constraints, records their exact bodies
and rejects additional unchecked/deferred/generic jobs. Generated callback
bodies and target layouts are checked again after the module is complete.
Identical foreign C declarations are coalesced only with identical symbol,
callable type, C ABI and declaration flag; conflicts fail.

The actual returned object still depends on the independently qualified C
collection algorithms. This is development integration, not replacement by an
SG runtime, a complete compiler CLI or an S1/S2 fixed point. Zero-sized and
over-aligned layouts, arbitrary constructor-containing source functions,
optional borrows and other collection operations remain later work.

The executable source fixture has no physical runtime implementation import.
An independent C caller validates the Windows SG aggregate shape, i64/bool
storage and moves, actual nominal owning Drop with a release and a separate
observation, callback reuse and opaque resource restoration. Rejection cases
run in fresh processes and require no output object. The owning type's actual
two-i64 layout has natural alignment eight.

The source profile recursively restricts owned ADT elements to actual struct
declarations without a recorded parent relation. Class/enum elements and owned
fields containing them reject before lowering; their inherited/variant Drop
chains have no source constructor qualification here. Owned function names must
retain their full bytes in the borrowed string view, so NUL cannot select a
truncated prefix.

The qualification runner captures the entire import closure and actual tool,
runtime, provider, header and link branches before execution, keeps failed
trials separately and repeats object emission. Its regression branch runs
ordinary MIR, projected ownership, disarm and the complete existing source
native corpus. Known frozen-reference gaps remain explicitly classified.

An API-only follow-up may reuse these ordinary regression reports only after
comparing their complete compiler producer, harness, runtime, stdlib and tool
inputs byte for byte. It validates the immutable child reports and artifacts
again and keeps the old source-native receipt limitation explicit: that older
child schema has no per-stream producer hashes, so those logs are retained
terminal observations. A changed ordinary producer requires a fresh regression.
