#!/usr/bin/env bash
set -euo pipefail

MODE="hard"
SAMPLE=""
BASELINE_PROFILE="bench/frontend-memory-baseline.json"
RUN_BENCH=0
SKIP_ABSOLUTE=0
P0_EVIDENCE_ONLY=0

usage() {
  cat <<'EOF'
Usage: frontend-1000k-perf-gate.sh [--mode soft|hard] [--sample PATH]
       [--baseline-profile PATH] [--run-bench] [--skip-absolute-targets]
       [--p0-evidence-only]
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      MODE="$2"
      shift 2
      ;;
    --sample)
      SAMPLE="$2"
      shift 2
      ;;
    --baseline-profile)
      BASELINE_PROFILE="$2"
      shift 2
      ;;
    --run-bench)
      RUN_BENCH=1
      shift
      ;;
    --skip-absolute-targets)
      SKIP_ABSOLUTE=1
      shift
      ;;
    --p0-evidence-only)
      P0_EVIDENCE_ONLY=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ "$RUN_BENCH" -eq 1 ]]; then
  echo "==> advanced_pipeline_bench.py"
  BENCH_ARGS=("$ROOT/bench/advanced_pipeline_bench.py")
  if [[ "$P0_EVIDENCE_ONLY" -eq 1 ]]; then
    BENCH_ARGS+=(--p0-evidence-only)
  fi
  python "${BENCH_ARGS[@]}"
fi

if [[ -z "$SAMPLE" ]]; then
  SAMPLE="$(ls -1t "$ROOT/bench/results/"*-advanced-pipeline.json 2>/dev/null | head -n 1 || true)"
fi

if [[ -z "$SAMPLE" || ! -f "$SAMPLE" ]]; then
  echo "no advanced pipeline report found; pass --sample or --run-bench" >&2
  exit 2
fi

if [[ "$BASELINE_PROFILE" != /* ]]; then
  BASELINE_PROFILE="$ROOT/$BASELINE_PROFILE"
fi

GATE_ARGS=(
  "$ROOT/bench/scripts/advanced-kpi-gate.py"
  --mode "$MODE"
  --sample "$SAMPLE"
  --baseline-profile "$BASELINE_PROFILE"
)
if [[ "$SKIP_ABSOLUTE" -eq 1 ]]; then
  GATE_ARGS+=(--skip-absolute-targets)
fi
if [[ "$P0_EVIDENCE_ONLY" -eq 1 ]]; then
  GATE_ARGS+=(--p0-evidence-only)
fi

echo "==> advanced-kpi-gate mode=$MODE"
python "${GATE_ARGS[@]}"
