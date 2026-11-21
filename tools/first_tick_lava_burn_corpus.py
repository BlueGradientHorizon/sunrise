#!/usr/bin/env python3
"""Minecraft 26.3 first-tick lava burn corpus and vanilla witness recorder."""
import argparse
import json
from pathlib import Path
import struct
import subprocess
import sys

GROUP = 'mob first-tick lava burn'
FIRES = (-1, 0, 1, 19, 20, 21, 40, 320, 321)
TICKS = (1, 2, 10, 11)
SPOTS = ((2, 2), (7, 2), (12, 2), (2, 7), (7, 7), (12, 7), (2, 12), (7, 12), (12, 12))
WITNESS = Path(__file__).resolve().parent / 'witness/26.3/first_tick_lava_burn.tsv'


def first_tick_lava_burn(cmd, setup):
    """Summon NoAI pigs in lava with initial Fire values and read HurtTime, Fire and Health."""
    x0, y0, z0 = 3760, 260, 3776
    # Entities tick once the forced chunk's 2-column neighbourhood is loaded.
    chunks = [[(x0 >> 4) + i, (z0 >> 4) + k] for i in range(-2, 3) for k in range(-2, 3)]
    ops = [
        cmd(f'/forceload add {x0} {z0}', False),
        {'op': 'wait_chunks', 'chunks': chunks},
        cmd('/tick freeze', False),
        cmd(f'/fill {x0} {y0 - 1} {z0} {x0 + 15} {y0 + 2} {z0 + 15} air', False),
        cmd(f'/fill {x0} {y0 - 1} {z0} {x0 + 15} {y0} {z0 + 15} stone', False),
    ]
    for i, ((dx, dz), fire) in enumerate(zip(SPOTS, FIRES), 1):
        x, z = x0 + dx, z0 + dz
        ops.append(cmd(f'/setblock {x} {y0} {z} minecraft:lava', False))
        ops.append(cmd(
            f'/summon minecraft:pig {x}.5 {y0} {z}.5 '
            f'{{UUID:[I;2674,0,0,{i}],NoAI:1b,PersistenceRequired:1b,Fire:{fire}s}}',
            expect_result=1,
        ))
    done = 0
    for target in TICKS:
        ops += [cmd('/tick unfreeze', False), {'op': 'tick', 'n': target - done}, cmd('/tick freeze', False)]
        done = target
        for i in range(1, len(FIRES) + 1):
            uid = f'00000a72-0000-0000-0000-{i:012x}'
            for field in ('HurtTime', 'Fire', 'Health'):
                ops.append(cmd(f'/data get entity {uid} {field}'))
    return ops + setup(
        '/tick unfreeze',
        f'/kill @e[type=!player,x={x0},y={y0 - 4},z={z0},dx=16,dy=12,dz=16]',
        f'/fill {x0} {y0 - 1} {z0} {x0 + 15} {y0 + 2} {z0 + 15} air',
        f'/forceload remove {x0} {z0}',
    )


def _snbt_value(row):
    text = ''.join(span[0] for span in row['messages'][0])
    return text.split(': ', 1)[1]


def record_witness(work_dir=None, out=WITNESS):
    """Run VanillaCommandExecOracle on the 26.3 server jar and write the TSV witness."""
    root = Path(__file__).resolve().parents[1]
    scratch = Path(work_dir).resolve() if work_dir else root / '.local/first-tick-lava-burn-witness'
    subprocess.run(
        [sys.executable, str(root / 'tools/test_command_exec_parity.py'),
         '--only', GROUP, '--reference-only', '--work-dir', str(scratch)],
        cwd=root,
        check=True,
    )
    rows = [json.loads(line) for line in (scratch / 'vanilla.jsonl').read_text(encoding='utf-8').splitlines()]
    expected_count = len(FIRES) + len(TICKS) * len(FIRES) * 3
    if len(rows) != expected_count:
        raise RuntimeError(f'expected {expected_count} vanilla records, got {len(rows)}')
    idx = len(FIRES)
    lines = [
        '# vanilla 26.3 witness recorded via VanillaCommandExecOracle (python3 tools/first_tick_lava_burn_corpus.py --record)',
        'initial_fire\tticks\thurt_time\tfire\thealth_bits',
    ]
    for target in TICKS:
        for i, initial_fire in enumerate(FIRES, 1):
            uid = f'00000a72-0000-0000-0000-{i:012x}'
            values = {}
            for field in ('HurtTime', 'Fire', 'Health'):
                row = rows[idx]
                idx += 1
                expected_cmd = f'/data get entity {uid} {field}'
                if row['cmd'] != expected_cmd:
                    raise RuntimeError(f'unexpected record {row["cmd"]!r}, want {expected_cmd!r}')
                values[field] = _snbt_value(row)
            hurt_time = int(values['HurtTime'].removesuffix('s'))
            fire = int(values['Fire'].removesuffix('s'))
            health = float(values['Health'].removesuffix('f'))
            health_bits = struct.pack('>f', health).hex()
            lines.append(f'{initial_fire}\t{target}\t{hurt_time}\t{fire}\t{health_bits}')
    out = Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print(f'Wrote {len(lines) - 2} 26.3 witness rows to {out}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true', help='record the 26.3 witness via VanillaCommandExecOracle')
    parser.add_argument('--work-dir', type=Path, help='scratch directory for the Java oracle run')
    parser.add_argument('--out', type=Path, default=WITNESS, help='output TSV path')
    args = parser.parse_args()
    if not args.record:
        parser.error('pass --record to generate the 26.3 witness')
    record_witness(args.work_dir, args.out)


if __name__ == '__main__':
    main()
