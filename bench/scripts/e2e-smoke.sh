#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
if [[ -n "${SENGOO_ROOT:-}" ]]; then
  PROJECT_ROOT="${SENGOO_ROOT}"
else
  PROJECT_ROOT="$(cd "${BENCH_ROOT}/.." && pwd)"
fi
cd "${PROJECT_ROOT}"

SAMPLE="${BENCH_ROOT}/tests/hello_print.sg"
OUT_DIR="${BENCH_ROOT}/tests/build/e2e"
LLVM_OUT="${OUT_DIR}/hello_print.ll"

mkdir -p "${OUT_DIR}"

if [[ ! -f "${SAMPLE}" ]]; then
  echo "[e2e] sample not found: ${SAMPLE}" >&2
  exit 2
fi

echo "[e2e] checking sample program..."
cargo run -q -p sgc -- check "${SAMPLE}"

echo "[e2e] building llvm ir sample..."
cargo run -q -p sgc -- build "${SAMPLE}" --emit-llvm --output "${LLVM_OUT}"

if [[ ! -f "${LLVM_OUT}" ]]; then
  echo "[e2e] expected LLVM IR output not found: ${LLVM_OUT}" >&2
  exit 1
fi

DAEMON_ADDR="${SENGOO_DAEMON_ADDR:-127.0.0.1:48767}"
FALLBACK_ADDR="127.0.0.1:59999"
DAEMON_OUT="${OUT_DIR}/hello_print_daemon.ll"
FALLBACK_OUT="${OUT_DIR}/hello_print_fallback.ll"

echo "[e2e] starting daemon for happy-path smoke..."
"${PROJECT_ROOT}/target/debug/sgc" daemon --addr "${DAEMON_ADDR}" > /dev/null 2>&1 &
DAEMON_PID=$!
trap 'kill "${DAEMON_PID}" >/dev/null 2>&1 || true' EXIT
sleep 2

echo "[e2e] build via daemon (happy path)..."
HAPPY_LOG="$(cargo run -q -p sgc -- build "${SAMPLE}" --emit-llvm --output "${DAEMON_OUT}" --daemon --daemon-addr "${DAEMON_ADDR}")"
echo "${HAPPY_LOG}"
if ! grep -q "daemon build: request completed by daemon" <<< "${HAPPY_LOG}"; then
  echo "[e2e] daemon happy path output missing success marker" >&2
  exit 1
fi
if [[ ! -f "${DAEMON_OUT}" ]]; then
  echo "[e2e] daemon output missing: ${DAEMON_OUT}" >&2
  exit 1
fi

kill "${DAEMON_PID}" >/dev/null 2>&1 || true
wait "${DAEMON_PID}" >/dev/null 2>&1 || true
trap - EXIT

echo "[e2e] build via daemon fallback path..."
FALLBACK_LOG="$(cargo run -q -p sgc -- build "${SAMPLE}" --emit-llvm --output "${FALLBACK_OUT}" --daemon --daemon-addr "${FALLBACK_ADDR}")"
echo "${FALLBACK_LOG}"
if ! grep -q "daemon fallback (build):" <<< "${FALLBACK_LOG}"; then
  echo "[e2e] daemon fallback output missing fallback marker" >&2
  exit 1
fi
if [[ ! -f "${FALLBACK_OUT}" ]]; then
  echo "[e2e] fallback output missing: ${FALLBACK_OUT}" >&2
  exit 1
fi

echo "[e2e] smoke test passed"
