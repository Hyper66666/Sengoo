# Read and validate package archives in SG

Implement the tar and archive-path algorithms needed by the package-manager
port. The first executable boundary is a caller-owned, read-only tar scan; it
does not create, replace or publish files. Existing sgpm produces GNU tar and
reads it through tar 0.4.46 and first-member flate2 GzDecoder.

The child owns tar framing, GNU/local PAX metadata, effective-content budgets,
portable safe paths and entry tables. Minimal C adapters only access a byte or
call an OS character-comparison primitive. Raw DEFLATE/gzip retain their
separate owner and qualification. Package staging, identity/manifest checks,
checksum draining and 256 MiB streaming gzip integration remain required
subsequent work, not consequences of a successful tar scan.
