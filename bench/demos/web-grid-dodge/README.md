# Web Mini Game Demo: Meteor Dodge

This demo answers a practical question:
- Can Sengoo be used in a web game workflow today?

Short answer:
- **Yes, as a hybrid architecture**.
- Browser rendering/game loop stays in HTML/JS.
- Sengoo is used to generate deterministic level data (`level_generator.sg`).

## Why this is useful

Current Sengoo toolchain in this repo does not target browser WASM directly.
So the pragmatic path is:
- UI/interaction in web tech
- numeric/content generation in Sengoo

That is enough to build playable web demos while reusing Sengoo logic.

## Files

- `level_generator.sg`
  - Sengoo program that generates obstacle sequence.
- `generate_level.py`
  - Compiles and runs Sengoo generator, writes:
  - `level_data.js`
  - `level_data.json`
- `index.html`
  - Playable browser game.
- `serve.py`
  - Optional local static server.

## Quick start

From repository root:

```bash
python demos/web-grid-dodge/generate_level.py
python demos/web-grid-dodge/serve.py --port 8088
```

Then open:
- `http://127.0.0.1:8088/`

If you do not want a server, you can open `index.html` directly, but local static server is recommended.

## Controls

- Left / Right arrows (or A / D)
- Space to start
- Restart button to reset
