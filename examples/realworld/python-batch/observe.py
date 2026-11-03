"""Exact runtime-handle checkpoints, plus descriptive Python memory samples."""
import argparse
import gc
import importlib
import json
import os
from pathlib import Path
import platform
import time
import tracemalloc


def observe(module_names, *, iterations=20000, checkpoint=2000, payload_size=64):
    if iterations < 1 or checkpoint < 1 or not 0 <= payload_size <= 65536:
        raise ValueError("iterations/checkpoint must be positive; payload size must be 0..65536")
    modules = [importlib.import_module(name) for name in module_names]
    values = [(index % 17 - 8) * 0.5 for index in range(payload_size)]
    expected = [value * 2.0 + 1.0 for value in values]

    def counts():
        return {module.__name__: {"strings": module.native_live_strings(),
                                 "opaque_collections": module.native_live_collections()} for module in modules}

    for module in modules:
        assert module.calibrated("warmup", values, 2.0, 1.0) == expected
        assert module.echo_text("warmup 世界") == "warmup 世界"
    baseline = counts()
    gc.collect()
    tracemalloc.start()
    started = time.perf_counter()
    samples = []
    for iteration in range(1, iterations + 1):
        for module in modules:
            assert module.calibrated("循环校准 🐍", values, 2.0, 1.0) == expected
            assert module.owned_text("repeat 你好") == "repeat 你好"
            assert module.echo_text("borrowed 😀") == "borrowed 😀"
            try:
                module.calibrated("", values, 2.0, 1.0)
            except module.SengooError as error:
                assert error.code == 22
            else:
                raise AssertionError("expected owned Result error")
            try:
                module.calibrated("valid earlier buffers", values, 2.0, "invalid last argument")
            except TypeError:
                pass
            else:
                raise AssertionError("partial input conversion was accepted")
        if iteration % checkpoint == 0 or iteration == iterations:
            gc.collect()
            native = counts()
            assert native == baseline, f"native handles did not return to baseline: {native} != {baseline}"
            current, peak = tracemalloc.get_traced_memory()
            samples.append({"iteration": iteration, "native_handles": native,
                            "python_traced_current_bytes": current, "python_traced_peak_bytes": peak})
    elapsed = time.perf_counter() - started
    tracemalloc.stop()
    return {"schema_version": 1, "candidate_sha": os.environ.get("CANDIDATE_SHA", os.environ.get("GITHUB_SHA")),
            "python": platform.python_version(), "platform": platform.platform(),
            "modules": module_names, "iterations_per_module": iterations, "payload_size": payload_size,
            "measured_calls_per_iteration_per_module": 5, "elapsed_seconds": elapsed,
            "native_baseline": baseline, "checkpoints": samples,
            "scope": "Exact String/opaque-handle counts; Python tracemalloc is descriptive and excludes native heap. This is not a general no-leak proof; bridge allocation failures also require sanitizer evidence."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module", action="append", dest="modules")
    parser.add_argument("--iterations", type=int, default=20000)
    parser.add_argument("--checkpoint", type=int, default=2000)
    parser.add_argument("--payload-size", type=int, default=64)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = observe(args.modules or ["sengoo_batch"], iterations=args.iterations,
                     checkpoint=args.checkpoint, payload_size=args.payload_size)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Resource observation passed: {args.report}")
