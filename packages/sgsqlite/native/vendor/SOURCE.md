# SQLite source provenance

Version: **3.54.1**, official amalgamation, public domain.

Downloaded from https://www.sqlite.org/2026/sqlite-amalgamation-3540100.zip
on 2026-11-03. The unmodified archive's SHA3-256 is:

```
92281c86e2957f9b9305890f9ac04204caa7b677cc868dc326d942689df28e30
```

The archive contains `sqlite3.c`, `sqlite3.h`, `sqlite3ext.h`, and `shell.c`.
Only `sqlite3.c` is included by `../sgsqlite.c`. No build step downloads source
or compiles the shell. Upstream copyright/public-domain notices are preserved
at the start of each file. SQLite's public-domain declaration is documented at
https://www.sqlite.org/copyright.html.

The wrapper defines `SQLITE_OMIT_LOAD_EXTENSION=1`, `SQLITE_THREADSAFE=1`,
`SQLITE_DQS=0`, and `SQLITE_DEFAULT_FOREIGN_KEYS=1`. Extension loading is absent
from the bundled library. Sengoo handles additionally opt out of Send and Sync.
