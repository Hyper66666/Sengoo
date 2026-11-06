# SQLite extension implementation owner

- [x] Implement connection-owned bounded context and read-only SQL scalar with transaction guard and final-release cleanup.
- [x] Add changes, full-i64 last_insert_rowid and synchronous online backup APIs.
- [x] Native acceptance covers trigger checks, connection isolation, statement-retained context, backup persistence and i64 identity on Windows x64.
- [x] Run the updated package/native ASan and public Sengoo wrapper tests after compiler receiver integration on Windows x64; SQLite/String/Buffer counts return to baseline.
- [ ] Execute affected package checks on actual Linux/macOS hosts.

Owner: process/system/package implementation. Existing package lifecycle
requirements stay in their original change; this delta owns only these additions.
