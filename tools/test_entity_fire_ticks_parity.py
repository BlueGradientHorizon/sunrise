#!/usr/bin/env python3
"""Compare Fire ticking on eyes, fangs, ominous spawners, clouds and shulker bullets against vanilla 26.3."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

try:
    from .command_exec_corpus import cmd, setup
except ImportError:
    from command_exec_corpus import cmd, setup


WITNESS_PATH = Path('tools/witness/26.3/entity_fire_ticks.tsv')

# (entity_type, extra_nbt) for each of the five #2683 entity types.
ENTITIES = (
    ('minecraft:eye_of_ender', ''),
    ('minecraft:evoker_fangs', 'Warmup:100'),
    ('minecraft:ominous_item_spawner', 'spawn_item_after_ticks:100L,item:{id:"minecraft:stone",count:1}'),
    ('minecraft:area_effect_cloud', 'Duration:200'),
    ('minecraft:shulker_bullet', 'NoGravity:1b'),
)

INITIAL_FIRES = (40, 21, 20, 1, 0, -5)
TICK_STEPS = (0, 1, 2)
DATA_SHORT_RE = re.compile(r'has the following entity data: (-?\d+)s$')


def cases():
    out = []
    index = 1
    for kind, extra in ENTITIES:
        for fire in INITIAL_FIRES:
            suffix = f',{extra}' if extra else ''
            nbt = f'{{Fire:{fire}s{suffix}}}'
            uuid_int = f'[I;2683,0,0,{index}]'
            uuid_str = f'00000a7b-0000-0000-0000-{index:012x}'
            out.append((kind, fire, extra, nbt, uuid_int, uuid_str))
            index += 1
    return out


def groups():
    ops = []
    for kind, fire, extra, _nbt, uuid_int, uuid_str in cases():
        suffix = f',{extra}' if extra else ''
        summon_nbt = f'{{UUID:{uuid_int},Fire:{fire}s{suffix}}}'
        ops.append(cmd(f'/summon {kind} 8.5 220 8.5 {summon_nbt}', False))
        prev = 0
        for tick in TICK_STEPS:
            if tick > prev:
                ops.append({'op': 'tick', 'n': tick - prev})
                prev = tick
            ops.append(cmd(f'/execute if entity {uuid_str}'))
            ops.append(cmd(f'/data get entity {uuid_str} Fire'))
        ops.extend(setup(f'/kill {uuid_str}'))
    return [('entity fire ticks 26.3', ops)]


def parse_vanilla_rows(vanilla_jsonl: Path):
    records = [json.loads(line) for line in vanilla_jsonl.read_text(encoding='utf-8').splitlines() if line.strip()]
    case_list = cases()
    expected_records = len(case_list) * len(TICK_STEPS) * 2
    if len(records) != expected_records:
        raise RuntimeError(f'expected {expected_records} records in {vanilla_jsonl}, got {len(records)}')
    rows = []
    cursor = 0
    for kind, initial_fire, _extra, nbt, _uuid_int, uuid_str in case_list:
        for tick in TICK_STEPS:
            alive_rec = records[cursor]
            fire_rec = records[cursor + 1]
            cursor += 2
            if alive_rec['cmd'] != f'/execute if entity {uuid_str}':
                raise RuntimeError(f'unexpected alive cmd: {alive_rec["cmd"]}')
            if fire_rec['cmd'] != f'/data get entity {uuid_str} Fire':
                raise RuntimeError(f'unexpected fire cmd: {fire_rec["cmd"]}')
            alive_ok, alive_count = alive_rec['results'][0]
            alive = bool(alive_ok and alive_count == 1)
            if alive:
                fire_ok, fire_val = fire_rec['results'][0]
                if not fire_ok:
                    raise RuntimeError(f'alive entity {kind} Fire={initial_fire} tick={tick} failed /data get: {fire_rec}')
                msg = ''.join(span[0] for span in fire_rec['messages'][0])
                match = DATA_SHORT_RE.search(msg)
                if not match:
                    raise RuntimeError(f'could not parse Fire short from {msg!r}')
                parsed = int(match.group(1))
                if parsed != fire_val:
                    raise RuntimeError(f'Fire value mismatch: text={parsed} callback={fire_val}')
                fire_out = str(parsed)
            else:
                fire_out = 'gone'
            rows.append((kind, str(initial_fire), nbt, str(tick), 'alive' if alive else 'gone', fire_out))
    return rows


def format_witness(rows):
    lines = [
        '# vanilla 26.3 witness recorded via tools/test_entity_fire_ticks_parity.py --record',
        '# oracle: VanillaCommandExecOracle (tools/test_command_exec_parity.py --reference-only)',
        'kind\tinitial_fire\tnbt\tticks\tstate\tfire',
    ]
    for row in rows:
        lines.append('\t'.join(row))
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--reference-only', action='store_true')
    parser.add_argument('--record', action='store_true', help='run vanilla 26.3 oracle only and write the witness TSV')
    parser.add_argument('--work-dir', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    work = (args.work_dir or root / '.local' / 'entity-fire-ticks-parity').resolve()
    work.mkdir(parents=True, exist_ok=True)
    cases_path = work / 'groups.json'
    cases_path.write_text(json.dumps(groups()), encoding='utf-8')
    run = [sys.executable, str(root / 'tools/test_command_exec_parity.py'),
           '--groups-file', str(cases_path), '--work-dir', str(work)]
    if args.binary:
        run.extend(['--binary', str(args.binary.resolve())])
    if args.prepare:
        run.append('--prepare')
    if args.reference_only or args.record:
        run.append('--reference-only')
    rc = subprocess.run(run, cwd=root).returncode
    if rc:
        return rc
    rendered = format_witness(parse_vanilla_rows(work / 'vanilla.jsonl'))
    target = root / WITNESS_PATH
    if args.record:
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(rendered, encoding='utf-8')
        print(f'Recorded {len(rendered.splitlines()) - 3} vanilla 26.3 witness rows to {WITNESS_PATH}')
    else:
        recorded = target.read_text(encoding='utf-8')
        if rendered != recorded:
            sys.exit(f'Vanilla 26.3 output differs from committed witness {WITNESS_PATH}; re-run with --record')
    return 0


if __name__ == '__main__':
    sys.exit(main())
