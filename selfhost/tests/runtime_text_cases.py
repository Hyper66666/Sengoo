"""Generate the independent strict-Python oracle corpus for SG UTF-8 primitives.

Format v1 (all integers little endian):
  magic b"SGUTF8\x01\x00", uint64 record count;
  repeated uint8 byte length, uint8 valid, ceil((length+1)/8) boundary bytes,
  followed by exactly length source bytes. The source length is 0..64.
A valid record marks byte offsets obtained by re-encoding the decoded Python
characters, including zero and the end. Invalid records have all-zero masks.
The C probe consumes expected facts; it does not contain a UTF-8 decoder.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import struct
import sys

MAGIC = b"SGUTF8\x01\x00"
MAX_LENGTH = 64
SEED = 0x36292026
SCALAR_COUNT = 0x110000 - 0x800


def oracle(data: bytes) -> tuple[bool, bytes]:
    """Use strict decode, then independent character re-encoding for offsets."""
    if len(data) > MAX_LENGTH:
        raise ValueError("record exceeds the bounded corpus length")
    width = (len(data) + 8) // 8
    try:
        characters = data.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return False, bytes(width)
    offset, bits = 0, 1
    for character in characters:
        offset += len(character.encode("utf-8", errors="strict"))
        bits |= 1 << offset
    if offset != len(data):
        raise AssertionError("strict decode/re-encode length mismatch")
    return True, bits.to_bytes(width, "little")


def scalar(rng: random.Random) -> str:
    value = rng.randrange(SCALAR_COUNT)
    if value >= 0xD800:
        value += 0x800
    return chr(value)


def records(random_cases: int):
    yield "empty", b""
    for first in range(256):
        yield "all-one-byte", bytes([first])
    for value in range(65536):
        yield "all-two-byte", value.to_bytes(2, "big")
    for value in range(0x110000):
        if not 0xD800 <= value <= 0xDFFF:
            yield "every-unicode-scalar", chr(value).encode("utf-8", errors="strict")
    for value in range(0xD800, 0xE000):
        yield "every-surrogate-invalid", chr(value).encode("utf-8", errors="surrogatepass")
    # These are encoded byte examples, never a second implementation of the
    # subject algorithm. Python's strict decoder alone decides their validity.
    examples = [
        "c0 80", "c1 bf", "e0 80 80", "e0 9f bf", "f0 80 80 80",
        "f0 8f bf bf", "ed a0 80", "ed af bf", "ed b0 80", "ed bf bf",
        "f4 90 80 80", "f4 bf bf bf", "f5 80 80 80", "f7 bf bf bf",
        "f8 88 80 80 80", "fc 84 80 80 80 80", "fe", "ff", "80", "bf",
        "c2", "e0", "e0 a0", "f0", "f0 90", "f0 90 80",
        "c2 41", "e2 28 a1", "e2 82 41", "f0 28 8c bc", "f0 90 28 bc",
        "f0 90 80 41", "41 c2 a2 ff", "41 e2 82 ac 80", "41 00 42",
        "ed 9f bf", "ee 80 80", "ef bf bf", "f4 8f bf bf", "ef bb bf",
    ]
    for example in examples:
        yield "rfc3629-explicit", bytes.fromhex(example)
    edges = [0, 0x7F, 0x80, 0x7FF, 0x800, 0xD7FF, 0xE000,
             0xFEFF, 0xFFFF, 0x10000, 0x1F600, 0x10FFFF]
    for value in edges:
        encoded = chr(value).encode("utf-8", errors="strict")
        for end in range(len(encoded)):
            yield "scalar-edge-truncation", encoded[:end]
        for index in range(len(encoded)):
            for replacement in range(256):
                changed = encoded[:index] + bytes([replacement]) + encoded[index+1:]
                yield "scalar-edge-byte-neighbors", changed
        for suffix in (b"\x80", b"\xbf", b"\xc0", b"\xff"):
            yield "scalar-edge-invalid-suffix", encoded + suffix
    rng = random.Random(SEED)
    for index in range(random_cases):
        length = rng.randrange(MAX_LENGTH + 1)
        mode = index % 4
        if mode == 0:
            data = bytes(rng.randrange(256) for _ in range(length))
        elif mode == 1:
            data = bytes(rng.randrange(128) for _ in range(length))
        else:
            data = b""
            while len(data) < length:
                part = scalar(rng).encode("utf-8", errors="strict")
                if len(part) > length - len(data):
                    part = bytes([rng.randrange(128)])
                data += part
            if mode == 3 and data:
                at = rng.randrange(len(data))
                data = data[:at] + bytes([rng.randrange(256)]) + data[at+1:]
        yield "fixed-seed-concatenations", data


def generate(output: Path, manifest: Path, random_cases: int) -> dict:
    if random_cases < 0 or random_cases > 1000000:
        raise ValueError("random case count must be within 0..1000000")
    output = output.resolve()
    manifest = manifest.resolve()
    if output == manifest:
        raise ValueError("corpus and manifest paths must differ")
    output.parent.mkdir(parents=True, exist_ok=True)
    manifest.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + ".tmp-" + str(os.getpid()))
    groups, valid_counts = Counter(), Counter()
    valid_records = invalid_records = count = 0
    try:
        with temporary.open("wb", buffering=1048576) as stream:
            stream.write(MAGIC + struct.pack("<Q", 0))
            for group, data in records(random_cases):
                valid, boundaries = oracle(data)
                stream.write(bytes([len(data), int(valid)]) + boundaries + data)
                count += 1
                groups[group] += 1
                valid_counts[group] += int(valid)
                valid_records += int(valid)
                invalid_records += int(not valid)
            stream.seek(8)
            stream.write(struct.pack("<Q", count))
        if groups["every-unicode-scalar"] != SCALAR_COUNT:
            raise AssertionError("Unicode scalar coverage differs")
        if valid_counts["every-unicode-scalar"] != SCALAR_COUNT:
            raise AssertionError("Python rejected a generated Unicode scalar")
        if valid_counts["every-surrogate-invalid"] != 0:
            raise AssertionError("strict Python accepted an encoded surrogate")
        if valid_counts["all-one-byte"] != 128 or valid_counts["all-two-byte"] != 18304:
            raise AssertionError("short byte strict-oracle counts differ")
        if groups["all-one-byte"] != 256 or groups["all-two-byte"] != 65536:
            raise AssertionError("short byte enumeration incomplete")
        temporary.replace(output)
    finally:
        temporary.unlink(missing_ok=True)
    result = {
        "schema": 1, "format": "SGUTF8-v1", "oracle": "Python strict utf-8 decode + per-character encode byte offsets",
        "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "python": platform.python_version(), "python_executable_sha256": hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest(),
        "seed": SEED, "random_cases": random_cases, "max_record_length": MAX_LENGTH,
        "corpus": str(output), "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
        "bytes": output.stat().st_size, "records": count,
        "valid_records": valid_records, "invalid_records": invalid_records,
        "groups": {group: {"records": number, "valid": valid_counts[group], "invalid": number-valid_counts[group]} for group, number in groups.items()},
        "boundary_mask": "little-endian bits for offsets zero through length inclusive; invalid input all zero",
        "out_of_range": "C probe additionally tests -1 and length+1 with zero reads per record",
    }
    manifest.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--random-cases", type=int, default=32768)
    args = parser.parse_args()
    manifest = args.manifest or args.output.with_suffix(args.output.suffix + ".json")
    result = generate(args.output, manifest, args.random_cases)
    print(json.dumps({key: result[key] for key in ("sha256", "bytes", "records", "valid_records", "invalid_records")}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
