## Why

Sencode needs local image and document inputs without application-specific native
parsers. Existing `file_read_into` caps the copied prefix but does not prove EOF,
bind a metadata check to the same read handle, or expose a frozen byte digest.
`hash_sha256_file` safely bounds a separate regular-file read, but reopening it
does not bind that digest to the bytes an application sends.

## What Changes

- Add an owned bounded regular-file byte snapshot with same-handle metadata,
  explicit oversize/change errors and a digest of exactly the returned bytes.
- Add pure Sengoo byte classification and PNG/JPEG/PDF header inspection.
- Separate PDF header facts from capabilities requiring a real PDF engine.
- Keep explicit request-size admission as a separately verified consumer dependency.

## Capabilities

- New proposal: `bounded-file-media-inspection`, owned by Sengoo foundation.
- Consumer: Sencode `add-cli-local-attachments`, owned by the application layer.
- Windows snapshot and pure Sengoo header implementation have named local tests;
  responsive reads, full media acceptance and release gates remain open.

## Non-Goals

No provider protocol, attachment database, terminal image renderer, image decode,
resize, OCR, PDF rendering/text extraction, dependency download or installation
is added here. Edition 2026, runtime ABI 1 and existing file APIs remain intact.

## Impact

Native code is limited to regular-file ownership and OS metadata/read operations.
Classification and format parsing belong in Sengoo. Additive imports, runtime
bridge registration and independent tests are included. Working-tree evidence,
compiler SHA and host are in `docs/bounded-file-media.md`; header-only support
cannot close a full media acceptance gate.
