"""Independently inspect physical-file tables of a qualified SG COFF/PDB pair."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from pathlib import Path
import platform
import struct
import subprocess
import sys

from link_debug_gate import ROOT, msf_streams, sha, u16, u32
from pdb_type_hash_oracle import validate_tpi_hashes
from link_multi_debug_gate import coff_fragments


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def check_checksums(raw, strings, paths, contents):
    require(len(raw) == 40 * len(paths), "file checksum count/extent")
    offset = 1
    for index, (path, content) in enumerate(zip(paths, contents)):
        row = raw[index * 40:(index + 1) * 40]
        require(u32(row, 0) == offset and row[4:6] == b"\x20\x03", "file checksum path byte offset/type")
        require(strings[offset:offset + len(path) + 1] == path + b"\0", "file checksum path identity")
        require(row[6:38] == hashlib.sha256(content).digest() and row[38:40] == b"\0\0", "physical checksum/padding")
        offset += len(path) + 1


def validate(streams, obj, paths, contents):
    expected_strings = b"\0" + b"".join(path + b"\0" for path in paths)
    fragments = coff_fragments(obj)
    require(set(fragments) == {0xF1, 0xF2, 0xF3, 0xF4} and len(fragments[0xF2]) == 2, "COFF subsection identities")
    for kind in (0xF1, 0xF3, 0xF4):
        require(len(fragments[kind]) == 1, "duplicate COFF source subsection")
    _, at, length = fragments[0xF3][0]
    require(obj[at:at + length] == expected_strings, "COFF F3 physical paths")
    _, at, length = fragments[0xF4][0]
    check_checksums(obj[at:at + length], expected_strings, paths, contents)

    require(len(streams) == 12, "PDB stream count")
    validate_tpi_hashes(streams)
    names = streams[5]
    require(u32(names, 0) == 0xEFFEEFFE and u32(names, 4) == 1, "PDB /names identity")
    size = u32(names, 8)
    require(names[12:12 + size] == expected_strings, "PDB /names physical paths")
    at = 12 + size
    capacity = u32(names, at)
    require(capacity >= 4 and capacity & (capacity - 1) == 0 and len(names) == at + 8 + 4 * capacity, "PDB /names bucket extent")
    offsets = [u32(names, at + 4 + index * 4) for index in range(capacity)]
    expected_offsets, offset = [], 1
    for path in paths:
        expected_offsets.append(offset)
        offset += len(path) + 1
    require(sorted(value for value in offsets if value) == expected_offsets and u32(names, at + 4 + capacity * 4) == len(paths), "PDB /names bucket contents")

    dbi = streams[3]
    require(u32(dbi, 4) == 19990903 and u16(dbi, 58) == 0x8664, "DBI identity")
    module_size, contributions, section_map, file_size = struct.unpack_from("<4I", dbi, 24)
    require(u16(dbi, 64 + 48) == len(paths) and u16(dbi, 64 + 34) == 6, "one DBI module / physical file count")
    module_names = paths[0] + b"\0" + paths[0] + b"\0"
    require(dbi[128:128 + len(module_names)] == module_names and module_size == (64 + len(module_names) + 3) & ~3, "DBI one-module path table")
    file_at = 64 + module_size + contributions + section_map
    table = dbi[file_at:file_at + file_size]
    require(struct.unpack_from("<4H", table) == (1, len(paths), 0, len(paths)), "DBI file-info module/file counts")
    offset, expected = 0, bytearray(struct.pack("<4H", 1, len(paths), 0, len(paths)))
    for path in paths:
        expected.extend(struct.pack("<I", offset))
        offset += len(path) + 1
    expected.extend(b"".join(path + b"\0" for path in paths))
    expected.extend(b"\0" * (-len(expected) % 4))
    require(table == expected, "DBI file-info offsets/physical paths")

    symbols_size, c13_size = u32(dbi, 64 + 36), u32(dbi, 64 + 44)
    module, at, checksum_seen, line_files = streams[6], symbols_size, False, []
    while at < symbols_size + c13_size:
        kind, length = struct.unpack_from("<II", module, at)
        data = module[at + 8:at + 8 + length]
        require(len(data) == length, "PDB C13 extent")
        if kind == 0xF4:
            require(not checksum_seen, "duplicate PDB checksum subsection")
            check_checksums(data, expected_strings, paths, contents)
            checksum_seen = True
        else:
            require(kind == 0xF2 and length >= 32, "PDB C13 kind/line extent")
            file_offset, count, block_size = struct.unpack_from("<III", data, 12)
            require(file_offset in (0, 40) and count > 0 and block_size == 12 + count * 8 and length == 24 + count * 8, "PDB C13 file-block byte reference")
            require(u16(data, 4) == 1 and u16(data, 6) == 0, "PDB C13 code segment/flags")
            numbers = [u32(data, 28 + index * 8) & 0xFFFFFF for index in range(count)]
            require(numbers == ([2, 3, 4, 5] if file_offset == 0 else [1, 2, 3, 4]), "physical declaration/statement lines")
            line_files.append(file_offset)
        at += 8 + length
    require(at == symbols_size + c13_size and checksum_seen and sorted(line_files) == [0, 40], "PDB complete physical file coverage")
    return {"physical_files": len(paths), "pdb_modules": 1, "coff_f3_f4": True, "pdb_names_dbi_c13": True, "paths": [path.decode("utf-8") for path in paths], "source_sha256": [hashlib.sha256(raw).hexdigest() for raw in contents]}


def comparable_pair(image, pdb, pdb_path):
    streams = [bytearray(value) for value in msf_streams(pdb)]
    normalized = bytearray(image)
    sections = u16(image, 70)
    row = 328 + (sections - 1) * 40
    require(image[row:row + 8] == b".debug\0\0", "compatibility final debug section")
    raw, padded, size = u32(image, row + 20), u32(image, row + 16), u32(image, row + 8)
    path = pdb_path.as_posix().encode("utf-8") + b"\0"
    require(size == 52 + len(path) and image[raw + 52:raw + size] == path and not any(image[raw + size:raw + padded]), "compatibility RSDS path/padding")
    require(image[raw + 28:raw + 32] == b"RSDS" and image[raw + 32:raw + 48] == streams[1][12:28] and u32(image, raw + 48) == 1 and u32(image, raw + 16) == size - 28, "compatibility RSDS/PDB association")
    normalized[row + 8:row + 12] = b"\0" * 4
    normalized[raw + 16:raw + 20] = b"\0" * 4
    normalized[raw + 32:raw + 48] = b"\0" * 16
    normalized[raw + 52:raw + padded] = b"\0" * (padded - 52)
    require(streams[1][4:8] == streams[1][12:16], "compatibility PDB signature")
    streams[1][4:8] = b"\0" * 4
    streams[1][12:28] = b"\0" * 16
    dbi = streams[3]
    section_map = 64 + u32(dbi, 24) + u32(dbi, 28)
    field = section_map + 4 + (sections - 1) * 20 + 16
    require(u16(dbi, section_map) == sections + 1 and u32(dbi, field) == size, "compatibility DBI debug-section extent")
    dbi[field:field + 4] = b"\0" * 4
    require(len(streams[7]) == sections * 40 and u32(streams[7], (sections - 1) * 40 + 8) == size, "compatibility PDB section headers")
    streams[7][(sections - 1) * 40 + 8:(sections - 1) * 40 + 12] = b"\0" * 4
    return bytes(normalized), [bytes(value) for value in streams]


def compatibility(reference, candidate, inputs):
    reports = []
    for path in (reference, candidate):
        raw = path.read_bytes()
        report = json.loads(raw)
        require(report["result"] == "passed" and not report["changed_sources"] and not report["changed_tools"], "single-source compatibility producer not qualified")
        inputs[str(path.relative_to(ROOT))] = hashlib.sha256(raw).hexdigest()
        reports.append(report)
    result = []
    for name in ("multiple-functions", "multiple-returns", "floating-parameters", "mixed-scalar-parameters", "readonly-reference", "mutable-reference", "raw-void-pointer", "aggregate-locals", "tagged-enum-local"):
        pairs = []
        for path, report in zip((reference, candidate), reports):
            blobs = []
            for suffix in (".obj", ".exe", ".pdb"):
                artifact = path.parent / (name + suffix)
                raw = artifact.read_bytes()
                digest = hashlib.sha256(raw).hexdigest()
                require(digest == report["artifacts"][artifact.name], "single-source artifact differs from producer")
                inputs[str(artifact.relative_to(ROOT))] = digest
                blobs.append(raw)
            pairs.append(blobs)
        require(pairs[0][0] == pairs[1][0], "single-source COFF bytes changed")
        left = comparable_pair(pairs[0][1], pairs[0][2], reference.parent / (name + ".pdb"))
        right = comparable_pair(pairs[1][1], pairs[1][2], candidate.parent / (name + ".pdb"))
        require(left == right, "single-source PE/PDB differs beyond its output path and derived identity")
        changed_image = bytearray(pairs[1][1])
        changed_image[u32(changed_image, 328 + 20)] ^= 1
        require(comparable_pair(bytes(changed_image), pairs[1][2], candidate.parent / (name + ".pdb")) != right, "compatibility oracle masked a code byte")
        result.append({"fixture": name, "coff_bytes_identical": True, "pe_pdb_equal_after_output_path_and_derived_identity_fields": True, "reference_raw_pe_sha256": hashlib.sha256(pairs[0][1]).hexdigest(), "candidate_raw_pe_sha256": hashlib.sha256(pairs[1][1]).hexdigest(), "reference_raw_pdb_sha256": hashlib.sha256(pairs[0][2]).hexdigest(), "candidate_raw_pdb_sha256": hashlib.sha256(pairs[1][2]).hexdigest()})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--single-source-reference", type=Path)
    parser.add_argument("--single-source-candidate", type=Path)
    args = parser.parse_args()
    evidence, output = args.evidence.resolve(), args.output.resolve()
    require(output.is_relative_to(ROOT / "target"), "output must stay inside target")
    output.mkdir(parents=True, exist_ok=False)
    raw_report = evidence.read_bytes()
    producer = json.loads(raw_report)
    require(producer["result"] == "passed" and producer["stage"] == "one-module-multi-source-native-debug" and not any(producer.get(key) for key in ("changed_sources", "changed_tools", "changed_cargo_configs")), "unqualified multi-source producer")
    inputs = {str(evidence.relative_to(ROOT)): hashlib.sha256(raw_report).hexdigest()}
    for name, digest in producer["sources"].items():
        require(sha(ROOT / name) == digest, "producer source manifest changed")
        inputs[name] = digest
    binary_paths = [evidence.parent / name for name in ("physical.obj", "physical.pdb")]
    blobs = []
    for path in binary_paths:
        raw = path.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        require(digest == producer["artifacts"][path.name], "producer artifact differs")
        inputs[str(path.relative_to(ROOT))] = digest
        blobs.append(raw)
    paths = [evidence.parent / "sources 世界 &" / name for name in ("main.sg", "worker.sg")]
    contents = []
    for path in paths:
        raw = path.read_bytes()
        require(hashlib.sha256(raw).hexdigest() == producer["sources"][str(path.relative_to(ROOT))], "physical input differs from producer")
        contents.append(raw)
        inputs[str(path.relative_to(ROOT))] = hashlib.sha256(raw).hexdigest()
    for path in (Path(__file__).resolve(), ROOT / "selfhost/link_debug_gate.py", ROOT / "selfhost/link_multi_debug_gate.py"):
        inputs[str(path.relative_to(ROOT))] = sha(path)
    report = {"schema": 1, "stage": "multi-source-debug-independent-byte-oracle", "result": "running", "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(), "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "sources": inputs, "tools": {"python": {"path": sys.executable, "sha256": sha(Path(sys.executable)), "version": sys.version}}, "producer_evidence": str(evidence.relative_to(ROOT))}
    try:
        streams = msf_streams(blobs[1])
        report["inspection"] = validate(streams, blobs[0], [str(path).encode("utf-8") for path in paths], contents)
        controls = []
        for stream, offset, label in ((3, 64 + 48, "dbi-file-count"), (3, 128, "dbi-module-path"), (5, 13, "names-physical-path"), (6, u32(streams[3], 64 + 36) + 8 + 12, "c13-file-reference")):
            changed = list(streams)
            value = bytearray(changed[stream])
            value[offset] ^= 0x80
            changed[stream] = bytes(value)
            try:
                validate(changed, blobs[0], [str(path).encode("utf-8") for path in paths], contents)
            except AssertionError:
                controls.append(label)
            else:
                raise AssertionError("oracle negative control accepted: " + label)
        report.update(result="passed", oracle_negative_controls=controls)
        require(bool(args.single_source_reference) == bool(args.single_source_candidate), "both single-source producers are required")
        if args.single_source_reference:
            report["single_source_compatibility"] = compatibility(args.single_source_reference.resolve(), args.single_source_candidate.resolve(), inputs)
            report["single_source_normalization_scope"] = "Only the exact RSDS output path, its containing debug-section extent copies, and the path-derived PDB GUID/signature are removed; every other PE byte and every other byte of all 11 PDB streams must be identical. Raw PE/PDB bytes are intentionally not claimed identical across different output paths."
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        report["changed_sources"] = [name for name, digest in inputs.items() if sha(ROOT / name) != digest]
        report.update(completed_at=dt.datetime.now(dt.timezone.utc).isoformat(), source_revision_end=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip())
        if report["changed_sources"]:
            report.update(result="failed", error="oracle inputs changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        require(not report["changed_sources"], "oracle inputs changed")


if __name__ == "__main__":
    main()
