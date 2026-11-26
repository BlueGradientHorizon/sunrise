//! `/item` (ItemCommands) and `if items`/`if slots`: addressing the item
//! slots of the player, mobs, armor stands, item frames and containers.
use super::{Arg, Context, Game, R, Target, err};
use sunrise_client::{
    command::{CommandError, args::ItemInput},
    containers::BlockEntity,
    items::ItemStack,
    scoreboard_cmd::Holder,
    text::Text,
};
use sunrise_core::nbt::Tag;

pub(in crate::game) mod contents;
pub(in crate::game) mod slot_sources;
use contents::Component;

/// One addressable slot.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) enum Slot {
    /// `inventory[0..36]`: hotbar then the main rows.
    Inv(usize),
    /// The selected hotbar slot as `weapon.mainhand` names it: the equipment's
    /// `setItemSlot`, where `hotbar.N` is the inventory's `setItem`.
    MainHand,
    /// `armor[0..4]` (head, chest, legs, feet).
    Armor(usize),
    Offhand,
    /// The player's body or saddle (`EquipmentSlot` ordinal, `Game::equipment`).
    Equipment(usize),
    Ender(usize),
    /// A mob's equipment by name (`mainhand`, `offhand`, `head`, ...).
    Mob(u32, &'static str),
    /// An armor stand's equipment (`EquipmentSlot` ordinal).
    Stand(u32, usize),
    /// The ender dragon's equipment (`EquipmentSlot` ordinal).
    Dragon(u32, usize),
    Frame(u32),
    /// A dropped item's stack: `ItemEntity.getSlot(0)`, the `contents` slot.
    Item(u32),
    /// A chest or hopper minecart's slot (`ContainerEntity.getChestVehicleSlot`).
    Cart(u32, usize),
    Block([i32; 3], usize),
    /// A slot of another player (`for_player`): reached with their state in place.
    Guest(u32, Box<Slot>),
    /// A component collection for writes, or an indexed component item for reads.
    Contents(Box<Slot>, Component),
    ContentItem(Box<Slot>, Component, usize),
}
pub(in crate::game) struct SlotCandidate {
    pub(in crate::game) slot: Slot,
    pub(in crate::game) filters: Vec<serde_json::Value>,
    pub(in crate::game) limit: usize,
}
/// Whose slots a command addresses.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) enum Owner {
    Entity(Target),
    Block([i32; 3]),
}

pub(in crate::game) fn mob_slot(name: &str) -> Option<usize> {
    Some(match name {
        "mainhand" => 0,
        "offhand" => 1,
        "feet" => 2,
        "legs" => 3,
        "chest" => 4,
        "head" => 5,
        "body" => 6,
        "saddle" => 7,
        _ => return None,
    })
}

/// The player's slot names and the inventory index each addresses.
pub(in crate::game) fn player_slot_names() -> Vec<(String, usize)> {
    let mut v = Vec::new();
    for i in 0..36 {
        v.push((format!("container.{i}"), i));
    }
    for i in 0..9 {
        v.push((format!("hotbar.{i}"), i));
    }
    for i in 0..27 {
        v.push((format!("inventory.{i}"), 9 + i));
    }
    v
}
/// `name` against a group: `container.*` matches every `container.N`.
pub(in crate::game) fn matches_name(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix(".*") {
        Some(prefix) => name.starts_with(&format!("{prefix}.")),
        None => pattern == name,
    }
}

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn owner_of(
        &mut self,
        ctx: &Context,
        prefix: &str,
        many: bool,
    ) -> R<Vec<Owner>> {
        let kind = ctx
            .literals
            .iter()
            .filter(|l| matches!(l.as_str(), "entity" | "block"))
            .nth(usize::from(prefix == "source" && ctx.has("from")))
            .cloned()
            .unwrap_or_default();
        if kind == "block" {
            let p = self.block_pos_arg(ctx, &format!("{prefix}Pos"))?;
            return Ok(vec![Owner::Block(p)]);
        }
        let targets = self.entities(ctx, prefix, false)?;
        let _ = many;
        Ok(targets.into_iter().map(Owner::Entity).collect())
    }
    /// The slots a name (or wildcard group) addresses on an owner.
    pub(in crate::game) fn slots_named(&mut self, owner: &Owner, name: &str) -> Vec<Slot> {
        if let Owner::Entity(Target::Guest(conn)) = *owner {
            let slots = self.for_player(Target::Guest(conn), |g| {
                g.slots_named(&Owner::Entity(Target::Player), name)
            });
            return slots
                .unwrap_or_default()
                .into_iter()
                .map(|s| Slot::Guest(conn, Box::new(s)))
                .collect();
        }
        let mut out = Vec::new();
        let mut add = |s: Slot| out.push(s);
        match owner {
            Owner::Block(p) => {
                let size = self.block_container_size(*p).unwrap_or(0);
                for i in 0..size {
                    if matches_name(name, &format!("container.{i}")) {
                        add(Slot::Block(*p, i));
                    }
                }
            }
            Owner::Entity(Target::Player) => {
                for (n, i) in player_slot_names() {
                    if matches_name(name, &n) {
                        add(Slot::Inv(i));
                    }
                }
                for (i, n) in ["armor.head", "armor.chest", "armor.legs", "armor.feet"]
                    .into_iter()
                    .enumerate()
                {
                    if matches_name(name, n) {
                        add(Slot::Armor(i));
                    }
                }
                if matches_name(name, "weapon") || matches_name(name, "weapon.mainhand") {
                    add(Slot::MainHand);
                }
                if matches_name(name, "weapon.offhand") {
                    add(Slot::Offhand);
                }
                for (i, n) in [
                    (sunrise_items::equipment::BODY, "armor.body"),
                    (sunrise_items::equipment::SADDLE, "saddle"),
                ] {
                    if matches_name(name, n) {
                        add(Slot::Equipment(i));
                    }
                }
                for i in 0..27 {
                    if matches_name(name, &format!("enderchest.{i}")) {
                        add(Slot::Ender(i));
                    }
                }
            }
            Owner::Entity(Target::Mob(id)) => {
                for (n, key) in [
                    ("weapon", "mainhand"),
                    ("weapon.mainhand", "mainhand"),
                    ("weapon.offhand", "offhand"),
                    ("armor.head", "head"),
                    ("armor.chest", "chest"),
                    ("armor.legs", "legs"),
                    ("armor.feet", "feet"),
                    ("armor.body", "body"),
                    ("saddle", "saddle"),
                ] {
                    if matches_name(name, n) {
                        add(Slot::Mob(*id, key));
                    }
                }
            }
            // A stand and the dragon hold the shared `Equipment`: every slot.
            Owner::Entity(target @ (Target::Stand(id) | Target::Dragon(id))) => {
                for (i, key) in sunrise_items::equipment::SLOT_NAMES.iter().enumerate() {
                    let aliases: &[&str] = match *key {
                        "mainhand" => &["weapon", "weapon.mainhand"],
                        "offhand" => &["weapon.offhand"],
                        "feet" => &["armor.feet"],
                        "legs" => &["armor.legs"],
                        "chest" => &["armor.chest"],
                        "head" => &["armor.head"],
                        "body" => &["armor.body"],
                        _ => &["saddle"],
                    };
                    if aliases.iter().any(|a| matches_name(name, a)) {
                        add(if matches!(target, Target::Dragon(_)) {
                            Slot::Dragon(*id, i)
                        } else {
                            Slot::Stand(*id, i)
                        });
                    }
                }
            }
            Owner::Entity(Target::Frame(id)) => {
                if matches_name(name, "container.0") || matches_name(name, "contents") {
                    add(Slot::Frame(*id));
                }
            }
            // `ItemEntity.getSlot(0)`: `contents`, `container.0` and `hotbar.0`
            // are all slot 0 (`SlotRanges`), and it holds the stack.
            Owner::Entity(Target::Item(id)) => {
                if matches_name(name, "contents")
                    || matches_name(name, "container.0")
                    || matches_name(name, "hotbar.0")
                {
                    add(Slot::Item(*id));
                }
            }
            // `getChestVehicleSlot` takes any index below `getContainerSize`.
            Owner::Entity(Target::Cart(id)) => {
                let size = self
                    .carts
                    .iter()
                    .find(|c| c.id == *id)
                    .map_or(0, |c| c.kind.slots());
                for i in 0..size {
                    if matches_name(name, &format!("container.{i}")) {
                        add(Slot::Cart(*id, i));
                    }
                }
            }
            Owner::Entity(_) => {}
        }
        out
    }
    /// `ItemCommands.getSlotsFromProvider`: evaluate inline sources against
    /// the accessor's live owner. Named ranges retain their existing adapter.
    pub(in crate::game::server_side) fn command_slots(
        &mut self,
        owner: &Owner,
        raw: &str,
    ) -> R<Vec<Slot>> {
        if let Owner::Block(p) = owner
            && self.block_container_size(*p).is_none()
        {
            return Err(err(
                "commands.item.source.not_a_container",
                &[&p[0], &p[1], &p[2]],
            ));
        }
        Ok(match self.resolved_slot_source(raw)? {
            Some(source) => self.inline_slots(owner, &source),
            None => self.slots_named(owner, raw),
        })
    }
    /// `SlotCollection.itemCopies` / `size`: transforms apply to the
    /// collection, before a command's item predicate or item modifier.
    pub(in crate::game::server_side) fn inline_slots(
        &mut self,
        owner: &Owner,
        source: &serde_json::Value,
    ) -> Vec<Slot> {
        if let Some(terms) = source.as_array() {
            return terms
                .iter()
                .flat_map(|s| self.inline_slots(owner, s))
                .collect();
        }
        match source["type"]
            .as_str()
            .unwrap_or_default()
            .trim_start_matches("minecraft:")
        {
            "group" => source["terms"].as_array().map_or_else(Vec::new, |terms| {
                terms
                    .iter()
                    .flat_map(|s| self.inline_slots(owner, s))
                    .collect()
            }),
            "slot_range" => self
                .slot_source_owner(owner, source)
                .map_or_else(Vec::new, |owner| {
                    self.slots_named(&owner, source["slots"].as_str().unwrap_or_default())
                }),
            "contents" => {
                let Some(component) = source["component"].as_str().and_then(Component::named)
                else {
                    return Vec::new();
                };
                let parents = self.inline_slots(owner, &source["slot_source"]);
                parents
                    .into_iter()
                    .flat_map(|p| self.contents_slots(p, component))
                    .collect()
            }
            "limit_slots" => {
                let mut slots = self.inline_slots(owner, &source["slot_source"]);
                slots.truncate(slot_sources::limit(source));
                slots
            }
            "filtered" => {
                let slots = self.inline_slots(owner, &source["slot_source"]);
                slots
                    .into_iter()
                    .filter(|slot| self.slot_filter(slot, &source["item_filter"]))
                    .collect()
            }
            _ => Vec::new(),
        }
    }
    pub(in crate::game::server_side) fn slot_filter(
        &mut self,
        slot: &Slot,
        filter: &serde_json::Value,
    ) -> bool {
        let stack = self
            .slot_get(slot)
            .unwrap_or_else(|| ItemStack::new("minecraft:air", 0));
        self.loot_item_filter(filter, &stack)
    }
    /// `SlotCollection.modifySlots` / `replaceSlotItems`: filters inspect the
    /// live stack when visited. Limits share the command's TrackingSelector
    /// count across owners and group terms, and count selected empty slots
    /// only for replacement (not modification).
    pub(in crate::game::server_side) fn command_slot_candidates(
        &mut self,
        owner: &Owner,
        raw: &str,
    ) -> R<Vec<SlotCandidate>> {
        if let Owner::Block(p) = owner
            && self.block_container_size(*p).is_none()
        {
            return Err(err(
                "commands.item.target.not_a_container",
                &[&p[0], &p[1], &p[2]],
            ));
        }
        if let Some(source) = self.resolved_slot_source(raw)? {
            let mut out = Vec::new();
            self.slot_candidates(owner, &source, &[], usize::MAX, &mut out);
            Ok(out)
        } else {
            Ok(self
                .slots_named(owner, raw)
                .into_iter()
                .map(|slot| SlotCandidate {
                    slot,
                    filters: Vec::new(),
                    limit: usize::MAX,
                })
                .collect())
        }
    }
    /// `CompositeSlotSource.provide` and `TransformedSlotSource.provide`:
    /// carry each leaf's filters and selection bound into live writes.
    pub(in crate::game::server_side) fn slot_candidates(
        &mut self,
        owner: &Owner,
        source: &serde_json::Value,
        filters: &[serde_json::Value],
        limit: usize,
        out: &mut Vec<SlotCandidate>,
    ) {
        if let Some(terms) = source.as_array() {
            for term in terms {
                self.slot_candidates(owner, term, filters, limit, out);
            }
            return;
        }
        match source["type"]
            .as_str()
            .unwrap_or_default()
            .trim_start_matches("minecraft:")
        {
            "group" => {
                if let Some(terms) = source["terms"].as_array() {
                    for term in terms {
                        self.slot_candidates(owner, term, filters, limit, out);
                    }
                }
            }
            "slot_range" => {
                let Some(owner) = self.slot_source_owner(owner, source) else {
                    return;
                };
                for slot in self.slots_named(&owner, source["slots"].as_str().unwrap_or_default()) {
                    out.push(SlotCandidate {
                        slot,
                        filters: filters.to_vec(),
                        limit,
                    });
                }
            }
            "contents" => {
                if let Some(component) = source["component"].as_str().and_then(Component::named) {
                    for parent in self.inline_slots(owner, &source["slot_source"]) {
                        out.push(SlotCandidate {
                            slot: Slot::Contents(Box::new(parent), component),
                            filters: filters.to_vec(),
                            limit,
                        });
                    }
                }
            }
            "filtered" => {
                let mut filters = filters.to_vec();
                filters.push(source["item_filter"].clone());
                self.slot_candidates(owner, &source["slot_source"], &filters, limit, out);
            }
            "limit_slots" => self.slot_candidates(
                owner,
                &source["slot_source"],
                filters,
                limit.min(slot_sources::limit(source)),
                out,
            ),
            _ => {}
        }
    }
    /// `SlotSelector.TrackingSelector.trySelectSlot`: count only slots that
    /// pass the predicate, before attempting the setter.
    pub(in crate::game) fn select_slot(
        &mut self,
        candidate: &SlotCandidate,
        selected: &mut usize,
        nonempty: bool,
    ) -> bool {
        if *selected >= candidate.limit
            || (nonempty && self.slot_get(&candidate.slot).is_none())
            || !candidate
                .filters
                .iter()
                .all(|f| self.slot_filter(&candidate.slot, f))
        {
            return false;
        }
        *selected += 1;
        true
    }
    pub(in crate::game::server_side) fn slot_accepts(
        &self,
        s: &Slot,
        stack: Option<&ItemStack>,
    ) -> bool {
        let Slot::Block(p, i) = s else {
            return true;
        };
        let air = ItemStack {
            item: "minecraft:air".into(),
            count: 0,
            damage: 0,
            extra: None,
        };
        self.block_can_place(*p, *i, stack.unwrap_or(&air))
    }
    pub(in crate::game::server_side) fn block_container_size(&self, p: [i32; 3]) -> Option<usize> {
        let size = match self.world.block_entities.get(&(p[0], p[1], p[2]))? {
            BlockEntity::Mod(_) => self
                .world
                .block_entities
                .mod_container(self.world.block(p[0], p[1], p[2]).0)
                .map_or(0, |c| c.slots),
            entity => entity.slots().len(),
        };
        (size > 0).then_some(size)
    }
    /// `LivingEntity.createEquipmentSlotAccess`'s predicate for the slots of a
    /// living entity that are not the head or a hand: an empty stack, or
    /// one whose `getEquipmentSlotForItem` is this very slot. A stand's
    /// `canUseSlot` refuses body, saddle and the slots its `DisabledSlots`
    /// lock, so such an item is MAINHAND's. Every other slot takes anything.
    pub(in crate::game::server_side) fn equipment_slot_accepts(
        &self,
        slot: &Slot,
        stack: &Option<ItemStack>,
    ) -> bool {
        use sunrise_items::equipment::{BODY, CHEST, FEET, LEGS, MAINHAND, SADDLE};
        let (index, stand) = match slot {
            Slot::Stand(id, i) => (*i, self.armor_stands.iter().find(|a| a.id == *id)),
            Slot::Dragon(_, i) => (*i, None),
            Slot::Guest(_, inner) => return self.equipment_slot_accepts(inner, stack),
            Slot::Armor(i @ 1..=3) => ([CHEST, LEGS, FEET][*i - 1], None),
            Slot::Mob(_, key) => match *key {
                "chest" => (CHEST, None),
                "body" => (BODY, None),
                "saddle" => (SADDLE, None),
                "legs" => (LEGS, None),
                "feet" => (FEET, None),
                _ => return true,
            },
            _ => return true,
        };
        let Some(stack) = stack else { return true };
        if ![FEET, LEGS, CHEST, BODY, SADDLE].contains(&index) {
            return true;
        }
        let worn = self.equippable_slot_sound(stack).map(|(s, _)| s as usize);
        let usable = |s: usize| match (&stand, slot) {
            (Some(a), _) => s != BODY && s != SADDLE && !a.is_disabled(s),
            (None, Slot::Stand(..)) => false,
            _ => true,
        };
        // SulfurCube.getEquipmentSlotForItem: swallowable items occupy the body slot.
        if let Slot::Mob(id, "body") = slot
            && self.mobs.iter().find(|m| m.id == *id).is_some_and(|m| {
                m.species() == sunrise_client::mobs::Species::SulfurCube
                    && sunrise_client::mobs::sulfur_cube::swallowable(m, &stack.item)
            })
        {
            return true;
        }
        worn.filter(|s| usable(*s)).unwrap_or(MAINHAND) == index
    }
    /// `SlotAccess.set` of a command: `slot_set` when the slot's predicate
    /// takes the stack. A mod's `set_equipment` calls `slot_set` itself and is
    /// not asked.
    pub(in crate::game::server_side) fn slot_set_by_command(
        &mut self,
        slot: &Slot,
        stack: Option<ItemStack>,
    ) -> bool {
        self.equipment_slot_accepts(slot, &stack) && self.slot_set(slot, stack)
    }
    /// Put a stack (or nothing) in a slot; false when the slot cannot take it.
    pub(in crate::game) fn slot_set(&mut self, s: &Slot, stack: Option<ItemStack>) -> bool {
        self.slot_set_with_effects(s, stack, true)
    }

    pub(in crate::game::server_side) fn owner_holder(&self, o: &Owner) -> Holder {
        match o {
            Owner::Entity(t) => Holder {
                name: String::new(),
                text: self.target_name(*t),
                display: None,
            },
            Owner::Block(p) => Holder {
                name: String::new(),
                text: Text::literal(format!("{}, {}, {}", p[0], p[1], p[2])),
                display: None,
            },
        }
    }
    pub(in crate::game::server_side) fn stack_name(&self, s: &ItemStack) -> Text {
        self.stack_display_name(s)
    }
    pub(in crate::game::server_side) fn item_from_input(
        &self,
        ctx: &Context,
    ) -> R<Option<ItemStack>> {
        let Some(Arg::Item(input)) = ctx.get("item") else {
            return Ok(None);
        };
        let count = Self::int(ctx, "count").unwrap_or(1).max(1) as u32;
        self.command_item_stack(input, count).map(Some)
    }
    /// `ItemInput.createItemStack`, shared by vanilla commands and native arguments.
    pub(in crate::game) fn command_item_stack(
        &self,
        ItemInput { id, components, .. }: &ItemInput,
        count: u32,
    ) -> R<ItemStack> {
        let (damage, extra) = match components
            .as_deref()
            .map(sunrise_client::item_data::command_components)
        {
            Some(Ok(tag)) => sunrise_client::item_data::decode_components(Some(&tag)),
            Some(Err(e)) => return Err(CommandError::new(Text::literal(e))),
            None => (0, None),
        };
        let stack = ItemStack {
            item: id.as_str().into(),
            count,
            damage,
            extra,
        };
        // ItemInput.createItemStack: a count above the stack's max (its
        // max_stack_size component, else the item's) is refused.
        let max_stack = self.stack_max_size(&stack);
        if count > max_stack {
            return Err(err("arguments.item.overstacked", &[id, &max_stack]));
        }
        // ItemInput.createItemStack validates effective components after count.
        // Materialization currently discards removals, so inspect the validated
        // command patch itself without changing the stack or its saved patch.
        let components = self.item_validation_components(id, components.as_deref())?;
        if components.get(&"minecraft:max_damage".to_owned()).is_some()
            && sunrise_world::datafix::dynamic::as_int(
                components.get(&"minecraft:max_stack_size".to_owned()),
                1,
            ) > 1
        {
            return Err(err(
                "arguments.item.malformed",
                &[&"Item cannot be both damageable and stackable"],
            ));
        }
        Ok(stack)
    }
    /// ItemInput.createItemStack: read-only view of the two prototype components used by this command
    /// validation, with all typed patch operations (including removals).
    pub(in crate::game) fn item_validation_components(
        &self,
        id: &str,
        components: Option<&str>,
    ) -> R<sunrise_world::component_patch::ComponentMap<String, Tag>> {
        use sunrise_client::command::{Reader, args::read_identifier, snbt::parse_tag_argument};
        use sunrise_world::component_patch::{ComponentMap, Patch, Prototype};
        let mut prototype = Vec::new();
        if let Some(item) = self.items().get(id) {
            for (key, value) in [
                ("minecraft:max_damage", item.max_damage),
                ("minecraft:max_stack_size", item.max_stack),
            ] {
                if item.components.iter().any(|component| component == key) {
                    prototype.push((key.to_owned(), Tag::Int(value as i32)));
                }
            }
        }
        let mut patch = Vec::new();
        if let Some(components) = components {
            // ItemInput has already validated IDs, values and duplicates. Reuse
            // its token/value parsers; do not derive removals from saved output.
            let mut reader = Reader::new(components);
            reader.expect('[')?;
            reader.skip_whitespace();
            while reader.can_read() && reader.peek() != ']' {
                let removed = reader.peek() == '!';
                if removed {
                    reader.skip();
                }
                let key = read_identifier(&mut reader)?;
                reader.skip_whitespace();
                let value = if removed {
                    None
                } else {
                    reader.expect('=')?;
                    reader.skip_whitespace();
                    Some(parse_tag_argument(&mut reader)?)
                };
                patch.push((key, value));
                reader.skip_whitespace();
                if reader.peek() == ']' {
                    break;
                }
                reader.expect(',')?;
                reader.skip_whitespace();
            }
            reader.expect(']')?;
        }
        Ok(ComponentMap::from_patch(
            Prototype::new(prototype),
            &Patch::new(patch),
        ))
    }
    pub(in crate::game) fn parse_inline_item_modifier(raw: &str) -> R<serde_json::Value> {
        let (tag, _) =
            sunrise_core::nbt::parse_snbt(raw).map_err(|e| err("argument.nbt.invalid", &[&e]))?;
        let value = sunrise_client::scoreboard::tag_to_json(&tag);
        sunrise_world::loot::validate_item_modifier(&value)
            .map_err(|e| err("argument.nbt.invalid", &[&e]))?;
        Ok(value)
    }
    /// `if items` and `if slots`: how many items match / how many slots.
    pub(in crate::game::server_side) fn condition_items(&mut self, c: &Context) -> R<(bool, i32)> {
        let owners = self.owner_of(c, "source", true)?;
        let Some(Arg::Str(slot_name)) = c.get("slots").cloned() else {
            return Ok((false, 0));
        };
        let predicate = match c.get("item_predicate") {
            Some(Arg::Item(p)) => Some(p.clone()),
            _ => None,
        };
        let mut count = 0;
        for o in owners {
            if let Owner::Block(p) = o
                && self.block_container_size(p).is_none()
            {
                return Err(err(
                    "commands.item.source.not_a_container",
                    &[&p[0], &p[1], &p[2]],
                ));
            }
            let slots = self.command_slots(&o, &slot_name)?;
            if predicate.is_none() {
                count += slots.len() as i32;
                continue;
            }
            for s in slots {
                if let Some(stack) = self.slot_get(&s)
                    && predicate
                        .as_ref()
                        .is_some_and(|p| self.stack_matches(p, &stack))
                {
                    count += stack.count as i32;
                }
            }
        }
        Ok((count > 0, count))
    }
}

/// Edit a stack's components as their saved compound.
pub(in crate::game) fn edit_components(
    stack: &mut ItemStack,
    f: impl FnOnce(&mut sunrise_core::nbt::Compound),
) {
    let mut c = match sunrise_client::item_data::component_snapshot(stack) {
        Some(Tag::Compound(c)) => c,
        _ => sunrise_core::nbt::Compound::new(),
    };
    f(&mut c);
    let tag = Tag::Compound(c);
    let (damage, extra) = sunrise_client::item_data::decode_components(Some(&tag));
    stack.damage = damage;
    stack.extra = extra;
    stack.tidy();
}
/// `SetCustomDataFunction.run`: `CustomData.update(stack, tag -> tag.merge(value))`.
pub(in crate::game) fn set_custom_data(stack: &mut ItemStack, value: &Tag) {
    edit_components(stack, |c| {
        let mut data = match c.get(&"minecraft:custom_data".into()) {
            Some(t @ Tag::Compound(_)) => sunrise_core::nbt::copy_tag(t),
            _ => Tag::Compound(Default::default()),
        };
        merge_compound(&mut data, value);
        // Vanilla CustomData.update removes the component when the result is empty.
        if matches!(&data, Tag::Compound(m) if m.is_empty()) {
            c.remove(&"minecraft:custom_data".into());
        } else {
            c.insert("minecraft:custom_data".into(), data);
        }
    });
}
/// `CompoundTag.merge`: nested compounds merge, other values replace.
pub(in crate::game) fn merge_compound(into: &mut Tag, from: &Tag) {
    let (Tag::Compound(a), Tag::Compound(b)) = (into, from) else {
        return;
    };
    for (k, v) in b {
        match (a.get_mut(k), v) {
            (Some(existing @ Tag::Compound(_)), Tag::Compound(_)) => merge_compound(existing, v),
            _ => {
                a.insert(k.clone(), sunrise_core::nbt::copy_tag(v));
            }
        }
    }
}
/// `ListOperation.apply` (`mode`: replace_all, replace_section, insert,
/// append) with a 256-entry limit (`SetLoreFunction`'s). An append, insert or section
/// replacement that cannot be done, an offset past the end or a result over the limit, gives
/// the old list back unchanged, as vanilla does (it logs an error). `replace_all` is the new
/// list as it is in vanilla, whose `ItemLore` then refuses more than 256 lines; here it is cut.
pub(in crate::game) fn list_operation(
    m: &serde_json::Value,
    old: Vec<Tag>,
    new: Vec<Tag>,
) -> Vec<Tag> {
    const LIMIT: usize = 256;
    let offset = m["offset"].as_u64().unwrap_or(0) as usize;
    match m["mode"].as_str().unwrap_or("") {
        "replace_all" => {
            let mut out = new;
            out.truncate(LIMIT);
            out
        }
        "insert" => {
            if offset > old.len() || old.len() + new.len() > LIMIT {
                return old;
            }
            let mut out = old[..offset].to_vec();
            out.extend(new);
            out.extend_from_slice(&old[offset..]);
            out
        }
        "replace_section" => {
            if offset > old.len() {
                return old;
            }
            let size = m["size"].as_u64().map_or(new.len(), |s| s as usize);
            let mut out = old[..offset].to_vec();
            out.extend(new);
            let end = offset.saturating_add(size);
            if end < old.len() {
                out.extend_from_slice(&old[end..]);
            }
            if out.len() > LIMIT {
                return old;
            }
            out
        }
        _ => {
            if old.len() + new.len() > LIMIT {
                return old;
            }
            let mut out = old;
            out.extend(new);
            out
        }
    }
}

/// `ListOperation.Append/Insert/ReplaceSection.apply` with `/item modify`'s lore limit of 256
/// (`SetLoreFunction` passes 256): an operation that cannot be done logs an error in vanilla and
/// gives the old list back unchanged. The expected lists are worked out from the vanilla bytecode
/// (see the pull request), not from the code under test.
#[cfg(test)]
pub(in crate::game) mod list_operation_tests {
    use super::*;
    use serde_json::json;

    fn ints(range: std::ops::Range<i32>) -> Vec<Tag> {
        range.map(Tag::Int).collect()
    }

    #[test]
    fn an_append_that_would_pass_256_entries_changes_nothing() {
        let old = ints(0..250);
        let m = json!({"mode": "append"});
        // 250 + 7 = 257 is over the limit: the old list, whole.
        assert_eq!(list_operation(&m, old.clone(), ints(1000..1007)), old);
        // 250 + 6 = 256 is exactly the limit: all of it.
        let mut full = old.clone();
        full.extend(ints(1000..1006));
        assert_eq!(list_operation(&m, old, ints(1000..1006)), full);
    }

    #[test]
    fn an_insert_past_the_end_or_over_the_limit_changes_nothing() {
        let old = ints(0..3);
        let at = |offset| json!({"mode": "insert", "offset": offset});
        // Offset 4 is past the end of three entries.
        assert_eq!(list_operation(&at(4), old.clone(), ints(9..10)), old);
        // Offset 3 is the end: an insert there is an append.
        assert_eq!(
            list_operation(&at(3), old.clone(), ints(9..11)),
            [0, 1, 2, 9, 10].map(Tag::Int)
        );
        // Offset 1 in the middle.
        assert_eq!(
            list_operation(&at(1), old, ints(9..11)),
            [0, 9, 10, 1, 2].map(Tag::Int)
        );
        // 255 + 2 = 257 is over the limit, at any offset.
        let big = ints(0..255);
        assert_eq!(list_operation(&at(0), big.clone(), ints(9..11)), big);
    }

    #[test]
    fn a_section_replaced_past_the_end_or_over_the_limit_changes_nothing() {
        let old = ints(0..4);
        let section = |offset, size: Option<u64>| {
            let mut m = json!({"mode": "replace_section", "offset": offset});
            if let Some(size) = size {
                m["size"] = json!(size);
            }
            m
        };
        // Offset 5 is past the end of four entries.
        assert_eq!(
            list_operation(&section(5, None), old.clone(), ints(9..10)),
            old
        );
        // Offset 4 is the end: nothing to replace, the new entries go after.
        assert_eq!(
            list_operation(&section(4, None), old.clone(), ints(9..10)),
            [0, 1, 2, 3, 9].map(Tag::Int)
        );
        // Entries 1 and 2 (size 2) become one entry.
        assert_eq!(
            list_operation(&section(1, Some(2)), old.clone(), ints(9..10)),
            [0, 9, 3].map(Tag::Int)
        );
        // No size: as many entries as the new ones.
        assert_eq!(
            list_operation(&section(0, None), ints(0..3), ints(7..9)),
            [7, 8, 2].map(Tag::Int)
        );
        // 256 entries, one replaced by two: 257 is over the limit.
        let full = ints(0..256);
        assert_eq!(
            list_operation(&section(0, Some(1)), full.clone(), ints(9..11)),
            full
        );
    }

    #[test]
    fn replacing_everything_gives_the_new_list() {
        let m = json!({"mode": "replace_all"});
        assert_eq!(
            list_operation(&m, ints(0..3), ints(7..9)),
            [7, 8].map(Tag::Int)
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod stand_equip_event_tests {
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_client::{chunk::Chunk, containers::BlockEntity, mobs::Who, sculk::Info};
    use sunrise_core::position::ChunkPos;

    fn candidate(game: &crate::game::ServerGame<'_>) -> Option<Info> {
        let Some(BlockEntity::Sculk(sensor)) = game.world.block_entities.get(&(9, 200, 10)) else {
            panic!("actual sensor must be registered")
        };
        sensor.vibration.selector.current.map(|(info, _)| info)
    }

    #[test]
    #[ignore = "daily"]
    fn commands_post_stand_equipment_events_after_its_first_tick() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir =
            Some(std::env::temp_dir().join(format!("sunrise-h130-unit-{}", std::process::id())));
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let registry = game.pack.assets.registry;
        let sensor = registry.default_state("minecraft:sculk_sensor").unwrap();
        let mut chunk = Chunk::empty(0, 0);
        chunk.set(9, 200, 10, sensor.0);
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("summon armor_stand 6.5 200 10.5 {UUID:[I;130,0,0,1],NoGravity:1b,attributes:[{id:\"minecraft:movement_speed\",base:0.1d}]}"));
        let id = game.armor_stands[0].id;
        assert_eq!(game.uuid_for(id), [130, 0, 0, 1]);
        assert!(
            game.armor_stands[0].speed_instance,
            "a loaded attribute is not firstTick"
        );
        game.sculk_prepare();
        game.sculk_sync_index();
        assert!(candidate(&game).is_none());
        let prefix = "item replace entity 00000082-0000-0000-0000-000000000001 ";
        for item in ["iron_helmet", "air"] {
            assert!(game.run_command(&format!("{prefix}armor.head with {item}")));
            assert!(candidate(&game).is_none(), "first-tick suppression");
        }
        game.tick_armor_stands().unwrap();
        assert!(
            game.run_command(
                "data merge entity 00000082-0000-0000-0000-000000000001 {ShowArms:1b}"
            )
        );
        for (slot, item, event) in [
            ("armor.head", "iron_helmet", Some("minecraft:equip")),
            ("armor.head", "iron_helmet", None),
            ("armor.head", "air", Some("minecraft:unequip")),
            ("armor.head", "air", None),
            ("weapon.mainhand", "stick", Some("minecraft:unequip")),
        ] {
            let Some(BlockEntity::Sculk(sensor)) = game.world.block_entities.get_mut(&(9, 200, 10))
            else {
                unreachable!()
            };
            sensor.vibration = Default::default();
            assert!(game.run_command(&format!("{prefix}{slot} with {item}")));
            let heard = candidate(&game);
            assert_eq!(heard.map(|info| info.event), event, "{slot} -> {item}");
            if let Some(info) = heard {
                assert_eq!(info.source, Some(Who::Mob(id).into()));
                assert_eq!(info.pos, [6.5, 200.0, 10.5]);
                assert!(info.owner.is_none());
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod mob_equipment_command_tests {
    use crate::game::commands::{Target, data::DataRef};
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_client::chunk::Chunk;
    use sunrise_core::position::ChunkPos;

    pub(in crate::game) fn with_game(test: impl FnOnce(&mut crate::game::ServerGame<'_>)) {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir = Some(
            std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap())
                .join(format!("h025-{:?}", std::thread::current().id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);

        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("difficulty normal"));
        assert!(game.run_command("forceload add 0 0"));
        test(&mut game);
    }

    pub(in crate::game) fn actor(
        game: &mut crate::game::ServerGame<'_>,
        kind: &str,
        n: i32,
    ) -> (String, u32) {
        let uuid = format!("00000019-0000-0000-0000-{n:012x}");
        let prev = game.side.chat_capture.replace(Vec::new());
        let ok = game.run_command(&format!(
            "summon {kind} 4.5 200 5.5 {{UUID:[I;25,0,0,{n}],NoAI:1b,NoGravity:1b,Silent:1b,h025_raw:7,Tags:[h025]}}"
        ));
        let lines = std::mem::replace(&mut game.side.chat_capture, prev).unwrap_or_default();
        assert!(ok, "{kind}/{n}: {lines:?}");
        let mob = game
            .mobs
            .iter()
            .find(|m| game.persist.uuid_of(m.id) == Some([25, 0, 0, n]))
            .unwrap();
        assert!(mob.no_ai);
        (uuid, mob.id)
    }

    #[test]
    #[ignore = "daily"]
    fn body_commands_reject_wrong_items_without_changing_head() {
        with_game(|game| {
            for (n, kind, slot, item) in [
                (1, "horse", "armor.body", "iron_horse_armor"),
                (2, "wolf", "armor.body", "wolf_armor"),
                (3, "husk", "armor.head", "iron_helmet"),
            ] {
                let (uuid, id) = actor(game, kind, n);
                assert!(
                    game.run_command(&format!("item replace entity {uuid} {slot} with {item}"))
                );
                let mob = game.mobs.iter().find(|m| m.id == id).unwrap();
                let actual = if slot == "armor.body" {
                    &mob.extra.equipment[sunrise_items::equipment::BODY]
                } else {
                    &mob.extra.equipment[sunrise_items::equipment::HEAD]
                };
                assert_eq!(
                    actual.as_ref().map(|s| &*s.item),
                    Some(format!("minecraft:{item}").as_str())
                );
            }
            for (n, kind) in [(4, "husk"), (5, "pig")] {
                let (uuid, id) = actor(game, kind, n);
                assert!(
                    game.run_command(&format!("item replace entity {uuid} armor.head with stone"))
                );
                assert!(
                    !game.run_command(&format!(
                        "item replace entity {uuid} armor.body with diamond_chestplate"
                    )),
                    "body must reject a chest item"
                );
                let mob = game.mobs.iter().find(|m| m.id == id).unwrap();
                assert!(
                    mob.extra
                        .slot_stack(sunrise_items::equipment::BODY)
                        .is_none()
                );
                assert_eq!(mob.extra.head_id(), Some("minecraft:stone"));
                let saved = game.data_of(&DataRef::Entity(Target::Mob(id))).unwrap();
                assert!(saved.get("equipment").unwrap().get("body").is_none());
                assert_eq!(
                    saved
                        .get("equipment")
                        .unwrap()
                        .get("head")
                        .unwrap()
                        .get("id"),
                    Some(&sunrise_core::nbt::Tag::String("minecraft:stone".into()))
                );
            }
        });
    }

    #[test]
    #[ignore = "daily"]
    fn saddle_commands_round_trip_existing_model_and_saved_equipment() {
        use super::Slot;
        use sunrise_core::nbt::Tag;
        with_game(|game| {
            // Pig is the existing species saver; Husk needs the common projection.
            for (n, kind) in [(6, "pig"), (7, "husk")] {
                let (uuid, id) = actor(game, kind, n);
                assert!(game.slot_get(&Slot::Mob(id, "saddle")).is_none());
                assert!(
                    game.run_command(&format!("item replace entity {uuid} armor.head with stone"))
                );
                assert!(
                    game.run_command(&format!("item replace entity {uuid} saddle with saddle")),
                    "command must reach the existing saddle model"
                );
                assert_eq!(
                    game.slot_get(&Slot::Mob(id, "saddle"))
                        .unwrap()
                        .item
                        .as_ref(),
                    "minecraft:saddle"
                );
                let saved = game.data_of(&DataRef::Entity(Target::Mob(id))).unwrap();
                assert_eq!(
                    saved
                        .get("equipment")
                        .unwrap()
                        .get("saddle")
                        .unwrap()
                        .get("id"),
                    Some(&Tag::String("minecraft:saddle".into()))
                );
                assert_eq!(
                    saved
                        .get("equipment")
                        .unwrap()
                        .get("head")
                        .unwrap()
                        .get("id"),
                    Some(&Tag::String("minecraft:stone".into()))
                );
                assert_eq!(saved.get("h025_raw"), Some(&Tag::Int(7)));
                assert_eq!(saved.get("Silent"), Some(&Tag::Byte(1)));
                assert_eq!(
                    saved.get("Tags"),
                    Some(&Tag::List(vec![Tag::String("h025".into())]))
                );
                assert!(
                    !game.run_command(&format!("item replace entity {uuid} saddle with stick"))
                );
                assert_eq!(
                    game.slot_get(&Slot::Mob(id, "saddle"))
                        .unwrap()
                        .item
                        .as_ref(),
                    "minecraft:saddle"
                );
                assert!(game.run_command(&format!("item replace entity {uuid} saddle with air")));
                assert!(game.slot_get(&Slot::Mob(id, "saddle")).is_none());
            }
        });
    }
    /// `SulfurCube.getEquipmentSlotForItem`: a `#sulfur_cube_swallowable`
    /// item is a body item, so the body's `SlotAccess` takes it (#12038).
    #[test]
    #[ignore = "daily"]
    fn a_sulfur_cubes_body_takes_a_swallowable_block() {
        with_game(|game| {
            assert!(game.run_command("summon sulfur_cube 4.5 200 4.5 {NoAI:1b,Tags:[\"cube\"]}"));
            let body = |game: &crate::game::ServerGame<'_>| {
                let m = game
                    .mobs
                    .iter()
                    .find(|m| m.kind.id == "minecraft:sulfur_cube");
                m.and_then(|m| {
                    m.extra.equipment[sunrise_items::equipment::BODY]
                        .as_ref()
                        .map(|stack| stack.item.clone())
                })
            };
            let replace = "item replace entity @e[tag=cube,limit=1] armor.body with";
            assert!(game.run_command(&format!("{replace} white_wool")));
            assert_eq!(body(game).as_deref(), Some("minecraft:white_wool"));
            // An item outside the tag stays out, and the body stays on.
            assert!(!game.run_command(&format!("{replace} diamond")));
            assert_eq!(body(game).as_deref(), Some("minecraft:white_wool"));
            // An empty stack always fits: the body comes off.
            assert!(game.run_command(&format!("{replace} air")));
            assert_eq!(body(game), None);
        });
    }
    #[test]
    #[ignore = "daily"]
    fn effective_item_composition_rejects_before_destination_mutation() {
        use super::Slot;
        with_game(|game| {
            assert!(game.run_command("setblock 4 200 4 chest"));
            let slot = Slot::Block([4, 200, 4], 0);
            for item in [
                "iron_helmet[max_stack_size=2]",
                "iron_helmet[max_stack_size=99]",
                "stone[max_damage=10,max_stack_size=2]",
            ] {
                assert!(game.run_command("item replace block 4 200 4 container.0 with emerald[custom_data={sentinel:86}] 4"));
                let before = game.slot_get(&slot).unwrap();
                assert!(
                    !game.run_command(&format!(
                        "item replace block 4 200 4 container.0 with {item}"
                    )),
                    "invalid effective composition must reject {item}"
                );
                assert_eq!(game.slot_get(&slot), Some(before));
            }
            for item in [
                "iron_helmet",
                "stone[max_stack_size=2]",
                "stone[max_stack_size=99]",
                "iron_helmet[!max_damage,max_stack_size=2]",
            ] {
                assert!(
                    game.run_command(&format!(
                        "item replace block 4 200 4 container.0 with {item}"
                    )),
                    "valid effective composition must accept {item}"
                );
            }
        });
    }

    #[test]
    #[ignore = "daily"]
    fn effective_item_validation_view_uses_loaded_prototypes_and_typed_removals() {
        use sunrise_core::nbt::Tag;
        with_game(|game| {
            let damage = "minecraft:max_damage".to_owned();
            let size = "minecraft:max_stack_size".to_owned();
            let helmet = game.items().get("minecraft:iron_helmet").unwrap();
            assert!(helmet.components.contains(&damage));
            let plain = game
                .item_validation_components("minecraft:iron_helmet", None)
                .unwrap();
            assert_eq!(
                plain.get(&damage),
                Some(&Tag::Int(helmet.max_damage as i32))
            );
            assert_eq!(plain.get(&size), Some(&Tag::Int(helmet.max_stack as i32)));
            let removed = game
                .item_validation_components(
                    "minecraft:iron_helmet",
                    Some("[!max_damage,max_stack_size=2,custom_data={keep:86}]"),
                )
                .unwrap();
            assert!(removed.get(&damage).is_none());
            assert_eq!(removed.get(&size), Some(&Tag::Int(2)));
            assert!(removed.get(&"minecraft:custom_data".to_owned()).is_some());
            let fallback = game
                .item_validation_components("minecraft:iron_helmet", Some("[!max_stack_size]"))
                .unwrap();
            assert!(fallback.get(&damage).is_some());
            assert_eq!(
                sunrise_world::datafix::dynamic::as_int(fallback.get(&size), 1),
                1
            );
            let stone = game
                .item_validation_components("minecraft:stone", None)
                .unwrap();
            assert!(stone.get(&damage).is_none());
            assert_eq!(
                stone.get(&size),
                Some(&Tag::Int(
                    game.items().get("minecraft:stone").unwrap().max_stack as i32
                ))
            );
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod mob_equipment_stack_tests {
    use crate::game::commands::{Target, data::DataRef};
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_client::{chunk::Chunk, mobs::finalize};
    use sunrise_core::nbt::Tag;
    use sunrise_core::position::ChunkPos;
    fn with_game(test: impl FnOnce(&mut crate::game::ServerGame<'_>)) {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir = Some(
            std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap())
                .join(format!("h023-{:?}", std::thread::current().id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);

        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("difficulty normal"));
        assert!(game.run_command("forceload add 0 0"));
        test(&mut game);
    }

    fn actor(game: &mut crate::game::ServerGame<'_>, kind: &str, n: i32) -> (String, u32) {
        let uuid = format!("00000017-0000-0000-0000-{n:012x}");
        let prev = game.side.chat_capture.replace(Vec::new());
        let ok = game.run_command(&format!(
            "summon {kind} 4.5 200 5.5 {{UUID:[I;23,0,0,{n}],NoAI:1b,NoGravity:1b,Silent:1b,h023_raw:7,Tags:[h023]}}"
        ));
        let lines = std::mem::replace(&mut game.side.chat_capture, prev).unwrap_or_default();
        assert!(ok, "{kind}/{n}: {lines:?}");
        let mob = game
            .mobs
            .iter()
            .find(|m| game.persist.uuid_of(m.id) == Some([23, 0, 0, n]))
            .unwrap();
        assert!(mob.no_ai);
        (uuid, mob.id)
    }

    fn saved(game: &mut crate::game::ServerGame<'_>, id: u32) -> Tag {
        game.data_of(&DataRef::Entity(Target::Mob(id))).unwrap()
    }
    fn audit(stack: &sunrise_client::items::ItemStack, value: i32) {
        assert_eq!(
            stack.custom_data().and_then(|c| c.get(&"audit".into())),
            Some(&Tag::Int(value))
        );
    }
    #[test]
    #[ignore = "daily"]
    fn offhand_commands_adopt_and_copy_the_whole_stack() {
        use super::Slot;
        with_game(|game| {
            let (uuid, id) = actor(game, "husk", 1);
            assert!(game.run_command("setblock 4 200 4 chest"));
            assert!(game.slot_get(&Slot::Mob(id, "offhand")).is_none());
            assert!(game.run_command(&format!(
                "item replace entity {uuid} weapon.mainhand with stone"
            )));
            assert!(game.run_command(&format!(
                "item replace entity {uuid} weapon.offhand with stick[custom_data={{audit:7}}] 3"
            )));
            let mob = game.mobs.iter().find(|m| m.id == id).unwrap();
            assert_eq!(mob.extra.off_hand_id(), Some("minecraft:stick"));
            let adopted = finalize::item_stack(mob, finalize::OFFHAND).unwrap();
            assert_eq!(
                adopted.count, 3,
                "actual command adoption must retain count"
            );
            audit(&adopted, 7);
            let tag = saved(game, id);
            assert_eq!(
                tag.get("equipment")
                    .unwrap()
                    .get("offhand")
                    .unwrap()
                    .get("count"),
                Some(&Tag::Int(3))
            );
            assert_eq!(
                tag.get("equipment")
                    .unwrap()
                    .get("mainhand")
                    .unwrap()
                    .get("id"),
                Some(&Tag::String("minecraft:stone".into()))
            );
            assert!(game.run_command(&format!(
                "item replace block 4 200 4 container.0 from entity {uuid} weapon.offhand"
            )));
            let copied = game.slot_get(&Slot::Block([4, 200, 4], 0)).unwrap();
            assert_eq!(copied.count, 3);
            audit(&copied, 7);
            assert!(game.run_command(&format!(
                "item replace entity {uuid} weapon.offhand with air"
            )));
            assert!(game.slot_get(&Slot::Mob(id, "offhand")).is_none());
            assert!(game.run_command(&format!(
                "item replace entity {uuid} weapon.offhand with stick"
            )));
            let plain = game.slot_get(&Slot::Mob(id, "offhand")).unwrap();
            assert_eq!(plain.count, 1);
            assert!(
                plain.custom_data().is_none(),
                "stale components must not return for the same id"
            );
        });
    }
    #[test]
    #[ignore = "daily"]
    fn armor_command_copy_keeps_the_already_adopted_stack() {
        use super::Slot;
        with_game(|game| {
            let (uuid, id) = actor(game, "husk", 2);
            assert!(game.run_command("setblock 4 200 4 chest"));
            assert!(game.run_command(&format!("item replace entity {uuid} armor.head with iron_helmet[damage=3,custom_data={{audit:8}}]")));
            let mob = game.mobs.iter().find(|m| m.id == id).unwrap();
            let stacks = mob.equipment_stacks();
            let adopted = stacks[5].as_ref().unwrap();
            assert_eq!(
                adopted.damage, 3,
                "existing armor state already adopts the full stack"
            );
            audit(adopted, 8);
            assert!(game.run_command(&format!(
                "item replace block 4 200 4 container.0 from entity {uuid} armor.head"
            )));
            let copied = game.slot_get(&Slot::Block([4, 200, 4], 0)).unwrap();
            assert_eq!(
                copied.damage, 3,
                "actual command copy must use the full stack"
            );
            audit(&copied, 8);
            let tag = saved(game, id);
            let components = tag
                .get("equipment")
                .unwrap()
                .get("head")
                .unwrap()
                .get("components")
                .unwrap();
            assert_eq!(components.get("minecraft:damage"), Some(&Tag::Int(3)));
            assert_eq!(
                components
                    .get("minecraft:custom_data")
                    .unwrap()
                    .get("audit"),
                Some(&Tag::Int(8))
            );
            assert!(game.run_command(&format!(
                "item replace entity {uuid} armor.head with iron_helmet"
            )));
            let plain = game.slot_get(&Slot::Mob(id, "head")).unwrap();
            assert_eq!(plain.damage, 0);
            assert!(plain.custom_data().is_none());
            assert!(game.run_command(&format!("item replace entity {uuid} armor.head with air")));
            assert!(game.slot_get(&Slot::Mob(id, "head")).is_none());
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod builtin_component_delivery_tests {
    use crate::game::lan::LanHost;
    use std::sync::{Arc, atomic::AtomicI32, mpsc};
    use sunrise_client::{
        items::ItemStack,
        lan::{Joined, NetConfig, Out, PlayerSender, guest::Guest, hub::HubConfig},
    };
    use sunrise_core::nbt::Tag;
    use sunrise_core::position::ChunkPos;
    use sunrise_core::vanilla_bytes::Writer;
    use sunrise_protocol::{
        buf::{Codec, Reader},
        item::{Slot, component},
        packets::{configuration::ClientInformation, play},
    };

    #[test]
    #[ignore = "daily"]
    fn h283_actual_command_inventory_bridge_encodes_four_builtin_components() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            game.build_registries();
            game.set_allow_commands(true);
            game.block_ticks
                .add_empty_chunk(ChunkPos::new(0, 0))
                .unwrap();
            assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
            assert!(game.world.loaded(8, 8));
            game.lan_host(
                crate::game::lan::LanSettings {
                    port: 0,
                    guest_commands: true,
                    force_game_mode: false,
                },
                NetConfig {
                    announce: false,
                    ..NetConfig::loopback(0, "component-wire-test".into(), 2)
                },
                HubConfig {
                    default_game_mode: 0,
                    guest_commands: true,
                    force_game_mode: false,
                    hardcore: false,
                    view_distance: 2,
                    simulation_distance: 2,
                    dimension: "minecraft:overworld".into(),
                    allow_flight: false,
                    seed: 0,
                    max_players: 2,
                    integrated: false,
                    levels: Default::default(),
                    level_seeds: Default::default(),
                },
            )
            .unwrap();
            let (tx, rx) = mpsc::channel();
            let mut guest = Guest::new(
                Joined {
                    conn: 2,
                    uuid: 283,
                    name: "ComponentGuest".into(),
                    properties: Vec::new(),
                    info: ClientInformation::new(2),
                    brand: "test".into(),
                    sender: PlayerSender::new(tx, Arc::default()),
                    latency: Arc::new(AtomicI32::new(0)),
                    address: "127.0.0.1:25565".parse().unwrap(),
                    expected: false,
                },
                2,
                [8.5, 200.0, 8.5],
                2,
            );
            guest.dimension = "minecraft:overworld".into();
            guest.placed = true;
            guest.loaded = true;
            guest.selected = 1;
            guest.permission = 2;
            game.lan.as_mut().unwrap().hub.guests.push(guest);
            let clock = game.world.game_time;
            let root =
                std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
            let registries: serde_json::Value =
                serde_json::from_slice(&std::fs::read(root.join("registries.json")).unwrap())
                    .unwrap();
            let id = |registry: &str, name: &str| {
                registries[registry]["entries"][name]["protocol_id"]
                    .as_i64()
                    .unwrap() as i32
            };
            let command = |game: &mut crate::game::ServerGame<'_>, input: &str| {
                assert!(
                    game.as_guest(2, |g| g.run_command(input)).unwrap(),
                    "{input}"
                );
            };
            let capture = |host: &mut LanHost| {
                while rx.try_recv().is_ok() {}
                host.hub.send_inventory(0);
                let mut packets = Vec::new();
                for out in rx.try_iter() {
                    match out {
                        Out::Packet(body) => packets.push(body),
                        Out::Packets(bodies) => packets.extend(bodies),
                        Out::Disconnect(_) => panic!("unexpected disconnect"),
                    }
                }
                let mut contents = Vec::new();
                for body in packets {
                    let mut r = Reader::new(&body);
                    let packet = r.varint().unwrap();
                    if packet == sunrise_protocol::ids::play::clientbound::CONTAINER_SET_CONTENT {
                        let content = play::ContainerSetContent::read(&mut r).unwrap();
                        assert_eq!(r.remaining(), 0, "complete enclosing content payload");
                        assert_eq!(content.container, 0);
                        assert_eq!(content.items.len(), 46);
                        assert!(content.carried.is_none());
                        contents.push(content);
                    }
                }
                assert_eq!(contents.len(), 1, "one actual inventory content packet");
                contents.pop().unwrap().items[9].clone().unwrap()
            };
            command(
                game,
                "item replace entity @s inventory.0 with stone[minecraft:custom_name=\"H283 control\"] 3",
            );
            let control = capture(game.lan.as_mut().unwrap());
            assert_eq!(control.item, id("minecraft:item", "minecraft:stone"));
            assert_eq!(control.count, 3);
            assert!(control.component(component::CUSTOM_NAME).is_some());
            command(game, "item replace entity @s inventory.0 with gold_ingot");
            let plain = capture(game.lan.as_mut().unwrap());
            assert!(
                plain
                    .component(id(
                        "minecraft:data_component_type",
                        "minecraft:provides_trim_material"
                    ))
                    .is_none()
            );
            assert!(plain.removed.is_empty());
            let mut failures = Vec::new();
            for (name, item, value, expected) in [
                (
                    "minecraft:provides_trim_material",
                    "minecraft:diamond",
                    "\"minecraft:gold\"",
                    Tag::String("minecraft:gold".into()),
                ),
                (
                    "minecraft:provides_banner_patterns",
                    "minecraft:paper",
                    "\"#minecraft:pattern_item/globe\"",
                    Tag::String("#minecraft:pattern_item/globe".into()),
                ),
                (
                    "minecraft:can_place_on",
                    "minecraft:stone",
                    "{blocks:\"minecraft:dirt\"}",
                    Tag::compound([("blocks", Tag::String("minecraft:dirt".into()))]),
                ),
                (
                    "minecraft:can_break",
                    "minecraft:diamond_pickaxe",
                    "{blocks:\"minecraft:stone\",state:{}}",
                    Tag::compound([
                        ("blocks", Tag::String("minecraft:stone".into())),
                        ("state", Tag::compound(std::iter::empty::<(&str, Tag)>())),
                    ]),
                ),
            ] {
                command(
                    game,
                    &format!("item replace entity @s inventory.0 with {item}[{name}={value}]"),
                );
                let source: ItemStack = game.lan.as_ref().unwrap().hub.guest(2).unwrap().inventory
                    [9]
                .clone()
                .unwrap();
                assert_eq!(source.item.as_ref(), item);
                assert_eq!(source.count, 1);
                assert_eq!(
                    source.data().unwrap().other(name),
                    Some(&expected),
                    "actual registered command adoption"
                );
                assert!(source.wire_patch().is_none(), "no cached wire fallback");
                let received = capture(game.lan.as_mut().unwrap());
                assert_eq!(received.item, id("minecraft:item", item));
                assert_eq!(received.count, 1);
                assert!(received.removed.is_empty());
                let kind = id("minecraft:data_component_type", name);
                let mut w = Writer::new();
                Slot::write(&Some(received.clone()), &mut w);
                let mut r = Reader::new(&w.data);
                let reread = Slot::read(&mut r).unwrap().unwrap();
                assert_eq!(r.remaining(), 0, "complete nested slot payload");
                assert_eq!(reread.component(kind), received.component(kind));
                if received.component(kind).is_none() {
                    failures.push((name, received));
                }
                assert_eq!(game.lan.as_ref().unwrap().hub.guest(2).unwrap().uuid, 283);
                assert_eq!(
                    game.lan.as_ref().unwrap().hub.guest(2).unwrap().health,
                    20.0
                );
                assert_eq!(
                    game.lan.as_ref().unwrap().hub.guest(2).unwrap().position,
                    [8.5, 200.0, 8.5]
                );
                assert_eq!(game.world.game_time, clock);
            }
            assert!(
                failures.is_empty(),
                "actual command -> network_stack -> full protocol packet missing builtin components: {failures:?}"
            );
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod component_modifier_tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn item_modify_removal_hides_the_prototype_and_a_set_clears_the_marker() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            let stack = ItemStack::new("minecraft:bread", 1);
            assert_eq!(stack.villager_food(game.items().get(&stack.item)), Some(4));
            let mut random = sunrise_core::random::LegacyRandom::new(1);
            let remove = serde_json::json!({"type":"minecraft:set_components", "components":{"!villager_food":{}}});
            let removed = game
                .apply_item_modifier(&remove, Some(stack), &mut random, None, [0.0; 3], 0)
                .unwrap();
            assert_eq!(removed.villager_food(game.items().get(&removed.item)), None);
            let set = serde_json::json!({"type":"minecraft:set_components", "components":{"minecraft:villager_food":{"nutrition":7}}});
            let result = game
                .apply_item_modifier(&set, Some(removed), &mut random, None, [0.0; 3], 0)
                .unwrap();
            assert_eq!(
                result.villager_food(game.items().get(&result.item)),
                Some(7)
            );
            assert!(
                result
                    .data()
                    .unwrap()
                    .other("!minecraft:villager_food")
                    .is_none()
            );
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod registered_slot_source_read_tests {}

#[cfg(test)]
pub(in crate::game) mod registered_slot_source_preservation_tests {
    use super::*;

    #[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::commands_server::items::registered_slot_source_preservation_tests::scene;

    pub(in crate::game) fn count(game: &mut crate::game::ServerGame<'_>, expected: i32) {
        assert!(game.run_command("execute store result score #r slotprobe run execute if slots block 0 100 0 parity:first_three"));
        assert_eq!(
            game.score.board.score("#r", "slotprobe").unwrap().value,
            expected
        );
    }

    #[test]
    #[ignore = "daily"]
    fn loaded_snapshot_changes_only_on_existing_reload_with_pack_precedence() {
        let mut resources = None;
        let directory = tempfile::tempdir().unwrap();
        let world = directory.path().join("slot-source-snapshot-world");
        let (mut game, file) = scene(&mut resources, world.to_str().unwrap());
        let before = game.block_entity_nbt([0, 100, 0]).unwrap();
        let clock = game.world.game_time;
        count(&mut game, 3); // Higher-priority pack overrides the same id.
        std::fs::write(&file, r#"{"type":"minecraft:limit_slots","limit":2,"slot_source":{"type":"minecraft:slot_range","slots":"container.*"}}"#).unwrap();
        count(&mut game, 3); // Disk edits do not change the loaded snapshot.
        game.reload_datapacks(true, &game.level.enabled_features.clone())
            .unwrap();
        count(&mut game, 2);
        std::fs::write(file, "{").unwrap();
        game.reload_datapacks(true, &game.level.enabled_features.clone())
            .unwrap();
        count(&mut game, 1); // Existing policy: invalid higher JSON is skipped.
        assert!(!game.run_command("execute if slots block 0 100 0 first_three"));
        assert_eq!(game.block_entity_nbt([0, 100, 0]).as_ref(), Some(&before));
        assert_eq!(game.world.game_time, clock);
    }

    /// SlotSources.CODEC is a holder codec: a quoted id is the same reference
    /// as an unquoted one, and so is a group's single quoted term.
    /// LimitSlotSource's limit is ExtraCodecs.POSITIVE_INT: a string or 0
    /// fails to parse, and the command changes nothing.
    #[test]
    #[ignore = "daily"]
    fn quoted_holders_resolve_and_a_non_int_limit_fails_to_parse() {
        let mut resources = None;
        let (mut game, _) = scene(&mut resources, "slot-source-quoted-world");
        let before = game.block_entity_nbt([0, 100, 0]).unwrap();
        let clock = game.world.game_time;
        count(&mut game, 3);
        for source in [
            r#""parity:first_three""#,
            r#"{type:"group",terms:"parity:first_three"}"#,
        ] {
            assert!(game.run_command("scoreboard players set #q slotprobe -99"));
            assert!(game.run_command(&format!(
                "execute store result score #q slotprobe run execute if slots block 0 100 0 {source}"
            )));
            assert_eq!(
                game.score.board.score("#q", "slotprobe").unwrap().value,
                3,
                "{source}"
            );
        }
        for limit in [r#""x""#, "0"] {
            assert!(game.run_command("scoreboard players set #q slotprobe -99"));
            let command = format!(
                "execute store result score #q slotprobe run execute if slots block 0 100 0 {{type:\"limit_slots\",limit:{limit},slot_source:{{type:\"slot_range\",slots:\"container.*\"}}}}"
            );
            assert!(!game.run_command(&command), "{command}");
            assert_eq!(
                game.score.board.score("#q", "slotprobe").unwrap().value,
                -99,
                "{command}"
            );
        }
        assert_eq!(game.block_entity_nbt([0, 100, 0]).as_ref(), Some(&before));
        assert_eq!(game.world.game_time, clock);
    }

    /// `Codec.INT` gives `limit:1b` and `limit:2.5d` their `Number.intValue()`:
    /// a read counts that many slots and a write changes that many.
    #[test]
    #[ignore = "daily"]
    fn a_byte_or_fractional_limit_selects_its_int_value() {
        let mut resources = None;
        let (mut game, _) = scene(&mut resources, "slot-source-numeric-limit-world");
        let source = |limit: &str| {
            format!(
                "{{type:\"limit_slots\",limit:{limit},slot_source:{{type:\"slot_range\",slots:\"container.*\"}}}}"
            )
        };
        for (limit, slots) in [("1b", 1), ("2.5d", 2)] {
            assert!(game.run_command("scoreboard players set #q slotprobe -99"));
            assert!(game.run_command(&format!(
                "execute store result score #q slotprobe run execute if slots block 0 100 0 {}",
                source(limit)
            )));
            assert_eq!(
                game.score.board.score("#q", "slotprobe").unwrap().value,
                slots,
                "{limit}"
            );
        }
        // `/item override` writes the stack over the selected slots and no further:
        // one slot for `1b`, two for `2.5d`, the stone in the fourth slot never.
        let held = |game: &mut crate::game::ServerGame<'_>| {
            [0, 1, 3].map(|slot| {
                game.slot_get(&Slot::Block([0, 100, 0], slot))
                    .map(|stack| (stack.item.to_string(), stack.count))
            })
        };
        let stack = |item: &str, count| Some((format!("minecraft:{item}"), count));
        assert!(game.run_command(&format!(
            "item override block 0 100 0 {} with minecraft:diamond 2",
            source("1b")
        )));
        assert_eq!(
            held(&mut game),
            [stack("diamond", 2), stack("dirt", 2), stack("stone", 7)]
        );
        assert!(game.run_command(&format!(
            "item override block 0 100 0 {} with minecraft:emerald 2",
            source("2.5d")
        )));
        assert_eq!(
            held(&mut game),
            [stack("emerald", 2), None, stack("stone", 7)]
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod set_custom_data_tests {
    use super::*;

    #[test]
    fn merging_an_empty_tag_into_a_stack_without_custom_data_adds_no_component() {
        let mut stack = ItemStack::new("minecraft:stick", 1);
        set_custom_data(&mut stack, &Tag::Compound(Default::default()));
        let saved = sunrise_client::item_data::component_snapshot(&stack);
        assert!(saved.is_none_or(|c| c.get("minecraft:custom_data").is_none()));
    }
}

#[cfg(test)]
pub(in crate::game) mod transient_modifier_tests {

    #[test]
    fn inline_command_modifier_refuses_whole_sequence_before_execution() {
        let error = crate::game::ServerGame::parse_inline_item_modifier(
            r#"[{type:"minecraft:set_count",count:2},{type:"minecraft:set_components",components:{"!minecraft:creative_slot_lock":{}}}]"#,
        ).unwrap_err();
        let message = format!("{error:?}");
        assert!(message.contains("minecraft:creative_slot_lock"));
        assert!(message.contains("not a persistent component"));
        let accepted = crate::game::ServerGame::parse_inline_item_modifier(
            r#"[{type:"minecraft:copy_components",source:"block_entity",include:["minecraft:additional_trade_cost"]},{type:"minecraft:set_custom_data",tag:{components:{"minecraft:map_post_processing":"opaque"}}}]"#,
        ).unwrap();
        assert_eq!(accepted.as_array().unwrap().len(), 2);
        assert_eq!(
            accepted[1]["tag"]["components"]["minecraft:map_post_processing"],
            "opaque"
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod item_entity_contents_tests;
