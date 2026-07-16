# Compatibility fixture: v0.2.0-rc.1

Frozen package used as the v0.2 candidate retained surface. CI should copy this
tree outside the checkout and exercise the locked package loop with both the
named candidate toolchain and subsequent patch toolchains.

Avoid newly added APIs that are not part of the frozen candidate claim.
