#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
stdlib="$root/tools/stdlib"
build="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/sengoo-runtime-sanitizers-$$"
mkdir -p "$build"
trap 'rm -rf "$build"' EXIT

compile_flags=(
  -O1
  -g
  -fno-omit-frame-pointer
  -fsanitize=address,undefined
  -I "$stdlib"
)
sources=(
  runtime.c
  runtime_breadth.c
  runtime_collections.c
  runtime_json.c
  runtime_process.c
  runtime_string.c
)
objects=()

for source in "${sources[@]}"; do
  object="$build/${source%.c}.o"
  clang "${compile_flags[@]}" -c "$stdlib/$source" -o "$object"
  objects+=("$object")
done

probe="$build/runtime-sanitizer-probe.o"
clang "${compile_flags[@]}" -c \
  "$stdlib/tests/runtime_sanitizer_probe.c" -o "$probe"

binary="$build/runtime-sanitizer-probe"
clang -fsanitize=address,undefined -fno-omit-frame-pointer \
  "$probe" "${objects[@]}" -pthread -ldl -lm -o "$binary"

ASAN_OPTIONS="detect_leaks=1:halt_on_error=1:strict_string_checks=1" \
UBSAN_OPTIONS="halt_on_error=1:print_stacktrace=1" \
  "$binary"
