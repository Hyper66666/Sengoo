# Admit bounded native COFF leaf inputs

## Why
Actual source-locked C provider objects fail before symbol resolution because the SG linker rejects uninitialized sections, standard section/file auxiliary records and LLVM address-significance metadata. Supporting a bounded real leaf profile is a prerequisite to using verified OS/runtime shims in a self-linked compiler. Full static CRT inputs additionally require COMDAT, directives, initialization and more than 128 extracted objects and are not covered here.

## What Changes
Promote the bounded parser/writer profile for nonzero zero-initialized BSS, ordinary section-definition and file auxiliary records, and validated discardable .llvm_addrsig metadata after its dedicated source-qualified gate. Preserve original ordinary symbol indices through explicit section and symbol maps. Empty ordinary sections contribute no PE bytes; malformed relocations on them still reject. Keep COMDAT, weak/common, directives, DLL/TLS/resources and unsupported auxiliaries rejected.

## Impact
This child uniquely owns this bounded COFF input profile. It does not authorize native providers, replace SDK provenance or complete M5/selfhosting.
