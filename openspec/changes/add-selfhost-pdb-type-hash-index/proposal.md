# Add deterministic PDB type hash indexes

## Why
The current nonempty TPI stream advertises no hash stream. Actual Windows DbgHelp fails to load it while LLVM inspection and typed SG LLDB tests accept it. A private SG writer with unchanged original type records and a valid hash/index stream makes real DbgHelp name lookup work.

## What Changes
Emit bounded, deterministic TPI hashes and index-offset entries in single and independent-module PDB writers. Domain-separate the updated producer identity to prevent stale GUID/age cache matches. Add independent format and actual DbgHelp checks and retain all existing typed SG and mixed-object debug boundaries.

## Impact
This child uniquely owns the PDB type hash/index requirement. PDB/RSDS identity changes for the updated producer; original CodeView type bytes and IDs remain unchanged. LLDB19 public-only C name lookup and complete M5 remain open.
