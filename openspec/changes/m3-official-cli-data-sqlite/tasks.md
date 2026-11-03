# Implementation and verification

- [x] 1. Implement sgcli 0.1.0 with declarative options, one subcommand and deterministic errors/help.
- [x] 2. Implement sgdata 0.1.0 bounded UTF-8/JSON/CSV and an exact i64 JSON constructor.
- [x] 3. Vendor verified SQLite 3.54.1 and implement sgsqlite owning statements, transactions and diagnostics.
- [x] 4. Verify lifecycle, negative, compatibility and outside-checkout package consumption tests.
- [x] 5. Add the file/CSV index consumer without application manual resource-release calls.
- [x] Validate this change and repository OpenSpec strictly (51/51).

Local Windows evidence: all three package `.sg` suites and native ASan harnesses
passed; independent registry consumption, persisted queries, Unicode argv and
delivered executable without development tools passed. See
`docs/development-evidence.md` and `target/package-native-tests/evidence.json`.
The new-candidate four-host installed matrix remains an M5 gate.
