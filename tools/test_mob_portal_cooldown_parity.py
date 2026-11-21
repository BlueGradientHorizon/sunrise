#!/usr/bin/env python3
"""Compare a mob's live PortalCooldown across summon, tick, chunk reload, and /data edits."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

try:
    from . import lab
    from .command_exec_corpus import cmd, setup
except ImportError:
    import lab
    from command_exec_corpus import cmd, setup

INITIAL_COOLDOWN = 20
MERGED_COOLDOWN = 50
WITNESS_PATH = 'tools/witness/26.3/mob_portal_cooldown.json'


def steps():
    # Use a distant chunk beyond player tickets so /forceload exercises real chunk save and load.
    chunk = '4096 4096'
    wait = {'op': 'wait_chunks', 'chunks': [[256, 256]]}
    live = '@e[type=pig,tag=mob_portal_cooldown_live,limit=1]'
    plain = '@e[type=pig,tag=mob_portal_cooldown_plain,limit=1]'
    return [
        *((None, op) for op in setup(f'/forceload add {chunk}')),
        (None, wait),
        (None, cmd(
            f'/summon minecraft:pig 4100.5 300 4104.5 '
            f'{{NoAI:1b,NoGravity:1b,PortalCooldown:{INITIAL_COOLDOWN},Tags:["mob_portal_cooldown_live"]}}'
        )),
        ('after_summon', cmd(f'/data get entity {live} PortalCooldown')),
        (None, {'op': 'tick', 'n': 1}),
        ('after_first_tick', cmd(f'/data get entity {live} PortalCooldown')),
        *((None, op) for op in setup(f'/forceload remove {chunk}')),
        (None, {'op': 'tick', 'n': 60}),
        (None, cmd('/execute unless loaded 4096 300 4096', expect_result=1)),
        (None, cmd(f'/execute unless entity {live}', expect_result=1)),
        *((None, op) for op in setup(f'/forceload add {chunk}')),
        (None, wait),
        (None, cmd(f'/execute if entity {live}', expect_result=1)),
        ('after_chunk_reload', cmd(f'/data get entity {live} PortalCooldown')),
        (None, {'op': 'tick', 'n': 1}),
        ('after_reload_and_tick', cmd(f'/data get entity {live} PortalCooldown')),
        (None, cmd(f'/data merge entity {live} {{PortalCooldown:{MERGED_COOLDOWN}}}')),
        ('after_data_merge', cmd(f'/data get entity {live} PortalCooldown')),
        (None, {'op': 'tick', 'n': 1}),
        ('after_merge_and_tick', cmd(f'/data get entity {live} PortalCooldown')),
        (None, cmd(f'/data remove entity {live} PortalCooldown')),
        ('after_data_remove', cmd(f'/data get entity {live} PortalCooldown')),
        *((None, op) for op in setup(f'/kill {live}', f'/forceload remove {chunk}')),
        (None, cmd(
            '/summon minecraft:pig 12.5 234 8.5 '
            '{NoAI:1b,NoGravity:1b,Tags:["mob_portal_cooldown_plain"]}'
        )),
        ('plain_after_summon', cmd(f'/data get entity {plain} PortalCooldown')),
        (None, {'op': 'tick', 'n': 1}),
        ('plain_after_first_tick', cmd(f'/data get entity {plain} PortalCooldown')),
        *((None, op) for op in setup(f'/kill {plain}')),
    ]


def groups():
    return [('mob portal cooldown', [op for _, op in steps()])]


def extract_witness(vanilla_jsonl):
    recorded_steps = [
        (label, op) for label, op in steps()
        if (op['op'] == 'cmd' and op.get('record', True)) or (op['op'] == 'tick' and op.get('record'))
    ]
    rows = [json.loads(line) for line in vanilla_jsonl.read_text(encoding='utf-8').splitlines() if line.strip()]
    if len(rows) != len(recorded_steps):
        raise SystemExit(f'expected {len(recorded_steps)} recorded vanilla rows, got {len(rows)}')
    expected = {}
    for (label, op), row in zip(recorded_steps, rows):
        if row.get('cmd') != op.get('cmd'):
            raise SystemExit(f'command mismatch: expected {op.get("cmd")!r}, got {row.get("cmd")!r}')
        if label is None:
            continue
        results = row.get('results')
        if not (isinstance(results, list) and len(results) == 1
                and isinstance(results[0], list) and len(results[0]) == 2
                and results[0][0] is True and type(results[0][1]) is int):
            raise SystemExit(f'probe {label!r} did not return a single integer result: {row}')
        expected[label] = results[0][1]
    return {
        'minecraft': lab.VERSION,
        'oracle': 'dev.sunriselab.parity.VanillaCommandExecOracle',
        'harness': 'tools/test_mob_portal_cooldown_parity.py',
        'record_cmd': 'python3 tools/test_mob_portal_cooldown_parity.py --record',
        'inputs': {
            'initial_cooldown': INITIAL_COOLDOWN,
            'merged_cooldown': MERGED_COOLDOWN,
        },
        'expected': expected,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--reference-only', action='store_true')
    parser.add_argument('--record', action='store_true')
    parser.add_argument('--work-dir', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    work = (args.work_dir or root / '.local' / 'mob-portal-cooldown-parity').resolve()
    work.mkdir(parents=True, exist_ok=True)
    cases = work / 'groups.json'
    cases.write_text(json.dumps(groups()), encoding='utf-8')
    run = [sys.executable, str(root / 'tools/test_command_exec_parity.py'),
           '--groups-file', str(cases), '--work-dir', str(work)]
    if args.binary:
        run.extend(['--binary', str(args.binary.resolve())])
    if args.prepare:
        run.append('--prepare')
    if args.reference_only or args.record:
        run.append('--reference-only')
    result = subprocess.run(run, cwd=root)
    if result.returncode:
        return result.returncode
    witness = extract_witness(work / 'vanilla.jsonl')
    witness_file = root / WITNESS_PATH
    if args.record:
        lab.write_json(witness_file, witness)
        print(f'Recorded {lab.VERSION} witness: {witness_file}')
        return 0
    committed = json.loads(witness_file.read_text(encoding='utf-8'))
    if committed != witness:
        raise SystemExit(
            f'live {lab.VERSION} vanilla run disagrees with {WITNESS_PATH}: '
            f'live={witness["expected"]} committed={committed.get("expected")}'
        )
    return 0


if __name__ == '__main__':
    sys.exit(main())
