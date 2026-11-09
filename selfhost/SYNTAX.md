# SG syntax implementation

This is the syntax implementation for the self-hosting program. Its modules
are built by the locked Rust stage0 compiler while semantic analysis, MIR,
native code generation and the SG linker are still being ported. Successful
lexing or parsing does not establish compiler self-hosting or source semantic
compatibility.

## Token stream and literals

`sh_lex(text: &String, max_tokens: i64)` returns `ShLexed` with an owning
`tokens` arena, `error`, `start` and `end`. A successful stream includes EOF.
The source remains owned by the caller; token text is retrieved by byte span
as a fresh owned String. `sh_token_get` returns an invalid kind `-1` for an
invalid index. Token kinds are:

| Kind | Meaning |
| --- | --- |
| 0 | EOF |
| 1–4 | identifier, integer, float, invalid numeric spelling |
| 5–9 | ordinary/triple string, raw string, bytes, char, formatted string |
| 10–12 | whitespace, line comment, block comment |
| 100–156 | reference keywords, in reference enum order |
| 200–250 | reference operators/punctuation, in reference enum order |

`sh_kind(spelling)` resolves reference keywords and operators; unrecognized
spellings resolve to identifier kind 1. Parser code must distinguish an
unrecognized lookup from a requested identifier rather than accidentally
matching it as punctuation. Only a leading UTF-8 BOM is whitespace; its
original byte span is retained. Embedded unknown characters are errors.

`sh_literal_decode` implements the current literal payload rules in SG,
including escape handling, triple-string dedent and exact embedded NUL bytes.
It uses existing length-aware runtime String operations, not C-string copies
which truncate at NUL. `sh_fstring_fields` extracts interpolation byte spans
for recursive expression parsing. Source spelling is retained independently
of payload decoding.

Lexer errors are 1101 (capacity/allocation), 1102 (UTF-8), 1103 (unknown
character), 1104 (unterminated/malformed literal), 1105 (unterminated comment)
and 1106 (invalid literal decoding). The old reference lexer silently skips
some unknown characters; the SG lexer reports them. Differential tests must
retain this explicit rejection difference rather than silently discard bytes.

## Parser and formatting

`sh_parse(text, source_id, max_nodes)` returns an owning `ShParsed` containing
the structural AST and a first located diagnostic. Node IDs refer to the
owning AST. Expressions use Pratt parsing, and declaration, statement, type
and pattern children are actual edges. This is more than delimiter matching.
The parser checks its input/node/depth limits before capacity arithmetic.
Expansion parity is a separate gate: raw syntax acceptance alone does not
close macros, cfg, deprecation, derive or Formatter/Hasher synthesis.

The SG formatter retains comment/literal tokens and changes whitespace. It
lexes its own output and checks every non-whitespace token's kind and exact
spelling before returning success. Native tests require idempotence, custom
indentation and exact resource recovery. This preview does not yet replace
the full existing `sgfmt` configuration/CLI compatibility gate.
Width is a soft byte-column limit, with breaks at comma opportunities. It is
not a terminal-display/grapheme measurement. The token layout helper validates
lexing and balanced delimiters; the CLI additionally requires the structural
parser to accept the input before it prints or writes a result.
The Windows preview sets its own stdout descriptor to binary mode before
printing formatted bytes and flushes checked output. This prevents CRT newline
translation from changing output relative to `--write`, and makes broken-pipe
errors observable. The CRT boundary follows Microsoft's
[`_setmode` contract](https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/setmode?view=msvc-170).

## Differential reference

`selfhost/reference/` is an explicit Rust test oracle using the existing
compiler. It accepts JSON lines with `source` and optional `parse: true` and
returns reference tokens and parse acceptance. It is never part of an SG
self-build. Its own Cargo workspace and lockfile avoid changing the frozen
production Rust workspace.

The SG `tools/syntax.sg` program dumps SG tokens and parse results.
`syntax_gate.py` compares token kinds and UTF-8 byte offsets against the Rust
oracle across repository SG inputs, and checks reference-valid parse cases.
It reads disk bytes without newline normalization so CRLF offsets are real.
Development scan limits and unfinished transform coverage remain explicit in
the evidence; they do not constitute a completed M2 gate.

For local builds use the locked seed/clang/source runtime and map
`selfhost_core`, `selfhost_lexer`, `selfhost_parser` and `selfhost_formatter`
to the corresponding modules under `selfhost/src`. Generated artifacts and
evidence remain under ignored `target/selfhost/`.

## Known seed implementation boundary

The current seed can leak the container owner of an owning temporary followed
immediately by field extraction, such as `sh_lex(...).error`. Tests and SG
implementation use an explicit local owner and normal automatic Drop; they do
not free values manually or weaken resource expectations. This seed defect
requires its own regression and eventual repair. The SG semantic/lowering
port must cover the general temporary-owner lifetime rather than reproduce
that defect.
