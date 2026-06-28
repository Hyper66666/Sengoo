# Standard Library Examples

Run each file with `sgc run <path>`.

| File | Demonstrates | Expected output |
|---|---|---:|
| [`01_strings.sg`](01_strings.sg) | Importing `std::string` helpers for append, repeat, equality, search, and empty checks | `8` |
| [`02_math.sg`](02_math.sg) | Importing `std::math` helpers for integer bounds, signs, powers, and common divisors | `50` |
| [`03_error.sg`](03_error.sg) | Importing `std::error` assertion helpers for booleans, integers, strings, and floats | `7` |
| [`04_option_result.sg`](04_option_result.sg) | Importing `std::option` and `std::result` bool constructors, unwrap helpers, and projections | `7` |
| [`05_file.sg`](05_file.sg) | Importing `std::file` helpers for write, append, read, length, existence, and removal | `15` |
| [`06_env_time.sg`](06_env_time.sg) | Importing `std::env` and `std::time` helpers for environment checks, clock reads, sleep, and exit-code selection | `6` |
| [`07_random.sg`](07_random.sg) | Importing `std::random` helpers for deterministic seeding, non-negative i64 values, bounded ranges, and booleans | `8` |
| [`08_path.sg`](08_path.sg) | Importing `std::path` helpers for separator discovery, absolute checks, joining, extraction, and lexical normalization | `9` |
| [`09_process.sg`](09_process.sg) | Importing `std::process` helpers for process metadata, current working directory copy, and exit-code selection | `10` |
| [`10_collections.sg`](10_collections.sg) | Importing `std::collections` helpers for runtime-backed vectors, maps, copied text lists, string-key maps, and iterator flows | `60` |
| [`11_args.sg`](11_args.sg) | Importing `std::args` helpers for user argument count, length checks, and Buffer-backed copy | `11` |
| [`12_dir.sg`](12_dir.sg) | Importing `std::dir` helpers for directory existence, creation, recursive creation, and empty-directory removal | `12` |
| [`13_io.sg`](13_io.sg) | Importing `std::io` helpers for exact stdout/stderr writes and flushing | `13` |
| [`14_strconv.sg`](14_strconv.sg) | Importing `std::strconv` helpers for decimal i64 parsing and Buffer-backed formatting | `14` |
| [`15_dir_listing.sg`](15_dir_listing.sg) | Importing `std::dir` helpers for deterministic listing and bounded recursive walking | `15` |
| [`16_file_copy_move.sg`](16_file_copy_move.sg) | Importing `std::file` helpers for binary copy, host-rename move, explicit overwrite selection, and metadata | `16` |
| [`17_process_run.sg`](17_process_run.sg) | Importing `std::process` helpers for synchronous shell-free child execution with explicit arguments | `17` |
| [`18_status_buffer.sg`](18_status_buffer.sg) | Importing `std::status` categories and composable Buffer text helpers | `18` |
| [`18_json.sg`](18_json.sg) | Importing `std::json` helpers for parse/build status and parse diagnostics | `18` |
| [`19_process_capture.sg`](19_process_capture.sg) | Importing `std::process` command builders for captured child output | `19` |
| [`20_owned_string.sg`](20_owned_string.sg) | Importing `std::string` owned String helpers for copy, clone, append, and Buffer output | `20` |
| [`21_assert.sg`](21_assert.sg) | Importing `std::assert` as the primary assertion helper module | `21` |
| [`22_regex_log.sg`](22_regex_log.sg) | Importing `std::regex` and `std::log` helpers for bounded matching and log configuration | `22` |
| [`23_config_hash.sg`](23_config_hash.sg) | Importing `std::config` and `std::hash` helpers with Buffer output | `23` |
| [`24_compress.sg`](24_compress.sg) | Importing `std::compress` gzip Buffer helpers for JSON bytes | `24` |
| [`25_formatting.sg`](25_formatting.sg) | Formatting owned `String` values with positional placeholders and f-strings | `25` |
