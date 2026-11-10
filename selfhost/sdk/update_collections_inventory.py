"""Write namespace-two source inventory; an object needs an independent caller pin."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SYMBOLS = [
    'sengoo_raw_vec_new_parts', 'sengoo_raw_vec_iter_new',
    'sengoo_raw_vec_iter_next', 'sengoo_raw_vec_iter_free',
    'sengoo_raw_vec_free', 'sengoo_raw_vec_push',
]
SOURCES = [
    'tools/stdlib/runtime_collections.c', 'tools/stdlib/runtime_shared.h',
    'selfhost/sdk/collections_x64_abi1/prelude.sg', 'tools/stdlib/collections.sg',
]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inventory(root: Path, artifact: Path | None = None) -> dict:
    raw = (root / SOURCES[0]).read_bytes()
    exports = []
    for number, symbol in enumerate(SYMBOLS):
        matches = list(re.finditer(rb'(?m)^(?:long long|void\*) ' + symbol.encode() + rb'\([^;{]*\)\s*\{', raw))
        if len(matches) != 1:
            raise ValueError(f'expected one actual C definition of {symbol}')
        begin = matches[0].start()
        end = raw.index(b'{', begin)
        prototype = b' '.join(raw[begin:end].split()).decode('ascii')
        exports.append(dict(id=number, symbol=symbol, prototype=prototype, requires=1 << number,
                            prototype_begin=begin, prototype_end=end))
    artifacts = []
    if artifact is not None:
        path = artifact.resolve()
        relative = path.relative_to(root.resolve()).as_posix()
        artifacts.append(dict(path=relative, sha256=digest(path), format='coff-object', source=SOURCES[0]))
    return dict(schema='sengoo.collections-sdk.v1', namespace=2,
                target='x86_64-pc-windows-msvc', abi_version=1, pointer_bits=64,
                endianness='little', **{'class': 'reference-runtime'},
                sources=[dict(path=name, sha256=digest(root / name), bytes=(root / name).stat().st_size)
                         for name in SOURCES], exports=exports, artifacts=artifacts)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    destination = args.output or args.root / 'selfhost/sdk/collections_x64_abi1/manifest.json'
    destination.write_text(json.dumps(inventory(args.root, args.artifact), indent=2) + '\n', encoding='utf8')


if __name__ == '__main__':
    main()
