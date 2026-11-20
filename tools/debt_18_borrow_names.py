#!/usr/bin/env python3
"""Migrate resident and pinned registry-name consumers (debt item 18)."""

import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WAVE3 = Path("/box/shared/sunrise/debt-1.0/wave3-files.txt")
V = "crates/sunrise-mod-vocab/src/"
A = "crates/sunrise-mod-api/src/"
G = "crates/sunrise-mod-api-gen/src/"
C = "crates/sunrise-client/src/game/"


def resident(replace):
    keys = V + "host/keys.rs"
    states = C + "modding/native_block_states.rs"
    for kind, key, lookup, description in [
        ("block", "BlockKey", "block_key_id", "block"),
        ("item", "ItemKey", "item_key_id", "item"),
        ("effect", "EffectKey", "effect_key_id", "effect"),
        ("fluid", "FluidKey", "fluid_id", "fluid"),
        ("block_entity_type", "BlockEntityTypeKey", "block_entity_type_name", "block entity type"),
    ]:
        method = f"{kind}_key_id(&self, key: {key})"
        replace(keys, f"fn {method} -> String;", f"fn {method} -> Option<&str>;")
        replace(
            states,
            f'''fn {method} -> String {{
        mod_api::__private::{lookup}(key)
            .expect("invalid {description} key slot")
            .to_owned()
    }}''',
            f'''fn {method} -> Option<&str> {{
        mod_api::__private::{lookup}(key)
    }}''',
        )
    replace(
        V + "content/registry_keys.rs",
        "Some(&crate::parts::resolved_effects().get()?.names.entries[key.0 as usize].id)",
        "crate::parts::resolved_effects().get()?.names.id(key.0)",
    )
    replace(keys, "Result<String, BiomeKeyError>", "Result<&str, BiomeKeyError>")
    replace(V + "world/biome_key.rs", "Result<String, BiomeKeyError>", "Result<&str, BiomeKeyError>")
    replace(
        C + "modding/native_biome_keys.rs",
        "Result<String, mod_api::BiomeKeyError>",
        "Result<&str, mod_api::BiomeKeyError>",
    )
    replace(C + "modding/native_biome_keys.rs", "maps.biome_keys.id(key).map(str::to_owned)", "maps.biome_keys.id(key)")
    dimension = "fn dimension_key_id(&self, key: DimensionKey) -> "
    replace(V + "host/level_dimension.rs", dimension + "Option<String>", dimension + "Option<&str>")
    replace(C + "modding/native_entity_refs.rs", dimension + "Option<String>", dimension + "Option<&str>")
    replace(C + "modding/native_entity_refs.rs", "mod_api::__private::dimension_id(key).map(str::to_owned)", "mod_api::__private::dimension_id(key)")
    for indent in (16, 20):
        replace(
            C + "modding/native_entity_refs.rs",
            '.id(game),\n' + ' ' * indent + '"minecraft:stone"',
            '.id(game),\n' + ' ' * indent + 'Some("minecraft:stone")',
        )

    replace(
        G + "host.rs",
        '''        // A key's name is read with an owned answer.
        let key = entry("fn block_key_id(&self, key: BlockKey) -> String;");''',
        '''        // The registry name borrows the host and is absent without a backend.
        let key = entry("fn block_key_id(&self, key: BlockKey) -> Option<&str>;");''',
    )
    replace(
        G + "host.rs",
        'assert!(code.contains("backend_of(host).expect("));',
        '''assert!(code.contains("pub fn id (self , host : & dyn Host) -> Option < & str >"));
        assert!(code.contains("backend_of(host)?.block_key_id(self)"));''',
    )

    maps = V + "world/data_maps.rs"
    replace(maps, "use sunrise_core::nbt::Tag;", "use std::borrow::Cow;\nuse sunrise_core::nbt::Tag;")
    replace(
        maps,
        "fn id(&self, backend: &dyn Backend) -> Result<String, DataMapError>",
        "fn id<'a>(&self, backend: &'a dyn Backend) -> Result<Cow<'a, str>, DataMapError>",
        count=3,
    )
    replace(maps, "key_id_result(backend.$id(*self))", "backend.$id(*self).into_result()")
    replace(
        maps,
        '''trait KeyIdResult {
    fn into_result(self) -> Result<String, DataMapError>;
}
impl KeyIdResult for String {
    fn into_result(self) -> Result<String, DataMapError> {
        Ok(self)
    }
}
impl KeyIdResult for Result<String, crate::GameEventKeyError> {
    fn into_result(self) -> Result<String, DataMapError> {
        self.map_err(|error| match error {
            crate::GameEventKeyError::Unavailable => DataMapError::Unavailable,
            crate::GameEventKeyError::InvalidKey => DataMapError::InvalidKey,
        })
    }
}
fn key_id_result(value: impl KeyIdResult) -> Result<String, DataMapError> {
    value.into_result()
}''',
        '''trait KeyIdResult<'a> {
    fn into_result(self) -> Result<Cow<'a, str>, DataMapError>;
}
impl<'a> KeyIdResult<'a> for String {
    fn into_result(self) -> Result<Cow<'a, str>, DataMapError> {
        Ok(Cow::Owned(self))
    }
}
impl<'a> KeyIdResult<'a> for Option<&'a str> {
    fn into_result(self) -> Result<Cow<'a, str>, DataMapError> {
        self.map(Cow::Borrowed).ok_or(DataMapError::InvalidKey)
    }
}
impl<'a> KeyIdResult<'a> for Result<String, crate::GameEventKeyError> {
    fn into_result(self) -> Result<Cow<'a, str>, DataMapError> {
        self.map(Cow::Owned).map_err(|error| match error {
            crate::GameEventKeyError::Unavailable => DataMapError::Unavailable,
            crate::GameEventKeyError::InvalidKey => DataMapError::InvalidKey,
        })
    }
}''',
    )
    replace(maps, "backend.biome_key_id(*self).map_err", "backend.biome_key_id(*self).map(Cow::Borrowed).map_err")
    replace(
        maps,
        "fn id(&self, _: &dyn Backend) -> Result<String, DataMapError> {\n        Ok(self.0.to_string())",
        "fn id<'a>(&self, _: &'a dyn Backend) -> Result<Cow<'a, str>, DataMapError> {\n        Ok(Cow::Owned(self.0.to_string()))",
    )
    replace(maps, "backend.data_map_value(registry, &id, map.id())?", "backend.data_map_value(registry, id.as_ref(), map.id())?")
    for kind, ty, names in [
        ("item", "ItemKey", '["test:coal", "test:stick", "test:air"]'),
        ("block", "BlockKey", '["test:stone", "test:dirt"]'),
    ]:
        signature = f"fn {kind}_key_id(&self, key: {ty}) -> "
        replace(
            maps,
            signature + f"String {{\n            {names}[key.0 as usize].into()",
            signature + f"Option<&str> {{\n            {names}.get(key.0 as usize).copied()",
        )

    replace(
        V + "item/item_stack.rs",
        '''        let id = self.item.id(host);
        if id.is_empty() {
            return None;
        }''',
        "        let id = self.item.id(host)?;",
    )
    replace(V + "item/item_stack.rs", "Tag::String(id.as_str().into())", "Tag::String(id.into())")
    replace(
        V + "item/item_stack_damage_tests.rs",
        '''fn item_key_id(&self, item: ItemKey) -> String {
        match item.0 {
            0 => "minecraft:iron_sword",
            1 => "minecraft:stone",
            _ => "",
        }
        .into()
    }''',
        '''fn item_key_id(&self, item: ItemKey) -> Option<&str> {
        ["minecraft:iron_sword", "minecraft:stone"].get(item.0 as usize).copied()
    }''',
    )
    replace(
        V + "testing.rs",
        '''fn item_key_id(&self, key: crate::ItemKey) -> String {
        crate::content::registry_keys::item_id(key)
            .expect("fake item key belongs to its load")
            .to_owned()
    }''',
        '''fn item_key_id(&self, key: crate::ItemKey) -> Option<&str> {
        crate::content::registry_keys::item_id(key)
    }''',
    )
    effects = A + "parts/effects.rs"
    replace(
        effects,
        '''fn effect_key_id(&self, key: EffectKey) -> String {
                mod_vocab::__private::parts_installation::effect_id(key)
                    .expect("invalid effect key slot")
                    .to_owned()
            }''',
        '''fn effect_key_id(&self, key: EffectKey) -> Option<&str> {
                mod_vocab::__private::parts_installation::effect_id(key)
            }''',
    )
    replace(effects, 'keys[0].id(&Table), "minecraft:speed"', 'keys[0].id(&Table), Some("minecraft:speed")')
    replace(effects, 'keys[1].id(&Table), "key_test:later"', 'keys[1].id(&Table), Some("key_test:later")')
    replace(effects, "assert!(std::panic::catch_unwind(|| keys[0].id(&NoGame)).is_err());", "assert_eq!(keys[0].id(&NoGame), None);")
    block_entities = A + "parts/block_entities.rs"
    replace(
        block_entities,
        '''fn block_entity_type_key_id(&self, key: BlockEntityTypeKey) -> String {
                mod_vocab::__private::block_entity_type_name(key)
                    .unwrap()
                    .to_owned()
            }''',
        '''fn block_entity_type_key_id(&self, key: BlockEntityTypeKey) -> Option<&str> {
                mod_vocab::__private::block_entity_type_name(key)
            }''',
    )
    replace(block_entities, 'keys[0].id(&Table), "owner:press"', 'keys[0].id(&Table), Some("owner:press")')

    for receiver, name in [
        ("blocks[0]", "minecraft:air"),
        ("blocks.last().unwrap()", "minecraft:firefly_bush"),
        ("items[0]", "minecraft:air"),
        ("items.last().unwrap()", "minecraft:ominous_bottle"),
    ]:
        replace(states, f'{receiver}.id(host), "{name}"', f'{receiver}.id(host), Some("{name}")')
    for kind, name in [("block", "minecraft:stone"), ("effect", "minecraft:speed")]:
        call = f'host.{kind}_key("{name}").unwrap().id(host),\n                    '
        replace(states, f'{call}"{name}"', f'{call}Some("{name}")')
    replace(C + "modding/native_biome_keys_tests.rs", 'plains.id(game), Ok("minecraft:plains".into())', 'plains.id(game), Ok("minecraft:plains")')
    replace(
        C + "modding/native_block_entity_refs.rs",
        'chest.kind(game).map(|kind| kind.id(game)),\n                Some("minecraft:chest".to_owned())',
        'chest.kind(game).and_then(|kind| kind.id(game)),\n                Some("minecraft:chest")',
    )
    replace(C + "modding/native_item_entity_refs.rs", "read.item().id(game).as_str()", "read.item().id(game)")
    replace(C + "modding/native_item_entity_refs.rs", '("minecraft:diamond_pickaxe", 1, 17)', '(Some("minecraft:diamond_pickaxe"), 1, 17)')
    replace(
        C + "modding/native_level_placement_tests.rs",
        ".map(|stack| (stack.item().id(host), stack.count())),",
        ".and_then(|stack| Some((stack.item().id(host)?.to_owned(), stack.count()))),",
    )
    replace(C + "modding/native_living_effects.rs", 'listed[0].id.id(game), "minecraft:speed"', 'listed[0].id.id(game), Some("minecraft:speed")')
    world_read = C + "modding/native_world_read.rs"
    replace(
        world_read,
        '''                    tags.items(oak_tag)
                        .map(|keys| keys.into_iter().map(|key| key.id(host)).collect::<Vec<_>>())
                        .as_deref(),
                    Some(&oak[..])''',
        '''                    tags.items(oak_tag)
                        .and_then(|keys| keys.into_iter().map(|key| key.id(host)).collect::<Option<Vec<_>>>())
                        .as_deref(),
                    Some(&oak.each_ref().map(String::as_str)[..])''',
    )
    for path, old, new in [
        ("items/item_use.rs", 'stack.item().id(host) == "minecraft:rotten_flesh"', 'stack.item().id(host) == Some("minecraft:rotten_flesh")'),
        ("native_hooks/block_entity_client_tick_tests.rs", "event.state.block().id(host), event.entity.id()", "event.state.block().id(host), Some(event.entity.id())"),
        ("native_hooks/entity_interact_specific.rs", 'event.stack.unwrap().item().id(host), "minecraft:stick"', 'event.stack.unwrap().item().id(host), Some("minecraft:stick")'),
        ("native_hooks/render_tooltip_texture.rs", 'stack.item().id(host) == "minecraft:stick"', 'stack.item().id(host) == Some("minecraft:stick")'),
        ("native_hooks/on_block_placed_by_tests.rs", 'event.state.block().id(host), "minecraft:air"', 'event.state.block().id(host), Some("minecraft:air")'),
        ("native_hooks/on_block_placed_by_tests.rs", "let item = event.stack.item().id(host);", 'let item = event.stack.item().id(host).expect("placed item is registered").to_owned();'),
        ("network/remote/client_handles_tests.rs", "level.dimension().id(game).as_deref()", "level.dimension().id(game)"),
        ("network/lan/mod.rs", 'event.effect.id.id(host) == "minecraft:trial_omen"', 'event.effect.id.id(host) == Some("minecraft:trial_omen")'),
        ("network/lan/mod.rs", "let name = event.effect.id.id(host).to_string();", 'let name = event.effect.id.id(host).expect("active effect is registered");'),
        ("network/lan/mod.rs", "                    name.clone(),\n                    event.effect.amplifier,", "                    name.to_owned(),\n                    event.effect.amplifier,"),
        ("network/lan/mod.rs", 'match name.as_str() {\n                    "minecraft:speed" if id == ZOMBIE', 'match name {\n                    "minecraft:speed" if id == ZOMBIE'),
        ("network/remote_tags.rs", "items: items.map(|keys| keys.into_iter().map(|key| key.id(host)).collect()),", "items: items.and_then(|keys| keys.into_iter().map(|key| key.id(host).map(str::to_owned)).collect()),"),
        ("runtime/core_tests.rs", "let name = stone_key.id(game);", 'let name = stone_key.id(game).expect("stone is registered").to_owned();'),
        ("runtime/core_tests.rs", "stone_key.id(game), name", "stone_key.id(game), Some(name.as_str())"),
    ]:
        replace(C + path, old, new)


def snapshots(replace):
    states = C + "modding/native_block_states.rs"
    for entry, expected in [
        ("entities[0]", "minecraft:acacia_boat"),
        ("entities.last().unwrap()", "minecraft:fishing_bobber"),
        ("menus[0]", "minecraft:generic_9x1"),
        ("menus.last().unwrap()", "minecraft:stonecutter"),
        ("enchantments[0]", "minecraft:aqua_affinity"),
        ("enchantments.last().unwrap()", "minecraft:wind_burst"),
    ]:
        gap = "\n                " if entry.startswith(("entities.last", "enchantments.last")) else " "
        replace(states, f'{entry}.id(host),{gap}"{expected}"', f'{entry}.id(host).as_deref(),{gap}Some("{expected}")')

    for path, host, count in [
        ("modding/native_dragon_refs.rs", "game", 1),
        ("modding/native_entities.rs", "host", 1),
        ("modding/native_entity_refs.rs", "game", 2),
        ("modding/native_item_places.rs", "host", 1),
        ("modding/native_level_items.rs", "game", 1),
        ("modding/native_level_spawning.rs", "game", 2),
        ("modding/native_living_refs.rs", "game", 1),
        ("modding/native_player_destroyed_block_tests.rs", "host", 1),
        ("network/remote/native_entity_tests.rs", "&game", 3),
    ]:
        call = f"(|kind| kind.id({host}))"
        replace(C + path, f".map{call}", f".and_then{call}", count=count)
    for kind in ("robot", "ghost"):
        call = f"{kind}.kind(game).unwrap().id(game)"
        replace(C + "entities/general/save_load_tests.rs", f'{call}, "mymod:{kind}"', f'{call}.as_deref(), Some("mymod:{kind}")')
    for path, old, new, count in [
        ("entities/explosion/explosion_part_tests.rs", ".map(|kind| kind.id(host).to_owned())", ".and_then(|kind| kind.id(host)).map(|name| name.as_str().to_owned())", 1),
        ("entities/lightning.rs", ".and_then(|entity| entity.kind(host).map(|kind| kind.id(host)))", ".and_then(|entity| entity.kind(host)?.id(host)).map(|name| name.as_str().to_owned())", 2),
        ("modding/native_game_event_keys_tests.rs", 'step.id(game), Ok("minecraft:step".into())', 'step.id(game).as_deref(), Ok("minecraft:step")', 1),
        ("native_hooks/death_source_tests.rs", '.expect("dying entity has a type").id(host);', '.expect("dying entity has a type").id(host).expect("dying entity type is registered").as_str().to_owned();', 1),
        ("runtime/transaction_entities_tests.rs", ".map(|key| key.id(host))", ".and_then(|key| key.id(host)).map(|name| name.as_str().to_owned())", 1),
    ]:
        replace(C + path, old, new, count=count)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--step", type=int, choices=(1, 2), default=1)
    parser.add_argument("--protected-list", type=Path, default=WAVE3)
    args = parser.parse_args()
    protected = set(args.protected_list.read_text(encoding="utf-8").splitlines())
    pending = {}

    def replace(path, old, new, *, count=1):
        file = ROOT / path
        if path in protected or file.resolve() != file:
            raise SystemExit(f"refusing to edit protected or redirected path: {path}")
        if file not in pending:
            pending[file] = file.read_text(encoding="utf-8")
        source = pending[file]
        matches = source.count(old)
        if matches != count:
            raise SystemExit(f"{path}: expected {count} matches for {old!r}, found {matches}")
        pending[file] = source.replace(old, new)

    (resident if args.step == 1 else snapshots)(replace)
    for file, source in pending.items():
        file.write_text(source, encoding="utf-8")
    print(f"Migrated {len(pending)} files; format changed Rust files with rustfmt --edition 2024.")


if __name__ == "__main__":
    main()
