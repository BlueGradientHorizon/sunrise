#!/usr/bin/env python3
"""Compare the client ticks of spawners, trial spawners and vaults with vanilla 26.3.

The Java oracle (VanillaBlockEntityClientTickOracle) runs BaseSpawner.clientTick,
TrialSpawner.tickClient and VaultBlockEntity.Client.tick as ClientLevel.tickBlockEntities does:
every loaded one, each tick, in the level's ticker order, sharing the level random. The game
test `game::spawner::tests::client_ticks_match_the_vanilla_cases` runs the same cases
through `Game::tick_remote_spawners`. Each tick compares every particle and local sound in
order, each block entity's spin and the level random's state, bit for bit.

Cases: a spawner at its required range's edge (inside, on, outside), with a dead or spectating
player; every trial spawner state, ominous or not, around a spawn and a 20-tick boundary; vaults
with and without a display item and a connected player around the particle range; block entities
far beyond 64 blocks, several sharing the level random, and a chunk unloaded and loaded again.

  test_block_entity_client_tick_parity.py [--filter SUBSTRING] [--detail N]
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

try:
    from . import lab, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, test_rust_parity as base, test_model_baking_parity as baking

PARITY = lab.ROOT / 'mods/sunrise/src/test/java/dev/sunriselab/parity'
TEST = 'game::spawner::tests::client_ticks_match_the_vanilla_cases'
TRIAL_STATES = ['inactive', 'waiting_for_players', 'active', 'waiting_for_reward_ejection', 'ejecting_reward', 'cooldown']
FACINGS = ['north', 'south', 'west', 'east']


def player(x, y, z, dead=False, spectator=False):
    return {'pos': [x, y, z], 'dead': dead, 'spectator': spectator}


def spawner(pos, range_=16, delay=20):
    return {'kind': 'spawner', 'pos': pos, 'range': range_, 'delay': delay}


def trial(pos, state, ominous=False, next_mob=0):
    return {'kind': 'trial_spawner', 'pos': pos, 'state': state, 'ominous': ominous, 'nextMobSpawnsAt': next_mob}


def vault(pos, facing='north', ominous=False, display=True, connected=True, state='active', particles_range=4.5):
    return {'kind': 'vault', 'pos': pos, 'facing': facing, 'ominous': ominous, 'display': display,
            'connected': connected, 'state': state, 'particlesRange': particles_range}


def cases():
    out = []

    def add(name, entities, who, ticks=24, game_time=1000, events=None):
        case = {'name': name, 'seed': 0x5EED0000 + len(out), 'gameTime': game_time, 'ticks': ticks,
                'player': who, 'entities': entities}
        if events:
            case['events'] = events
        out.append(case)

    # BaseSpawner.isNearPlayer: strictly closer than the range to the block's centre.
    for dx in (15.5, 16.0, 16.5, -15.25):
        add(f'spawner range {dx}', [spawner([8, 64, 8])], player(8.5 + dx, 64.5, 8.5))
    add('spawner diagonal', [spawner([8, 64, 8], range_=6)], player(8.5 + 4.2, 64.5, 8.5 - 4.2))
    add('spawner dead player', [spawner([8, 64, 8])], player(9.5, 64.0, 8.5, dead=True))
    add('spawner spectator', [spawner([8, 64, 8])], player(9.5, 64.0, 8.5, spectator=True))
    add('spawner counts down', [spawner([8, 64, 8], delay=3)], player(9.5, 64.0, 8.5), ticks=8)
    add('spawner any range', [spawner([8, 64, 8], range_=-1)], player(500.5, 64.0, 8.5))
    for state in TRIAL_STATES:
        for ominous in (False, True):
            add(f'trial {state} {ominous}', [trial([8, 64, 8], state, ominous, next_mob=1010)], player(9.5, 64.0, 8.5),
                ticks=24, game_time=995)
    for facing in FACINGS:
        add(f'vault {facing}', [vault([8, 64, 8], facing)], player(10.5, 64.0, 9.5), game_time=1000)
    add('vault ominous inactive', [vault([8, 64, 8], ominous=True, state='inactive', display=False)], player(9.5, 64.0, 8.5))
    # VaultBlockEntity.Client.isWithinConnectionRange: block positions, <= the range.
    for dx in (4, 5, -3):
        add(f'vault connection {dx}', [vault([8, 64, 8])], player(8.5 + dx, 64.0, 8.5), game_time=1000)
    add('vault not connected', [vault([8, 64, 8], connected=False)], player(9.5, 64.0, 8.5))
    # ClientLevel ticks every loaded block entity, however far from the player.
    far = [trial([8 + 70, 64, 8], 'cooldown'), vault([8 - 130, 64, 8 + 200]), trial([8, 64, 8 + 300], 'active')]
    add('far block entities', far, player(8.5, 64.0, 8.5), game_time=1000)
    # Several draw from the one level random, in the level's ticker order.
    many = [trial([1, 64, 1], 'active'), vault([3, 64, 1]), spawner([5, 64, 1]), trial([1, 64, 5], 'waiting_for_players'),
            vault([20, 64, 3], 'east', ominous=True), trial([3, 64, 12], 'cooldown', True), spawner([12, 64, 12])]
    add('shared random', many, player(6.5, 64.0, 4.5), game_time=1000)
    # A chunk that goes and comes back: its block entities stop, then tick again from a chunk packet.
    chunked = many + [trial([18, 64, 2], 'active'), vault([30, 64, 9], 'west'), spawner([17, 64, 14])]
    add('chunk reload', chunked, player(6.5, 64.0, 4.5), ticks=30, game_time=990,
        events={'6': [['unload', 0, 0]], '13': [['load', 0, 0]], '20': [['unload', 1, 0], ['load', 1, 0]]})
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--filter', default='')
    parser.add_argument('--detail', type=int, default=2)
    parser.add_argument('--resources', type=Path, default=Path(os.environ.get('SUNRISE_TEST_RESOURCES', lab.RESOURCES)))
    args = parser.parse_args()
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'block-entity-client-tick-parity'
    build.mkdir(parents=True, exist_ok=True)
    classes = build / 'classes'
    classes.mkdir(exist_ok=True)
    dependencies = os.pathsep.join(map(str, [jars['client'], *libraries]))
    chosen = [c for c in cases() if args.filter in c['name']]
    inputs = build / 'inputs.jsonl'
    inputs.write_text(''.join(json.dumps(c) + '\n' for c in chosen))

    def run(command, name, cwd=lab.ROOT, env=None):
        path = build / (name + '.log')
        with path.open('w') as log:
            result = subprocess.run(list(map(str, command)), cwd=cwd, env=env, timeout=3600, stdout=log,
                                    stderr=subprocess.STDOUT)
        if result.returncode:
            print(path.read_text()[-6000:])
            sys.exit(f'{name} failed')
        return path

    run(['javac', '--release', lock['minecraft']['java_major'], '-proc:none', '-nowarn', '-cp', dependencies, '-d', classes,
         PARITY / 'ParityData.java', PARITY / 'VanillaBlockEntityClientTickOracle.java'], 'compile-java')
    run(['java', '-Xmx1G', '-Djava.awt.headless=true', '-cp', str(classes) + os.pathsep + dependencies,
         'dev.sunriselab.parity.VanillaBlockEntityClientTickOracle', inputs, build / 'vanilla.jsonl'], 'vanilla')
    with tempfile.TemporaryDirectory(prefix='sunrise-client-tick-') as tmp:
        scratch = Path(tmp)
        homes = {k: scratch / k for k in ('SUNRISE_HOME', 'SUNRISE_LAUNCHER_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME')}
        env = lab.game_env(os.environ, **homes, RUST_MIN_STACK='16777216')
        compiled = run(['cargo', 'test', '--locked', '-p', 'sunrise-client', '--lib', '--no-run', '--message-format=json'],
                       'compile', env=env)
        binaries = []
        for line in compiled.read_text(encoding='utf-8').splitlines():
            if not line.startswith('{'):
                continue
            row = json.loads(line)
            if (row.get('reason') == 'compiler-artifact' and row.get('executable')
                    and row.get('target', {}).get('name') == 'sunrise_client' and row.get('profile', {}).get('test')):
                binaries.append(Path(row['executable']))
        assert len(binaries) == 1, 'expected one Game test binary'
        env['SUNRISE_TEST_RESOURCES'] = str(args.resources.resolve())
        env['SUNRISE_CLIENT_TICK_CASES'] = str(inputs)
        env['SUNRISE_CLIENT_TICK_OUT'] = str(build / 'rust.jsonl')
        log = run([binaries[0], TEST, '--exact', '--ignored', '--nocapture'], 'game', cwd=scratch, env=env)
        assert '1 passed; 0 failed' in log.read_text(encoding='utf-8'), 'the game test did not run'
    vanilla = [json.loads(l) for l in (build / 'vanilla.jsonl').read_text().splitlines()]
    rust = [json.loads(l) for l in (build / 'rust.jsonl').read_text().splitlines()]
    assert len(vanilla) == len(rust) == len(chosen), (len(vanilla), len(rust), len(chosen))
    failed = 0
    for case, v, r in zip(chosen, vanilla, rust):
        if 'error' in v:
            print(f'ORACLE ERROR {case["name"]}: {v["error"][:400]}')
            failed += 1
            continue
        for t, (vt, rt) in enumerate(zip(v['ticks'], r['ticks'])):
            # The oracle logs particles and sounds as they come; the game keeps them apart. The
            # level random's state after the tick still pins their draws' order.
            vt = dict(vt, particles=[e for e in vt['events'] if e[0] == 'particle'],
                      sounds=[e for e in vt['events'] if e[0] == 'sound'])
            for field in ('particles', 'sounds', 'spins', 'random'):
                if vt[field] != rt[field]:
                    # Names the tick and where the two first part, never vanilla's values.
                    if field == 'random':
                        print(f'MISMATCH {case["name"]}: tick {t} leaves the level random elsewhere')
                        failed += 1
                        break
                    at = next((i for i, (a, b) in enumerate(zip(vt[field], rt[field])) if a != b),
                              min(len(vt[field]), len(rt[field])))
                    print(f'MISMATCH {case["name"]}: tick {t} {field} differ from entry {at} '
                          f'({len(vt[field])} vanilla, {len(rt[field])} Sunrise)')
                    failed += 1
                    break
            else:
                continue
            break
        else:
            if len(v['ticks']) != len(r['ticks']):
                print(f'MISMATCH {case["name"]}: {len(v["ticks"])} vanilla ticks, {len(r["ticks"])} Sunrise ticks')
                failed += 1
    print(f'{len(chosen) - failed} of {len(chosen)} cases match')
    if failed:
        sys.exit(1)
    print('block entity client tick parity ok')


if __name__ == '__main__':
    main()
