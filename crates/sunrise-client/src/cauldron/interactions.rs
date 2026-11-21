//! Resolved cauldron dispatch, shared by vanilla and mod entries.
use super::{Action, COLORS};
/// Prebind vanilla entries directly into a preparation candidate.
fn vanilla<H>() -> [std::collections::HashMap<String, Entry<H>>; 4] {
    use Action::*;
    let mut maps = std::array::from_fn::<_, 4, _>(|_| std::collections::HashMap::new());
    for map in &mut maps {
        for (item, action) in [
            ("lava_bucket", EmptyLava),
            ("water_bucket", EmptyWater),
            ("powder_snow_bucket", EmptySnow),
        ] {
            map.insert(format!("minecraft:{item}"), Entry::Vanilla(action));
        }
    }
    for (kind, item, action) in [
        (0, "potion", EmptyPotion),
        (1, "bucket", FillWater),
        (1, "glass_bottle", FillBottle),
        (1, "potion", TopUpPotion),
        (2, "bucket", FillLava),
        (3, "bucket", FillSnow),
    ] {
        maps[kind].insert(format!("minecraft:{item}"), Entry::Vanilla(action));
    }
    for color in COLORS {
        maps[1].insert(
            format!("minecraft:{color}_banner"),
            Entry::Vanilla(CleanBanner),
        );
        maps[1].insert(
            format!("minecraft:{color}_shulker_box"),
            Entry::Vanilla(CleanShulker),
        );
    }
    maps
}

use std::{collections::HashMap, sync::OnceLock};
/// One entry in a prepared cauldron dispatcher.
#[derive(Clone)]
pub enum Entry<H> {
    /// Prebound vanilla operation.
    Vanilla(super::Action),
    /// Engine-owned mod callback handle.
    Mod(H),
    /// Terminal vanilla dye-tag branch.
    Dye,
}
/// A single immutable lookup table after preparation.
#[derive(Clone)]
pub struct Dispatcher<H> {
    maps: [HashMap<String, Entry<H>>; 4],
    custom: HashMap<String, HashMap<String, Entry<H>>>,
    /// Item-tag entries by block, in registration order; asked before exact item entries.
    tagged: HashMap<String, Vec<(String, H)>>,
}
impl<H> Default for Dispatcher<H> {
    fn default() -> Self {
        Self {
            maps: vanilla(),
            ..Self::empty()
        }
    }
}
fn index(block: &str) -> Option<usize> {
    match block {
        "minecraft:cauldron" => Some(0),
        "minecraft:water_cauldron" => Some(1),
        "minecraft:lava_cauldron" => Some(2),
        "minecraft:powder_snow_cauldron" => Some(3),
        _ => None,
    }
}
impl Dispatcher<()> {
    /// Vanilla-only snapshot for resource-independent callers.
    pub fn vanilla() -> &'static Self {
        static TABLE: OnceLock<Dispatcher<()>> = OnceLock::new();
        TABLE.get_or_init(Self::default)
    }
}
impl<H> Dispatcher<H> {
    /// An unseeded batch of pending registrations, not a vanilla snapshot.
    pub fn empty() -> Self {
        Self {
            maps: std::array::from_fn(|_| HashMap::new()),
            custom: HashMap::new(),
            tagged: HashMap::new(),
        }
    }

    /// Read an exact registration without considering the dye branch or tags.
    pub fn exact_entry(&self, block: &str, item: &str) -> Option<&Entry<H>> {
        match index(block) {
            Some(i) => self.maps[i].get(item),
            None => self.custom.get(block)?.get(item),
        }
    }

    /// Publish a batch of pending registrations, overwriting matching entries in place.
    pub fn append_registrations(&mut self, pending: Self) {
        for (map, entries) in self.maps.iter_mut().zip(pending.maps) {
            map.extend(entries);
        }
        for (block, entries) in pending.custom {
            self.custom.entry(block).or_default().extend(entries);
        }
        for (block, entries) in pending.tagged {
            for (tag, handler) in entries {
                self.register_tag(block.clone(), tag, handler);
            }
        }
    }

    /// Take the callbacks out of a discarded batch so each can be dropped in isolation.
    pub fn into_mod_handlers(self) -> impl Iterator<Item = H> {
        self.maps
            .into_iter()
            .chain(self.custom.into_values())
            .flat_map(HashMap::into_values)
            .filter_map(|entry| match entry {
                Entry::Mod(handler) => Some(handler),
                _ => None,
            })
            .chain(
                self.tagged
                    .into_values()
                    .flatten()
                    .map(|(_, handler)| handler),
            )
    }

    /// Matches vanilla `CauldronInteraction.Dispatcher.get`: item-tag entries precede exact
    /// item entries. Vanilla's dye tag on `minecraft:water_cauldron` is checked first (or a
    /// mod's replacement for `minecraft:cauldron_can_remove_dye`), then other registered tag
    /// entries in registration order against the tags loaded now, and finally exact item
    /// entries.
    pub fn select(
        &self,
        block: &str,
        item: &str,
        removes_dye: bool,
        in_tag: impl Fn(&str) -> bool,
    ) -> Option<Entry<&H>> {
        if block == "minecraft:water_cauldron" && removes_dye {
            if !self.tagged.is_empty()
                && let Some((_, handler)) = self.tagged.get(block).and_then(|entries| {
                    entries
                        .iter()
                        .find(|(tag, _)| tag == "minecraft:cauldron_can_remove_dye")
                })
            {
                return Some(Entry::Mod(handler));
            }
            return Some(Entry::Dye);
        }
        if !self.tagged.is_empty()
            && let Some((_, handler)) = self
                .tagged
                .get(block)
                .and_then(|entries| entries.iter().find(|(tag, _)| in_tag(tag)))
        {
            return Some(Entry::Mod(handler));
        }
        self.exact_entry(block, item).map(|entry| match entry {
            Entry::Vanilla(action) => Entry::Vanilla(*action),
            Entry::Mod(handler) => Entry::Mod(handler),
            Entry::Dye => Entry::Dye,
        })
    }
    /// Explicit membership rather than a block-name convention.
    pub fn contains_block(&self, block: &str) -> bool {
        index(block).is_some() || self.custom.contains_key(block)
    }
    /// Preparation-only insertion. Overwrites any existing entry in place.
    pub fn register(&mut self, block: String, item: String, handler: H) {
        let map = match index(&block) {
            Some(i) => &mut self.maps[i],
            None => self.custom.entry(block).or_default(),
        };
        map.insert(item, Entry::Mod(handler));
    }
    /// Preparation-only insertion of an item-tag entry, which declares a custom block as
    /// `register` does. Overwrites an existing entry for the same tag in place, keeping its
    /// registration position.
    pub fn register_tag(&mut self, block: String, tag: String, handler: H) {
        if index(&block).is_none() {
            self.custom.entry(block.clone()).or_default();
        }
        let entries = self.tagged.entry(block).or_default();
        match entries.iter().position(|(held, _)| *held == tag) {
            Some(at) => entries[at].1 = handler,
            None => entries.push((tag, handler)),
        }
    }
    /// Complete prepared dispatcher set, including declared custom blocks.
    pub fn blocks(&self) -> Vec<String> {
        let mut blocks = [
            "minecraft:cauldron",
            "minecraft:water_cauldron",
            "minecraft:lava_cauldron",
            "minecraft:powder_snow_cauldron",
        ]
        .map(str::to_owned)
        .to_vec();
        blocks.extend(self.custom.keys().cloned());
        blocks.sort();
        blocks
    }
}
