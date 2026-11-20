#!/usr/bin/env python3
"""What a player holds on the cursor when they die, on a dedicated server, against vanilla's, seen by a vanilla client.

A vanilla dedicated server and Sunrise's (`sunrise-client --dedicated --nogui`) each serve a copy of one flat world
vanilla generated first, with Alice an operator in survival and keep_inventory off. LocatorBarClient (a player made
of vanilla's protocol code) is given seven diamonds, picks them up onto its inventory menu's cursor (a PICKUP click)
ten blocks from the spawn, runs /kill on itself, waits on the death screen and respawns (PERFORM_RESPAWN). Twenty
ticks after dying vanilla removes the dead player (LivingEntity.tickDeath), and Player.remove's
inventoryMenu.removed drops the cursor's stack where it died. Alice then asks whether an item lies around the death
site; the reply, and the inventory slots, respawns and teleports she is sent from the death on, are compared
exactly. Before the death sequence, the same client compares a matching prediction, a wrong damage hash in
the claimed slot, and a missing tool removal in the claimed cursor, using vanilla's HashedStack codec.
"""
import json
import sys

try:
    from . import test_locator_bar_parity as locator
except ImportError:
    import test_locator_bar_parity as locator

NAME = 'cursor-death-parity'
# server_dir and generate_world write these server.properties.
locator.PROPERTIES = dict(locator.PROPERTIES, gamemode='survival')
PICKAXE = 'minecraft:diamond_pickaxe[damage=5,!tool]'
PREDICTION_STEPS = ('prediction-match', 'prediction-slot', 'prediction-cursor', 'prediction-return')
SCRIPT = [
    ('setup', ['menus on', 'cmd Alice gamerule keep_inventory false', 'cmd Alice clear @s',
               'cmd Alice give @s minecraft:diamond 7', 'cmd Alice tp @s 10.5 -60 0.5',
               f'cmd Alice item replace entity @s hotbar.1 with {PICKAXE}']),
    # Pick up, put back, pick up, and put back one tool in inventory menu slot 37.
    ('prediction-match', [f'click-predicted Alice 0 37 - {PICKAXE}']),
    ('prediction-slot', ['click-predicted Alice 0 37 minecraft:diamond_pickaxe[damage=6,!tool] -']),
    ('prediction-cursor', ['click-predicted Alice 0 37 - minecraft:diamond_pickaxe[damage=5]']),
    ('prediction-return', [f'click-predicted Alice 0 37 {PICKAXE} -']),
    # Hotbar slot 0 is the inventory menu's slot 36.
    ('pickup', ['pickup Alice 36']),
    ('death', ['cmd Alice kill @s']),
    ('respawn', ['respawn Alice']),
    ('check', ['chat on', 'cmd Alice execute if entity @e[type=minecraft:item,x=10.5,y=-60,z=0.5,distance=..4]']),
]


def shown(lines):
    """Compare post-click menu packets and slots, respawns, teleports and chat after death."""
    out = []
    for line in lines:
        parts = line.split(' ', 3)
        if len(parts) != 4:
            continue
        player, step, kind, body = parts
        if step.removeprefix('step=') in PREDICTION_STEPS and kind == 'menu-wire':
            packet = json.loads(body)
            if packet['kind'] in ('slot', 'cursor', 'content'):
                # The latest state is supplied independently per server. Compare the
                # correction's kind, destination and full decoded payload, not that counter.
                del packet['wire']
                packet.pop('state', None)
                out.append(f'{player} {step} {kind} {json.dumps(packet, sort_keys=True)}')
        elif step in ('step=death', 'step=respawn', 'step=check') and kind in (
                'slot', 'inventory', 'respawn', 'teleport', 'chat'):
            out.append(line)
    return out


def check(vanilla, _):
    corrections = [(parts[1], json.loads(parts[3])) for line in vanilla
                   if (parts := line.split(' ', 3))[2] == 'menu-wire']
    if [(step, packet['kind']) for step, packet in corrections] != [
            ('step=prediction-slot', 'slot'), ('step=prediction-cursor', 'cursor')]:
        return 'expected only a slot correction for the bad hash and a cursor correction for the missing removal'
    slot, cursor = (p for _, p in corrections)
    if slot['window'] != 0 or slot['slot'] != 37 or slot['item'] is None or cursor['item'] != slot['item']:
        return 'expected the same tool in the slot and cursor corrections'
    if not any(l.startswith('Alice step=check chat') for l in vanilla):
        return 'vanilla answered no check command'
    return None


def main():
    lines = locator.one_player(NAME, SCRIPT, shown, check, 'A player dying with a stack on the inventory menu cursor '
                               '(keep_inventory off) and respawning on a dedicated server: the stack is dropped '
                               'where it died, not kept. Matching and mismatching hashed slot and cursor '
                               'predictions compare the corrections before death.')
    print(f'cursor death: {len(lines)} lines identical')


if __name__ == '__main__':
    try:
        main()
    except AssertionError as error:
        sys.exit(f'{NAME}: {error}')
