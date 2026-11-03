"""Measure synchronous GIL-held call and copy costs on this exact machine."""
import argparse
import gc
import importlib
import json
import os
from pathlib import Path
import platform
import statistics
import time


def benchmark(module_name="sengoo_batch", *, sizes=(0, 1, 32, 1024, 65536), samples=7, target_elements=131072):
    if samples < 3 or target_elements < 1 or any(size < 0 for size in sizes):
        raise ValueError("at least 3 samples, positive target elements, and nonnegative sizes required")
    module = importlib.import_module(module_name)
    descriptor = json.loads((Path(module.__file__).parent / "_bindings.json").read_text(encoding="utf-8"))

    def measure(name, operation, repetitions, size=None):
        operation()
        timings = []
        for _ in range(samples):
            started = time.perf_counter_ns()
            for _ in range(repetitions):
                operation()
            timings.append((time.perf_counter_ns() - started) / repetitions)
        timings.sort()
        median = statistics.median(timings)
        return {"operation": name, "elements": size, "repetitions_per_sample": repetitions,
                "samples": samples, "median_ns_per_call": median,
                "min_ns_per_call": min(timings), "max_ns_per_call": max(timings),
                "median_ns_per_element": median / size if size else None}

    gc.collect()
    records = [measure("python_no_op", lambda: None, 20000), measure("sengoo_no_op", module.no_op, 20000)]
    for size in sizes:
        values = [(index % 129 - 64) * 0.25 for index in range(size)]
        expected = [value * 2.0 + 1.0 for value in values]
        assert module.affine(values, 2.0, 1.0) == expected
        repetitions = max(1, min(2000, target_elements // max(1, size)))
        records.append(measure("sengoo_affine_with_input_output_copies", lambda: module.affine(values, 2.0, 1.0), repetitions, size))
        records.append(measure("python_affine_list", lambda: [value * 2.0 + 1.0 for value in values], repetitions, size))
    return {"schema_version": 1, "candidate_sha": os.environ.get("CANDIDATE_SHA", os.environ.get("GITHUB_SHA")),
            "python": platform.python_version(), "implementation": platform.python_implementation(),
            "platform": platform.platform(), "machine": platform.machine(), "processor": platform.processor(),
            "toolchain": descriptor["toolchain_version"], "target": descriptor["target"], "binding_identity": descriptor["identity"],
            "clock": "perf_counter_ns", "records": records,
            "scope": "Descriptive host-local measurements, no speed or regression threshold. Native calls retain the GIL and copy vectors in both directions. Compare batches to scalar calls; this is not a cross-machine performance promise."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module", default="sengoo_batch")
    parser.add_argument("--sizes", nargs="+", type=int, default=[0, 1, 32, 1024, 65536])
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = benchmark(args.module, sizes=args.sizes, samples=args.samples)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Call-cost report written: {args.report}")
