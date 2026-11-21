#!/usr/bin/env python3
"""Strict actual Game/Java /damage and low-health armor stand parity (vanilla 26.3)."""
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

import argparse

import test_command_exec_parity as driver

# main's driver reads an option no argument defines (#12376 removes the read).
argparse.Namespace.seed_sync_gaps_diagnostic = False
from stand_command_hurt_corpus import LOW_HEALTH_CASES, stand_command_hurt, stand_low_health

DEFAULT_LOW_HEALTH_WITNESS = Path(__file__).resolve().parent / 'fixtures/26.3/stand_low_health.tsv'


def plain_message(row):
    return ''.join(span[0] for line in row['messages'] for span in line)


def parse_low_health_witness(ops, rows):
    grouped = {}
    order = []
    for op, row in zip(ops, rows):
        meta = op.get('witness')
        if meta is None:
            continue
        name, nbt, tag, step_name, field = meta
        key = (name, tag)
        if key not in grouped:
            grouped[key] = {'case': name, 'nbt': nbt, 'step': step_name}
            order.append(key)
        entry = grouped[key]
        text = plain_message(row)
        results = row['results']
        if field == 'alive':
            assert len(results) == 1 and results[0][0] in (True, False), (op, row)
            entry['alive'] = 1 if results[0][0] else 0
        elif field == 'health':
            if results and results[0][0]:
                match = re.search(r'has the following entity data: ([-0-9.eE+]+)f$', text)
                assert match, (op, text)
                bits = struct.unpack('>I', struct.pack('>f', float(match.group(1))))[0]
                entry['health_bits'] = f'{bits:08x}'
            else:
                entry['health_bits'] = '-'
        elif field == 'deathtime':
            if results and results[0][0]:
                match = re.search(r'has the following entity data: (-?\d+)s$', text)
                assert match, (op, text)
                entry['death_time'] = match.group(1)
            else:
                entry['death_time'] = '-'
        elif field == 'drops':
            assert len(results) == 1, (op, row)
            entry['drop_count'] = results[0][1] if results[0][0] else 0
        elif field == 'drop_item':
            if results and results[0][0]:
                match = re.search(r'id: "([^"]+)"', text)
                assert match, (op, text)
                entry['drop_item'] = match.group(1)
            else:
                entry['drop_item'] = '-'
    case_order = {name: i for i, (name, *_) in enumerate(LOW_HEALTH_CASES)}
    by_case = {name: [] for name, *_ in LOW_HEALTH_CASES}
    for key in order:
        by_case[key[0]].append(grouped[key])
    lines = [
        '# Minecraft 26.3 vanilla armor stand low-health and DeathTime witness (recorded by tools/test_stand_command_hurt_parity.py --record-low-health)',
        'case\tnbt\tstep\talive\thealth_bits\tdeath_time\tdrop_count\tdrop_item',
    ]
    for name in sorted(by_case, key=case_order.__getitem__):
        for entry in by_case[name]:
            lines.append('\t'.join([
                entry['case'],
                entry['nbt'],
                entry['step'],
                str(entry['alive']),
                entry['health_bits'],
                entry['death_time'],
                str(entry['drop_count']),
                entry['drop_item'],
            ]))
    return '\n'.join(lines) + '\n'


if __name__ == '__main__':
    full = '--full-diagnostic' in sys.argv
    if full:
        sys.argv.remove('--full-diagnostic')
    low_health = '--low-health' in sys.argv
    if low_health:
        sys.argv.remove('--low-health')
    record_low_health = '--record-low-health' in sys.argv
    if record_low_health:
        sys.argv.remove('--record-low-health')
        low_health = True
        if '--reference-only' not in sys.argv:
            sys.argv.append('--reference-only')
    witness_out = DEFAULT_LOW_HEALTH_WITNESS
    if '--witness-out' in sys.argv:
        i = sys.argv.index('--witness-out')
        witness_out = Path(sys.argv[i + 1]).resolve()
        del sys.argv[i:i + 2]
    reference = None
    if '--reference' in sys.argv:
        i = sys.argv.index('--reference')
        reference = Path(sys.argv[i + 1]).resolve()
        del sys.argv[i:i + 2]
    if '--work-dir' not in sys.argv:
        default_dir = 'stand-low-health-parity' if low_health else 'stand-command-hurt-parity'
        sys.argv += ['--work-dir', str(driver.lab.LOCAL / default_dir)]
    build = Path(sys.argv[sys.argv.index('--work-dir') + 1]).resolve()
    ops = stand_low_health(full) if low_health else stand_command_hurt(full)
    group_name = 'stand low health' if low_health else 'stand command hurt'
    driver.corpus_module.groups = lambda: [(group_name, ops)]
    sys.argv.append('--allow-gaps')
    driver.main()
    recorded = [op for op in ops if op['op'] == 'cmd' and op.get('record', True)]
    java = [json.loads(s) for s in (build / 'vanilla.jsonl').read_text().splitlines()]
    assert len(java) == len(recorded)
    for op, row in zip(recorded, java):
        if 'expect_results' in op:
            assert row['results'] == op['expect_results'], ('Java action/callback admission', op, row)
    if record_low_health:
        witness_out.parent.mkdir(parents=True, exist_ok=True)
        witness_out.write_text(parse_low_health_witness(recorded, java), encoding='utf-8')
        print(f'Recorded vanilla 26.3 witness: {witness_out}')
    if '--reference-only' not in sys.argv:
        source = build / 'vanilla.jsonl' if full else (reference or build / 'vanilla.jsonl')
        if reference and not full:
            assert source.read_bytes() == (build / 'vanilla.jsonl').read_bytes(), 'fresh Java bytes changed'
        expected = [json.loads(s) for s in source.read_text().splitlines()]
        native = [json.loads(s) for s in (build / 'rust.jsonl').read_text().splitlines()]
        assert len(expected) == len(native) == len(recorded)
        for op, a, b in zip(recorded, expected, native):
            if op['admission'] and not full:
                assert a == b, ('actual actor/state/control admission', op, a, b)
        differences = [{'label': op['label'], 'expected': a, 'actual': b}
                       for op, a, b in zip(recorded, expected, native) if a != b]
        report = {'records': len(native), 'passed': len(native) - len(differences),
                  'comparison': 'complete raw records, no masks/tolerances',
                  'reference_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
                  'fresh_java_sha256': hashlib.sha256((build / 'vanilla.jsonl').read_bytes()).hexdigest(),
                  'differences': differences}
        (build / 'strict-report.json').write_text(json.dumps(report, indent=2) + '\n')
        print(f'Strict raw records: {report["passed"]}/{report["records"]}')
        if differences:
            raise SystemExit(1)
