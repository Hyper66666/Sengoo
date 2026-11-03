# Python batch example

Requires toolchain **0.2.1-dev.0 or newer** containing `sgc build --emit-shared --python-export`
and `sengoo-python` 0.1.0 to build from source. CPython 3.11–3.14 can consume
the matching platform wheel without Sengoo, Rust or clang.

Copy this entire directory outside the Sengoo checkout, then:

```sh
python -m pip install /path/to/Sengoo/tools/sengoo-python build
sgpm update
python -m sengoo_python check
python -m build --wheel --no-isolation
python -m venv consumer
consumer/bin/python -m pip install --no-index dist/*.whl
consumer/bin/python verify.py
consumer/bin/python observe.py --iterations 20000 --report resources.json
consumer/bin/python benchmark.py --report call-cost.json
```

On Windows use `consumer\Scripts\python.exe`. For a source checkout set
`SENGOO_PYTHON_RUNTIME_MODE=source-development`, `SGPM_SGC` to the built compiler,
and `SENGOO_SGPM` to the built package manager. Installed releases use their
default installed runtime mode. A wheel uses neither environment variable.

The affine function copies numeric arrays in both directions. Strings are UTF-8
and cannot contain NUL. `checked_divide` exposes explicit Result failures as
`SengooError.code`; native traps still terminate the process. All calls retain
the GIL. This example also checks exact large integers, Unicode, empty arrays,
conversion failures and repeated calls. Linux release wheels must be built and
repaired with auditwheel in a pinned manylinux container; local linux wheels
deliberately retain a `linux_*` tag.

`batch_cli.py` is a complete bounded-memory CSV calibration task. It reads the
header `sample_id,label,value`, preserves UTF-8 labels, and sends numeric values
to Sengoo in batches. For example:

```sh
consumer/bin/python batch_cli.py measurements.csv calibrated.csv --batch-size 1024 --scale 2 --bias 1 --label "daily calibration"
```

The command prints a JSON summary. Invalid columns, non-finite numbers and bad
UTF-8 fail with exit status 2. Output is replaced only after every input row
passes; an invalid late row preserves any previous result. Only one batch is
held in memory. `verify.py` checks 10,003 rows with batch sizes 127, 1024 and
4096, quoted/newline/Unicode labels, empty input and late failure cleanup. It
also checks numeric sizes 0, 1, 31, 1024 and 65,536, exact large integers, NUL
and invalid Unicode, owned Result errors and input/output aliases.

`observe.py` records exact live String and opaque collection handle counts at
checkpoints, including successful, Result error and interrupted input conversion
paths. Both counts must return to the post-warmup baseline. Python tracemalloc
samples are descriptive: they exclude native memory and are not a no-leak proof.
Allocation-failure cleanup also has a separate native bridge sanitizer probe.
The example exports `native_live_strings` and `native_live_collections` using
existing runtime counters through ordinary typed i64 functions; no extra
binding ABI or public runtime symbols are added.

`benchmark.py` writes host/interpreter/toolchain identity and repeated median,
minimum and maximum times. It compares an empty Python call, a generated Sengoo
unit call, copied affine batches and Python list processing. The measurements
include input/output conversion and retain the GIL. They describe this machine;
there is no cross-machine speed promise or timing threshold in acceptance.

The wheel workflow builds two independent modules outside the checkout, repairs
their dependencies with delvewheel (Windows), delocate (macOS), or auditwheel
(fixed manylinux image), then runs offline installation and this task on all
four hosts with CPython 3.11–3.14. Original source projects are removed before
consumption; install and run PATH exclude Sengoo/Rust/C compilers. Wheels and
JSON evidence are separate artifacts keyed to the exact candidate SHA.
