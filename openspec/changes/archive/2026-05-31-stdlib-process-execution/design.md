## Context

`std::process` currently exposes process ID, current-directory copy, and
exit-code selection. `std::args` already established an opt-in argument ABI for
the current program. The remaining everyday scripting gap is launching a child
program.

This slice must stay shell-free. Passing one command string to `system()` would
make quoting platform-dependent and turn ordinary data into shell syntax. The
runtime should start the requested executable directly and pass each argument
as one literal argv entry.

Sengoo does not yet expose a general string-slice or `Vec<&str>` ABI. Existing
FFI/Lua wrappers already use fixed arities as a bounded bridge, so process
execution should follow that established pattern until a collection-backed
argv design is possible.

## Goals / Non-Goals

**Goals:**

- Add portable synchronous direct-executable child-process helpers.
- Support common zero-to-three-argument script calls without raw pointers.
- Return the normal child exit code.
- Inherit stdin, stdout, and stderr.
- Keep the runtime dependency-free.

**Non-Goals:**

- No implicit shell command execution or shell-string parser.
- No arbitrary-length argv collection yet.
- No stdout/stderr capture, stdin piping, or general pipe API.
- No cwd/environment override.
- No detached/background process handles, timeout, signal, cancellation, or
  async execution.
- No new string ownership, slice, collection, or Unicode-path ABI.

## API Shape

`std::process` gains:

- `process_run(executable: &str) -> Result<i64, i64>`
- `process_run_1(executable: &str, arg0: &str) -> Result<i64, i64>`
- `process_run_2(executable: &str, arg0: &str, arg1: &str)
  -> Result<i64, i64>`
- `process_run_3(executable: &str, arg0: &str, arg1: &str, arg2: &str)
  -> Result<i64, i64>`
- `process_run_raw(executable_ptr: i64, arg0_ptr: i64, arg1_ptr: i64,
  arg2_ptr: i64, arg_count: i64) -> Result<i64, i64>`

The C runtime exposes one `sengoo_process_run` entry point. The source wrapper
converts normal `&str` values to pointers and supplies an explicit argument
count. The raw bridge accepts counts from zero through three and rejects
missing pointers for used argument slots.

## Runtime Contract

On Windows, the runtime uses `CreateProcessA`, waits for completion, and reads
the process exit code. It lets the host resolve the executable from the
constructed command line, escaping and quoting argv entries as needed
according to the Windows C-runtime parsing convention so spaces, quotes, and
backslashes remain inside the intended argument.

On Unix-like hosts, the runtime uses `fork`, `execvp`, and `waitpid`.
`execvp` preserves host PATH lookup behavior while passing explicit argv
entries without a shell.

The helpers block until the child exits. Standard input, output, and error are
inherited from the current process. A normally exited child returns its
non-negative exit code, including nonzero codes. Startup failures, wait
failures, invalid arguments, and abnormal signal termination return an
error-shaped result.

## Security Boundary

The implementation must not call `system`, `popen`, `cmd.exe`, `/bin/sh`, or
any equivalent shell internally. Shell metacharacters such as `;`, `&`, `$`,
and `|` remain literal argument bytes unless the caller explicitly chooses a
shell executable as the child program.

Fixed-arity wrappers are intentionally limited. They close the immediate
usability gap without inventing an argv encoding that future owned-string and
slice work would need to replace.

## Risks / Trade-offs

- **Risk:** Fixed arity is less ergonomic than a normal argv slice.
  **Mitigation:** mirror the existing fixed-arity wrapper pattern and require a
  follow-up OpenSpec for collection-backed argv.
- **Risk:** Windows command-line construction can accidentally merge or split
  arguments.
  **Mitigation:** centralize quoting in one runtime helper and test an argument
  containing spaces.
- **Risk:** Child processes can have side effects.
  **Mitigation:** make executable and arguments explicit and avoid implicit
  shell interpretation.
- **Risk:** Synchronous execution can block indefinitely.
  **Mitigation:** document the blocking contract and defer timeout/background
  APIs to a separately specified process-handle design.

## Verification

- Compiler surface tests cover the wrappers and runtime symbol.
- `sgc` import expansion tests expose the process helpers.
- `sglsp` symbol/signature tests expose the public signatures.
- Native runtime smoke coverage verifies a child exit code and one argument
  containing a space.
- A runnable stdlib example demonstrates explicit shell selection only at the
  call site and returns a deterministic score.
