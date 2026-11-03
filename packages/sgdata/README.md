# sgdata 0.1.0

Requires Sengoo **0.2.1-dev.0**, edition 2026, native backend. This official package
provides bounded UTF-8 text files, JSON helpers and CSV tables; it remains outside
the automatically imported standard library.

```toml
[registries.official]
path = "registry"
[dependencies]
sgdata = { version = "=0.1.0", registry = "official" }
```

Use the released local registry bundle, run `sgpm update`, and retain the lockfile.
No network download occurs when building from that bundle.

```sengoo
import sgdata;

def convert() -> Result<i64, i64> {
    let csv = csv_read("input.csv", 1048576)?;
    let first = csv.field(0, 0)?;
    println(first.as_str());
    csv.write("output.csv")?;
    let document = json_doc_object()?;
    let id = document.new_i64(9007199254740993)?;
    document.root().object_set("id", id)?;
    json_write("record.json", &document)?;
    let loaded = json_read("record.json", 4096)?;
    println(json_field_i64(&loaded, "id")?);
    Result { is_ok: true, value: 0, error: 0 }
}

def main() -> i64 { convert().unwrap_or(1) }
```

`text_read(path, maximum_bytes)` and `csv_read` / `json_read` require an explicit
maximum. The limit is measured in input bytes and enforced while reading,
including files that grow during the operation. A file larger than the limit
returns status 4, invalid UTF-8 returns 20, file errors return 9. Files
containing NUL bytes return status 10: Sengoo borrowed strings
are NUL-terminated, so accepting such bytes would silently truncate subsequent
`as_str()`, CSV and JSON operations. This is a text-file interface; use binary
I/O for arbitrary bytes. Unicode file paths
use UTF-8 on Unix and conversion to wide Windows paths. `text_write(path,text)`
and `json_write(path,&doc)` replace files; writes are not atomic transactions.
`data_last_error()` returns an owned descriptive message for the latest sgdata
native error. JSON syntax/type error detail remains available through std::json.

`json_read` returns an owning `JsonDoc` from the existing JSON runtime.
`json_field_i64(&doc,key)` and `json_field_string(&doc,key)` type-check root-object
fields; use the std::json `JsonValue` methods for arrays/nested objects/bools/f64.
`JsonDoc.new_i64` preserves the entire signed i64 decimal value when constructing
and serializing JSON, including values beyond 2^53. Existing `new_number(f64)`
retains its behavior. No generic serialization derive, YAML, or TOML parser is
provided by this package.

`csv_parse(text)` returns an owning `CsvTable`; `rows()` and `columns()` count all
rows including any header. `field(row,column)` uses zero-based indices and returns
an independent owned String. CSV accepts quoted fields, doubled quote escaping,
commas/newlines inside quotes, CRLF and LF record endings, and empty fields.
Bare CR, embedded NUL, invalid UTF-8, unclosed quotes, quotes in unquoted fields,
characters after a closing quote, and inconsistent column counts are diagnosed.
Quoting/line-ending errors include a one-based logical record and field plus a
zero-based byte offset in the input; column-count errors include the logical
record and expected/actual counts. These record numbers count CSV records, so a
newline inside a quoted field does not start another record. Read
`data_last_error()` immediately after failure, before another sgdata operation.
Whitespace is preserved; headers have no special parser meaning. An empty file
has zero rows/columns; a blank record contains one empty field.

`csv_new(columns)` creates a table with a fixed positive column count. Call
`push_field(text)` for every field in row order. A row becomes visible when all
columns are supplied; serialization refuses an incomplete row. `serialize()`
returns owned text and `write(path)` writes canonical CRLF records, quoting
special and empty fields. CSV table Drop frees every copied field. Tables opt
out of Send and Sync. Parsing consumes bounded text already in memory; it is
not a streaming CSV reader.

Acceptance tests: `tests/data.sg` (including exact JSON bounds), `tests/native.c`
(invalid UTF-8, quoting, limits), and the persistent file-csv-index consumer.
