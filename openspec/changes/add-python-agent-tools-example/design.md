# Python Agent tool consumer

The library exports validation_report, require_valid_tool and canonical_arguments
through the existing compiler-generated typed bridge and ctypes wheel wrapper.
Signatures use borrowed string inputs and owned String/bool Result outputs;
resource counters use scalar i64. No owning Agent, process, callback or custom
object is claimed ABI-compatible.

Each input is parsed with explicit JSON byte/node bounds, then recursively
checked for NUL in owned keys and string values before any C-string view.
sgschema validates its supported explicit subset; reports preserve stage,
pointer and keyword. Canonical JSON retains exact integer decimal representation.
Schema validation grants neither permission nor tool execution.

The build frontend copies the manifest/relative lock/SG source plus actual
sgschema package outside the checkout. The unchanged PEP517 backend invokes
locked shared-library compilation and packages the native runtime and typed
binding description into a platform wheel. All copied build sources are removed
before a new venv installs that wheel using --no-index and --no-deps. The install
and test environment remove SENGOO_/SGPM_/PYTHONPATH/compiler lookup paths.

The independent consumer checks Unicode, exact i64, schema and instance errors,
literal and escaped NUL, capacity failure and repeated native resource counts.
Windows is the initial local gate; full CPython/platform release coverage is a
separate integration responsibility. The ABI/backend are existing infrastructure;
any newly found compiler/bridge defect must retain a minimal regression rather
than being hidden by manual native memory management.
