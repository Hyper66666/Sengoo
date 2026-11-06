# sgcli 0.1.0

Requires Sengoo **0.2.1-dev.0**, edition 2026, native backend. This independent
official package is distributed through the local registry bundle and is not
automatically part of the standard library.

```toml
[registries.official]
path = "registry"
[dependencies]
sgcli = { version = "=0.1.0", registry = "official" }
```

Copy the released bundle's `registry/` beside your manifest, then run `sgpm update`.
Subsequent `check`, `test`, `doc`, `build`, and `run` can use `--locked` offline.

```sengoo
import sgcli;

def execute() -> Result<i64, i64> {
    let mut app = cli_app("greet", "0.1.0", "A greeting tool")?;
    app.string_option("name", "n", false, true, "Name to greet")?;
    app.flag("verbose", "v", "Verbose output")?;
    let parsed = app.parse_args();
    if !parsed.is_ok() {
        let message = app.last_error()?;
        println(message.as_str());
        return Result { is_ok: true, value: 2, error: 0 };
    }
    let args = parsed.value;
    if args.flag("help") {
        let help = app.help()?;
        println(help.as_str());
    } else {
        if args.flag("version") {
            let version = app.version()?;
            println(version.as_str());
        } else {
            let name = args.string("name", 0)?;
            println(name.as_str());
        }
    }
    Result { is_ok: true, value: 0, error: 0 }
}

def main() -> i64 { execute().unwrap_or(1) }
```

The parser never prints or exits. Applications return 0 for success and 2 for
argument errors. Built-ins `--help`, `-h`, and `--version` bypass required values,
but still reject malformed/unknown arguments. Call `help()` or `version()` to get
owned text for output. Help order follows registration order and uses LF endings.

`flag(name, short, help)` registers a boolean; `string_option` and `int_option`
take `repeated` and `required` booleans. `command_flag`, `command_string_option`,
and `command_int_option` take the command name before these same arguments.
Short names are one ASCII alphanumeric character (or empty for no short form).
The low-level `option(command, name, short, kind,
repeated, required, help)` registers a command-specific option; kind 0 means
flag, 1 string, 2 i64. Register the command with `command(name, help)` first.
Options in a command apply after that subcommand. Global options apply before
and after it. `subcommand(parent,name,help)` adds a child beneath a registered
parent. Command paths use single spaces, for example `"config profile show"`;
`command(path,help)` also accepts a complete path once its parent is registered.
Paths contain up to 16 ASCII-name segments and 1024 bytes. Register ancestors
before descendants. `CliMatches.command()` returns the complete selected path,
preserving the previous single-level result for existing applications.

Options registered on an ancestor apply after that ancestor and throughout its
descendants; required ancestor options are checked at the final selected path.
An option for a leaf is accepted only after entering that leaf. Ancestors cannot
reuse an inherited long or short option name. Siblings can register independent
options with the same names. An unknown child is an error while a command still
has children; use `--` to begin literal positionals. A parent command can itself
be selected without a child, and built-in help still bypasses required options.
Help lists registered full paths in stable registration order.

```sengoo
app.command("config", "Configuration")?;
app.subcommand("config", "profile", "Profiles")?;
app.subcommand("config profile", "show", "Show profile")?;
app.command_int_option("config profile show", "limit", "l", false, false, "Limit")?;
// tool config profile show --limit=-3
```

`positionals(minimum, maximum)` enforces positional bounds. `parse_args()` reads
process arguments excluding argv[0]; `parse_from(&Vec<String>)` uses the same
convention and enables deterministic tests. The app can be reused for parsing.
On Windows, process arguments are converted from the operating system's wide
command line to UTF-8, independently of the active ANSI code page.

Accepted spellings include `--name=value`, `--name value`, `-n=value`, `-n value`,
negative integer values, and `--` followed by literal positional arguments.
Short flag clusters and concatenated short values are intentionally absent.
A string value beginning with `-` must use `--name=-value`. Integer options
check the full signed i64 range. Unknown options, missing values, invalid numbers,
and duplicate nonrepeated options return `Result` errors (status 2); allocation
and invalid-handle failures use the common Sengoo status codes.

`CliMatches` provides `flag`, `count`, `string(name,index)`, `integer(name,index)`,
`positional_count`, `positional(index)`, and `command`. Missing indexed values
return status 5. Returned strings are independent owned values. Applications
can inspect `app.last_error()` immediately after a failed registration or parse.

`CliApp` and `CliMatches` are owning checked handles with automatic Drop and
negative Send/Sync declarations. No shell completion or terminal UI is provided.
Executable acceptance examples: `tests/cli.sg`, `tests/native.c`, and
`examples/realworld/file-csv-index` in the source distribution.
