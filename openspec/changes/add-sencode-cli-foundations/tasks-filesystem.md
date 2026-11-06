# Synchronous Unicode filesystem implementation owner

- [x] Unify strict Windows UTF-8/wide conversion for synchronous file and directory OS calls used by the CLI.
- [x] Verify public native Chinese/emoji read, managed Buffer length, atomic replace, copy/append, metadata, list/walk/search and individual removal through `cargo test --locked -p sgc --test filesystem_unicode`.
- [x] Verify malformed UTF-8 rejection with existing status 20 without lossy alias writes.
- [x] Run 1,000 Unicode read/replace/copy/list/remove cycles with native AddressSanitizer on Windows x64.
- [ ] Execute synchronous filesystem integration on actual Linux/macOS hosts.
- [ ] Qualify asynchronous file paths separately; this change's execution evidence is synchronous.

Owner: process/system/package implementation. No application file-editing,
permission, search or path-containment business logic moves into the runtime.
