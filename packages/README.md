# Sengoo graphics packages

Official sgpm packages for windowing, games, and lightweight GUI on a shared
SDL2 platform layer. These live under `packages/` (not `tools/stdlib/`) to
exercise third-party-style packaging, linking, and documentation.

## Packages

| Package | Description | Docs / examples |
| --- | --- | --- |
| [**sgplatform**](sgplatform/) | SDL2-backed window, events, clock, 2D renderer | [Platform guide](../docs/sgplatform.md); `examples/blank_window.sg` |
| [**sggame**](sggame/) | Pygame-inspired game API on `sgplatform` | `examples/snake.sg` |
| [**sggui**](sggui/) | Self-drawn labels, buttons, panels on `sgplatform` | `examples/counter.sg` |

Dependency shape:

```text
sggame --\
          -> sgplatform -> SDL2 (system install)
sggui  --/
```

## Support matrix

Capability status, deferred features, CI skip policy, and archive gate rules:

- [GRAPHICS_SUPPORT_MATRIX.md](GRAPHICS_SUPPORT_MATRIX.md)

## Quick start

1. Install SDL2 development libraries; see [docs/sgplatform.md](../docs/sgplatform.md).
2. Set `SENGOO_SDL2_LIB_DIR` / `SENGOO_NATIVE_LIB_DIRS` if the linker cannot find SDL2.
3. From a package directory: `sgpm update`, then `sgpm test --locked` and
   `sgpm build --locked`.

## OpenSpec inventory (tasks 1.2-1.4)

Documentation for `openspec/changes/sggame-platform-gui` section 1 is complete
in this tree (checkboxes in `tasks.md` are updated separately):

| Task | Status | Where |
| --- | --- | --- |
| **1.2** Support matrix | done | [GRAPHICS_SUPPORT_MATRIX.md](GRAPHICS_SUPPORT_MATRIX.md) |
| **1.3** SDL2 version targets and hosts | done | [docs/sgplatform.md](../docs/sgplatform.md) SDL2 version targets, supported hosts |
| **1.4** Phase-1 scope confirmation | done | Matrix Phase-1 scope; [docs/sgplatform.md](../docs/sgplatform.md) Phase-1 non-goals |

## Related repository docs

- [Examples index - Graphics / packages](../examples/README.md#graphics--packages)
- OpenSpec change: `openspec/changes/sggame-platform-gui/`
