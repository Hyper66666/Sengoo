"""Stream a CSV measurement file through bounded Sengoo calibration batches."""
import argparse
import csv
import importlib
import json
import math
import os
from pathlib import Path
import tempfile
import time


def process(input_path, output_path, *, module="sengoo_batch", batch_size=1024,
            scale=1.0, bias=0.0, label="calibration"):
    if not 1 <= batch_size <= 1_000_000:
        raise ValueError("batch size must be between 1 and 1000000")
    if not math.isfinite(scale) or not math.isfinite(bias):
        raise ValueError("scale and bias must be finite")
    batch = importlib.import_module(module)
    label = batch.echo_text(label)
    if not label:
        raise ValueError("job label cannot be empty")
    input_path, output_path = Path(input_path), Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    rows, calls = 0, 0
    pending = []
    # Replace only after the complete input is valid. A bad late row preserves
    # any previous output and removes the temporary, partially written file.
    descriptor, temporary = tempfile.mkstemp(prefix=".sengoo-batch-", suffix=".csv", dir=output_path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="") as output, input_path.open(encoding="utf-8", newline="") as source:
            reader = csv.DictReader(source, strict=True)
            if reader.fieldnames != ["sample_id", "label", "value"]:
                raise ValueError("CSV header must be sample_id,label,value")
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(["sample_id", "label", "value", "calibrated"])

            def flush():
                nonlocal calls
                if not pending:
                    return
                converted = batch.calibrated(label, [row[2] for row in pending], scale, bias)
                if len(converted) != len(pending) or any(not math.isfinite(value) for value in converted):
                    raise ValueError("calibrated output is invalid or outside finite f64 range")
                for row, value in zip(pending, converted):
                    writer.writerow([row[0], row[1], repr(row[2]), repr(value)])
                calls += 1
                pending.clear()

            for row in reader:
                if None in row or any(row.get(key) is None for key in reader.fieldnames):
                    raise ValueError(f"CSV line {reader.line_num}: inconsistent column count")
                try:
                    value = float(row["value"])
                    if not math.isfinite(value):
                        raise ValueError("non-finite number")
                except ValueError as error:
                    raise ValueError(f"CSV line {reader.line_num}: invalid finite value {row['value']!r}") from error
                pending.append((row["sample_id"], row["label"], value))
                rows += 1
                if len(pending) == batch_size:
                    flush()
            flush()
        os.replace(temporary, output_path)
    except BaseException:
        Path(temporary).unlink(missing_ok=True)
        raise
    return {"schema_version": 1, "job": label, "rows": rows, "native_batches": calls,
            "batch_size": batch_size, "scale": scale, "bias": bias,
            "elapsed_seconds": time.perf_counter() - started}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--batch-size", type=int, default=1024)
    parser.add_argument("--scale", type=float, default=1.0)
    parser.add_argument("--bias", type=float, default=0.0)
    parser.add_argument("--label", default="calibration")
    parser.add_argument("--module", default="sengoo_batch")
    args = parser.parse_args(argv)
    try:
        report = process(args.input, args.output, module=args.module, batch_size=args.batch_size,
                         scale=args.scale, bias=args.bias, label=args.label)
    except (ValueError, UnicodeError, csv.Error, OSError) as error:
        parser.exit(2, f"{error}\n")
    # ASCII JSON remains valid Unicode JSON on redirected Windows consoles,
    # whose encoding can differ from the UTF-8 CSV files.
    print(json.dumps(report, ensure_ascii=True, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
