#!/usr/bin/env python3
"""Two-player arrow-hit ClientboundGameEventPacket parity against vanilla 26.3.

Vanilla 26.3 and Sunrise dedicated servers each serve a copy of one flat world
vanilla generated first, with Alice and Bob joined through LocatorBarClient
(vanilla's official 26.3 packet codecs). With ticks frozen, arrows owned by
Alice or Bob fall into Bob under five conditions: another player's non-silent
arrow, another player's silent arrow, Bob's own arrow, another player's arrow
with the pvp game rule disabled, and another player's arrow hitting a creative
victim. Every ClientboundGameEventPacket either player receives during each
step is recorded from the wire and compared exact.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys
import time
import uuid

try:
    from . import lab, test_locator_bar_parity as net, test_rust_parity as base
    from .test_dedicated_server_parity import Server, game_time
except ImportError:
    import lab
    import test_locator_bar_parity as net
    import test_rust_parity as base
    from test_dedicated_server_parity import Server, game_time

NAME = 'arrow-hit-ding-parity'
WITNESS_PATH = 'tools/witness/26.3/arrow_hit_ding.json'
ORACLE_SOURCE = 'mods/sunrise/src/test/java/dev/sunriselab/parity/LocatorBarClient.java'
PLAYERS = ['Alice', 'Bob']
# (case name, shooter player name, Silent flag, pvp game rule, victim game mode)
CASES = [
    ('other_player', 'Alice', False, True, 'survival'),
    ('silent_arrow', 'Alice', True, True, 'survival'),
    ('own_arrow', 'Bob', False, True, 'survival'),
    ('pvp_off', 'Alice', False, False, 'survival'),
    ('creative_victim', 'Alice', False, True, 'creative'),
]
HEALTH_RE = re.compile(r'has the following entity data:\s*(-?[0-9.]+)f')


def offline_uuid_ints(name):
    digest = bytearray(hashlib.md5(f'OfflinePlayer:{name}'.encode()).digest())
    digest[6] = (digest[6] & 0x0F) | 0x30
    digest[8] = (digest[8] & 0x3F) | 0x80
    raw = uuid.UUID(bytes=bytes(digest)).bytes
    return list(struct.unpack('>4i', raw))


def float_bits_hex(value):
    return f'{struct.unpack(">I", struct.pack(">f", float(value)))[0]:08x}'


def compile_java_sides(build):
    lock, jars, libraries = base.verify_artifacts()
    build.mkdir(parents=True, exist_ok=True)
    classes = build / 'classes'
    shutil.rmtree(classes, ignore_errors=True)
    classes.mkdir()
    dependencies = os.pathsep.join(map(str, [jars['server'], *libraries]))
    subprocess.run(
        ['javac', '--release', str(lock['minecraft']['java_major']), '-proc:none', '-nowarn',
         '-sourcepath', lab.ROOT / 'mods/sunrise/src/test/java', '-cp', dependencies,
         '-d', classes, lab.ROOT / ORACLE_SOURCE],
        cwd=lab.ROOT, check=True, capture_output=True,
    )
    vanilla_server = ['java', '-Xmx2g', '-Duser.timezone=UTC', '-cp', dependencies, 'net.minecraft.server.Main', 'nogui']
    client = ['java', '-Xmx1g', '-cp', str(classes) + os.pathsep + dependencies, 'dev.sunriselab.parity.LocatorBarClient']
    return lock, vanilla_server, client


def configure_net():
    net.PLAYERS = list(PLAYERS)
    net.PROPERTIES = dict(
        net.PROPERTIES,
        **{
            'server-ip': '127.0.0.1',
            'gamemode': 'survival',
            'force-gamemode': 'true',
            'difficulty': 'normal',
            'view-distance': '2',
            'simulation-distance': '2',
            'max-tick-time': '-1',
        },
    )


def run_side(label, server_command, client_command, build, world):
    directory = build / label
    port = net.server_dir(directory, world)
    out = build / f'{label}.txt'
    out.unlink(missing_ok=True)
    server = Server(server_command, directory, label)
    serial = 0

    def console_reply(command):
        nonlocal serial
        serial += 1
        marker = f'arrow_ding_barrier_{serial}'
        start = len(server.lines)
        server.console(command)
        server.console(f'say {marker}')
        server.wait_for(re.escape(f'[Server] {marker}') + '$', timeout=120)
        return server.lines[start:]

    def step_ticks(count):
        start = game_time(server)
        console_reply(f'tick step {count}')
        deadline = time.monotonic() + 60
        while game_time(server) < start + count:
            if time.monotonic() > deadline:
                raise AssertionError(f'{label}: tick step {count} timed out')
            time.sleep(0.02)
        if game_time(server) != start + count:
            raise AssertionError(f'{label}: expected game_time {start + count}, got {game_time(server)}')

    def read_health(player_name):
        lines = console_reply(f'data get entity {player_name} Health')
        for line in reversed(lines):
            match = HEALTH_RE.search(line)
            if match:
                return float(match.group(1))
        raise AssertionError(f'{label}: could not read Health for {player_name}: {lines}')

    def entity_present(selector):
        lines = console_reply(f'execute if entity {selector}')
        return any('Test passed' in line for line in lines)

    case_health = {}
    try:
        server.wait_for(r'Done \(')
        with (build / f'{label}-client.log').open('w') as log:
            client = net.Client(client_command, port, out, log)
            try:
                client.tell('step join')
                for name in PLAYERS:
                    client.tell(f'join {name}')
                    server.wait_for(rf'{name} joined the game', timeout=120)
                client.wait(server, PLAYERS, 10)
                console_reply('gamerule spawn_mobs false')
                console_reply('gamerule fall_damage false')
                console_reply('forceload add 0 0 15 15')
                deadline = time.monotonic() + 60
                while not any('Test passed' in line for line in console_reply('execute if loaded 8 200 8')):
                    if time.monotonic() > deadline:
                        raise AssertionError(f'{label}: arena chunk did not load')
                console_reply('fill 0 199 0 15 199 15 minecraft:stone')
                console_reply('fill 0 200 0 15 215 15 minecraft:air')
                console_reply('tp Alice 2.5 200.0 8.5')
                console_reply('tp Bob 8.5 200.0 8.5')
                console_reply('gamemode survival Alice')
                console_reply('gamemode survival Bob')
                console_reply('tick freeze')
                step_ticks(2)
                client.sync(PLAYERS)
                client.tell('game-events on')
                for case_name, shooter, silent, pvp, victim_mode in CASES:
                    client.tell(f'step setup_{case_name}')
                    console_reply('kill @e[type=minecraft:arrow]')
                    console_reply('gamemode survival Bob')
                    console_reply('effect give Bob minecraft:instant_health 1 10 true')
                    step_ticks(1)
                    console_reply('effect clear Bob')
                    console_reply(f'gamemode {victim_mode} Bob')
                    console_reply('tp Alice 2.5 200.0 8.5')
                    console_reply('tp Bob 8.5 200.0 8.5')
                    # Step past the 20-tick hurt invulnerability window before the next arrow lands.
                    step_ticks(20)
                    console_reply(f'gamerule pvp {"true" if pvp else "false"}')
                    before_health = read_health('Bob')
                    if before_health != 20.0:
                        raise AssertionError(f'{label}/{case_name}: expected full health 20.0, got {before_health}')
                    client.sync(PLAYERS)
                    client.tell(f'step {case_name}')
                    owner_ints = ','.join(map(str, offline_uuid_ints(shooter)))
                    nbt = (
                        f'{{Owner:[I;{owner_ints}],LeftOwner:1b,Silent:{int(silent)}b,'
                        f'NoGravity:1b,Motion:[0.0d,-1.0d,0.0d],damage:2.0d,'
                        f'Tags:["arrow_ding_{case_name}"]}}'
                    )
                    console_reply(f'summon minecraft:arrow 8.5 203.0 8.5 {nbt}')
                    if not entity_present(f'@e[type=minecraft:arrow,tag=arrow_ding_{case_name}]'):
                        raise AssertionError(f'{label}/{case_name}: summoned arrow missing')
                    step_ticks(5)
                    after_health = read_health('Bob')
                    arrow_still_exists = entity_present(f'@e[type=minecraft:arrow,tag=arrow_ding_{case_name}]')
                    client.sync(PLAYERS)
                    case_health[case_name] = {
                        'before': before_health,
                        'after': after_health,
                        'arrow_still_exists': arrow_still_exists,
                    }
                client.end()
            finally:
                client.kill()
    finally:
        server.stop()
    if not out.exists():
        raise AssertionError(f'{label}: client wrote no transcript (see {build}/{label}-client.log)')
    raw_lines = out.read_text(encoding='utf-8').splitlines()
    errors = [line for line in raw_lines if ' error ' in f' {line} ' or 'disconnected' in line]
    if errors:
        raise AssertionError(f'{label}: client errors: {errors[:10]}')
    events_by_case = {case_name: {'Alice': [], 'Bob': []} for case_name, *_ in CASES}
    for line in raw_lines:
        parts = line.split(' ', 3)
        if len(parts) == 4 and parts[2] == 'game-event':
            player = parts[0]
            step = parts[1].removeprefix('step=')
            if step in events_by_case and player in events_by_case[step]:
                events_by_case[step][player].append(json.loads(parts[3]))
    records = []
    for case_name, shooter, silent, pvp, victim_mode in CASES:
        health = case_health[case_name]
        hurt = health['after'] < health['before']
        expects_hurt = pvp and victim_mode == 'survival'
        if expects_hurt and not hurt:
            raise AssertionError(f'{label}/{case_name}: arrow did not hurt Bob ({health})')
        if not expects_hurt and (hurt or not health['arrow_still_exists']):
            raise AssertionError(f'{label}/{case_name}: refused hit hurt Bob or vanished ({health})')
        records.append({
            'name': case_name,
            'shooter': shooter,
            'victim': 'Bob',
            'silent': silent,
            'pvp': pvp,
            'victim_mode': victim_mode,
            'victim_hurt': hurt,
            'victim_health_before_bits': float_bits_hex(health['before']),
            'victim_health_after_bits': float_bits_hex(health['after']),
            'alice_game_events': events_by_case[case_name]['Alice'],
            'bob_game_events': events_by_case[case_name]['Bob'],
        })
    return records


def comparable(records):
    return [
        {
            'name': row['name'],
            'shooter': row['shooter'],
            'victim': row['victim'],
            'silent': row['silent'],
            'pvp': row['pvp'],
            'victim_mode': row['victim_mode'],
            'victim_hurt': row['victim_hurt'],
            'victim_health_before_bits': row['victim_health_before_bits'],
            'victim_health_after_bits': row['victim_health_after_bits'],
            'alice_game_events': [
                {'event': e['event'], 'param_bits': e['param_bits'], 'payload_hex': e['payload_hex']}
                for e in row['alice_game_events']
            ],
            'bob_game_events': [
                {'event': e['event'], 'param_bits': e['param_bits'], 'payload_hex': e['payload_hex']}
                for e in row['bob_game_events']
            ],
        }
        for row in records
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true', help='run vanilla 26.3 only and write the witness JSON')
    parser.add_argument('--binary', type=Path, help='prebuilt Sunrise server binary')
    parser.add_argument('--witness', type=Path, default=lab.ROOT / WITNESS_PATH, help='witness JSON path')
    args = parser.parse_args()
    configure_net()
    build = lab.LOCAL / NAME
    if args.record:
        lock, vanilla_server, client = compile_java_sides(build)
        world = net.generate_world(vanilla_server, build)
        vanilla = run_side('vanilla', vanilla_server, client, build, world)
        witness = {
            'schema': 1,
            'minecraft_version': lock['minecraft']['version'],
            'java_version': subprocess.check_output(['java', '--version'], text=True).splitlines()[0],
            'harness': 'tools/test_arrow_hit_ding_parity.py',
            'oracle_client': ORACLE_SOURCE,
            'cases': vanilla,
        }
        args.witness.parent.mkdir(parents=True, exist_ok=True)
        args.witness.write_text(json.dumps(witness, indent=2) + '\n', encoding='utf-8')
        print(f'Recorded {len(vanilla)} vanilla {lock["minecraft"]["version"]} cases to {args.witness}')
        return 0
    lock, build, vanilla_server, sunrise_server, client = net.sides(NAME, args.binary)
    world = net.generate_world(vanilla_server, build)
    vanilla = run_side('vanilla', vanilla_server, client, build, world)
    sunrise = run_side('sunrise', sunrise_server, client, build, world)
    if comparable(vanilla) != comparable(sunrise):
        raise AssertionError(
            f'Sunrise differs from vanilla (see {build}/vanilla.txt and sunrise.txt):\n'
            f'vanilla={json.dumps(comparable(vanilla), indent=2)}\n'
            f'sunrise={json.dumps(comparable(sunrise), indent=2)}'
        )
    if args.witness.exists():
        stored = json.loads(args.witness.read_text(encoding='utf-8'))
        if comparable(vanilla) != comparable(stored['cases']):
            raise AssertionError(f'committed witness {args.witness} disagrees with fresh vanilla run')
    lab.write_json(
        build / 'report.json',
        {
            'schema': 1,
            'minecraft': lock['minecraft']['version'],
            'platform': platform.platform(),
            'java': subprocess.check_output(['java', '--version'], text=True).splitlines()[0],
            'cases': len(vanilla),
            'output_sha256': lab.digest(build / 'sunrise.txt', 'sha256'),
            'scope': 'ClientboundGameEventPacket sent to shooter and victim on a player-owned arrow hit.',
        },
    )
    print(f'arrow hit ding: {len(vanilla)} cases identical to vanilla {lock["minecraft"]["version"]}')
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except AssertionError as error:
        sys.exit(f'{NAME}: {error}')
