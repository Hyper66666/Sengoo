"""Reject misleading platform tags and corrupted repaired-wheel records."""
import base64
import csv
import hashlib
import io
from pathlib import Path
import tempfile
import unittest
import zipfile

from repair_wheels import validate_archive


def archive(path, platform="win_amd64", corrupt=False):
    files = {"sample/_native.dll": b"native fixture", "sample/__init__.py": b"",
             "sample-0.1.dist-info/WHEEL": f"Wheel-Version: 1.0\nTag: py3-none-{platform}\n".encode()}
    records = io.StringIO(newline="")
    writer = csv.writer(records, lineterminator="\n")
    for name, content in files.items():
        digest = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode()
        writer.writerow([name, "sha256=" + digest, len(content)])
    record = "sample-0.1.dist-info/RECORD"
    writer.writerow([record, "", ""])
    files[record] = records.getvalue().encode()
    if corrupt:
        files["sample/_native.dll"] = b"tampered"
    with zipfile.ZipFile(path, "w") as wheel:
        wheel.writestr("sample/", b"")
        for name, content in files.items():
            wheel.writestr(name, content)


class WheelArtifactTests(unittest.TestCase):
    def test_repaired_archive_allows_directory_entries_and_hashes_every_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample-0.1-py3-none-win_amd64.whl"
            archive(path)
            evidence = validate_archive(path, "windows-x64")
            self.assertEqual(evidence["native_files"], ["sample/_native.dll"])
            self.assertEqual(len(evidence["sha256"]), 64)
            archive(path, corrupt=True)
            with self.assertRaisesRegex(RuntimeError, "invalid RECORD"):
                validate_archive(path, "windows-x64")

    def test_linux_and_macos_target_tags_must_match_actual_delivered_platform(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample.whl"
            for tag, platform in (("linux_x86_64", "linux-x64"), ("macosx_15_0_arm64", "macos-x64")):
                archive(path, platform=tag)
                with self.assertRaisesRegex(RuntimeError, "unexpected delivered platform"):
                    validate_archive(path, platform)
            archive(path, platform="manylinux_2_28_x86_64")
            validate_archive(path, "linux-x64")


if __name__ == "__main__":
    unittest.main()
