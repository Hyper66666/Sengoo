# P0 Foundations

This fixture keeps the roadmap P0 gate executable in one small program:

- owned `String` values are returned, printed, and dropped automatically;
- generic `Result<T, E>` is used as both `Result<i64, i64>` and `Result<String, i64>`;
- `println` formats an owned `String` and a user type through `Display`.

It intentionally avoids manual `.free()`, `.drop()`, or `.close()` calls.
