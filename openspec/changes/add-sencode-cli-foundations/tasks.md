# Tasks and evidence

- [x] Record both source baselines and split implementation ownership.
- [ ] Network/data requirements: see tasks-network.md (network owner).
- [ ] Process/terminal requirements: see tasks-process.md (process owner).
- [ ] System handles requirements: see tasks-system.md (system owner).
- [x] Wire every new module into compiler/LSP/native distribution (integrator).
- [ ] SQLite context/backup requirements: see tasks-sqlite.md (system owner).
- [ ] Hierarchical CLI requirements: see tasks-cli.md (package owner).
- [ ] Compiler cache identity requirements: see tasks-cache.md (system owner).
- [ ] Windows argument and current-directory requirements: see tasks-args.md (system owner).
- [ ] Windows Unicode filesystem requirements: see tasks-filesystem.md (system owner).
- [x] Reduce and repair reference projection/reborrow/assignment, attribute,
  scalar method-loan and empty-aggregate issues with independent regressions.
- [ ] Repair the nested owned-return false rejection in
  `docs/cli-borrow-limitations.md` using binding identities and positive/negative
  escape tests; the CLI candidate currently uses an independent owned copy.
- [x] Run Windows compiler/tool/distribution compatibility and native resource
  regressions; commands and results are in the Sencode candidate verification.
- [ ] Record actual revision/host/test commands; validate OpenSpec strictly.

P0 baseline was read on Windows x64, 2026-11-04. Existing debug binaries are
mixed-revision and clang is not initially on PATH. Rebuild/explicit environment
selection is required; this observation is not a passed native build baseline.

The subsequent Windows candidate records compiler 1152, sgc 511, LSP 190,
distribution 15 and attribute-evidence 1 passing checks. The generated LLVM and
split C AddressSanitizer ownership gate reports 26 resources allocated/released
once with String handles back at zero. See the sibling
`D:/sencode/windows/sencode-cli/docs/candidate-verification.md` for matching
artifact provenance and delivery limits. Referenced host-specific subtasks remain
open where actual Linux/macOS or published-toolchain execution is still pending.
