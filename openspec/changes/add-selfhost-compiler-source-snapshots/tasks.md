# Tasks

- [x] Implement SG source ownership, budgets, duplicate identity admission,
  complete read/EOF/metadata checks and existing SG hash/UTF-8 integration.
- [x] Implement and independently verify the minimal Windows guard adapter,
  exact native layout and real seed package-native linkage.
- [x] Integrate all actual module/HIR/origin/liveness/MIR source consumers,
  removing raw public FileSnapshot handles from compiler module ownership.
- [x] Execute boundary, duplicate/alias, malformed/I/O/race and first-call
  resource controls without changing public FileSnapshot capacity.
- [x] Load the actual full compiler closure through the integrated producer and
  record the next real self-build result without redefining success.
- [x] Reverify affected compiler regressions, freeze exact consumed inputs,
  tools, platform/header/library closure and output/command evidence.
- [x] Run strict child/all OpenSpec checks and publish the revision/host ledger.

The first five items are verified by `target/selfhost/compiler-snapshot-formal01/evidence.json`
(SHA-256 `a3ceb884c75588b09c5f6836da0bf0168c15a24ca51b940bded1ae2b6c9d8ec1`),
26 commands on Windows 11 x64 at revision
`3a20c17026cdbd099dd13a524e491b3c43cbadb7`. All input/tool/artifact guards are
empty. The actual self-input reaches semantics 5124 in standard `file.sg`,
bytes 6174..6327; it does not produce a compiler object or close M3/M6.
Ordinary, HIR, provider and collection-flow requalification passed. The flow
report `target/selfhost/compiler-snapshot-flow-qualified2/evidence.json`
(SHA-256 `124456ee04eed88f744896bd5227942adf3d22a6713f30686dcb9c4c505606c7`)
records 7,000 calls, 22 rejection cases and three identical objects. The final
cross-report audit (SHA-256
`d5ddc9152d2ac2266a91add6a4819eb06d62ebb273cb317364c2434b88aa8df2`)
rechecks 14 reports, 6,459 files and 26 configuration candidates. Child and all
strict checks pass (116 items). See `checkpoint.md` for exact paths, receipts,
the timestamp-only provider transformation and retained evidence limitations.
