# Corpus and daily workload freeze

`selfhost/baseline-workloads.json` pins the Windows target, edition, cache and
optimization policy, authoritative documents and retained gate groups. Expand
paths into actual SHA-256 inventories; retain the full revision and dirty state.
Inventory is coverage planning, not a declaration that the named tests passed.
Freeze compiler/runtime/tool build inputs as well as fixtures and reject source
additions, removals or changes across a measurement.

Measure startup, check, full/incremental build, format check, generated runtime,
locked package metadata and embedded LSP diagnostics. Use one warmup then ten
samples for each tool/workload. Candidate comparison uses separate project
caches and alternates AB/BA by sample. Body edits preserve function signature;
force-rebuild does not pretend shared native or OS caches were flushed. O2 is
the native program optimization setting; reference tool release qualification
is a separate evidence requirement.

Use the OS lifetime root working-set peak via the retained process handle,
including short commands. Reuse the current Windows descendant sampler with
all caveats: finite observation intervals, missed short children and summed
shared pages. Report raw samples and root/tree memory separately, fail on
missing root memory or sampler errors, and apply the stricter per-metric
candidate median ratio gates. Time ratio <=1.10 and memory ratios <=1.15 do not
replace existing performance/resource/artifact gates.

The LSP workload exercises the supported embedded analyzer through a
byte-identical isolated LSP image with empty PATH. It requires clean diagnostics
for valid source, the expected undefined-variable diagnostic after an edit,
and graceful stdio JSON-RPC shutdown/exit. External compiler diagnostics and
installed LSP integration retain their independent compatibility proof.

Missing release qualification yields a provisional reference result. Release
qualification must identify successful release/source build/test evidence and
the same four tool hashes. Later qualification may be bound to existing raw
samples only after revalidating unchanged source/tool identities; write a
separate attachment naming the original report digest instead of pretending
new samples ran. Comparison needs qualification for both reference/candidate.
Development or concurrent-load measurement cannot be promoted; quiet-host
measurement is an explicit operator declaration after competing builds finish,
not an assertion of VM isolation. Collect fresh samples instead of attaching
later qualification to a noisy development trial.

Evidence goes under ignored `target/selfhost/m0/`, records raw commands/output,
host, identities, scope and exclusions, and never serializes environment values.
Keep earlier failures rather than promoting their outputs into a passing run.
