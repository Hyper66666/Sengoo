#!/usr/bin/env bash
set -euo pipefail

capability="${1:-all}"

run_cmd() {
  local cmd="$1"
  echo " -> ${cmd}"
  eval "${cmd}"
}

run_capability() {
  local target="$1"
  echo "==> ${target}"
  case "${target}" in
    lsp-tooling-sglsp)
      run_cmd "cargo test -p sglsp"
      ;;
    formatter-tooling-sgfmt)
      run_cmd "cargo test -p sgfmt"
      ;;
    package-management-sgpm)
      run_cmd "cargo test -p sgpm"
      ;;
    generics-core)
      run_cmd "cargo test -p sengoo-compiler generic_"
      ;;
    async-concurrency-model)
      run_cmd "cargo test -p sengoo-compiler async_tests"
      run_cmd "cargo test -p sengoo-runtime async_runtime"
      ;;
    macro-system)
      run_cmd "cargo test -p sengoo-compiler macro_tests"
      run_cmd "cargo test -p sengoo-compiler derive_macro_tests"
      ;;
    incremental-compilation-accuracy)
      run_cmd "cargo test -p sgc edit_classifier_detects_"
      run_cmd "cargo test -p sgc interface_change_propagates_"
      ;;
    jit-aot-execution-modes)
      run_cmd "cargo test -p sgc cranelift"
      run_cmd "cargo test -p sgc build_aot_package_flag_parses"
      ;;
    python-interop-embedding)
      run_cmd "PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 cargo test -p sengoo-runtime --features python python_"
      run_cmd "cargo test -p sgc build_python_extension_flag_parses"
      ;;
    docs-and-api-reference)
      run_cmd "cargo test -p sgc doc_command_"
      run_cmd "cargo test -p sgc example_validation_scripts_cover_core_cases"
      ;;
    stdlib-core-collections)
      run_cmd "cargo test -p sengoo-compiler stdlib_surface_"
      run_cmd "cargo test -p sgc stdlib_surface_runtime_"
      run_cmd "cargo test -p sgc stdlib_runtime_exports_"
      ;;
    *)
      echo "Unknown capability: ${target}" >&2
      return 1
      ;;
  esac
}

if [[ "${capability}" == "list" ]]; then
  cat <<'LIST'
lsp-tooling-sglsp
formatter-tooling-sgfmt
package-management-sgpm
generics-core
async-concurrency-model
macro-system
incremental-compilation-accuracy
jit-aot-execution-modes
python-interop-embedding
docs-and-api-reference
stdlib-core-collections
LIST
  exit 0
fi

if [[ "${capability}" == "all" ]]; then
  while IFS= read -r target; do
    [[ -z "${target}" ]] && continue
    run_capability "${target}"
  done <<'LIST'
lsp-tooling-sglsp
formatter-tooling-sgfmt
package-management-sgpm
generics-core
async-concurrency-model
macro-system
incremental-compilation-accuracy
jit-aot-execution-modes
python-interop-embedding
docs-and-api-reference
stdlib-core-collections
LIST
else
  run_capability "${capability}"
fi

echo "OpenSpec acceptance suites completed."
