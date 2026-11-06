# Explicitly bounded large-buffer Base64

## Problem

The native CLI must send a user-attached original PNG larger than 1 MiB. The
existing Base64 helper rejects input above 65536 bytes. Changing that old
boundary silently would enlarge the resource policy of existing callers.

## Capabilities

Add `encoding_base64_encode_buffer_bounded` accepting an explicit maximum from
1 through 16777216 bytes. Implement chunk orchestration in Sengoo over the
existing native encoder; preserve legacy functions, runtime ABI and edition.

## Boundaries and ownership

The caller borrows initialized input and mutable output only for the call.
The helper retains no input, output or chunk resources. The CLI chooses its
own 8-MiB binary-image ceiling. This change belongs to the network/data owner;
its requirement is not duplicated by the CLI image-reference integration.

## Non-Goals

No decoding, incremental encoder handle, file I/O, automatic allocation of
caller output, old-limit expansion or claim of untested release platforms.
