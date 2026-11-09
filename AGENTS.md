<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# Local development commands

When using local `SENGOO_STDLIB`, `SENGOO_RUNTIME` or module-map overrides,
select the source runtime explicitly, before the subcommand:

```text
sgc --runtime-mode source-development check path/to/program.sg
sgc --runtime-mode source-development build path/to/program.sg
```

The default installed mode rejects source-runtime overrides; retrying the same
invocation will not fix that configuration error. Use a verified compiler and
clang path from the current checkout's development evidence rather than guessing
another application's packaged toolchain location.

Keep shell calls focused. A multi-command script has one final shell status;
an earlier successful command does not make a later failure successful. `rg`
exit 1 means no matches. Read the end of compiler/test diagnostics as well as
the initial warnings. Await builds with an adequate timeout or use the caller's
explicit managed-job facility: `Start-Process` and shell backgrounding do not
escape an executor that owns and waits for the complete process tree.
