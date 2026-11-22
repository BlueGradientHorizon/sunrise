#!/usr/bin/env python3
"""Real locator packets for player effect refreshes and saved effect promotion."""
import argparse
from pathlib import Path
import subprocess

import test_command_exec_parity as commands
import test_locator_bar_parity as locator


NAME = 'effect-waypoint-parity'
HIDE = ('attributes[{id:"minecraft:waypoint_transmit_range"}]'
        '.modifiers[{id:"minecraft:effect.waypoint_transmit_range_hide"}]')
SAVED = 'active_effects[{id:"minecraft:invisibility"'
FUNCTION = {'parity:refresh_waypoint': '''effect give @s minecraft:invisibility 30 0 true
effect give @s minecraft:invisibility 60 0 true
effect clear @s minecraft:invisibility
'''}
SCRIPT = [
    ('watcher', [], ['chat on', 'join Alice']),
    ('loaded', [], ['join Bob']),
    ('positioned', [], ['cmd Bob tp @s 6.5 -60 0.5']),
    ('promotion-input', [], ['send-cmd Alice execute as Bob if data entity @s '
                            + SAVED + ',hidden_effect:{duration:-1}}] unless data entity @s ' + HIDE
                            + ' run tellraw Alice {"text":"promotion-input-confirmed"}']),
    ('saved-promotion', [], []),
    ('promotion-output', [], ['send-cmd Alice execute as Bob if data entity @s ' + SAVED + ',duration:-1}] '
                             'unless data entity @s active_effects[0].hidden_effect '
                             'if data entity @s ' + HIDE + ' run tellraw Alice {"text":"promotion-output-confirmed"}']),
    ('clear-promoted', [], ['cmd Bob effect clear @s minecraft:invisibility']),
    ('pumpkin-default', [], ['cmd Bob item replace entity @s armor.head with minecraft:carved_pumpkin']),
    ('pumpkin-cleared', [], ['cmd Bob item replace entity @s armor.head with '
                            'minecraft:carved_pumpkin[!minecraft:attribute_modifiers]']),
    ('same-tick-refresh', [], ['cmd Bob function parity:refresh_waypoint']),
]
CHECKED = {'saved-promotion', 'clear-promoted', 'pumpkin-default', 'pumpkin-cleared', 'same-tick-refresh'}


def recording(lines):
    return [line for line in lines if (parts := line.split())[:1] == ['Alice']
            and len(parts) >= 4 and parts[1].removeprefix('step=') in CHECKED
            and parts[2] in ('track', 'untrack', 'update') and parts[3] == 'Bob']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--work-dir', type=Path)
    parser.add_argument('--reference-only', action='store_true')
    args = parser.parse_args()
    if args.reference_only and args.binary is None:
        parser.error('--reference-only needs --binary to skip the game build')
    name = args.work_dir.resolve() if args.work_dir else NAME
    binary = args.binary.resolve() if args.binary else None
    lock, build, java, native, client = locator.sides(name, binary)
    world = locator.generate_world(java, build)
    # Two servers load the identical synthetic save through PlayerDataStorage.
    playerdata = world / 'players' / 'data'
    playerdata.mkdir(parents=True, exist_ok=True)
    with (build / 'write-player-fixture.log').open('w') as log:
        source = locator.lab.ROOT / 'mods/sunrise/src/test/java/dev/sunriselab/parity/WaypointEffectSave.java'
        subprocess.run(['java', '-cp', java[java.index('-cp') + 1], source, playerdata],
                       stdout=log, stderr=subprocess.STDOUT, check=True)
    server_jar = Path(java[java.index('-cp') + 1].split(locator.os.pathsep)[0])
    commands.install_datapack_fixtures(world, {}, FUNCTION, server_jar)

    def run(label, server):
        lines = locator.run_side(label, server, client, build, world, script=SCRIPT,
                                 step_ticks={'saved-promotion': 200})
        assert not any(' error ' in line or 'disconnected' in line for line in lines), label
        assert any(line.startswith('Alice step=positioned ') and ' Bob ' in line
                   and line.split()[2] in ('track', 'update') for line in lines), label
        assert any('promotion-input-confirmed' in line for line in lines), label
        return recording(lines), any('promotion-output-confirmed' in line for line in lines)

    reference, vanilla_promoted = run('vanilla', java)
    assert vanilla_promoted
    expected = [
        ('step=saved-promotion', 'untrack'),
        ('step=clear-promoted', 'track'),
        ('step=pumpkin-default', 'untrack'),
        ('step=pumpkin-cleared', 'track'),
        ('step=same-tick-refresh', 'untrack'),
        ('step=same-tick-refresh', 'track'),
    ]
    assert [tuple(line.split()[1:3]) for line in reference] == expected, reference
    if args.reference_only:
        print(f'Effect waypoint packets: real {lock["minecraft"]["version"]} reference guards passed')
        return 0
    actual, native_promoted = run('sunrise', native)
    passed = native_promoted and actual == reference
    locator.lab.write_json(build / 'report.json', {'minecraft': lock['minecraft']['version'],
                           'pass': passed, 'promotion': native_promoted,
                           'vanilla_packets': reference, 'sunrise_packets': actual})
    print(f'Effect waypoint packets: {"PASS" if passed else "FAIL"} — {build / "report.json"}')
    return int(not passed)


if __name__ == '__main__':
    raise SystemExit(main())
