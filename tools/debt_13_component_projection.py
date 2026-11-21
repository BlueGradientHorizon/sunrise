#!/usr/bin/env python3
"""Add the debt 13 selector to the existing item component codec.

Run from the pre-projection tree; --check verifies the exact inputs without
writing. After applying, rustfmt the two changed Rust files.
"""

import argparse
from pathlib import Path


ITEM_DATA = Path(__file__).resolve().parents[1] / "crates/sunrise-items/src/item_data.rs"

# The presence conditions and matching keys, in encoder emission order.
MODELED = (
    ("!d.enchantments.is_empty()", "enchantments"),
    ("!d.stored_enchantments.is_empty()", "stored_enchantments"),
    ("d.repair_cost > 0", "repair_cost"),
    ("let Some(name) = &d.custom_name", "custom_name"),
    ("let Some(consumable) = &d.consumable", "consumable"),
    ("let Some(kinetic) = &d.kinetic_weapon", "kinetic_weapon"),
    ("d.potion.is_some() || !d.potion_extra.is_empty()", "potion_contents"),
    ("let Some(trim) = &d.trim", "trim"),
    ("let Some(id) = d.map_id", "map_id"),
    (
        "!d.ominous_banner && (!d.banner_layers().is_empty() || d.banner_patterns_set)",
        "banner_patterns",
    ),
    ("let Some(color) = &d.base_color", "base_color"),
    ("!d.writable_pages.is_empty()", "writable_book_content"),
    ("let Some(w) = &d.written", "written_book_content"),
    ("let Some((flight, explosions)) = &d.fireworks", "fireworks"),
    ("let Some(e) = &d.firework_explosion", "firework_explosion"),
    ("let Some(color) = d.dyed_color", "dyed_color"),
    ("let Some(sides) = &d.pot_decorations", "pot_decorations"),
    ("!d.bundle_contents.is_empty()", "bundle_contents"),
    ("!d.stew_effects.is_empty()", "suspicious_stew_effects"),
    ("!d.charged.is_empty()", "charged_projectiles"),
    ("let Some(a) = d.ominous_amplifier", "ominous_bottle_amplifier"),
    ("!d.container.is_empty()", "container"),
)


def replace_once(src: str, old: str, new: str) -> str:
    count = src.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one {old!r}, found {count}")
    return src.replace(old, new, 1)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="validate without writing")
    args = parser.parse_args()

    item_data = ITEM_DATA.read_text(encoding="utf-8")
    for call in (
        "encode_stack_components(stack, true, &raw_template_tag)\n",
        "encode_stack_components(stack, true, &raw_template_tag)?;\n",
        "components_tag_inner(damage, data, true, &raw_template_tag)\n",
    ):
        item_data = replace_once(
            item_data, call, call.replace("&raw_template_tag", "&|_| true, &raw_template_tag")
        )
    item_data = replace_once(
        item_data,
        'pub fn encode_extra_components(stack: &ItemStack) -> Option<Tag> {\n'
        '    encode_stack_components(stack, false, &raw_template_tag)\n'
        '}\n',
        'pub fn encode_extra_components(stack: &ItemStack) -> Option<Tag> {\n'
        '    encode_extra_components_matching(stack, |_| true)\n'
        '}\n\n'
        '/// Engine-only projection of the extra patch, excluding the typed damage scalar.\n'
        '/// The selector sees the component ID without `!` for removals. Selected nested\n'
        '/// stacks keep their full patches.\n'
        'pub fn encode_extra_components_matching(\n'
        '    stack: &ItemStack,\n'
        '    selector: impl Fn(&str) -> bool,\n'
        ') -> Option<Tag> {\n'
        '    encode_stack_components(stack, false, &selector, &raw_template_tag)\n'
        '}\n',
    )

    start = "fn components_tag_inner(\n"
    end = "/// The ASCII bytes of an NBT key, in `buf`."
    if item_data.count(start) != 1 or item_data.count(end) != 1:
        raise SystemExit("expected exactly one components_tag_inner section")
    before, _, rest = item_data.partition(start)
    encoder, _, after = rest.partition(end)
    if encoder.count("entries.push(") != len(MODELED) + 1:
        raise SystemExit("component encoder emission count changed")
    encoder = replace_once(
        encoder,
        "    include_damage: bool,\n    template_tag:",
        "    include_damage: bool,\n    selector: &impl Fn(&str) -> bool,\n    template_tag:",
    )
    encoder = replace_once(
        encoder,
        "    if include_damage\n        && (damage > 0\n",
        '    if include_damage\n        && selector("minecraft:damage")\n        && (damage > 0\n',
    )
    for condition, key in MODELED:
        # The potion's two presence cases must both be subject to selection.
        guarded = f"({condition})" if key == "potion_contents" else condition
        encoder = replace_once(
            encoder,
            f"        if {condition} {{\n",
            f'        if {guarded} && selector("minecraft:{key}") {{\n',
        )
    encoder = replace_once(
        encoder,
        "        entries.extend(d.implicit_banner_values().cloned());\n",
        "        entries.extend(d.implicit_banner_values().filter(|(id, _)| selector(id)).cloned());\n",
    )
    encoder = replace_once(
        encoder,
        "        for (k, v) in &d.other {\n            out.entry(JavaString::from(k.as_str()))\n",
        "        for (k, v) in &d.other {\n"
        "            if !selector(k.strip_prefix('!').unwrap_or(k.as_str())) {\n"
        "                continue;\n"
        "            }\n"
        "            out.entry(JavaString::from(k.as_str()))\n",
    )
    item_data = before + start + encoder + end + after

    persistence_path = ITEM_DATA.with_name("item_data") / "persistence.rs"
    persistence = persistence_path.read_text(encoding="utf-8")
    persistence = replace_once(
        persistence,
        "    include_damage: bool,\n    template:",
        "    include_damage: bool,\n    selector: &impl Fn(&str) -> bool,\n    template:",
    )
    persistence = replace_once(
        persistence,
        "components_tag_inner(stack.damage, stack.data(), include_damage, template)?;",
        "components_tag_inner(stack.damage, stack.data(), include_damage, selector, template)?;",
    )

    if not args.check:
        ITEM_DATA.write_text(item_data, encoding="utf-8")
        persistence_path.write_text(persistence, encoding="utf-8")


if __name__ == "__main__":
    main()
