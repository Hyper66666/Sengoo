# v0.1.0-rc.1 compatibility fixture

This package is compiled by both the published `v0.1.0-rc.1` toolchain and the
current toolchain in `.github/workflows/compatibility.yml`. The workflow uses
a copy outside the source checkout and runs locked check, test, format, doc,
and build commands without rewriting `Sengoo.lock`.
