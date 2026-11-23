#!/usr/bin/env python3
"""Bounced arrows and continuing wind charges keep their place in the 26.3 tick order. Dedicated servers.

`ServerLevel` ticks its entities in the order they joined (`EntityTickList`). An arrow that hits an entity it can't
hurt bounces off (`AbstractArrow.onHitEntity`, `deflect(REVERSE)`) without ever leaving the level, and a wind charge
that flies on after earlier charges break a boat in the same tick also stays in its original slot. Both still tick
before every entity that joined after them.

A vanilla 26.3 dedicated server and Sunrise's each load a copy of one flat world vanilla generated first. Ticking is
frozen. Four isolated lanes run together:
- Arrow bounce (`arrow`, `c1`, `c2`): falling arrow bounces off `c1` on tick 1 and meets rising `c2` on tick 3.
- Wind-charge continuation (`w6_main`, `a2_main`): 5 wind charges break an oak boat on tick 1; `w6_main` flies on and
  clears `x = 8.0` before falling `a2_main` ticks on tick 4.
- Four-charge control (`w6_ctrl4`): 4 wind charges leave the boat intact, so `w6_ctrl4` bursts on tick 1.
- Reversed-order control (`w6_rev`, `a2_rev`): `a2_rev` joins before `w6_rev`, so on tick 4 `a2_rev` bounces off and
  redirects `w6_rev`.

Pass `--record-wind-witness` to record the vanilla 26.3 wind-charge continuation witness file without building Sunrise.
"""
import os
from pathlib import Path
import re
import sys
import time

try:
    from . import lab, test_locator_bar_parity as locator, test_rust_parity as base
    from .test_dedicated_server_parity import Server
except ImportError:
    import lab
    import test_locator_bar_parity as locator
    import test_rust_parity as base
    from test_dedicated_server_parity import Server

NAME = 'bounced-projectile-tick-order-parity'
TICKS = 5
ARROW_TAGS = ['arrow', 'c2']
WIND_TAGS = ['w6_main', 'a2_main', 'w6_ctrl4', 'w6_rev', 'a2_rev']
TAGS = [*ARROW_TAGS, *WIND_TAGS]
ARROW_SETUP = [
    'summon minecraft:arrow 4.5 -50.0 4.5 {Motion:[0.0d,-0.2d,0.0d],pickup:0b,Tags:["arrow"]}',
    'summon minecraft:breeze_wind_charge 4.5 -50.2625 4.5 {Motion:[0.6d,0.0d,0.0d],acceleration_power:0.0d,Tags:["c1"]}',
    'summon minecraft:breeze_wind_charge 4.5 -51.33 4.5 {Motion:[0.0d,0.5d,0.0d],acceleration_power:0.0d,Tags:["c2"]}',
]
WIND_SETUP = [
    'summon minecraft:oak_boat 4.5 -60.0 8.5 {Tags:["boat_main"]}',
    *['summon minecraft:wind_charge 4.5 -59.2 8.5 {Motion:[0.0d,-0.5d,0.0d],acceleration_power:0.0d}'] * 5,
    'summon minecraft:breeze_wind_charge 3.5 -59.7 8.5 {Motion:[1.5d,0.0d,0.0d],acceleration_power:0.0d,Tags:["w6_main"]}',
    'summon minecraft:arrow 8.0 -58.6 8.5 {Motion:[0.0d,-0.2d,0.0d],pickup:0b,Tags:["a2_main"]}',
    'summon minecraft:oak_boat 4.5 -60.0 24.5 {Tags:["boat_ctrl4"]}',
    *['summon minecraft:wind_charge 4.5 -59.2 24.5 {Motion:[0.0d,-0.5d,0.0d],acceleration_power:0.0d}'] * 4,
    'summon minecraft:breeze_wind_charge 3.5 -59.7 24.5 {Motion:[1.5d,0.0d,0.0d],acceleration_power:0.0d,Tags:["w6_ctrl4"]}',
    'summon minecraft:oak_boat 4.5 -60.0 40.5 {Tags:["boat_rev"]}',
    *['summon minecraft:wind_charge 4.5 -59.2 40.5 {Motion:[0.0d,-0.5d,0.0d],acceleration_power:0.0d}'] * 5,
    'summon minecraft:arrow 8.0 -58.6 40.5 {Motion:[0.0d,-0.2d,0.0d],pickup:0b,Tags:["a2_rev"]}',
    'summon minecraft:breeze_wind_charge 3.5 -59.7 40.5 {Motion:[1.5d,0.0d,0.0d],acceleration_power:0.0d,Tags:["w6_rev"]}',
]
SETUP = ['time set 6000', *ARROW_SETUP, *WIND_SETUP]
WIND_WITNESS = lab.ROOT / 'tools/witness/26.3/wind_charge_continuation_tick_order.txt'
NUMBER = re.compile(r'(-?[0-9][0-9.E-]*)d')


def run(label, command, world, build, *, setup=SETUP, tags=TAGS, ticks=TICKS):
    directory = build / label
    locator.server_dir(directory, world)
    server = Server(command, directory, label)
    lines = []
    try:
        server.wait_for(r'Done \(', timeout=600)
        server.console('tick freeze')
        server.console('forceload add 0 0 31 63')
        server.console('say READY')
        server.wait_for(r'READY', timeout=120)
        time.sleep(3)
        for command_line in setup:
            server.console(command_line)
        time.sleep(1)
        for tick in range(ticks + 1):
            if tick:
                server.console('tick step 1')
                time.sleep(1.0)
            start = len(server.lines)
            for tag in tags:
                for path in ('Pos', 'Motion'):
                    server.console(f'data get entity @e[tag={tag},limit=1] {path}')
            server.console(f'say END {tick}')
            server.wait_for(rf'END {tick}\b', timeout=120)
            answers = [l.split('INFO]: ', 1)[-1] for l in server.lines[start:]
                       if 'entity data:' in l or 'No entity was found' in l or 'Found no' in l]
            for i, tag in enumerate(tags):
                pos, motion = answers[2 * i:2 * i + 2] if len(answers) >= 2 * i + 2 else ('', '')
                if 'entity data:' not in pos:
                    lines.append(f'tick {tick} {tag} gone')
                else:
                    lines.append(f'tick {tick} {tag} pos {" ".join(NUMBER.findall(pos))}'
                                 f' motion {" ".join(NUMBER.findall(motion))}')
    finally:
        server.stop()
    return lines


def motion(lines, tick, tag):
    line = next(l for l in lines if l.startswith(f'tick {tick} {tag} '))
    return [float(x) for x in line.split('motion ')[1].split()] if ' motion ' in line else None


def motion_y(lines, tick, tag):
    vel = motion(lines, tick, tag)
    return vel[1] if vel is not None else None


def proves_wind(vanilla):
    """Why vanilla's wind-charge continuation lanes do not prove tick order, or None."""
    if motion(vanilla, 1, 'w6_ctrl4') is not None:
        return 'the four-charge control did not burst on the intact boat'
    main_v = motion(vanilla, 4, 'w6_main')
    rev_v = motion(vanilla, 4, 'w6_rev')
    a2_main = motion_y(vanilla, 4, 'a2_main')
    a2_rev = motion_y(vanilla, 4, 'a2_rev')
    if main_v is None or rev_v is None or a2_main is None or a2_rev is None:
        return 'a wind-charge continuation entity disappeared unexpectedly'
    if not rev_v[0] > main_v[0]:
        return 'the reversed-order control did not redirect w6_rev on tick 4'
    if not a2_rev > a2_main:
        return 'the reversed-order control did not bounce a2_rev on tick 4'
    return None


def proves(vanilla):
    """Why vanilla's transcript does not show what this harness is for, or None."""
    ys = [motion_y(vanilla, t, 'arrow') for t in range(TICKS + 1)]
    if None in ys:
        return 'the arrow is gone'
    if not ys[2] < 0:
        return 'the arrow was not falling again after its first bounce'
    # A bounce turns the fall to a tenth of it, upwards, before inertia and gravity.
    if not ys[3] > ys[2]:
        return 'the arrow did not bounce off the second wind charge on the third tick'
    return proves_wind(vanilla)


def record_wind_witness():
    lock, jars, libraries = base.verify_artifacts()
    deps = os.pathsep.join(map(str, [jars['server'], *libraries]))
    vanilla_server = ['java', '-Xmx2g', '-Duser.timezone=UTC', '-cp', deps, 'net.minecraft.server.Main', 'nogui']
    build = lab.LOCAL / f'{NAME}-record'
    build.mkdir(parents=True, exist_ok=True)
    world = locator.generate_world(vanilla_server, build)
    vanilla = run('vanilla', vanilla_server, world, build, setup=['time set 6000', *WIND_SETUP], tags=WIND_TAGS, ticks=4)
    why = proves_wind(vanilla)
    if why:
        raise AssertionError(f'vanilla is no proof: {why}:\n' + '\n'.join(vanilla))
    header = [
        f'# Vanilla {lock["minecraft"]["version"]} dedicated-server witness for wind-charge continuation tick order.',
        '# Recorded by tools/test_bounced_projectile_tick_order_parity.py --record-wind-witness.',
    ]
    WIND_WITNESS.parent.mkdir(parents=True, exist_ok=True)
    WIND_WITNESS.write_text('\n'.join([*header, *vanilla]) + '\n', encoding='utf-8')
    print(f'recorded {len(vanilla)} lines to {WIND_WITNESS}')


def main():
    if '--record-wind-witness' in sys.argv[1:]:
        record_wind_witness()
        return
    lock, build, vanilla_server, sunrise_server, _client = locator.sides(NAME)
    world = locator.generate_world(vanilla_server, build)
    vanilla = run('vanilla', vanilla_server, world, build)
    (build / 'vanilla.txt').write_text('\n'.join(vanilla) + '\n', encoding='utf-8')
    why = proves(vanilla)
    if why:
        raise AssertionError(f'vanilla is no proof: {why}:\n' + '\n'.join(vanilla))
    sunrise = run('sunrise', sunrise_server, world, build)
    (build / 'sunrise.txt').write_text('\n'.join(sunrise) + '\n', encoding='utf-8')
    if vanilla != sunrise:
        differ = [f'- {v}\n+ {h}' for v, h in zip(vanilla, sunrise) if v != h]
        raise AssertionError(f'{len(differ)} lines differ from vanilla:\n' + '\n'.join(differ[:20]))
    print(f'bounced projectile tick order: {len(vanilla)} lines identical')


if __name__ == '__main__':
    try:
        main()
    except AssertionError as error:
        sys.exit(f'{NAME}: {error}')
