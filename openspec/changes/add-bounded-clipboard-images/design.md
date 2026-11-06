## Ownership and boundary

The runtime only reads clipboard formats and converts Windows image data.
Sengoo chooses when to request paste, which original paths to accept and where
to create a temporary cache when an image has no original file. File results
are a JSON array string obtained while holding one clipboard snapshot; separate
count/index calls would race with clipboard changes.

All String outputs own their UTF-8 bytes. The runtime never frees clipboard
memory. DIB and PNG data are copied before releasing the clipboard. A CF_BITMAP
handle remains clipboard-owned, so its clipboard lock lasts through disposal
of the GDI+ image. COM streams, images, global-memory copies, files and DLLs
are released on every success/failure path.

## Bounds and compatibility

Text is limited to 64KiB UTF-8. Dropped files are limited to 32 original paths,
32KiB per UTF-8 path and a 1MiB JSON output. Image input and encoded PNG output
are each limited to 64MiB, with caller-selected smaller output limits. Sides
are at most 8192 pixels and total pixel count at most 16Mi. Header dimensions
are validated before PNG/DIB decode; CF_BITMAP dimensions are queried first.

PNG is encoded before the destination is created. `CREATE_NEW` prevents
overwrite, and failed writes remove only the newly created file. NUL and
invalid UTF-8 paths fail before clipboard access. Existing interfaces and
edition 2026 remain unchanged; non-Windows implementations report unsupported.

## Evidence boundaries

The deterministic fixture substitutes clipboard state only. UTF conversion,
CF_HDROP parsing, GDI+, COM streams and file writes use real OS APIs. The
separate private-window-station probe provides a real clipboard test only when
the OS permits creation of an isolated station and desktop. Denial is recorded
as unavailable and cannot be counted as an integration pass.
