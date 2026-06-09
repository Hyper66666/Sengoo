# sggui

Self-drawn lightweight GUI widgets on [`sgplatform`](../sgplatform/). Phase 1
draws labels, buttons, and panels with the shared SDL2 renderer, not OS-native
controls.

## Requirements

- Sengoo toolchain (`sgc`, `sgpm`) from this repository
- [`sgplatform`](../sgplatform/) with SDL2 development libraries for real graphics
- Native linker configured per [docs/sgplatform.md](../../docs/sgplatform.md)

## Build And Test

From this package directory:

```bash
sgpm update
sgpm test --locked
sgpm build --locked
sgpm doc --locked
```

On hosts without SDL2 development files:

```bash
SGPLATFORM_SKIP_GRAPHICS=1 sgpm test --locked
```

Logic tests (`tests/hit_test.sg`) run without opening a visible window. They
cover button bounds hit-testing, constructed mouse events, label value updates,
and panel vertical layout.

## Run The Counter Example

`examples/counter.sg` opens a window with a label and increment button. The label
bar grows as the count increases; no font rendering is included in phase 1. The
example exits automatically after a short smoke interval so automated native
runs do not hang.

```bash
export SENGOO_MODULE_MAP="sgplatform=$(pwd)/../sgplatform/src/lib.sg;sggui=$(pwd)/src/lib.sg"
sgc run --engine native examples/counter.sg
```

Windows PowerShell:

```powershell
$root = (Get-Location).Path
$plat = (Resolve-Path "../sgplatform/src/lib.sg").Path
$env:SENGOO_MODULE_MAP = "sgplatform=$plat;sggui=$root/src/lib.sg"
sgc run --engine native examples\counter.sg
```

## Public API

| Symbol | Role |
| --- | --- |
| `App` | `app_new(title, width, height)`, `begin_frame`, `end_frame`, `poll`, `running`, `close`, `app_delay_ms` |
| `Label` | `label_new`, `label_set_value`, `label_increment`, `draw` |
| `Button` | `button_new`, `hit_test`, `handle_event`, `draw` |
| `Panel` | `panel_new`, `panel_alloc_row`, `draw` |
| `layout_*` | `layout_rect`, `layout_stack_y`, `layout_centered_x`, `layout_inset` |
| `hit_test_*` | `hit_test_point`, `event_mouse_down_at` |

Widgets use `sgplatform` for event polling and rendering only. There is no
separate SDL initialization in `sggui`.

## Deferred

- Text fields
- Menus
- Native dialogs
- Font rendering
- Native-themed widgets

For a richer immediate-mode UI, a future `sggui-imgui` change may wrap Dear
ImGui.
