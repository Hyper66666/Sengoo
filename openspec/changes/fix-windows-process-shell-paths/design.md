# Design

At ProcessCommand construction on Windows, the owned executable copy replaces
forward slashes with backslashes. The caller's string and all argument strings
remain untouched. POSIX construction is unchanged.

The dynamic Windows command-line encoder recognizes a cmd.exe/cmd executable
whose penultimate argument is /c or /k. Only the final command payload is
enclosed by one outer quote pair without CRT backslash escaping. This matches
the command processor's reparsing semantics; the explicitly selected shell
continues to own shell syntax. All other argument encoding is unchanged.
No process is started during command construction. Existing suspended launch,
owned process tree, bounded pipes, cancellation and cleanup remain in force.

The standalone C probe uses the same minimal quoting condition as production,
then launches a real system cmd from a forward-slash path, retaining embedded
quotes and a URL in the command tail. Existing floods, timeout, Unicode cwd,
stdin and descendant cleanup run in the same probe. The CLI executor adds a
real batch run from a Unicode directory with spaces and a quoted command.
