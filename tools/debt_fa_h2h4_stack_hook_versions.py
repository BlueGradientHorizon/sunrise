#!/usr/bin/env python3
"""Version the hook writebacks that admit damage through the shared stack validator."""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
HOOKS = ROOT / 'crates/sunrise-mod-vocab/src/hooks'

# Mutable stacks (including collections) and the handwritten wrapper conversions.
# container_loot_filled and build_creative_mode_tab_contents were bumped earlier.
VERSIONS = {
    'anvil_update': 1,
    'apply_enchantments': 1,
    'block_drops': 2,
    'bonemeal': 1,
    'clone_item_stack': 1,
    'crafting_remainder': 1,
    'dispense_item': 1,
    'dispense_method': 1,
    'grindstone_place': 2,
    'grindstone_take': 2,
    'item_after_vertical_collision': 1,
    'item_fished': 2,
    'item_picked_up_post': 3,
    'item_stacked_on_other': 4,
    'living_drops': 1,
    'living_entity_use_item_finish': 1,
    'living_get_projectile': 1,
    'loot_drops': 1,
    'menu_button': 2,
    'menu_quick_move': 3,
    'player_swap_hands': 2,
    'potion_brew_post': 1,
    'potion_brew_pre': 1,
    'pre_remove_side_effects': 2,
    'prepare_item_craft': 1,
    'prepare_smithing': 1,
    'trade_with_villager': 3,
}


def main():
    updated = {}
    for hook, old in VERSIONS.items():
        path = HOOKS / f'{hook}.rs'
        src = path.read_text()
        attrs = re.search(r'    #\[hook\(\n(.*?)\n    \)\]', src, re.S)
        assert attrs, hook
        arguments = attrs[1]
        versions = re.findall(r'^        version = (\d+)', arguments, re.M)
        assert versions == ([] if old == 1 else [str(old)]), hook
        if versions:
            arguments = arguments.replace(f'        version = {old}', f'        version = {old + 1}')
        else:
            arguments = arguments.rstrip(',') + ',\n        version = 2'
        updated[path] = src[:attrs.start(1)] + arguments + src[attrs.end(1):]
    for path, data in updated.items():
        path.write_text(data)


if __name__ == '__main__':
    main()
