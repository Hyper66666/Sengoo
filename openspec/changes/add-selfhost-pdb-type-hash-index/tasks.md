## Implementation
- [x] Emit required bounded TPI hash/index streams in both writers, preserving original records/IDs and deterministic identity.
- [x] Qualify independent hash/index oracle, malformed controls and first-call exact resources.
- [x] Qualify actual DbgHelp loaded-PDB identity/name lookup and missing-stream negative controls.
- [x] Run existing typed SG source/step/unwind, physical-file, independent-unit and mixed-object regression; retain public-only LLDB19 limitation.
- [x] Freeze evidence and run strict child/all OpenSpec validation; M5 remains open.

## Qualified checkpoint

The main qualification is `target/selfhost/m5-pdb-type-hash/final-checkpoint2/evidence.json`, SHA256 `1e1309b800242288e0b5cfdd496ca49cb09bd12b03fd88e9614cb7f346c83ca1`, full revision `2856dc2d4734006464f1d5da0a4106ac4be4f0ac` through `2856dc2d4734006464f1d5da0a4106ac4be4f0ac`, completed UTC `2026-11-10T04:00:13.605140+00:00`. Source, snapshot, checkout, tool and Cargo configuration changes are all empty. This remains bounded M5 qualification, not completion of selfhosting or public-only C support in LLDB19.

The focused SG test performs three first-call five-resource scopes, preserves all original type arena handles/bytes/offset/depth words, rejects 10 malformed/cap/header cases, and independently checks 541 original type records (struct/union/enum, UTF-8 names, extended numeric leaves, modifier/pointers/arrays). Hash bytes are 2164; index pairs are `(4096,0)` and `(4604,8176)`, exercising the 8192-byte crossing rule. The Python oracle rejects 11 hash/header/index corruptions.

Qualified debug child reports (all exact current source/tool/config/artifact identities reverified):
- `typed-source-step-unwind`: `D:\Sengoo\target\selfhost\m5-pdb-type-hash\checkpoint2\typed-source-step-unwind\evidence.json`, SHA256 `b00e4640e6ef2ebe64558c31f6b6fcc1c71635eb2a35c576e280b7d27f32e5dd`, 32 commands.
- `native-typed`: `D:\Sengoo\target\selfhost\m5-pdb-type-hash\checkpoint3\native-typed\evidence.json`, SHA256 `2e431c437d43f472887e951b08cdba7a8e1d76be9f40d256bc488546f44235af`, 79 commands.
- `mixed`: `D:\Sengoo\target\selfhost\m5-pdb-type-hash\checkpoint2\mixed\evidence.json`, SHA256 `74ba5877128494dbe632ef393fbdf154688a00c398e9db90f015fe027fdae8e6`, 66 commands.
- `units`: `D:\Sengoo\target\selfhost\m5-pdb-type-hash\checkpoint3\units\evidence.json`, SHA256 `00c23a430e526c86943b194540cc39bbb4a65a5e60c93e3d0bf9778afab98051`, 109 commands.

The 32-command typed SG source/step/unwind report and 66-command mixed report were independently terminal PASS in checkpoint2, while that parent failed at the separate native oracle's obsolete stream count. The native oracle was corrected to required 12 streams plus strict hash/index checks; fresh native (79 commands) and independent units (109 commands) passed in checkpoint3. The final parent rechecks all four independently passed children. It rejects changed or unbound actual Python harness/imports against the prior pre-run SHA, reconstructs exact immutable copies, and revalidates every source, artifact, tool and configuration. It does not promote either failed parent or failed native binary. The auxiliary reconstruction evidence `target/selfhost/m5-pdb-type-hash/reused-child-snapshots/evidence.json`, SHA256 `e1e2bac7771a6710388b73b0fab0d788c9e8ec8894bb8426cfd8660ab9d08a73`, retains 254 content-addressed input snapshots and prior completed outer receipts.

Actual Windows DbgHelp causal qualification: `target/selfhost/m5-pdb-type-hash/counterfactual/evidence.json`, SHA256 `7260855d4bb8a00e49870e22d6ca12d58a247899857d0bfbf771cd0c9243dfca`, completed UTC `2026-11-10T04:00:35.681373+00:00`. Both processes use the identical System32 DLL SHA `2d2ec94891162a61d86789133623b47d64d74c15d6be99473b25ad7a6468cf39`. The indexed variant reports actual LoadedPdbName/file SHA/GUID/age and unmatched=false; four names resolve to independently bound PE/PDB public RVAs. In the no-hash variant only the TPI hash advertisement/ranges change; type bytes/IDs, GUID/age and other streams stay identical, and PE changes only its RSDS path. Its fresh DbgHelp load fails with `0xC0000005`, while the indexed product succeeds. The historical hashless negative control is retained separately. No C PROCREF or source/type records are fabricated.

Earlier development failures remain preserved: unrelated all-Python guard scope was narrowed to actual imports; the M0 formatter was rejected by the current tool pin and replaced by the current locked formatter; the native oracle's obsolete 11-stream assertion was updated; the cap fixture loop avoided S0's repeated nested borrowed-condition limitation. None of these reports was relabeled passed. The successful private eight-type pure SG hashstream/LLDB19 investigation remains separately frozen under `target/selfhost/m5-name-breakpoint-investigation`, evidence SHA `0737fc952de3032f6e8af79460b52337ca6b192155d4cdaa57f9c25ddcf86605`.

Strict child/all OpenSpec and final whitespace checks are recorded with the freeze manifest. LLDB19 public-only C name lookup remains pending for both SG and independent LLD products; full M5/SDK/debug coverage remains open.
