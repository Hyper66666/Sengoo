"""Independently rehash the integrated factory gate and its command receipts."""
from pathlib import Path
import argparse
import datetime
import hashlib
import importlib.util
import json
import os

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def audit(report_path, report_pin):
    if not __debug__:
        raise RuntimeError('auditor requires Python assertions enabled; do not use -O')
    observed, expected, configs, reports, documents = {}, {}, {}, [], []

    def check(path, digest):
        path = Path(path).resolve()
        key = str(path)
        assert len(digest) == 64 and all(c in '0123456789abcdef' for c in digest), key
        assert key not in expected or expected[key] == digest, 'conflicting expected hash: ' + key
        expected[key] = digest
        assert path.is_file(), 'missing: ' + key
        if key not in observed:
            observed[key] = sha(path)
        assert observed[key] == digest, 'changed: ' + key

    def read(path, result, historic_origin=False):
        path = Path(path).resolve()
        raw = path.read_bytes()
        check(path, hashlib.sha256(raw).hexdigest())
        doc = json.loads(raw)
        assert doc['result'] == result, (str(path), doc.get('result'))
        for key, value in doc.items():
            if key.startswith('changed_') or key.endswith('_changes'):
                assert not value, (str(path), key, value)
        for field in ['sources', 'producer_origins', 'artifacts', 'source_origins', 'link_inputs']:
            if historic_origin and field in ['producer_origins', 'source_origins']:
                continue
            for key, value in doc.get(field, {}).items():
                target = Path(key)
                if not target.is_absolute():
                    target = path.parent / target
                check(target, value)
        for value in doc.get('tools', {}).values():
            check(value['path'], value['sha256'])
        for field in ['cargo_configs', 'cargo_config_candidates']:
            for key, value in doc.get(field, {}).items():
                current = sha(key) if Path(key).is_file() else None
                assert current == value, 'configuration changed: ' + key
                assert key not in configs or configs[key] == value
                configs[key] = value
        def hash_tree(tree):
            for key, value in tree.items():
                if isinstance(value, dict):
                    hash_tree(value)
                else:
                    check(key, value)
        for field in ['c_headers', 'c_header_closures', 'c_link_libraries', 'actual_link_libraries']:
            hash_tree(doc.get(field, {}))
        for field in ['selected_linker', 'selected_c_linker']:
            if field in doc:
                check(doc[field]['path'], doc[field]['sha256'])
        reports.append({'path': str(path), 'sha256': sha(path), 'result': result})
        documents.append((path, doc))
        return doc

    path = Path(report_path).resolve()
    # The caller supplies the trusted report pin; verify bytes before JSON parsing.
    check(path, report_pin)
    doc = read(path, 'passed-bounded-contracts-with-recorded-native-frontier')
    assert doc['source_revision'] == doc['source_revision_end']
    assert doc['m3_complete'] is False and doc['m6_complete'] is False
    assert doc['native_async_file_support'] is False
    dependencies = {
        'collections': ('5986c47f3547447e99091d1ab7bac5e046412c94d5904e88a04bf8c34824b97a', 'passed'),
        'readiness': ('77c51f9c587dc8b72959c4b8b6e9cfa286d115a0500e2fffd3bc83dc6a76ab9e', 'passed-development-readiness-provider-lifecycle'),
    }
    for key, (pin, result) in dependencies.items():
        dep = doc['qualified_dependencies'][key]
        assert dep['sha256'] == pin and dep['result'] == result
        check(dep['path'], pin)
        retained = read(dep['path'], result, historic_origin=True)
        if key == 'readiness':
            assert all(retained[k] is False for k in ['final_sdk', 'runtime_algorithms_ported', 'sg_lowering_qualified'])
            assert len(retained['completed_case_ids']) == 16 and retained['measured_scopes'] == 1600
        else:
            conversion = retained['coff_timestamp_normalization']
            for prefix in ['parent', 'qualified', 'raw', 'normalized']:
                check(conversion[prefix + ('_report' if prefix == 'parent' else '_object')], conversion[prefix + '_sha256'])
            raw = Path(conversion['raw_object']).read_bytes()
            converted = Path(conversion['normalized_object']).read_bytes()
            qualified = Path(conversion['qualified_object']).read_bytes()
            assert len(raw) == len(converted) == len(qualified)
            assert conversion['allowed_offsets'] == [4, 5, 6, 7]
            assert raw[:4] == converted[:4] and raw[8:] == converted[8:]
            assert converted == qualified
            assert conversion['non_timestamp_single_byte_rejections'] == len(raw) - 4
            assert conversion['length_rejections'] == 2
    children = {}
    results = {
        'source_report': 'passed-source-matrix-with-classified-parser-controls',
        'policy_report': 'passed-factory-case-policy-response-controls',
        'print_report': 'passed-with-recorded-reference-differences',
    }
    for key, result in results.items():
        child = doc[key]
        check(child['path'], child['sha256'])
        assert child['result'] == result
        children[key] = read(child['path'], result)
    assert len(children['source_report']['cases']) == 27
    printed = children['print_report']
    assert len(printed['cases']) == 10 and len(printed['ordinary_cases']) == 49
    assert [c['case'] for c in printed['reference_differences']] == ['shadow']
    assert sum(c['result'] == 'byte-identical-output' for c in printed['ordinary_cases']) == 46
    assert sum(c['result'].startswith('recorded-reference-') for c in printed['ordinary_cases']) == 3
    assert children['policy_report']['sg_resource_scopes'] == 3
    assert children['policy_report']['independent_c']['query_time_file_opens'] == 0
    assert printed['hir']['mutation_kinds'] == 21 and printed['hir']['scopes'] == 3
    assert printed['hir']['restored_from_first_scope'] is True

    # Every nested receipt must still exist and agree with its coordinator row.
    coordinator = {step['name']: step for step in doc['steps']}
    prefixes = {'source_report': 'factory-source-', 'policy_report': 'policy-', 'print_report': 'print-'}
    for key, prefix in prefixes.items():
        for step in children[key]['steps']:
            parent = coordinator[prefix + step['name']]
            assert parent['command'] == step['command']
            assert parent['exit_code'] == step.get('exit_code', step.get('callback_confirmed_exit_code'))
            assert parent['expected_exit_code'] == step['expected_exit_code']
            for stream in ['stdout', 'stderr']:
                assert parent[stream + '_sha256'] == step[stream + '_sha256']
    nested_receipts = 0
    for nested_path, nested in documents[1:]:
        for step in nested['steps']:
            for stream in ['stdout', 'stderr']:
                check(nested_path.parent / (step['name'] + '.' + stream + '.log'), step[stream + '_sha256'])
            if 'exit_code' in step:
                assert step['exit_code'] == step['expected_exit_code']
            nested_receipts += 1

    names = set()
    for step in doc['steps']:
        name = step['name']
        assert name not in names, 'duplicate command receipt: ' + name
        names.add(name)
        for stream in ['stdout', 'stderr']:
            check(path.parent / (name + '.' + stream), step[stream + '_sha256'])
        if step.get('timeout'):
            assert name == 'full-compiler-self-input' and step['exit_code'] is None
        elif step['expected_exit_code'] is not None:
            assert step['exit_code'] == step['expected_exit_code'], name
        else:
            assert step['exit_code'] in [0, 65], (name, step['exit_code'])
    assert {'joint-build', 'factory-hir', 'source-selection', 'observer', 'identity',
            'full-compiler-self-input', 'native-std-file-frontier',
            'trace-std-file-equivalence'} <= names
    for key in ['std_file_frontier', 'full_self_input']:
        frontier = doc[key]
        if frontier.get('timed_out'):
            assert key == 'full_self_input' and not (path.parent / 'self-input.obj').exists()
        else:
            protocol = json.loads(frontier['stdout'])
            assert protocol['schema_version'] == 1
            assert protocol['ok'] == (frontier['exit_code'] == 0)
            assert frontier['exit_code'] in [0, 65]
            if frontier['exit_code'] == 65:
                assert protocol['phase'] in ['modules', 'frontend', 'semantics', 'lowering', 'native']
                assert protocol['code'] > 0
                assert not (path.parent / ('self-input.obj' if key == 'full_self_input' else 'std-file.obj')).exists()
        receipt = 'full-compiler-self-input' if key == 'full_self_input' else 'native-std-file-frontier'
        assert frontier['stdout'].encode('utf8') == (path.parent / (receipt + '.stdout')).read_bytes()
        assert frontier['stderr'].encode('utf8') == (path.parent / (receipt + '.stderr')).read_bytes()
    assert not (path.parent / 'std-file.obj').exists()
    snapshot = path.parent / 'implementation'
    spec = importlib.util.spec_from_file_location('factory_audit_input_closure', snapshot / 'selfhost/compiler_inputs.py')
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    entries = [snapshot / 'selfhost/tests/async_factory/driver.sg', snapshot / 'selfhost/tools/native.sg']
    entries.extend(snapshot / 'selfhost/tests' / (name + '.sg') for name in doc['regressions'])
    actual = helper.compiler_closure(snapshot, entries)
    recorded = {str((snapshot / name).resolve()): value for name, value in doc['compiler_closure'].items()}
    assert {str(p): h for p, h in actual.items()} == recorded, 'actual compiler closure differs'
    for key, value in actual.items():
        check(key, value)
    # Close the audit itself against additions to formerly absent configuration.
    assert all((sha(p) if Path(p).is_file() else None) == h for p, h in configs.items())
    assert all(Path(p).is_file() and sha(p) == h for p, h in observed.items())
    return {'schema': 1, 'result': 'passed-independent-integrated-factory-audit',
            'checked_files': len(observed), 'checked_configuration_candidates': len(configs),
            'command_receipts': len(doc['steps']), 'nested_command_receipts': nested_receipts,
            'reports': reports,
            'completed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'sources': observed, 'cargo_configs': configs,
            'm3_complete': False, 'm6_complete': False,
            'native_async_file_support': False,
            'scope': 'Current retained inputs, producer origins, tool/header/library/configuration bytes and recorded command receipts. No compiler commands rerun. Historical dependency checkout origins are not current-source qualification.',
            'native_frontiers': {key: doc[key] for key in ['std_file_frontier', 'full_self_input']}}


def atomic_json(path, value):
    path = Path(path)
    temporary = path.with_name(path.name + '.tmp-' + str(os.getpid()))
    try:
        temporary.write_text(json.dumps(value, indent=2) + '\n', 'utf8')
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('report', type=Path)
    parser.add_argument('--report-pin', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    # Invoke from the checkout root. Audit output is a disposable target artifact.
    root = Path.cwd().resolve()
    if not (root / 'selfhost/bootstrap.lock').is_file():
        raise RuntimeError('invoke from repository root')
    output = args.output.resolve()
    if not output.is_relative_to(root / 'target') or output.suffix != '.json':
        raise ValueError('output must be a target JSON file')
    if output == args.report.resolve():
        raise ValueError('output must not overwrite input report')
    output.parent.mkdir(parents=True, exist_ok=True)
    status = {'schema': 1, 'result': 'running', 'report': str(args.report.resolve()),
              'report_sha256': args.report_pin}
    atomic_json(output, status)  # Invalidate an old pass before any report read.
    try:
        result = audit(args.report, args.report_pin)
        result['auditor'] = {'path': str(Path(__file__).resolve()), 'sha256': sha(__file__)}
    except BaseException as error:
        status.update(result='failed', error=type(error).__name__ + ': ' + str(error),
                      completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        atomic_json(output, status)
        raise
    atomic_json(output, result)
    print(json.dumps({k: v for k, v in result.items() if k not in {'sources', 'cargo_configs'}}))


if __name__ == '__main__':
    main()
