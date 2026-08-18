# senline_build_identity

This product-specific package contains generated consistency values for the
Senline dogfood worker startup handshake. Run
`scripts/generate-build-identity.ps1` with an independently produced bundle
manifest identity before a release build.

The generator writes both this Sengoo source and the byte-exact external
handshake record from the same validated inputs. Repeating identical inputs is
byte reproducible. The worker's self-report is not a trust root: the host must
verify the complete external bundle manifest and reject any mismatch.
