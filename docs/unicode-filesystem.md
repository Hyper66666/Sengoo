# Windows synchronous filesystem paths

The synchronous `std::file`/`std::dir` boundary converts UTF-8 paths to UTF-16
before calling Windows. File open/read/write/append, size/kind/timestamp,
same-file checks, copy/move, removal, directory creation and enumeration use
wide OS APIs. Enumeration converts names back to strict UTF-8 and retains the
existing deterministic byte ordering. No ANSI fallback is used.

`file_read_into` now records its managed Buffer's initialized length after a
successful read, so `string_from_buffer` consumes exactly those bytes. Raw
pointer reads keep their existing contract. API signatures, `Result` shape,
overwrite protection and same-file copy rejection remain unchanged. The
runtime preserves the existing move/rename behavior used by the CLI's adjacent
temporary-file atomic replacement.

Malformed UTF-8 paths return existing status 20 from result-valued synchronous
operations. Existing boolean existence/removal interfaces reject them without
addressing a replacement-character or codepage alias. Paths retain the existing
32 KiB UTF-8 runtime bound. This does not add path sandboxing or promise that a
path observed by one call cannot be replaced before another call.

On Windows x64, run the public compiled integration:

```powershell
cargo test --locked -p sgc --test filesystem_unicode
```

It starts under a Chinese/emoji directory and exercises Unicode file content,
managed reads, adjacent temporary-file replacement, copy/append and overwrite
errors, same-file rejection, metadata, deterministic list/walk/search, individual
file and empty-directory cleanup, and malformed raw-path rejection. The program
runs twice to verify repeat use.

The independent native allocation/OS probe uses the real runtime and String
implementation and repeats 1,000 file lifecycles:

```powershell
clang -std=c11 -O1 -g -ffunction-sections -fdata-sections -fsanitize=address `
  -fno-omit-frame-pointer tools/stdlib/runtime.c tools/stdlib/runtime_string.c `
  tools/stdlib/tests/filesystem_unicode_probe.c '-Wl,/OPT:REF' `
  -o target/filesystem-unicode-native-asan.exe
$env:ASAN_OPTIONS="log_path='D:\Sengoo\target\filesystem-unicode-asan'"
./target/filesystem-unicode-native-asan.exe
```

Both checks passed on 2026-11-04 with clang 19.1.7/MSVC, based on
`d46b3f6cd8d6e770734f98edc57aa67cb931e26a` plus the dirty working tree. Windows
ASan does not provide LeakSanitizer qualification. Linux/macOS branches and
asynchronous file operations have no new execution evidence from this check;
rerun final candidate gates before release claims.
