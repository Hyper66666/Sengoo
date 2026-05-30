# Hot-Path Demo: Transaction Risk Pre-Filter

This demo is designed for developers evaluating Sengoo for practical workloads.

It proves one specific point with a reproducible setup:
- For numeric, branch-heavy hot paths, Sengoo native runtime can outperform Python runtime.

## Why this scenario is practical

Many backend systems have a lightweight risk/rule layer before expensive operations:
- payment/fraud pre-check
- abuse throttling
- high-frequency data quality gating

These paths are often simple rules but run millions of times. This is where interpreter overhead matters.

## Files

- `risk_scoring.sg`
  - Sengoo implementation (3,000,000 synthetic transactions).
- `risk_scoring_python.py`
  - Python implementation with the same scoring logic.
- `run_demo.py`
  - One-command benchmark harness:
  - builds Sengoo native binary
  - runs timed samples for Sengoo and Python
  - verifies output parity (`score_sum`, `flagged`)
  - reports runtime ratio and compile break-even estimate

## Run

From repository root:

```bash
python demos/hotpath-risk-scoring/run_demo.py
```

Windows PowerShell:

```powershell
python .\demos\hotpath-risk-scoring\run_demo.py
```

Options:

```bash
python demos/hotpath-risk-scoring/run_demo.py --samples 7 --warmup 2
python demos/hotpath-risk-scoring/run_demo.py --skip-compile
```

## Output

The script prints:
- runtime table (`avg`, `p50`)
- `Python vs Sengoo` speed ratio
- Sengoo compile time
- estimated compile break-even runs
- path to JSON report under `demos/hotpath-risk-scoring/results/`

## How to present this to developers

Use this message:
- "Keep Python for orchestration, move hotspot rule evaluation to Sengoo when loop-heavy latency dominates."
- "This demo includes parity check, so speedup is not from changing business logic."
