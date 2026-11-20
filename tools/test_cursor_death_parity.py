#!/usr/bin/env python3
"""What a player holds on the cursor when they die, on a dedicated server, against vanilla's, seen by a vanilla client.

A vanilla dedicated server and Sunrise's (`sunrise-client --dedicated --nogui`) each serve a copy of one flat world
vanilla generated first, with Alice an operator in survival and keep_inventory off. LocatorBarClient (a player made
of vanilla's protocol code) is given seven diamonds, picks them up onto its inventory menu's cursor (a PICKUP click)
ten blocks from the spawn, runs /kill on itself, waits on the death screen and respawns (PERFORM_RESPAWN). Twenty
ticks after dying vanilla removes the dead player (LivingEntity.tickDeath), and Player.remove's
inventoryMenu.removed drops the cursor's stack where it died. Alice then asks whether an item lies around the death
site; the reply, and the inventory slots, respawns and teleports she is sent from the death on, are compared
exactly. Before the death sequence, the same client predicts clicks in window 0 and a crafting table,
including incorrect slot and cursor hashes and a patch that only restates the item prototype.
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
PREDICTION_STEPS = ('prediction-match', 'prediction-slot', 'prediction-cursor', 'prediction-return',
                    'table-pickup', 'table-slot', 'default-pickup', 'default-return')
SCRIPT = [
    ('setup', ['menus on', 'cmd Alice gamerule keep_inventory false', 'cmd Alice clear @s',
               'cmd Alice give @s minecraft:diamond 7', 'cmd Alice tp @s 10.5 -60 0.5',
               f'cmd Alice item replace entity @s hotbar.1 with {PICKAXE}']),
    # Pick up, put back, pick up, and put back one tool in inventory menu slot 37.
    ('prediction-match', [f'click-predicted Alice 0 37 - {PICKAXE}']),
    ('prediction-slot', ['click-predicted Alice 0 37 minecraft:diamond_pickaxe[damage=6,!tool] -']),
    ('prediction-cursor', ['click-predicted Alice 0 37 - minecraft:diamond_pickaxe[damage=5]']),
    ('prediction-return', [f'click-predicted Alice 0 37 {PICKAXE} -']),
    ('table-setup', ['cmd Alice setblock 11 -60 0 minecraft:crafting_table',
                     'cmd Alice item replace entity @s hotbar.2 with minecraft:stone[max_stack_size=64,!damage]']),
    ('table-open', ['use-on Alice 11 -60 0']),
    # The first opened menu is window 1: its slots 38 and 39 are hotbar.1 and hotbar.2.
    ('table-pickup', [f'click-predicted Alice 1 38 - {PICKAXE}']),
    ('table-slot', ['click-predicted Alice 1 38 minecraft:diamond_pickaxe[damage=6,!tool] -']),
    ('default-pickup', ['click-predicted Alice 1 39 - minecraft:stone']),
    ('default-return', ['click-predicted Alice 1 39 minecraft:stone -']),
    ('table-close', ['close-menu Alice 1']),
    ('prediction-cleanup', ['menus off', 'cmd Alice item replace entity @s hotbar.1 with minecraft:air',
                            'cmd Alice item replace entity @s hotbar.2 with minecraft:air']),
    # Hotbar slot 0 is the inventory menu's slot 36.
    ('pickup', ['pickup Alice 36']),
    ('death', ['cmd Alice kill @s']),
    ('respawn', ['respawn Alice']),
    ('check', ['chat on', 'cmd Alice execute if entity @e[type=minecraft:item,x=10.5,y=-60,z=0.5,distance=..4]']),
]


def shown(lines):
    """Compare post-click menu packets and slots, respawns, teleports and chat after death."""
    out, echoed = [], {}
    for line in lines:
        parts = line.split(' ', 3)
        if len(parts) != 4:
            continue
        player, step, kind, body = parts
        if step.removeprefix('step=') in PREDICTION_STEPS:
            if kind == 'menu-click':
                window, state = map(int, body.split())
                echoed[player, step, window] = state
            elif kind == 'menu-wire':
                packet = json.loads(body)
                if packet['kind'] not in ('slot', 'cursor', 'content'):
                    continue
                del packet['wire']
                if 'state' in packet:
                    before = echoed.get((player, step, packet['window']))
                    # The starting counters may differ. Compare the exact increment from the
                    # click that received the correction, including the 15-bit wraparound.
                    packet['state'] = None if before is None else (packet['state'] - before) & 0x7fff
                out.append(f'{player} {step} {kind} {json.dumps(packet, sort_keys=True)}')
        elif step in ('step=death', 'step=respawn', 'step=check') and kind in (
                'slot', 'inventory', 'respawn', 'teleport', 'chat'):
            out.append(line)
    return out


def check(vanilla, transcript):
    table_open = any(packet['kind'] == 'open' and packet['type'] == 'minecraft:crafting' and packet['window'] == 1
                     for line in transcript if line.startswith('Alice step=table-open menu-wire ')
                     for packet in [json.loads(line.split(' ', 3)[3])])
    if not table_open:
        return 'expected a real crafting table to open'
    if [line.split()[1].removeprefix('step=') for line in transcript if ' menu-click ' in line] != list(PREDICTION_STEPS):
        return 'expected every scripted prediction to be sent'
    corrections = [(parts[1], json.loads(parts[3])) for line in vanilla
                   if (parts := line.split(' ', 3))[2] == 'menu-wire']
    if [(step, packet['kind']) for step, packet in corrections] != [
            ('step=prediction-slot', 'slot'), ('step=prediction-cursor', 'cursor'), ('step=table-slot', 'slot')]:
        return 'expected two selective slot corrections and one cursor correction'
    inventory, cursor, table = (p for _, p in corrections)
    for packet, window, slot in [(inventory, 0, 37), (table, 1, 38)]:
        if (packet['window'], packet['slot'], packet['state']) != (window, slot, 1):
            return 'expected the corrected slot to carry the exact successor of its echoed state'
    if inventory['item'] is None or cursor['item'] != inventory['item'] or table['item'] != inventory['item']:
        return 'expected the same tool in all corrections'
    if not any(l.startswith('Alice step=check chat') for l in vanilla):
        return 'vanilla answered no check command'
    return None


def main():
    lines = locator.one_player(NAME, SCRIPT, shown, check, 'A player dying with a stack on the inventory menu cursor '
                               '(keep_inventory off) and respawning on a dedicated server: the stack is dropped '
                               'where it died, not kept. Matching and mismatching hashed slot and cursor '
                               'predictions compare selective corrections and their exact state increments in '
                               'both inventory and crafting menus before death.')
    print(f'cursor death: {len(lines)} lines identical')


if __name__ == '__main__':
    try:
        main()
    except AssertionError as error:
        sys.exit(f'{NAME}: {error}')
