"""Verify an installed wheel and a real CSV task, without a build toolchain."""
import argparse
import csv
import importlib
import math
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile


def verify(module="sengoo_batch"):
    batch = importlib.import_module(module)
    baseline = (batch.native_live_strings(), batch.native_live_collections())
    for length in (0, 1, 31, 1024, 65536):
        values = [(index % 129 - 64) * 0.25 for index in range(length)]
        expected = [value * 2.0 + 1.0 for value in values]
        assert batch.affine(values, 2.0, 1.0) == expected
        assert batch.calibrated("传感器校准 🐍", values, 2.0, 1.0) == expected
        integers = [(index % 19) - 9 for index in range(length)]
        assert batch.echo_i64(integers) == integers
        assert batch.sum_i64(integers) == sum(integers)
        assert (batch.native_live_strings(), batch.native_live_collections()) == baseline
    assert batch.sum_i64([1, 2, -3, (1 << 53) + 1]) == (1 << 53) + 1
    assert batch.echo_i64([-(1 << 63), (1 << 63) - 1]) == [-(1 << 63), (1 << 63) - 1]
    for text in ("", "Sengoo 你好 🐍", "e\u0301 / é", "line\nquoted \"text\"", "世界" * 4096):
        assert batch.echo_text(text) == text
        assert batch.owned_text(text) == text
    assert batch.checked_divide(value=42, divisor=2) == 21
    assert batch.negate(True) is False
    assert batch.no_op() is None
    assert batch.completed() is None
    assert batch.accepts_unit(None) is True
    for choose_left in (False, True):
        assert batch.choose_text("first 世界", "second 🐍", choose_left) == ("first 世界" if choose_left else "second 🐍")
        assert batch.choose_text("equal input", "equal input", choose_left) == "equal input"
    assert batch.checked_text("returned input 🐍", False) == "returned input 🐍"
    assert batch.consume_owned("discarded input", [1, 2, 3]) is None
    assert (batch.native_live_strings(), batch.native_live_collections()) == baseline

    def broken_sequence():
        yield 1.0
        yield 2.0
        raise ValueError("producer failed after two values")

    for function, arguments, error_type in [
        (batch.sum_i64, ([1, 2, 1 << 63],), OverflowError),
        (batch.sum_i64, ([1, 2, "invalid"],), TypeError),
        (batch.checked_divide, (True, 2), TypeError),
        (batch.echo_text, ("contains\0NUL",), ValueError),
        (batch.echo_text, ("\ud800",), UnicodeError),
        (batch.negate, (1,), TypeError),
        (batch.accepts_unit, (0,), TypeError),
        # Earlier Python buffers exist when the final scalar conversion fails;
        # no native invocation or allocation may happen in this case.
        (batch.calibrated, ("先转换字符串", [1.0, 2.0], 2.0, "invalid"), TypeError),
        (batch.calibrated, ("先转换字符串", broken_sequence(), 2.0, 0.0), ValueError),
        (batch.affine, ([1.0, 2.0, 10 ** 400], 1.0, 0.0), OverflowError),
    ]:
        try:
            function(*arguments)
        except error_type:
            pass
        else:
            raise AssertionError(f"{function.__name__} accepted invalid input")
        assert (batch.native_live_strings(), batch.native_live_collections()) == baseline

    for function, arguments, code in [
        (batch.checked_divide, (1, 0), 22),
        (batch.calibrated, ("", [1.0], 1.0, 0.0), 22),
        (batch.calibrated, ("invalid scale", [1.0], math.nan, 0.0), 34),
        (batch.checked_text, ("owned error placeholder", True), 22),
    ]:
        try:
            function(*arguments)
        except batch.SengooError as error:
            assert error.code == code
        else:
            raise AssertionError("Result failure did not raise SengooError")
        assert (batch.native_live_strings(), batch.native_live_collections()) == baseline
    verify_csv_task(module)
    assert (batch.native_live_strings(), batch.native_live_collections()) == baseline
    print(f"Python batch verification passed: {module}")


def verify_csv_task(module):
    process = runpy.run_path(str(Path(__file__).with_name("batch_cli.py")))["process"]
    with tempfile.TemporaryDirectory(prefix="sengoo-csv-consumer-") as temporary:
        root = Path(temporary)
        source, output = root / "测量输入.csv", root / "校准输出.csv"
        labels = ["北京 🐍", "quoted \"sensor\"", "line one\nline two", "e\u0301", ""]
        with source.open("w", encoding="utf-8", newline="") as stream:
            writer = csv.writer(stream, lineterminator="\r\n")
            writer.writerow(["sample_id", "label", "value"])
            for index in range(10003):
                writer.writerow([f"样本-{index}", labels[index % len(labels)], (index % 129 - 64) * 0.25])
        for batch_size in (127, 1024, 4096):
            report = process(source, output, module=module, batch_size=batch_size, scale=2.0, bias=1.0, label="每日校准")
            assert report["rows"] == 10003
            assert report["native_batches"] == math.ceil(10003 / batch_size)
            with output.open(encoding="utf-8", newline="") as stream:
                rows = list(csv.DictReader(stream))
            for index, row in enumerate(rows):
                assert row["sample_id"] == f"样本-{index}"
                assert row["label"] == labels[index % len(labels)]
                assert float(row["calibrated"]) == (index % 129 - 64) * 0.5 + 1.0
        original = output.read_bytes()
        command = [sys.executable, "-I", str(Path(__file__).with_name("batch_cli.py")),
                   str(source), str(output), "--module", module, "--batch-size", "1024",
                   "--scale", "2", "--bias", "1", "--label", "命令行校准 🐍"]
        completed = subprocess.run(command, text=True, encoding="utf-8", capture_output=True)
        assert completed.returncode == 0, completed.stderr
        assert json.loads(completed.stdout)["rows"] == 10003
        assert output.read_bytes() == original
        with source.open("a", encoding="utf-8", newline="") as stream:
            csv.writer(stream).writerow(["late-invalid", "错误", "not-a-number"])
        try:
            process(source, output, module=module, batch_size=127)
        except ValueError as error:
            assert "CSV line" in str(error)
        else:
            raise AssertionError("invalid late row was accepted")
        assert output.read_bytes() == original
        failed = subprocess.run(command, text=True, encoding="utf-8", capture_output=True)
        assert failed.returncode == 2 and "CSV line" in failed.stderr
        assert output.read_bytes() == original
        assert not list(root.glob(".sengoo-batch-*"))
        source.write_text("sample_id,label,value\n", encoding="utf-8")
        assert process(source, output, module=module)["rows"] == 0
        assert output.read_text(encoding="utf-8") == "sample_id,label,value,calibrated\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module", default="sengoo_batch")
    verify(parser.parse_args().module)
