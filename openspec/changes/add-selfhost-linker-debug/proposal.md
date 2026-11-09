# SG native linker and Windows debug artifacts

The approved self-hosting pipeline needs an SG-owned linker after the native
backend's COFF output. This change owns COFF/archive/import-library reading,
symbol resolution, PE32+ image layout and relocation, Windows imports/exports,
unwind/debug linking, CodeView and PDB production. The native backend continues
to own instruction and object emission; the program umbrella owns final
self-build, compatibility and performance qualification.

The first implementation checkpoint links bounded AMD64 COFF objects into an
actual Windows executable, including a minimal explicit kernel32 ExitProcess
import. It must execute through the SG-generated PE bytes, with no external
linker generating the image. Transitional tools may independently inspect or
compare artifacts. Complete archives, DLLs and native debugging remain open
until their named executable evidence passes.

Edition 2026, runtime ABI 1 and existing supported native behavior remain the
compatibility target. Unsupported object records fail explicitly. No failed
native link silently calls the Rust compiler, clang or an external linker.
