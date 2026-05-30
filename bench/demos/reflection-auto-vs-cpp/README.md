# Reflection Demo: Sengoo Auto Reflection vs C++ Manual Registry

This demo shows a practical developer-experience contrast:
- Sengoo: `import reflect;` + compile => symbols are auto-discovered in sidecar metadata.
- C++: manual reflection-like registry wiring is required; missing registration causes runtime misses.

## What it demonstrates

Scenario: dynamic rule dispatch for a risk pipeline (`rule_fast_track`, `rule_review`, `rule_block`).

Sengoo path:
- no manual registry list
- build with default `--reflect=auto`
- sidecar includes discovered rule symbols automatically
- dynamic invocation resolves all requested rules

C++ path:
- explicit `registry.reg("name", fn)` calls are required
- demo intentionally omits one registration to show the common failure mode

## Run

From repository root:

```bash
python bench/demos/reflection-auto-vs-cpp/run_demo.py
```

## Output

The script prints:
- side-by-side table (`LOC`, registry entries, requested/missing dynamic rules, decision sum)
- whether Sengoo auto reflection was enabled
- JSON report path under `bench/demos/reflection-auto-vs-cpp/results/`
