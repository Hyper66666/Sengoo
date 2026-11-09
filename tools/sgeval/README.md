# sgeval 0.1.0 (experimental)

Offline Agent event recording and deterministic assertions, implemented in
Sengoo. The executable has no model, process, MCP or tool-dispatch dependency.
Replaying a `run_command` event evaluates data; it cannot run the command.

```text
sgpm update
sgpm build --locked
sgeval record input.jsonl recording.jsonl redactions.json
sgeval replay recording.jsonl assertions.json report.json
```

`record` validates the complete input before writing its output. It recursively
redacts credential fields (`Authorization`, API keys, cookies, passwords,
access/refresh tokens, credentials), case-insensitively, and replaces explicit
literal secrets from the optional redactions file in all strings and keys:

```json
{"secrets":["my-private-value"]}
```

Literal redactions must be supplied for secrets embedded in free text or
serialized provider bodies. This is not automatic detection of arbitrary
personal data. Raw input is never copied into diagnostics. Keep unredacted
exports and redaction files out of Git. Secret values are read from a file,
not command-line arguments or process listings.

Input is UTF-8 JSONL, one object with a nonempty string `type` per line. Blank
lines and CRLF are accepted. Embedded NUL is unsupported: raw NUL bytes and
escaped `\u0000` in any key or string are rejected recursively before creating
C-compatible views. This also applies to recordings, redaction configuration,
assertion paths and expected payloads, so truncation cannot create a false pass
or a partially redacted recording. Other Unicode remains supported. Output has versioned sequence envelopes:

```json
{"schema":1,"sequence":0,"event":{"type":"text_delta","text":"Hello"}}
```

Assertions require a version, exact event count, ordered list of event types
and optional value comparisons. Paths are object-key paths; `equals` compares
JSON recursively, preserving exact integer representation rather than converting
to f64. Every supplied assertion must be understood; unknown assertion keys,
broken paths, invalid JSON, wrong versions and duplicate/gapped sequence values
are errors. `absent_strings` searches decoded strings and keys.

```json
{
  "schema":1,
  "event_count":1,
  "types":["text_delta"],
  "checks":[{"event":0,"path":["text"],"equals":"Hello"}],
  "absent_strings":["fixture-private-key"]
}
```

The report is JSON with mode `offline-data-only`, event count, overall `passed`
and named check results. Exit 0 means assertions pass, 1 means an assertion
failed, 2 means invalid input or an I/O/resource error. Existing output files
are refused. No success report is created for malformed data. The redacted
recording and report must use distinct paths from the inputs.

Limits are explicit resource bounds: 8 MiB raw event input, 16 MiB recording files, 1 MiB raw event lines,
2 MiB recorded lines, 4096 events, 16 MiB recorded output, JSON depth 48, 65536
nodes/document, 128 literal secrets of at most 4096 bytes, 1024 value checks.
They are unrelated to model request budgets. A capacity error never silently
truncates events or declares a partial evaluation successful.

The independent Python test driver only launches this executable, supplies
temporary fixtures and checks redaction/report/exit behavior. The evaluation
logic itself is Sengoo. See `tests/offline.py` for record → redact → replay
and malformed/incorrect evidence cases. No API key or paid provider is needed.
