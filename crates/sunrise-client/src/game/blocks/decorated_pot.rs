//! Decorated pots, which Sunrise keeps as loaded (the chunk's saved block
//! entities): the loot table a trial chamber pot holds, rolled into its item
//! the first time the item is read (`RandomizableContainer.unpackLootTable`),
//! and `DecoratedPotBlock.useItemOn` / `useWithoutItem`.
#[allow(unused_imports)]
use super::{Game, spill};
#[allow(unused_imports)]
use sunrise_client::{
    items::ItemStack,
    mobs::Who,
    particles::{ParticleOptions, ParticleType},
    random_sequences::RandomSequences,
    save,
};
#[allow(unused_imports)]
use sunrise_core::random::LegacyRandom;
#[allow(unused_imports)]
use {
    sunrise_core::nbt::Tag,
    sunrise_world::loot::{ContainerLootContext, LootData},
};

pub(in crate::game) const DECORATED_POT: &str = "minecraft:decorated_pot";

/// `DecoratedPotBlockEntity.WobbleStyle` ordinals.
pub(in crate::game) const WOBBLE_POSITIVE: i32 = 0;
pub(in crate::game) const WOBBLE_NEGATIVE: i32 = 1;

/// `RandomizableContainer.unpackLootTable(null)` on a pot's saved tag: the
/// loot table, if any, is removed and `LootTable.fill` puts its roll in
/// `item`. The roll draws from `LootTableSeed`, else the table's random
/// sequence, else the level random (`LootContext.Builder.create`).
pub(in crate::game::server_side) fn unpack_loot(
    tag: &mut Tag,
    loot: Option<&LootData>,
    random: &mut LegacyRandom,
    sequences: &mut RandomSequences,
    world_seed: i64,
    items: &crate::items::Items,
    context: ContainerLootContext<'_>,
) {
    let Tag::Compound(map) = tag else {
        return;
    };
    let Some(Tag::String(table)) = map.remove(&"LootTable".into()) else {
        return;
    };
    let seed = match map.remove(&"LootTableSeed".into()) {
        Some(Tag::Long(seed)) => seed,
        _ => 0,
    };
    map.remove(&"item".into());
    let Some(loot) = loot else {
        return;
    };
    let table = table.to_string_lossy();
    // No origin biome: the pot tables have no location checks.
    let slots = if seed != 0 {
        loot.fill_in(&table, &mut LegacyRandom::new(seed), 1, context)
    } else if let Some(sequence) = loot.random_sequence(&table) {
        loot.fill_in(&table, sequences.get(sequence, world_seed), 1, context)
    } else {
        loot.fill_in(&table, random, 1, context)
    };
    if let Some(stack) = slots.into_iter().next().flatten() {
        let item = sunrise_client::structure_blocks::loot_item_with_components(&stack);
        map.insert("item".into(), save::item_to_nbt(&item, items));
    }
}

/// [`Game::pot_data`] over the world and the block facts.
pub(in crate::game) fn pot_data_in(
    world: &sunrise_client::world::World,
    facts: &sunrise_client::blocks::BlockFacts,
    pos: [i32; 3],
) -> Option<Tag> {
    let state = world.block(pos[0], pos[1], pos[2]);
    if &*facts.get(state).name != "minecraft:decorated_pot" {
        return None;
    }
    let mut tag = spill::saved_entity(world, pos).unwrap_or_else(|| {
        Tag::compound([
            ("id", Tag::String("minecraft:decorated_pot".into())),
            ("x", Tag::Int(pos[0])),
            ("y", Tag::Int(pos[1])),
            ("z", Tag::Int(pos[2])),
        ])
    });
    tag = sunrise_client::block_entity_nbt::canonical(&tag)?;
    if let Tag::Compound(map) = &mut tag {
        map.remove(&"keepPacked".into());
    }
    Some(tag)
}

impl Game<'_> {
    /// `DecoratedPotBlockEntity.saveAdditional` for commands: a pure view of
    /// the existing saved state. Unlike pot_tag, this never unpacks loot.
    pub(in crate::game) fn pot_data(&self, pos: [i32; 3]) -> Option<Tag> {
        pot_data_in(&self.world, self.res.facts(), pos)
    }
    /// `DecoratedPotBlockEntity.loadAdditional`: command edits and BlockInput
    /// loading write back to the same saved tag that live pot interactions use.
    pub(in crate::game::server_side) fn set_pot_data(
        &mut self,
        pos: [i32; 3],
        input: &Tag,
    ) -> Option<bool> {
        let before = self.pot_data(pos)?;
        let mut input = input.clone();
        if let Tag::Compound(map) = &mut input {
            for key in ["id", "x", "y", "z"] {
                map.insert(key.into(), before.get(key)?.clone());
            }
        }
        let mut tag = sunrise_client::block_entity_nbt::canonical(&input)?;
        if let Tag::Compound(map) = &mut tag {
            map.remove(&"keepPacked".into());
        }
        let changed = tag != before;
        spill::put_saved_entity(&mut self.world, pos, tag);
        self.mark_block_entity_updated((pos[0], pos[1], pos[2]));
        Some(changed)
    }
    /// The pot's saved tag at `pos` with its loot unpacked (a new one for a
    /// pot without), or None when no pot is there.
    pub(in crate::game) fn pot_tag(&mut self, pos: [i32; 3]) -> Option<Tag> {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if &*self.res.facts().get(state).name != "minecraft:decorated_pot" {
            return None;
        }
        let mut tag = spill::saved_entity(&self.world, pos).unwrap_or_else(|| {
            Tag::compound([
                ("id", Tag::String("minecraft:decorated_pot".into())),
                ("x", Tag::Int(pos[0])),
                ("y", Tag::Int(pos[1])),
                ("z", Tag::Int(pos[2])),
            ])
        });
        let loot = self.server_loot.0.get();
        unpack_loot(
            &mut tag,
            loot.as_deref(),
            &mut self.world.random,
            &mut self.server.rand_sequences,
            self.level.seed,
            self.res.items(),
            ContainerLootContext::default(),
        );
        Some(tag)
    }
}
