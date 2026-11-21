#!/usr/bin/env python3
"""Compare projectile Entity fields on summon, tick and chunk reload, and fireball hit-tick rotation."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

from command_exec_corpus import cmd, setup


FIELDS = ('Fire', 'Air', 'HasVisualFire', 'PortalCooldown')
KINDS = ('ender_pearl', 'small_fireball')
HIT_TURN_GROUP = 'fireball hit-tick rotation'
HIT_TURN_WITNESS = Path('tools/fixtures/26.3/fireball_hit_turn.tsv')
OWNER_UUID = '[I;2388,0,0,1]'
# (name, kind, mode, target_kind, target_pos, start_pos, motion, rotation, acceleration, uniquifier)
HIT_TURN_CASES = (
    ('dragon_owner_axis', 'dragon_fireball', 'owner', 'pig',
     (12.5, 240.0, 9.0), (12.5, 240.25, 8.25), (0.0, 0.0, 0.5), (0.0, 0.0), 0.1, None),
    ('dragon_owner_diag', 'dragon_fireball', 'owner', 'pig',
     (12.5, 240.0, 9.0), (12.25, 240.25, 8.25), (0.25, 0.125, 0.5), (45.0, -10.0), 0.1, None),
    ('small_breeze_axis', 'small_fireball', 'breeze', 'breeze',
     (14.5, 240.0, 9.0), (14.5, 240.5, 8.25), (0.0, 0.0, 0.5), (0.0, 0.0), 0.1, 23880),
    ('fireball_breeze_diag', 'fireball', 'breeze', 'breeze',
     (14.5, 240.0, 9.0), (14.25, 240.5, 8.25), (0.25, 0.125, 0.5), (45.0, -10.0), 0.1, 23881),
    ('wither_skull_breeze_axis', 'wither_skull', 'breeze', 'breeze',
     (14.5, 240.0, 9.0), (14.5, 240.5, 8.25), (0.0, 0.0, 0.5), (0.0, 0.0), 0.1, 23882),
)
NUMBER_RE = re.compile(r'-?[0-9]+(?:\.[0-9]+)?(?:[Ee][+-]?[0-9]+)?')
INT_RE = re.compile(r'-?[0-9]+')


def fields(target):
    return [cmd(f'/data get entity {target} {key}') for key in FIELDS]


def groups():
    summon = []
    for kind in KINDS:
        target = f'@e[type={kind},limit=1]'
        for saved in (
            '',
            ',Fire:100s,Air:5s,HasVisualFire:1b,PortalCooldown:100',
            ',Fire:65541,Air:65541,HasVisualFire:257,PortalCooldown:9.75d',
            ',Fire:-4s,Air:-5s,HasVisualFire:0b,PortalCooldown:-3',
            ',Fire:"bad",Air:"bad",HasVisualFire:"bad",PortalCooldown:"bad"',
        ):
            summon.extend([
                cmd(f'/summon {kind} 12.5 234 8.5 {{NoGravity:1b{saved}}}'),
                *fields(target),
                {'op': 'tick', 'n': 3},
                *fields(target),
                *setup(f'/kill {target}'),
            ])

    # A chunk beyond player tickets, with its own forced entity-ticking ticket.
    chunk = '4096 4096'
    wait = {'op': 'wait_chunks', 'chunks': [[256, 256]]}
    reloads = [*setup(f'/forceload add {chunk}'), wait]
    for index, kind in enumerate(KINDS):
        reloads.append(cmd(
            f'/summon {kind} {4100 + 4 * index}.5 300 4104.5 '
            '{NoGravity:1b,Fire:170s,Air:17s,HasVisualFire:1b,PortalCooldown:180}'
        ))
    reloads.append({'op': 'tick', 'n': 3})
    for kind in KINDS:
        reloads.extend(fields(f'@e[type={kind},limit=1]'))
    reloads.extend([
        *setup(f'/forceload remove {chunk}'),
        {'op': 'tick', 'n': 60},
        cmd('/execute unless loaded 4096 300 4096', expect_result=1),
        *(cmd(f'/execute unless entity @e[type={kind}]', expect_result=1) for kind in KINDS),
        *setup(f'/forceload add {chunk}'),
        wait,
    ])
    for kind in KINDS:
        target = f'@e[type={kind},limit=1]'
        reloads.extend([
            cmd(f'/execute if entity {target}', expect_result=1),
            *fields(target),
            *setup(f'/kill {target}'),
        ])
    reloads.extend(setup(f'/forceload remove {chunk}'))

    # Release the ticket before ticking and read the skull as soon as its
    # entity chunk reloads. Its first dry tick must come after that read.
    skull = '@e[type=wither_skull,limit=1]'
    skull_chunk = '4144 4096'
    wait_for_skull = {'op': 'wait_chunks', 'chunks': [[259, 256]]}
    skull_saved = [
        cmd(f'/data get entity {skull} Fire', expect_result=100),
        cmd(f'/data get entity {skull} Air', expect_result=23),
        cmd(f'/data get entity {skull} HasVisualFire', expect_result=1),
        cmd(f'/data get entity {skull} PortalCooldown', expect_result=90),
    ]
    skull_reload = [
        *setup(f'/forceload add {skull_chunk}'),
        wait_for_skull,
        cmd('/summon wither_skull 4152.5 300 4104.5 '
            '{NoGravity:1b,Fire:100s,Air:23s,HasVisualFire:1b,PortalCooldown:90}'),
        *skull_saved,
        *setup(f'/forceload remove {skull_chunk}'),
        {'op': 'tick', 'n': 60},
        cmd('/execute unless loaded 4144 300 4096', expect_result=1),
        cmd(f'/execute unless entity {skull}', expect_result=1),
        *setup(f'/forceload add {skull_chunk}'),
        wait_for_skull,
        cmd(f'/execute if entity {skull}', expect_result=1),
        *skull_saved,
        {'op': 'tick', 'n': 1},
        cmd(f'/data get entity {skull} Fire', expect_result=0),
        cmd(f'/data get entity {skull} Air', expect_result=23),
        cmd(f'/data get entity {skull} HasVisualFire', expect_result=1),
        cmd(f'/data get entity {skull} PortalCooldown', expect_result=89),
        *setup(f'/kill {skull}', f'/forceload remove {skull_chunk}'),
    ]

    block_effects = setup('/setblock 12 233 8 netherrack')
    for kind in (*KINDS, 'wither_skull'):
        target = f'@e[type={kind},limit=1]'
        block_effects.extend([
            *setup('/setblock 12 234 8 fire'),
            cmd(f'/summon {kind} 12.5 234.25 8.5 {{NoGravity:1b,HasVisualFire:1b}}'),
            {'op': 'tick', 'n': 1},
            cmd(f'/data get entity {target} Fire', expect_result=159),
            *setup('/setblock 12 234 8 water'),
            {'op': 'tick', 'n': 1},
            cmd(f'/data get entity {target} Fire', expect_result=20 if kind == 'small_fireball' else 0),
            cmd(f'/data get entity {target} HasVisualFire', expect_result=1),
            *setup(f'/kill {target}'),
        ])

    # Other entities that use the same ThrowableItemProjectile or Fireball codec.
    controls = []
    for kind in ('snowball', 'experience_bottle', 'fireball'):
        target = f'@e[type={kind},limit=1]'
        controls.extend([
            cmd(f'/summon {kind} 12.5 236 8.5 '
                '{NoGravity:1b,Fire:100s,Air:5s,HasVisualFire:1b,PortalCooldown:5}'),
            *fields(target),
            {'op': 'tick', 'n': 1},
            *fields(target),
            *setup(f'/kill {target}'),
        ])
    # These two use the shared portal timer, but have their own fire behavior.
    for kind in ('wind_charge', 'shulker_bullet'):
        target = f'@e[type={kind},limit=1]'
        controls.extend([
            cmd(f'/summon {kind} 12.5 236 8.5 {{NoGravity:1b,PortalCooldown:5}}'),
            cmd(f'/data get entity {target} PortalCooldown', expect_result=5),
            {'op': 'tick', 'n': 1},
            cmd(f'/data get entity {target} PortalCooldown', expect_result=4),
            *setup(f'/kill {target}'),
        ])
    hit_turn = hit_turn_ops()
    return [
        ('projectile common state', summon),
        ('projectile common state reload', reloads),
        ('wither skull chunk reload', skull_reload),
        ('projectile fire and water', block_effects),
        ('other projectile codecs', controls),
        (HIT_TURN_GROUP, hit_turn),
    ]


def hit_turn_ops():
    ops = setup('/fill 10 239 6 15 243 12 air', '/kill @e[type=!player]')
    owner_spawned = False
    breeze_spawned = False
    for name, kind, mode, target_kind, tpos, spos, mot, rot, acc, uniq in HIT_TURN_CASES:
        if mode == 'owner' and not owner_spawned:
            ops.extend(setup(
                f'/summon {target_kind} {tpos[0]} {tpos[1]} {tpos[2]} '
                f'{{NoAI:1b,Silent:1b,NoGravity:1b,UUID:{OWNER_UUID}}}'
            ))
            owner_spawned = True
        if mode == 'breeze' and not breeze_spawned:
            ops.extend(setup(
                f'/summon {target_kind} {tpos[0]} {tpos[1]} {tpos[2]} '
                '{NoAI:1b,Silent:1b,NoGravity:1b}'
            ))
            breeze_spawned = True
        if uniq is not None:
            ops.append({'op': 'sync_randoms', 'uniquifier': uniq, 'entity_id': uniq * 100, 'level_seed': 2388})
        nbt = (
            f'Motion:[{mot[0]}d,{mot[1]}d,{mot[2]}d],'
            f'Rotation:[{rot[0]}f,{rot[1]}f],'
            f'acceleration_power:{acc}d'
        )
        if mode == 'owner':
            nbt += f',LeftOwner:1b,Owner:{OWNER_UUID}'
        target = f'@e[type={kind},limit=1]'
        ops.extend([
            cmd(f'/summon {kind} {spos[0]} {spos[1]} {spos[2]} {{{nbt}}}'),
            {'op': 'tick', 'n': 1},
            cmd(f'/data get entity {target} Pos'),
            cmd(f'/data get entity {target} Motion'),
            cmd(f'/data get entity {target} Rotation'),
            cmd(f'/data get entity {target} acceleration_power'),
        ])
        if uniq is not None:
            ops.append(cmd(f'/data get entity {target} UUID'))
        ops.extend(setup(f'/kill {target}'))
    ops.extend(setup('/kill @e[type=!player]'))
    return ops


def entity_data_value(record, expected_cmd):
    if record.get('cmd') != expected_cmd:
        raise RuntimeError(f'command mismatch: expected {expected_cmd!r}, got {record.get("cmd")!r}')
    lines = [''.join(span[0] for span in line) for line in record.get('messages', [])]
    marker = ' has the following entity data: '
    if len(lines) != 1 or marker not in lines[0]:
        raise RuntimeError(f'unexpected /data get feedback for {expected_cmd!r}: {lines!r}')
    return lines[0].split(marker, 1)[1]


def parse_vec(text, count, suffix):
    inner = text.strip()
    if not (inner.startswith('[') and inner.endswith(']')):
        raise RuntimeError(f'expected SNBT list, got {text!r}')
    parts = [part.strip() for part in inner[1:-1].split(',')]
    if len(parts) != count or not all(part.endswith(suffix) for part in parts):
        raise RuntimeError(f'expected {count} {suffix}-suffixed values, got {text!r}')
    values = [part[:-len(suffix)] for part in parts]
    if not all(NUMBER_RE.fullmatch(value) for value in values):
        raise RuntimeError(f'non-numeric SNBT list entry in {text!r}')
    return values


def parse_scalar(text, suffix):
    value = text.strip()
    if not value.endswith(suffix) or not NUMBER_RE.fullmatch(value[:-len(suffix)]):
        raise RuntimeError(f'expected {suffix}-suffixed scalar, got {text!r}')
    return value[:-len(suffix)]


def parse_uuid(text):
    inner = text.strip()
    if not (inner.startswith('[I;') and inner.endswith(']')):
        raise RuntimeError(f'expected SNBT int array UUID, got {text!r}')
    parts = [part.strip() for part in inner[3:-1].split(',')]
    if len(parts) != 4 or not all(INT_RE.fullmatch(part) for part in parts):
        raise RuntimeError(f'expected 4 UUID ints, got {text!r}')
    return ','.join(parts)


def record_hit_turn_witness(all_groups, vanilla_path, out_path):
    records = [json.loads(line) for line in vanilla_path.read_text(encoding='utf-8').splitlines()]
    counts = [(name, sum(1 for op in ops if op['op'] == 'cmd' and op.get('record', True)))
              for name, ops in all_groups]
    if sum(n for _, n in counts) != len(records):
        raise RuntimeError(f'record count mismatch: expected {sum(n for _, n in counts)}, got {len(records)}')
    offset = 0
    group_records = None
    for name, n in counts:
        if name == HIT_TURN_GROUP:
            group_records = records[offset:offset + n]
            break
        offset += n
    if group_records is None:
        raise RuntimeError(f'missing group {HIT_TURN_GROUP!r}')
    rows = [
        '# Vanilla Minecraft 26.3 fireball hit-tick rotation witness (tools/test_projectile_common_state_parity.py --record)',
        '\t'.join([
            'case', 'kind', 'mode', 'target_kind',
            'target_x', 'target_y', 'target_z',
            'start_x', 'start_y', 'start_z',
            'motion_x', 'motion_y', 'motion_z',
            'start_yaw', 'start_pitch', 'start_accel', 'uniquifier',
            'end_x', 'end_y', 'end_z',
            'end_vx', 'end_vy', 'end_vz',
            'end_yaw', 'end_pitch', 'end_accel', 'uuid',
        ]),
    ]
    cursor = 0
    for name, kind, mode, target_kind, tpos, spos, mot, rot, acc, uniq in HIT_TURN_CASES:
        target = f'@e[type={kind},limit=1]'
        # Skip the /summon command record.
        cursor += 1
        pos = parse_vec(entity_data_value(group_records[cursor], f'/data get entity {target} Pos'), 3, 'd')
        cursor += 1
        vel = parse_vec(entity_data_value(group_records[cursor], f'/data get entity {target} Motion'), 3, 'd')
        cursor += 1
        end_rot = parse_vec(entity_data_value(group_records[cursor], f'/data get entity {target} Rotation'), 2, 'f')
        cursor += 1
        end_acc = parse_scalar(entity_data_value(group_records[cursor], f'/data get entity {target} acceleration_power'), 'd')
        cursor += 1
        uuid_text = '-'
        if uniq is not None:
            uuid_text = parse_uuid(entity_data_value(group_records[cursor], f'/data get entity {target} UUID'))
            cursor += 1
        # Verify the real vanilla 26.3 run actually hit the target on this tick.
        if not (spos[2] < float(pos[2]) < tpos[2]):
            raise RuntimeError(f'{name}: fireball did not stop on target box: pos={pos}')
        if mode == 'owner' and not (float(vel[2]) > 0.0 and float(end_acc) == acc):
            raise RuntimeError(f'{name}: owner-hit dragon fireball did not keep forward motion: vel={vel}, acc={end_acc}')
        if mode == 'breeze' and not (float(vel[2]) < 0.0 and float(end_acc) == acc * 0.5):
            raise RuntimeError(f'{name}: breeze-hit fireball did not deflect: vel={vel}, acc={end_acc}')
        if float(end_rot[0]) == rot[0] and float(end_rot[1]) == rot[1]:
            raise RuntimeError(f'{name}: fireball rotation did not change on hit tick: rot={end_rot}')
        rows.append('\t'.join([
            name, kind, mode, target_kind,
            *(str(v) for v in tpos),
            *(str(v) for v in spos),
            *(str(v) for v in mot),
            str(rot[0]), str(rot[1]), str(acc),
            '-' if uniq is None else str(uniq),
            *pos, *vel, *end_rot, end_acc, uuid_text,
        ]))
    if cursor != len(group_records):
        raise RuntimeError(f'unconsumed hit-turn records: {cursor} != {len(group_records)}')
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text('\n'.join(rows) + '\n', encoding='utf-8')
    print(f'Recorded {len(HIT_TURN_CASES)} vanilla 26.3 fireball hit-tick witness rows to {out_path}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--reference-only', action='store_true')
    parser.add_argument('--record', action='store_true',
                        help='record the vanilla 26.3 fireball hit-tick witness from Java only')
    parser.add_argument('--work-dir', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    work = (args.work_dir or root / '.local' / 'projectile-common-state-parity').resolve()
    work.mkdir(parents=True, exist_ok=True)
    all_groups = groups()
    cases = work / 'groups.json'
    cases.write_text(json.dumps(all_groups), encoding='utf-8')
    run = [sys.executable, str(root / 'tools/test_command_exec_parity.py'),
           '--groups-file', str(cases), '--work-dir', str(work)]
    if args.binary:
        run.extend(['--binary', str(args.binary.resolve())])
    if args.prepare:
        run.append('--prepare')
    if args.reference_only or args.record:
        run.append('--reference-only')
    code = subprocess.run(run, cwd=root).returncode
    if code == 0 and args.record:
        record_hit_turn_witness(all_groups, work / 'vanilla.jsonl', root / HIT_TURN_WITNESS)
    return code


if __name__ == '__main__':
    sys.exit(main())
