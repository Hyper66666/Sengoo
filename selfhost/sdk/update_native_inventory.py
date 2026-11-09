"""Explicit development refresh of source fingerprints, never SDK admission.

Physical prototypes, declarations, target, component classes and language roles
remain reviewed inputs. A changed prototype fails instead of updating its ABI.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / 'selfhost/sdk/native_x64_abi1/manifest.json'


def render() -> bytes:
    catalog = json.loads(CATALOG.read_text(encoding='utf-8'))
    if catalog['schema'] != 'sengoo.native-sdk.catalog.v1':
        raise ValueError('unsupported reviewed inventory schema')
    for source in catalog['sources']:
        path = ROOT / source['path']
        data = path.read_bytes()
        source['sha256'] = hashlib.sha256(data).hexdigest()
        source['bytes'] = len(data)
    prelude = ROOT / catalog['prelude']['path']
    catalog['prelude']['sha256'] = hashlib.sha256(prelude.read_bytes()).hexdigest()
    for entry in catalog['exports']:
        path = ROOT / catalog['sources'][entry['source']]['path']
        text = path.read_bytes().decode('utf-8')
        pattern = (r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+'
                   if entry['component'] == 1 else r'(?:long long|void)\s+')
        matches = list(re.finditer(pattern + re.escape(entry['symbol']) + r'\s*\(', text))
        if len(matches) != 1:
            raise ValueError('provider export is missing or duplicated: ' + entry['symbol'])
        match = matches[0]
        begin, end = match.start(), text.index('{', match.end())
        while text[end - 1].isspace():
            end -= 1
        actual = ' '.join(text[begin:end].split())
        if actual != entry['prototype']:
            raise ValueError('reviewed physical prototype changed: ' + entry['symbol'])
        entry['prototype_begin'] = len(text[:begin].encode('utf-8'))
        entry['prototype_end'] = len(text[:end].encode('utf-8'))
    return (json.dumps(catalog, ensure_ascii=False, indent=2) + '\n').encode('utf-8')


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True,
                        help='explicit destination; checked-in inventory is never updated implicitly')
    args = parser.parse_args()
    data = render()
    args.output.write_bytes(data)
    print(hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
