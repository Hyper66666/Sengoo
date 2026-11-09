"""Verify exact SG decimal-to-IEEE conversion with C and Python oracles."""
from __future__ import annotations

import argparse
import datetime as dt
from decimal import Decimal, localcontext
from fractions import Fraction
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import random
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def signed64(value):
    return value - (1 << 64) if value >= 1 << 63 else value


def clean(text):
    return text.removesuffix("f32").removesuffix("f64").replace("_", "")


def exact_ieee(text, bits):
    """Independent high-level Fraction oracle for nearest-even target storage."""
    decimal = Decimal(clean(text))
    sign = int(decimal.is_signed()) << (bits - 1)
    value = abs(Fraction(decimal))
    if not value:
        return signed64(sign) if bits == 64 else sign
    precision, bias = (53, 1023) if bits == 64 else (24, 127)
    # Determine power by exact Fraction comparisons, not target arithmetic.
    exponent = value.numerator.bit_length() - value.denominator.bit_length()
    power = Fraction(1 << exponent) if exponent >= 0 else Fraction(1, 1 << -exponent)
    if value < power:
        exponent -= 1
    if exponent > bias:
        pattern = sign | (((1 << (11 if bits == 64 else 8)) - 1) << (precision - 1))
    else:
        scale = precision - 1 - max(exponent, 1 - bias)
        scaled = value * (1 << scale) if scale >= 0 else value / (1 << -scale)
        quotient, remainder = divmod(scaled.numerator, scaled.denominator)
        if remainder * 2 > scaled.denominator or remainder * 2 == scaled.denominator and quotient % 2:
            quotient += 1
        if exponent < 1 - bias:
            pattern = sign | quotient
        else:
            if quotient == 1 << precision:
                exponent += 1
                quotient //= 2
            if exponent > bias:
                pattern = sign | (((1 << (11 if bits == 64 else 8)) - 1) << (precision - 1))
            else:
                pattern = sign | ((exponent + bias) << (precision - 1)) | (quotient - (1 << (precision - 1)))
    return signed64(pattern) if bits == 64 else pattern


def decimal_fraction(value):
    with localcontext() as context:
        context.prec = 6000
        text = format(Decimal(value.numerator) / Decimal(value.denominator), "f")
    return text if "." in text else text + ".0"


def fixtures():
    cases = []

    def add(text, bits):
        cases.append({"bits": bits, "literal": text})

    for bits in (32, 64):
        for text in ("0.0", "-0.0", "+0.0e999999", "1.0", "-1.0", "42.0", "0.1", "1_234.5_00e-0_2", "1.0e999999", "-1.0e-999999"):
            add(text, bits)
        precision, bias = (53, 1023) if bits == 64 else (24, 127)
        for exponent in (-bias - precision - 1, 1 - bias, -1, 0, 1, bias):
            for significand in (0, 1, 2, 3, (1 << (precision - 1)) - 1, 1 << (precision - 1), (1 << precision) - 1):
                power = exponent - (precision - 1)
                value = Fraction(significand) * (Fraction(1 << power) if power >= 0 else Fraction(1, 1 << -power))
                add(decimal_fraction(value), bits)
                # Decimal exact halfway values and one decimal quantum either side.
                half_power = power - 1
                step = Fraction(1 << half_power) if half_power >= 0 else Fraction(1, 1 << -half_power)
                middle = decimal_fraction(value + step)
                add(middle, bits)
                decimal_digits = len(middle.partition(".")[2])
                epsilon = Fraction(1, 10 ** (decimal_digits + 3))
                add(decimal_fraction(value + step + epsilon), bits)
                if value + step > epsilon:
                    add(decimal_fraction(value + step - epsilon), bits)
        # Long significands and cancellation remain inside the documented cap.
        add("1." + "2345678901" * 300 + "e-12", bits)
        add("0." + "0" * 1100 + "1e1100", bits)
        add("1." + "0" * 4000 + "1e-20", bits)
    rng = random.Random(0x53474C4954455241)
    for bits in (32, 64):
        for _ in range(1500):
            length = rng.randint(2, 80)
            digits = str(rng.randint(1, 9)) + "".join(str(rng.randrange(10)) for _ in range(length - 1))
            split = rng.randrange(1, length)
            exponent = rng.randint(-380, 340) if bits == 64 else rng.randint(-65, 55)
            text = ("-" if rng.randrange(2) else "") + digits[:split] + "." + digits[split:] + "e" + str(exponent)
            if rng.randrange(3) == 0:
                text += "f" + str(bits)
            add(text, bits)
    unique = {(case["bits"], case["literal"]): case for case in cases}
    return list(unique.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--reference", type=Path, default=ROOT / "target/selfhost/seed-reference/release/sgc.exe")
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m4-float-literal-final")
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must remain inside target/")
    output.mkdir(parents=True, exist_ok=True)
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter", "reference")}
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text(encoding="utf-8"))
    reference_lock = json.loads((ROOT / "selfhost/reference-baseline.lock").read_text(encoding="utf-8"))
    inputs = [Path(__file__).resolve(), ROOT / "selfhost/bootstrap.lock", ROOT / "selfhost/reference-baseline.lock", ROOT / "selfhost/src/core.sg", ROOT / "selfhost/src/float_literal.sg", ROOT / "selfhost/tests/float_literal.sg", ROOT / "selfhost/tests/float_literal_probe.c"]
    inputs += sorted((ROOT / "tools/stdlib").rglob("*.sg"))
    inputs += [ROOT / "tools/stdlib/runtime.c", ROOT / "tools/stdlib/runtime_abi.h"]
    report = {"schema": 1, "result": "running", "stage": "exact-decimal-ieee-literal",
              "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(), "python": sys.version,
              "sources": {str(p.relative_to(ROOT)): sha(p) for p in inputs if p.is_file()}, "tools": {}, "steps": [],
              "m4_complete": False, "limits": ["pure SG rational converter, at most 16384 source bytes/4096 significant decimal digits/1024 base-2^30 limbs",
                  "sign, decimal point with digits on both sides, e/E exponent, digit separators and f32/f64 suffix are accepted; inf/nan/hex/integer-only forms reject",
                  "requested target width determines direct nearest-even storage; source suffix type validation belongs to semantic analysis",
                  "f32 direct rounding may differ from the current Rust f64-intermediate reference on double-rounding boundary inputs; this gate does not bless an unreviewed language parity change"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(tools["clang"]),
               SENGOO_MODULE_MAP="selfhost_core=" + str(ROOT / "selfhost/src/core.sg") + ";selfhost_float_literal=" + str(ROOT / "selfhost/src/float_literal.sg"))
    env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")

    def run(name, command, code=0, marker=None):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=600)
        (output / (name + ".stdout.log")).write_bytes(done.stdout)
        (output / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode, "expected_exit_code": code, "seconds": time.monotonic() - started})
        if done.returncode != code or marker and marker not in done.stdout + done.stderr:
            raise AssertionError(f"{name}: {done.returncode}")
        print(name, done.returncode, flush=True)
        return done

    try:
        for name, path in tools.items():
            digest = sha(path)
            expected_identity = reference_lock["tools"]["sgc"]["sha256"] if name == "reference" else lock[name]["sha256"]
            if digest != expected_identity:
                raise ValueError(f"{name} identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        for folder in ("src", "tests"):
            run("format-" + folder, [tools["formatter"], "--check", ROOT / "selfhost" / folder / "float_literal.sg"])
        binary = output / "literal-tests.exe"
        binary.unlink(missing_ok=True)
        run("build-sg", [tools["seed"], "--runtime-mode", "source-development", "build", ROOT / "selfhost/tests/float_literal.sg", "--force-rebuild", "-O", "0", "-o", binary])
        run("native-scopes", [binary], marker=b"selfhost exact decimal IEEE literals: ok")
        cases = fixtures()
        fixture = output / "decimal corpus 世界 &.txt"
        fixture.write_bytes("".join(str(case["bits"]) + "\t" + case["literal"] + "\n" for case in cases).encode("utf-8"))
        report["fixture"] = {"path": str(fixture), "sha256": sha(fixture), "cases": len(cases)}
        actual = run("sg-corpus", [binary, fixture]).stdout.decode("utf-8").splitlines()
        repeated = run("sg-repeat", [binary, fixture]).stdout.decode("utf-8").splitlines()
        compatible = run("sg-reference-rounding", [binary, fixture, "reference"]).stdout.decode("utf-8").splitlines()
        if actual != repeated or len(actual) != len(cases):
            raise AssertionError("SG corpus count or repeated bytes differ")
        probe = output / "literal-probe.exe"
        run("build-c", [tools["clang"], ROOT / "selfhost/tests/float_literal_probe.c", "-std=c11", "-Werror", "-o", probe])
        expected = run("c-corpus", [probe, fixture]).stdout.decode("utf-8").splitlines()
        if len(expected) != len(cases):
            raise AssertionError("C corpus count differs")
        failures, c_discrepancies, double_round = [], [], []
        for case, sg, c, reference_line in zip(cases, actual, expected, compatible):
            oracle = exact_ieee(case["literal"], case["bits"])
            python_line = "ok " + str(oracle)
            if sg != python_line:
                failures.append({**case, "sg": sg, "c": c, "python_exact": python_line})
            if c != python_line:
                c_discrepancies.append({**case, "sg": sg, "c_runtime": c, "python_exact": python_line})
            if case["bits"] == 64:
                builtin_bits = struct.unpack("<q", struct.pack("<d", float(clean(case["literal"]))))[0]
                if builtin_bits != oracle:
                    failures.append({**case, "python_builtin": builtin_bits, "python_exact": oracle})
                if reference_line != python_line:
                    failures.append({**case, "sg_reference": reference_line, "python_f64": python_line})
            else:
                try:
                    double_bits = struct.unpack("<I", struct.pack("<f", float(clean(case["literal"]))))[0]
                except OverflowError:
                    double_bits = 0xff800000 if case["literal"].startswith("-") else 0x7f800000
                if double_bits != oracle:
                    double_round.append({**case, "strict_bits": oracle, "f64_intermediate_bits": double_bits})
                if reference_line != "ok " + str(double_bits):
                    failures.append({**case, "sg_reference": reference_line, "python_f64_intermediate": double_bits})
        report.update(failures=failures, c_runtime_discrepancies=c_discrepancies, f32_double_rounding_examples=double_round, python_exact_cases=len(cases), c_cases=len(cases))
        (output / "fixtures.json").write_text(json.dumps(cases, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if failures:
            raise AssertionError(f"conversion mismatches: {len(failures)}")
        if c_discrepancies:
            c_source = output / "c-runtime-discrepancy.c"
            body = ["#include <stdint.h>", "#include <stdio.h>", "#include <string.h>", "int main(void) {"]
            for index, case in enumerate(c_discrepancies):
                if case["bits"] != 64:
                    raise AssertionError("unreviewed f32 C runtime oracle discrepancy")
                body += [f"double value{index}={clean(case['literal'])}; int64_t bits{index};",
                         f'memcpy(&bits{index},&value{index},8); printf("ok %lld\\n",(long long)bits{index});']
            body += ["return 0; }"]
            c_source.write_text("\n".join(body) + "\n", encoding="utf-8")
            c_binary = output / "c-runtime-discrepancy.exe"
            run("build-c-constant-oracle", [tools["clang"], c_source, "-O0", "-Werror", "-o", c_binary])
            compiled = run("c-constant-oracle", [c_binary]).stdout.decode("utf-8").splitlines()
            if compiled != [case["python_exact"] for case in c_discrepancies]:
                raise AssertionError("clang constant oracle does not corroborate exact rounding")
            for case, value in zip(c_discrepancies, compiled):
                case["clang_constant"] = value
            report["c_constant_oracle"] = {"source_sha256": sha(c_source), "artifact_sha256": sha(c_binary)}
        if double_round:
            sg_source = output / "reference-f32-double-rounding.sg"
            sg_text = 'extern "C" { fn sh_literal_f32_bits(value:f32)->i64; }\ndef main()->i64{\n'
            sg_text += "".join("sh_literal_f32_bits(" + clean(case["literal"]) + "f32);\n" for case in double_round) + "0}\n"
            sg_source.write_text(sg_text, encoding="utf-8")
            ir = output / "reference-f32-double-rounding.ll"
            run("reference-f32-ir", [tools["reference"], "--runtime-mode", "source-development", "build", sg_source, "--force-rebuild", "-O", "0", "--emit-llvm", "-o", ir])
            observer = output / "reference-f32-observer.c"
            observer.write_text('#include <stdint.h>\n#include <stdio.h>\n#include <string.h>\nint64_t sh_literal_f32_bits(float value){uint32_t bits;memcpy(&bits,&value,4);printf("ok %llu\\n",(unsigned long long)bits);return 0;}\n', encoding="utf-8")
            reference_binary = output / "reference-f32-double-rounding.exe"
            run("reference-f32-link", [tools["clang"], observer, ir, "-O0", "-o", reference_binary])
            reference_actual = run("reference-f32-execute", [reference_binary]).stdout.decode("utf-8").splitlines()
            if reference_actual != ["ok " + str(case["f64_intermediate_bits"]) for case in double_round]:
                raise AssertionError("frozen reference does not match the f64-intermediate contract")
            report["reference_f32_double_rounding"] = {"source_sha256": sha(sg_source), "ir_sha256": sha(ir), "artifact_sha256": sha(reference_binary), "cases": len(double_round)}
        report["artifacts"] = {p.name: sha(p) for p in (binary, probe)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items() if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools,
                      source_revision_end=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                      completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if changed or changed_tools:
            report.update(result="failed", error="float literal inputs changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("float literal inputs changed")


if __name__ == "__main__":
    main()
