# Tasks

- [x] Capture a failing real hidden-console reproduction of ready input blocked by unrelated records (`target/terminal-polling/before.txt`, exit 11).
- [x] Bound Windows ready-record draining and preserve split UTF-16, timeout and cancellation behavior.
- [x] Pass independent native and public Sengoo polling (1/1), process/terminal (2/2), screen restoration (3/3) and Unicode (1/1) regressions on Windows x64; rerun strengthened polling bound/positive-timeout assertions (1/1).
- [x] Record Windows host, full base revision, working-tree source hashes and commands in `target/terminal-polling/evidence.json`; `openspec validate fix-terminal-ready-input-drain --strict` and `openspec validate --all --strict` pass (67/67, 2026-11-05).
