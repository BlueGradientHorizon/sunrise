#!/usr/bin/env python3
"""Migrate owned stack snapshots in four separate debt 12 caller batches.

Run `python3 tools/debt12_migrate_reads.py BATCH --check` to validate a batch
without writing. Omit `--check` to apply it before the corresponding API and
hand edits, and rustfmt the changed Rust files before committing the batch.
Borrow-sensitive writes, backend adapters and documentation are left to
hand edits.
"""

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import subprocess


ROOT = Path(__file__).resolve().parents[1]
WAVE3 = Path("/box/shared/sunrise/debt-1.0/wave3-files.txt")
GAME = "crates/sunrise-client/src/game/"
MODDING = GAME + "modding/"


@dataclass(frozen=True)
class Edit:
    old: str
    new: str
    count: int = 1


def inventory_snapshot(call: str, *, prefix="", suffix="", count=1) -> Edit:
    receiver, _, method_call = call.rpartition(".")
    method, _, args = method_call.partition("(")
    reader = "with_ender_chest" if method == "ender_chest" else "with_inventory"
    rows = "view.iter()"
    if method == "inventory":
        rows += ".take_while(|(slot, _)| *slot < 36).map(|(_, stack)| stack.project())"
    else:
        rows += ".map(|(slot, stack)| Some((slot, stack.project()?)))"
    # Collect into Option as the old conversion refused the whole snapshot if any row failed.
    new = f"{receiver}.{reader}({args[:-1]}, |view| {rows}.collect::<Option<Vec<_>>>()).flatten()"
    return Edit(prefix + call + suffix, prefix + new + suffix, count)


def stack_snapshot(call: str, *, prefix="", suffix="", count=1) -> Edit:
    receiver, _, method_call = call.rpartition(".")
    method, _, args = method_call.partition("(")
    new = f"{receiver}.with_{method}({args[:-1]}, |stack| stack.project()).flatten()"
    return Edit(prefix + call + suffix, prefix + new + suffix, count)


# A receiver spelling is only meaningful in its audited file and expression context.
BATCHES = {
    "inventory": {
        MODDING + "native_entity_refs.rs": (
            inventory_snapshot("player.inventory(game)", suffix=",\n"),
        ),
        MODDING + "native_menu_inventory_tests.rs": (
            inventory_snapshot("player.inventory_slots(game)", suffix=", Some(vec!"),
        ),
        MODDING + "native_menus.rs": (
            inventory_snapshot("opener.inventory_slots(host)", suffix=",\n"),
        ),
        MODDING + "native_player_inventory_tests.rs": (
            inventory_snapshot("b.inventory_slots(game)", suffix=",\n            Some(vec!"),
            inventory_snapshot("a.inventory_slots(game)", suffix=",\n            Some(vec!"),
            inventory_snapshot("a.inventory_slots(game)", suffix=", Some(vec!"),
            inventory_snapshot("player.inventory_slots(game)", prefix="let before = ", suffix=";", count=2),
            inventory_snapshot("player.inventory_slots(game)", suffix=", before);", count=2),
            inventory_snapshot("player.inventory_slots(game)", suffix=", Some(vec!"),
            inventory_snapshot("player.inventory_slots(host)", prefix="let outer = ", suffix=";"),
            inventory_snapshot("player.inventory_slots(host)", suffix=", outer);"),
            inventory_snapshot("player.inventory_slots(host)", suffix=", Some(vec!"),
            inventory_snapshot("player.ender_chest(game)", prefix="let before = ", suffix=";"),
            inventory_snapshot("player.ender_chest(game)", suffix=", before);"),
        ),
        MODDING + "native_player_ender_chest_tests.rs": (
            inventory_snapshot("a.ender_chest(game)", suffix=", Some(vec!", count=2),
            inventory_snapshot("b.ender_chest(game)", suffix=", Some(vec!", count=2),
        ),
        GAME + "network/remote/client_player_reads_tests.rs": (
            inventory_snapshot("player.inventory(game)", suffix=",\n"),
        ),
    },
    "living": {
        GAME + "blocks/dispenser/decision_tests.rs": (
            Edit(
                ".equipment(host, mod_api::EquipmentSlot::Head)",
                ".with_equipment(host, mod_api::EquipmentSlot::Head, |_| ())",
            ),
        ),
        GAME + "items/consume.rs": (
            stack_snapshot("next.hand_item(host, Hand::Main)", suffix=",\n"),
            Edit(
                "next_living.hand_item(host, Hand::Main).unwrap().count(), 7",
                "next_living.with_hand_item(host, Hand::Main, |stack| stack.count()), Some(7)",
            ),
            Edit(
                "first.hand_item(host, Hand::Main).unwrap().count(), 6",
                "first.with_hand_item(host, Hand::Main, |stack| stack.count()), Some(6)",
            ),
        ),
        GAME + "items/equip_answers.rs": (
            Edit(
                "event.entity.equipment(host, mod_api::EquipmentSlot::Head)",
                "event.entity.with_equipment(host, mod_api::EquipmentSlot::Head, |_| ())",
            ),
            Edit("event.entity.hand_item(host, mod_api::Hand::Main)",
                 "event.entity.with_hand_item(host, mod_api::Hand::Main, |_| ())"),
        ),
        GAME + "items/player_destroy_item_tests.rs": (
            stack_snapshot("living.hand_item(host_ref, hand)"),
            stack_snapshot("living.hand_item(host, Hand::Main)", suffix=".unwrap()", count=2),
        ),
        GAME + "mobs/creatures/death_loot_context_tests.rs": (
            stack_snapshot("living.hand_item(host, mod_api::Hand::Main)", suffix=",\n"),
        ),
        MODDING + "native_living_refs.rs": (
            stack_snapshot("player.hand_item(game, Hand::Main)", suffix=",\n"),
            stack_snapshot("player.hand_item(game, Hand::Off)", suffix=",\n"),
            stack_snapshot("zombie.equipment(game, EquipmentSlot::Head)", suffix=",\n"),
            Edit("mob.equipment(game, EquipmentSlot::MainHand).is_none()",
                 "mob.with_equipment(game, EquipmentSlot::MainHand, |_| ()).is_none()"),
        ),
        MODDING + "native_entity_refs.rs": (
            Edit(
                ".equipment(game, EquipmentSlot::MainHand)\n                        .unwrap()\n                        .item()\n                        .id(game)",
                ".with_equipment(game, EquipmentSlot::MainHand, |stack| stack.item().id(game))\n                        .unwrap()",
            ),
            Edit(
                ".equipment(game, EquipmentSlot::MainHand)\n                    .unwrap()\n                    .item()\n                    .id(game)",
                ".with_equipment(game, EquipmentSlot::MainHand, |stack| stack.item().id(game))\n                    .unwrap()",
            ),
        ),
        GAME + "network/remote/client_handles_tests.rs": (
            Edit("living.equipment(game, EquipmentSlot::Head)",
                 "living.with_equipment(game, EquipmentSlot::Head, |_| ())"),
            Edit("living.hand_item(game, Hand::Off)",
                 "living.with_hand_item(game, Hand::Off, |_| ())"),
        ),
    },
    "item-entity": {
        MODDING + "block_entity_tick_items_tests.rs": (
            Edit(
                "item.stack(host).unwrap().count()",
                "item.with_stack(host, |stack| stack.count()).unwrap()",
                count=5,
            ),
            Edit(
                "host.item_entity(first)\n                .unwrap()\n                .stack(host)\n                .unwrap()\n                .count()",
                "host.item_entity(first)\n                .unwrap()\n                .with_stack(host, |stack| stack.count())\n                .unwrap()",
            ),
        ),
        MODDING + "block_entity_tick_reply_tests/fixture.rs": (
            stack_snapshot(
                "host.item_entity(entity).unwrap().stack(host)",
                prefix="event.entity.items[0] = Some(",
                suffix=".unwrap());",
            ),
            stack_snapshot("item.stack(host)", prefix="let mut stack = ", suffix=".unwrap();"),
            Edit(
                "item.stack(host).unwrap().count()",
                "item.with_stack(host, |stack| stack.count()).unwrap()",
                count=3,
            ),
        ),
        MODDING + "native_item_entity_refs.rs": (
            stack_snapshot("item.stack(game)", prefix="assert_eq!(", suffix=", Some(changed));"),
            stack_snapshot("item.stack(game)", prefix="let mut empty = ", suffix=".unwrap();"),
            Edit(
                "item.stack(game).unwrap().damage(), 21",
                "item.with_stack(game, |stack| stack.damage()), Some(21)",
            ),
            Edit("item.stack(game).is_none()", "item.with_stack(game, |_| ()).is_none()"),
        ),
        GAME + "network/lan_actions/tests/pickup_post_tests.rs": (
            stack_snapshot("event.item.stack(host)", prefix="let read = ", suffix=";"),
        ),
    },
    "item-place": {
        MODDING + "native_book_apply.rs": (
            stack_snapshot("place.stack(host)", prefix="if ", suffix=".as_ref() != before.as_ref()"),
        ),
        MODDING + "native_book_apply/tests.rs": (
            stack_snapshot("ItemPlace::MenuSlot { menu, slot: 0 }.stack(host)", suffix=".unwrap()"),
        ),
        MODDING + "native_book_backings.rs": (
            stack_snapshot(
                "ItemPlace::MenuSlot { menu, slot }.stack(host)",
                prefix="&& ",
                suffix=".as_ref()\n                            == before.stack.as_ref()",
            ),
        ),
        MODDING + "native_book_scope.rs": (
            stack_snapshot(
                "ItemPlace::MenuSlot { menu, slot }.stack(host)", prefix="let stack = ", suffix=";",
            ),
        ),
        MODDING + "native_item_places.rs": (
            stack_snapshot("slot.stack(host)", prefix="assert_eq!(", suffix=", Some(", count=2),
            stack_snapshot("filled.stack(game)", suffix=",\n"),
            stack_snapshot("place.stack(game)", prefix="assert_eq!(", suffix=", Some(", count=2),
        ),
        MODDING + "native_menu_places.rs": (
            stack_snapshot(
                "ItemPlace::MenuSlot { menu, slot }.stack(game)",
                prefix=".map(|slot| ",
                suffix=")\n                .collect::<Vec<_>>()",
            ),
        ),
        MODDING + "native_menus.rs": (
            stack_snapshot("place.stack(host)", prefix="inner_restored = ", suffix=";"),
            stack_snapshot("place.stack(host)", prefix="restored.push((inner_restored, ", suffix="));"),
            stack_snapshot("place.stack(host)", suffix=",\n                            Some(stack.clone()", count=2),
        ),
        GAME + "runtime/transactions.rs": (
            stack_snapshot("place(0).stack(host)", suffix=",\n                Some(sticks(", count=2),
            stack_snapshot("entity.stack(host)", prefix="assert_eq!(", suffix=", Some(sticks(", count=2),
        ),
        "crates/sunrise-mod-api/src/parts/items.rs": (
            stack_snapshot("in_bag.stack(host)", prefix="assert_eq!(", suffix=", Some(stack("),
            stack_snapshot("HAND.stack(host)", prefix="assert_eq!(", suffix=", Some(stack(", count=2),
            stack_snapshot("charm.stack(host)", prefix="assert_eq!(", suffix=", Some(stack("),
        ),
    },
}


def migrate(original: str, name: str, edits: tuple[Edit, ...]) -> str:
    replacements = []
    for edit in edits:
        # Don't match `item.stack` within a different receiver such as `event.item.stack`.
        pattern = r"(?<![\w.])" + re.escape(edit.old)
        matches = list(re.finditer(pattern, original))
        if len(matches) != edit.count:
            raise SystemExit(f"{name}: expected {edit.count}, found {len(matches)} for {edit.old!r}")
        replacements.extend((match.start(), match.end(), edit.new) for match in matches)

    parts = []
    end = 0
    for start, stop, replacement in sorted(replacements):
        if start < end:
            raise SystemExit(f"{name}: overlapping edits")
        parts.extend((original[end:start], replacement))
        end = stop
    parts.append(original[end:])
    return "".join(parts)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("batch", choices=BATCHES)
    parser.add_argument("--check", action="store_true", help="validate counts without writing")
    args = parser.parse_args()
    batch = BATCHES[args.batch]

    if not WAVE3.is_file():
        parser.exit(1, f"missing protected-path list: {WAVE3}\n")
    protected = set(WAVE3.read_text(encoding="utf-8").splitlines())
    if blocked := protected.intersection(batch):
        parser.exit(1, "wave 3 owns: " + ", ".join(sorted(blocked)) + "\n")
    tracked = set(subprocess.check_output(
        ["git", "ls-files", "-z", "--", *batch], cwd=ROOT, text=True,
    ).split("\0"))
    if missing := batch.keys() - tracked:
        parser.exit(1, "not tracked: " + ", ".join(sorted(missing)) + "\n")

    # Preflight every file before writing any of them.
    updated = {}
    for name, edits in batch.items():
        path = ROOT / name
        if path.resolve() != path or not path.is_file():
            parser.exit(1, f"not a regular checkout file: {name}\n")
        original = path.read_bytes().decode("utf-8")
        updated[path] = migrate(original, name, edits)

    for path, contents in updated.items():
        if not args.check:
            path.write_bytes(contents.encode("utf-8"))
        name = path.relative_to(ROOT).as_posix()
        print(f"{name}: {sum(edit.count for edit in batch[name])} snapshot reads")


if __name__ == "__main__":
    main()
