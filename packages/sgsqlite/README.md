# sgsqlite 0.1.0

Requires Sengoo **0.2.1-dev.0**, edition 2026, native backend. The package embeds
official SQLite **3.54.1**, verified against its upstream SHA3-256. Source,
license, URL, hash, and build defines are recorded in `native/vendor/SOURCE.md`.
Builds do not contact the network and require no system SQLite installation.
Runtime extension loading is compiled out.

```toml
[registries.official]
path = "registry"
[dependencies]
sgsqlite = { version = "=0.1.0", registry = "official" }
```

Place the released registry bundle beside your manifest and run `sgpm update`.
The package includes its native C source; use `--locked` on subsequent builds.

```sengoo
import sgsqlite;

def work() -> Result<i64, i64> {
    let db = sqlite_open("notes.db")?;
    db.busy_timeout(2000)?;
    db.exec("CREATE TABLE IF NOT EXISTS notes(id INTEGER PRIMARY KEY, text TEXT)")?;
    let mut tx = db.transaction()?;
    let mut insert = tx.prepare("INSERT OR REPLACE INTO notes VALUES(?, ?)")?;
    insert.bind_i64(1, 1)?;
    insert.bind_text(2, "世界 and SQL punctuation: ' ; --")?;
    insert.step()?;
    tx.commit()?;
    let mut query = db.prepare("SELECT text FROM notes WHERE id = ?")?;
    query.bind_i64(1, 1)?;
    while query.step()? {
        let text = query.text(0)?;
        println(text.as_str());
    }
    Result { is_ok: true, value: 0, error: 0 }
}

def main() -> i64 { work().unwrap_or(1) }
```

`sqlite_open(path)` opens a file or `:memory:` database. `SqliteDb` supplies `exec`,
`prepare`, `transaction`, and a nonnegative i32-millisecond `busy_timeout`.
`exec` accepts trusted SQL scripts; use prepared statements with bound values
for application input. `prepare` accepts exactly one statement. Bind indices are
one-based, column indices zero-based. Binding methods support `null`, `i64`, `f64`,
`text`, and `blob(&Buffer)`; SQLite copies bound string/binary data before returning.

`changes() -> Result<i64,i64>` returns SQLite's direct change count for the most
recent INSERT/UPDATE/DELETE; trigger side effects do not inflate this count.
`last_insert_rowid()` preserves the full signed i64 value, including negative
rowids. Both observe this connection and follow the active-transaction guard.
`backup_to(path)` uses SQLite's online backup API to copy a consistent main
database to a persistent file, including committed WAL data. The destination
must differ from the source; `:memory:` is rejected. The configured busy timeout
also applies to destination locking. Backup is synchronous and can report
busy/I/O; it does not publish an atomic renamed archive. Use a separate temporary
destination and an application-controlled final rename when needed.

`set_context(key,value)` stores a private owned string on this connection.
Keys contain 1–128 UTF-8 bytes, values 0–4096; NUL is rejected and there are at
most 64 keys. Updating a key replaces its owned value. The SQL scalar
`sengoo_context(key)` reads a value, returning SQL NULL when missing. It can be
used by triggers and cannot change context through SQL. Set context before
starting a transaction; connection mutation remains rejected during an active
transaction. Context survives while a statement retains its connection, is
freed on final connection cleanup, and is excluded from database backups.
It is application metadata, not a secret store or a sandbox against code that
owns the connection API.

`step()` returns `true` for a current row and `false` for completion. Accessors
`integer`, `number`, `text`, `blob`, and `is_null` validate the current row and
column type. Text/binary outputs are independent owned values and survive the
next step, reset, statement Drop, and connection Drop. `reset()` clears bindings
as well as resetting execution. The API does not coerce text to numbers.

Connections, statements, and transactions use checked monotonically identified
handles. A statement keeps its connection alive until finalized, even when the
owning public connection has been dropped. All three handle types are !Send and
!Sync; callers must keep them on their creating thread. The package does not
provide ORM, pooling, async execution, nested transactions, or extension loading.

Transactions require explicit `commit()`. Drop or explicit `rollback()` rolls an
active transaction back; SQL execution failures roll it back immediately.
The original extended SQLite error is preserved. A transaction's statements
become unusable after commit/rollback; drop them or prepare new connection
statements. During an active transaction, connection operations and statements
prepared outside that transaction are rejected; use `tx.exec` / `tx.prepare`.
Nested transactions and manually issuing BEGIN/COMMIT/SAVEPOINT through SQL are
rejected by the SQLite authorizer. `commit()` failure rolls back.

Public operations return `Result<_,i64>` with existing Sengoo statuses: 2 invalid
argument/type/constraint, 3 invalid handle, 7 transaction authorization, 9 I/O,
11 busy/locked timeout, 12 interruption, 13 range/size overflow, 14 out of memory,
or 1 other SQLite error. Read `sqlite_error_code()` (extended SQLite code) and
`sqlite_error_message()` immediately after failure, before another operation
clears the error channel. Native traps and process termination are not errors
that this API can recover from.

Executable evidence: `tests/sqlite.sg` verifies the owned wrapper and automatic
Drop; `tests/native.c` checks persistence, SQL parameter injection prevention,
Unicode, i64 bounds, blob copying, commit/rollback/error, active-transaction
guards, busy timeout, stale handles, lifetimes, and disabled extensions.
