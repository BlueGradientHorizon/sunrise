#!/usr/bin/env python3
"""Compare live mob equipment immunity, Resistance credit, and cooldown wake-up."""
import argparse
import json
import os
from pathlib import Path
import struct
import subprocess

try:
    from . import lab, oracle_harness, test_rust_parity as base
    from .integration_env import run_game
except ImportError:
    import lab
    import oracle_harness
    import test_rust_parity as base
    from integration_env import run_game

RECORDING = 'mob_damage_outcome'


def scene():
    ops = []
    fields = {}

    def command(cmd, field=None, result=None):
        ops.append({'op': 'cmd', 'cmd': '/' + cmd, 'record': field is not None})
        if field is not None:
            fields[(len(fields), ops[-1]['cmd'])] = field, result

    command('tick freeze')
    command('gamerule spawn_mobs false')
    command('forceload add 0 0')
    ops.append({'op': 'settle', 'required_chunks': [[0, 0]]})
    command('tp @s 14.5 100 14.5')
    command('fill 0 99 0 15 99 15 stone')
    boot = '{id:"minecraft:leather_boots",count:1,components:{' \
           '"minecraft:attribute_modifiers":[],"minecraft:enchantments":{"minecraft:frost_walker":1}}}'
    for n, (name, slot, damage, accepts) in enumerate([
        ('immune', 'feet', 'campfire', False),
        ('wrong_source', 'feet', 'in_fire', True),
        ('wrong_slot', 'mainhand', 'campfire', True),
    ], 1):
        uuid = f'000000cb-0000-0000-0000-{n:012x}'
        x = n * 3
        command(f'summon pig {x + .5} 100 5.5 {{UUID:[I;203,0,0,{n}],NoAI:1b,NoGravity:1b,'
                f'Health:10f,equipment:{{{slot}:{boot}}}}}', f'{name}/summon', True)
        command(f'execute if data entity {uuid} equipment.{slot}.components."minecraft:enchantments".'
                '"minecraft:frost_walker"', f'{name}/equipped', True)
        command(f'damage {uuid} 2 minecraft:{damage} at {x + 1.5} 100 5.5', f'{name}/hit', accepts)
        for key in ['Health', 'HurtTime', 'Motion[0]', 'Motion[1]', 'Motion[2]']:
            command(f'data get entity {uuid} {key}', f'{name}/{key}', True)
    command('scoreboard objectives add resisted minecraft.custom:minecraft.damage_dealt_resisted')
    command('scoreboard players set @s resisted 0')
    arrow = '000000cb-0000-0000-0000-000000000010'
    command('summon arrow 14.5 100 5.5 {UUID:[I;203,0,0,16],NoGravity:1b}', 'projectile/summon', True)
    other = '000000cb-0000-0000-0000-000000000011'
    command('summon pig 14.5 100 8.5 {UUID:[I;203,0,0,17],NoAI:1b}', 'non_player/summon', True)
    for n, (name, damage, source) in enumerate([
        ('melee', 'player_attack', 'by @s'),
        ('projectile', 'arrow', f'by {arrow} from @s'),
        ('non_player', 'arrow', f'by {arrow} from {other}'),
        ('direct_player_only', 'player_attack', f'by @s from {other}'),
        ('bypass', 'out_of_world', 'by @s'),
        ('rounded_zero', 'player_attack', 'by @s'),
    ], 32):
        uuid = f'000000cb-0000-0000-0000-{n:012x}'
        command(f'summon pig {n - 28}.5 100 12.5 {{UUID:[I;203,0,0,{n}],NoAI:1b,NoGravity:1b}}',
                f'{name}/victim', True)
        command(f'effect give {uuid} minecraft:resistance infinite 0 true', f'{name}/effect', True)
        amount = '0.125' if name == 'rounded_zero' else '5.25'
        command(f'damage {uuid} {amount} minecraft:{damage} {source}', f'{name}/hit', True)
        command(f'data get entity {uuid} Health', f'{name}/Health', True)
        ops.append({'op': 'cmd', 'cmd': '/scoreboard players get @s resisted'})
        fields[(len(fields), ops[-1]['cmd'])] = f'{name}/stat', True

    villager = '000000cb-0000-0000-0000-000000000040'
    command('setblock 8 100 8 red_bed[part=head,facing=north,occupied=true] strict')
    command('setblock 8 100 9 red_bed[part=foot,facing=north,occupied=true] strict')
    command('setblock 9 100 8 stone strict')
    command('summon villager 8.5 100 8.5 {UUID:[I;203,0,0,64],NoAI:1b,NoGravity:1b}', 'wake/summon', True)
    command(f'damage {villager} 4 minecraft:generic at 10.5 100 8.5', 'wake/seed_hit', True)
    command(f'data merge entity {villager} {{sleeping_pos:[I;8,100,8]}}', 'wake/sleep', True)
    ops.append({'op': 'mob_damage_state', 'uuid': villager, 'no_action_time': 611, 'label': 'wake/before'})
    fields[(len(fields), 'wake/before')] = 'wake/before', None
    command(f'damage {villager} 1 minecraft:generic at 10.5 100 8.5', 'wake/weak_hit', False)
    ops.append({'op': 'mob_damage_state', 'uuid': villager, 'label': 'wake/after'})
    fields[(len(fields), 'wake/after')] = 'wake/after', None
    command(f'execute unless data entity {villager} sleeping_pos', 'wake/cleared', True)
    command('execute if block 8 100 8 red_bed[part=head,facing=north,occupied=false]', 'wake/bed', True)
    command(f'data get entity {villager} Brain.memories."minecraft:last_woken".value', 'wake/last_woken', True)
    return ops, fields


def observed(path, fields):
    records = [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]
    if [(i, row['cmd']) for i, row in enumerate(records)] != list(fields):
        raise ValueError('Incomplete or reordered mob damage recording')
    observations = {}
    for i, row in enumerate(records):
        field, expected_success = fields[(i, row['cmd'])]
        if 'state' in row:
            state = row['state']
            if field == 'wake/before':
                # A just-loaded sleeper refreshes its bed position on its first tick;
                # this fixture deliberately does not tick between loading and the hit.
                state = {k: v for k, v in state.items() if k not in ('position', 'rotation')}
            observations.update((f'{field}/{k}', v) for k, v in state.items())
            continue
        result, = row['results']
        value = result
        if field.endswith(('/Health', '/HurtTime', '/Motion[0]', '/Motion[1]', '/Motion[2]')):
            # /data get prints the exact SNBT scalar, while the command result floors floats.
            message, = row['messages']
            scalar = ''.join(segment[0] for segment in message).rsplit(': ', 1)[-1]
            number, suffix = scalar[:-1], scalar[-1]
            if field.endswith('/Health') and suffix.lower() == 'f':
                value = struct.pack('>f', float(number)).hex()
            elif '/Motion[' in field and suffix.lower() == 'd':
                value = struct.pack('>d', float(number)).hex()
            elif field.endswith('/HurtTime') and suffix.lower() == 's':
                value = int(number)
            else:
                raise ValueError(f'Unexpected scalar type for {field}')
        if path.name == 'vanilla.jsonl' and result[0] != expected_success:
            raise ValueError(f'Vanilla fixture prerequisite failed: {field}')
        observations[field] = value
    return observations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work-dir', type=Path, default=lab.LOCAL / RECORDING)
    parser.add_argument('--reference-only', action='store_true')
    parser.add_argument('--binary', type=Path, help='use an already built Sunrise game')
    args = parser.parse_args()
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=False)
    ops, fields = scene()
    script = work / 'scene.jsonl'
    script.write_text(''.join(json.dumps(op) + '\n' for op in ops), encoding='utf-8')
    lock, jars, libraries = base.verify_artifacts()
    dependencies = os.pathsep.join(map(str, [jars['server'], *libraries]))
    classes = work / 'classes'
    oracle = lab.ROOT / 'mods/sunrise/src/test/java/dev/sunriselab/parity/VanillaCommandExecOracle.java'
    oracle_harness.run_logged(['javac', '--release', lock['minecraft']['java_major'], '-proc:none',
                               '-cp', dependencies, '-d', classes, oracle], work, 'javac')
    vanilla = work / 'vanilla.jsonl'
    oracle_harness.java_run(work, classes, dependencies, 'VanillaCommandExecOracle',
                            [work / 'universe', script, vanilla, '1'], 'java',
                            jvm=['-Dsunrise.parity.flat=true'])
    expected = observed(vanilla, fields)
    if not (expected['wake/before/sleeping'] and not expected['wake/after/sleeping']
            and expected['wake/before/cooldown'] == expected['wake/after/cooldown']
            and expected['wake/before/health'] == expected['wake/after/health']
            and expected['wake/before/no_action_time'] == 611 and expected['wake/after/no_action_time'] == 0):
        raise ValueError('Vanilla wake fixture did not reject a weak hit while waking the mob')
    if args.reference_only:
        print(f'{RECORDING}: Minecraft {lock["minecraft"]["version"]} reference captured')
        return 0

    binary = args.binary
    if binary is None:
        oracle_harness.run_logged(['cargo', 'build', '--locked', '--profile', 'fast',
                                   '-p', 'sunrise-game', '--bin', 'sunrise-client'], work, 'build')
        binary = lab.binary(lab.cargo_target() / 'fast', 'sunrise-client')
    scratch = work / 'native'
    scratch.mkdir()
    native = work / 'native.jsonl'
    env = lab.game_env(SUNRISE_COMMAND_SCRIPT=script, SUNRISE_COMMAND_OUTPUT=native,
                       SUNRISE_PLAYER_NAME='Player', SUNRISE_API_DISCOVERY='http://127.0.0.1:9/')
    with (work / 'native.log').open('w', encoding='utf-8') as log:
        status = run_game([str(binary.resolve(strict=True)), '--resources', str(lab.RESOURCES),
                           '--world', 'harness', '--seed', '1', '--radius', '4'],
                          cwd=scratch, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=900)
    if status.returncode:
        raise RuntimeError('Native fixture failed to run; see private native.log')
    actual = observed(native, fields)
    differences = {k: [expected[k], actual[k]] for k in expected if expected[k] != actual[k]}
    (work / 'differences.json').write_text(json.dumps(differences, indent=2) + '\n', encoding='utf-8')
    print(f'{RECORDING}: {"FAIL" if differences else "PASS"}')
    return bool(differences)


if __name__ == '__main__':
    raise SystemExit(main())
