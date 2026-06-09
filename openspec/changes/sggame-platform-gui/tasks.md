## 1. OpenSpec And Inventory

- [x] 1.1 Run `openspec validate sggame-platform-gui --strict`.
- [x] 1.2 Add `packages/GRAPHICS_SUPPORT_MATRIX.md` with capability/status/host
  scope/proof/diagnostic/dependency columns for windowing, input, audio, fonts,
  native GUI, SDL_image, static SDL2 vendoring, and CI skips.
- [x] 1.3 Record SDL2 version targets and supported hosts (Windows, Linux) in
  design-linked docs.
- [x] 1.4 Confirm phase-1 scope decisions in package docs: no SDL_image, no
  audio/fonts, no static SDL2 vendoring, no adventure shell archive gate.

## 2. Native Link Prerequisite

- [x] 2.1 Add/verify compiler interface extraction for `#[link(name = "...")]`
  metadata from imported package modules.
- [x] 2.2 Forward collected native link libraries to host-native `sgc build`,
  `sgc run --engine native`, and `sgpm build`.
- [x] 2.3 Add diagnostics for missing SDL2 libraries that name SDL2 and point to
  `docs/sgplatform.md`.
- [x] 2.4 Add tests proving a package FFI `#[link(name = "sample")]` reaches
  native linker arguments without requiring graphics hardware.
- [x] 2.5 Document Windows/Linux library search setup, including
  `SENGOO_SDL2_LIB_DIR` or the chosen equivalent if a custom env var is added.

## 3. sgplatform Package

- [x] 3.1 Create `packages/sgplatform/` with `Sengoo.toml`, `src/`, `tests/`,
  `examples/`, and README.
- [x] 3.2 Add SDL2 FFI bindings for init/quit, window, renderer, event poll,
  clock/tick, clear, rect/line draw, and the phase-1 input subset.
- [x] 3.3 Implement stable public API names from `design.md`: `Platform`,
  `Window`, `Renderer`, `Rect`, `Color`, `Event`, event/key constants, and
  status constants.
- [x] 3.4 Wrap handles in structs with documented `close()`/`destroy()` cleanup
  and invalid-handle errors.
- [x] 3.5 Add `examples/blank_window.sg` and document build/run commands.
- [x] 3.6 Add smoke test: init -> pump -> present -> quit with accepted headless
  skip policy.
- [x] 3.7 Document SDL2 install and link setup for Windows and Linux.

## 4. sggame Package

- [x] 4.1 Create `packages/sggame/` depending on `sgplatform` via sgpm path
  dependency.
- [x] 4.2 Implement `sggame_init`/`sggame_quit` and modules `display`, `event`, `time`,
  `draw`, `image`, `input` for the phase-1 subset.
- [x] 4.3 Make `sggame.image.load` explicitly documented as
  `STATUS_UNSUPPORTED` or rectangle/texture-only fallback unless SDL_image is
  covered by a future OpenSpec.
- [x] 4.4 Add snake example with keyboard control, clock, rectangle drawing,
  collision, and quit.
- [x] 4.5 Add logic tests for snake movement/collision without requiring GUI
  interaction.
- [x] 4.6 Run `sgpm doc` and ensure public module docs generate for `sggame`.

## 5. sggui Package

- [x] 5.1 Create `packages/sggui/` depending on `sgplatform`.
- [x] 5.2 Implement public API names from `design.md`: `App`, `Label`,
  `Button`, `Panel`, frame methods, button hit-testing, and event handling.
- [x] 5.3 Add counter example (button increments label).
- [x] 5.4 Add hit-test and constructed-event unit tests for button activation
  and counter state update without relying only on manual clicking.
- [x] 5.5 Document deferred widgets (text field, menu, native dialogs) in README
  and support matrix.

## 6. Tooling, Docs, And CI

- [x] 6.1 Wire `packages/sgplatform`, `packages/sggame`, and `packages/sggui`
  into repository docs (`examples/README.md` or `packages/README.md`).
- [x] 6.2 Add CI job or extend existing workflow with graphics package smoke
  tests and documented platform skips.
- [x] 6.3 Ensure CI cannot archive as all-skip: at least one reference host or
  local release-verification log must show a real `blank_window` smoke pass.
- [x] 6.4 Add `sglsp` completion/diagnostic smoke for `import sggame` and
  `import sggui` if reduced fixtures are practical.
- [x] 6.5 Ensure `sgpm update`, `sgpm test --locked`, and
  `sgpm build --locked` work per graphics package after SDL2 is available.
- [x] 6.6 Confirm graphics package manifests use only existing `Sengoo.toml`
  schema fields; document any manifest-native-link needs as follow-up.

## 7. Verification

- [ ] 7.1 Run `cargo fmt --check`.
- [x] 7.2 Run native-link metadata tests from task 2.4.
- [x] 7.3 Run `cargo test -p sgc` and package-specific tests for graphics
  packages.
- [x] 7.4 Run `sgpm test --locked` and `sgpm build --locked` in each graphics
  package on a host with SDL2 installed.
- [x] 7.5 Manually run blank window, snake, and counter examples on at least one
  desktop host.
- [x] 7.6 Run `openspec validate sggame-platform-gui --strict`.
- [x] 7.7 Run `openspec validate --all --strict`.

## Done Definition

- [x] `sgplatform`, `sggame`, and `sggui` exist as sgpm packages with README,
  tests, and examples.
- [x] Existing `#[link(name = "...")]` FFI metadata reaches host-native link
  commands used by `sgc` and `sgpm`.
- [x] `sgplatform` exposes the phase-1 platform API and returns stable
  diagnostics for missing SDL2 / invalid handles.
- [x] `sggame` exposes the pygame-inspired subset; game library name is
  `sggame`, not `sgpygame`.
- [x] `sggui` ships a working counter demo on the shared platform layer.
- [x] Graphics support matrix and native dependency docs are published.
- [x] At least one reference host has a real graphics smoke pass; other host
  skips are documented with stable reasons.

## Archive Gate

- [ ] All tasks above checked or explicitly marked as accepted platform/doc
  skips in the support matrix.
- [x] The archive evidence includes at least one non-skipped `blank_window`
  smoke run.
- [x] `openspec validate --all --strict` passes.

## Verification Notes

- 2026-06-08 local Windows host: `SGPLATFORM_SKIP_GRAPHICS=1` package loop
  passed for `sgplatform`, `sggame`, and `sggui` (`sgpm update --check`,
  `sgpm test --locked`, `sgpm build --locked`, `sgpm doc --locked`).
- 2026-06-08 local Windows host: real SDL2 smoke completed with temporary
  `SDL2-devel-2.32.8-VC.zip` from libsdl.org. `sgpm test --locked` and
  `sgpm build --locked` passed for `sgplatform`, `sggame`, and `sggui` without
  `SGPLATFORM_SKIP_GRAPHICS`; `blank_window`, `snake`, and `counter` examples
  passed `sgc run --engine native --force-rebuild` with exit code 0.
- 2026-06-08 local Windows host: `sgpm fmt --check --locked` and
  `sgpm doc --locked` passed for `sgplatform`, `sggame`, and `sggui`; real SDL2
  package test/build/example smoke was rerun after formatting.
- 2026-06-08 local Windows host: `cargo test -p sgpm` and `cargo test -p sgc`
  passed after the stdlib HTTP fallback fix; `https://127.0.0.1/` now maps to a
  stable TLS status category instead of `STATUS_UNSUPPORTED()`.
- 2026-06-08 local Windows host: targeted rustfmt checks passed for touched
  Rust files, but global `cargo fmt --check` remains unchecked because unrelated
  pre-existing workspace changes require formatting.
