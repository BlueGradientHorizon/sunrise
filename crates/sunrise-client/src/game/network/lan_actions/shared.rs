// Shared state, helpers and mixed tests stay at the original LAN action scope.

pub(in crate::game) const RECIPE_CHECK_COST: u32 = 1;
pub(in crate::game) const RECIPE_CHECK_LIMIT: u32 = 10;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MenuOpen {
    Opened,
    Cancelled,
    Refused,
    NoProvider,
    Stale,
}

/// Per-guest state of the gameplay code that lives outside `Guest`.
#[derive(Default)]
pub(in crate::game) struct GuestPlay {
    pub(in crate::game) mining: Option<Mining>,
    pub(in crate::game) destroy_delay: u32,
    pub(in crate::game) using: Option<(usize, u32)>,
    pub(in crate::game) use_duration: i32,
    pub(in crate::game) use_start: super::using_item::UseStart,
    pub(in crate::game) use_delay: u32,
    /// The shield's ItemCooldowns entry: an axe disabled it for this many ticks.
    pub(in crate::game) shield_cooldown: u32,
    /// `ServerGamePacketListenerImpl.dropSpamThrottler` (`TickThrottler(20, 1480)`): 20 for each
    /// item a creative player drops from the creative screen, one less each tick.
    pub(in crate::game) creative_drops: u32,
    /// `ServerGamePacketListenerImpl.chatSpamThrottler`: 20 for each chat message, one less each tick.
    pub(in crate::game) chat_spam: u32,
    /// `commandSpamThrottler`: 20 for each command, one less each tick.
    pub(in crate::game) command_spam: u32,
    /// `sunrise:recipe_check` requests still counting against the burst limit.
    pub(in crate::game) recipe_checks: u32,
    /// `ServerCommandSuggestionsProvider`: the latest suggestion request not answered yet (a
    /// newer one replaces it, which then never gets an answer), and whether this tick's one
    /// answer is still free.
    pub(in crate::game) suggestion: Option<Vec<u8>>,
    pub(in crate::game) suggestion_budget: bool,
    /// The guest's `Player.getCooldowns()` (`Game::cooldowns` for its turn).
    pub(in crate::game) cooldowns: sunrise_client::cooldowns::Cooldowns,
    pub(in crate::game) attack_ticks: u32,
    /// The main hand's item at the guest's last tick (`Game::attack_item`).
    pub(in crate::game) attack_item: Option<std::sync::Arc<str>>,
    pub(in crate::game) swing: sunrise_client::first_person::SwingState,
    /// The block being dug and the face the client aimed at.
    pub(in crate::game) digging: Option<([i32; 3], [i32; 3])>,
    /// The server tick the digging started (ServerPlayerGameMode.destroyProgressStart).
    pub(in crate::game) dig_start: u64,
    /// The block that clock is for (ServerPlayerGameMode.destroyPos). A START is the only writer
    /// that names one (death, a new ServerPlayerGameMode, clears it), so an abort, or a STOP that
    /// arrives out of reach, leaves it: the STOP of a dig resumed on the same block is still
    /// measured from the first START's ticks (#4124).
    pub(in crate::game) destroy_pos: Option<[i32; 3]>,
    /// A block the client stopped digging before the server's time was up: it breaks once
    /// its progress since `dig_start` is complete (ServerPlayerGameMode.hasDelayedDestroy).
    pub(in crate::game) delayed: Option<([i32; 3], u64)>,
    pub(in crate::game) last_progress: i32,
    /// Swings the guest's actions started this tick, for the players around
    /// (`LivingEntity.swing` sends ClientboundSwingAnimationPacket).
    pub(in crate::game) swings: u32,
    /// The guest's riding jump charge (`LocalPlayer.jumpRidingTicks`).
    pub(in crate::game) mount: super::mounts::MountState,
    /// The mount inventory the guest has open (game/network/lan_mount_menu.rs).
    pub(in crate::game) mount_window: Option<super::lan_mount_menu::GuestMount>,
    /// The lectern the guest reads (game/network/lan_stations.rs).
    pub(in crate::game) lectern: Option<super::lan_stations::GuestLectern>,
    /// The equipment the mods last heard this guest wear (`native_players_equipment`), by
    /// `EquipmentSlot`: main hand, off hand, feet, legs, chest, head.
    pub(in crate::game) equipment_heard: Option<Box<[Option<crate::items::ItemStack>; 6]>>,
    /// The slots of the guest's inventories the mods last heard (`native_players_inventory_changes`),
    /// by `NativeInventoryKind`: Player, Ender, Cursor, Crafting.
    pub(in crate::game) inventory_heard: Option<Box<[Vec<Option<crate::items::ItemStack>>; 4]>>,
    /// What the guest's body was given for its effects and enchantments.
    pub(in crate::game) potions: super::potions::BodyPotions,
    /// `ServerPlayer.getStats` and the inventory `inventory_changed` last saw.
    pub(in crate::game) stats: super::stats::Stats,
    /// `Stats.LEAVE_GAME` is counted: a guest is saved more than once on the
    /// way out, the statistic is awarded once (`Game::award_leave_game`).
    pub(in crate::game) left_game: bool,
    pub(in crate::game) last_inventory: u64,
    /// `ServerPlayer.levitationStartPos` and `levitationStartTime`.
    pub(in crate::game) levitation: Option<([f64; 3], u64)>,
    /// The fall, lava ride, Nether trip and blast the guest's advancements track.
    pub(in crate::game) marks: super::advancements::TravelMarks,
    /// `Entity.portalProcess` and `portalCooldown` (game/world/server_travel.rs).
    pub(in crate::game) portal: sunrise_client::nether_portal::PortalState,
    /// `Player.takeXpDelay`: ticks until the next orb can be taken.
    pub(in crate::game) xp_delay: u32,
    /// This player entity's age and level-up sound cooldown.
    pub(in crate::game) level_up_sound: sunrise_client::entities::LevelUpSound,
    /// What last killed the guest outright (`/kill`): the death message's key.
    pub(in crate::game) death_cause: Option<String>,
    /// The equipment modifiers on the guest's attributes (`apply_equipment`)
    /// and the armour and hands they were put on for: the hub's player
    /// is a `ServerPlayer` too, whose worn armour counts
    /// (`LivingEntity.collectEquipmentChanges`).
    pub(in crate::game) modifiers: Vec<(String, String)>,
    pub(in crate::game) equipped: Option<Vec<Option<ItemStack>>>,
    pub(in crate::game) craft_grid: Vec<Option<ItemStack>>,
    /// The guest's cast fishing hook (`Player.fishing`).
    pub(in crate::game) hook: sunrise_client::join_seq::ResidentSlot<super::fishing::Hook>,
    pub(in crate::game) sculk: super::sculk_server::PlayerEvents,
    /// What `ServerPlayer` last recorded for the guest's scoreboard criteria
    /// (`lastRecordedHealthAndAbsorption` and the others).
    pub(in crate::game) score_recorded: super::scoreboard::Recorded,
}

/// `LivingEntity.collectEquipmentChanges` for a guest: whether its armor, main hand or off
/// hand differs from what its modifiers were put on for (`equipped`), which then records them.
/// The body tick asks (`detectEquipmentUpdates`), so the packets of a tick act with the
/// modifiers from before it: a slot change and an attack sent in one tick hit with the old
/// item's attack damage and charge speed (the attribute swap).
pub(in crate::game) fn equipment_changed(
    equipped: &mut Option<Vec<Option<ItemStack>>>,
    armor: &[Option<ItemStack>],
    held: Option<&ItemStack>,
    offhand: Option<&ItemStack>,
) -> bool {
    let unchanged = equipped.as_deref().is_some_and(|eq| {
        eq.len() == armor.len() + 2
            && eq[..armor.len()] == armor[..]
            && eq[armor.len()].as_ref() == held
            && eq[armor.len() + 1].as_ref() == offhand
    });
    if !unchanged {
        let mut now = armor.to_vec();
        now.extend([held.cloned(), offhand.cloned()]);
        *equipped = Some(now);
    }
    !unchanged
}

/// A guest's command waiting for the hub (`LanHost::commands`).
pub(in crate::game) struct GuestCommand {
    pub(in crate::game) conn: u32,
    pub(in crate::game) command: String,
    pub(in crate::game) permission: u8,
    /// An integrated server's: no level teleports (yet).
    pub(in crate::game) integrated: bool,
    /// A mod's quiet command (game/network/lan_quiet.rs): its output suppressed, as
    /// `CommandSourceStack.withSuppressedOutput` does.
    pub(in crate::game) quiet: bool,
}

/// Why a guest's turn didn't start (`as_guest_at`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum TurnRefused {
    /// No guest has that `conn` at that place in the list, or it has no body.
    Gone,
    /// The guest is already taking a turn: its record holds an outer player's state.
    Nested,
}

impl GuestPlay {
    #[cfg(test)]
    pub(in crate::game) fn arm_mining_for_test(
        &mut self,
        pos: [i32; 3],
        start: u64,
        delayed: Option<([i32; 3], u64)>,
    ) {
        self.digging = Some((pos, [0, 1, 0]));
        self.dig_start = start;
        self.mining = Some(Mining {
            pos,
            progress: 0.0,
            per_tick: 0.0,
            ticks: 1,
            crack: None,
        });
        self.delayed = delayed;
        self.last_progress = -1;
    }

    /// The guest's `Player.attackStrengthTicker` outside its turn.
    pub(in crate::game) fn attack_ticks(&self) -> u32 {
        self.attack_ticks
    }
    /// The main-hand item of the guest's last tick (`Game.attack_item`).
    pub(in crate::game) fn attack_item(&self) -> Option<&str> {
        self.attack_item.as_deref()
    }
    /// The item in use: its slot and for how many ticks.
    pub(in crate::game) fn using(&self) -> Option<(usize, u32)> {
        self.using
    }
    /// `LivingEntity.getUsedItemHand` of the item in use, none when none is.
    pub(in crate::game) fn used_hand(&self) -> Option<Hand> {
        super::item_use::hand_of(self.using)
    }
    /// `LivingEntity.useItemRemaining` of the item in use, 0 when none is.
    pub(in crate::game) fn use_remaining(&self) -> i32 {
        super::item_use::remaining_of(self.using, self.use_duration)
    }
    /// Sets `useItemRemaining`; false when no item is in use.
    pub(in crate::game) fn set_use_remaining(&mut self, remaining: i32) -> bool {
        super::item_use::set_remaining(self.using, &mut self.use_duration, remaining)
    }
}

pub(in crate::game) struct ItemRules<'a>(
    pub(in crate::game) &'a sunrise_client::items::Items,
    pub(in crate::game) &'a sunrise_client::enchant::Enchantments,
    pub(in crate::game) Option<&'a dyn sunrise_client::menu::SlotRules>,
    pub(in crate::game) Option<&'a sunrise_client::menu::StackedOutcome>,
    pub(in crate::game) Vec<sunrise_client::containers::ModContainerInsertRule>,
);

/// `Player.isWithinEntityInteractionRange(entity, buffer)` for a guest: the
/// eyes closer than the entity interaction range (3, 5 in creative) plus
/// `buffer` to the entity's box.
pub(in crate::game) fn guest_within_entity_range(
    g: &sunrise_client::lan::guest::Guest,
    bounds: &sunrise_core::geometry::Aabb,
    buffer: f64,
) -> bool {
    let eye = [g.position[0], g.position[1] + g.eye_height(), g.position[2]];
    let (lo, hi) = (bounds.min_position(), bounds.max_position());
    let d = [
        eye[0] - eye[0].clamp(lo.x, hi.x),
        eye[1] - eye[1].clamp(lo.y, hi.y),
        eye[2] - eye[2].clamp(lo.z, hi.z),
    ];
    let range = if g.creative() { 5.0 } else { 3.0 } + buffer;
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < range * range
}
pub(in crate::game) fn building_player(g: &sunrise_client::lan::guest::Guest) -> BuildingPlayer {
    BuildingPlayer {
        conn: Some(g.conn),
        dimension: Dim::from_id(&g.dimension),
        bounds: (g.participates_in_player_simulation()
            && g.placed
            && g.loaded
            && !g.dead
            && !g.spectator())
        .then(|| g.bounds()),
        borrowed: false,
    }
}

thread_local! {
    /// The hub's bodies for the turns of one `tick_guests`, built once instead
    /// of for every turn (players x turns a tick). A guest's entry is refreshed
    /// when its turn ends ([`refresh_building_player`]), so the turns after it
    /// see it where it went (a pearl, a death, spectator mode).
    pub(in crate::game) static BUILDING_CACHE: std::cell::RefCell<Option<HubBodies>> =
        const { std::cell::RefCell::new(None) };
}

/// The hub's current bodies, across dimensions. A borrowed ancestor's
/// live body takes precedence over this saved state until its turn returns.
pub(in crate::game) fn building_players(host: &LanHost) -> HubBodies {
    let kept = BUILDING_CACHE.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|list| {
                list.iter().map(|p| p.get().conn).eq(host
                    .hub
                    .guests
                    .simulation_iter()
                    .map(|g| Some(g.conn)))
            })
            .cloned()
    });
    kept.unwrap_or_else(|| shared(host.hub.guests.simulation_iter().map(building_player)))
}

/// Builds the hub's bodies once for the turns to come (`Some`), or drops them.
pub(in crate::game) fn building_cache(host: Option<&LanHost>) {
    let bodies = host.map(|host| shared(host.hub.guests.simulation_iter().map(building_player)));
    BUILDING_CACHE.with(|c| *c.borrow_mut() = bodies);
}

/// The cached bodies as they are now in the hub, after `conn`'s turn (or its
/// death step). Every entry, not only the actor's: a turn can move, kill or
/// change another guest too (`/tp`, an attack, an explosion), and the turns
/// after it must see that guest where it is now, as a list built afresh would.
/// Copies into the shared cells, no allocation.
pub(in crate::game) fn refresh_building_player(host: &LanHost, _conn: u32) {
    BUILDING_CACHE.with(|c| {
        if let Some(list) = c.borrow().as_ref()
            && list.len() == host.hub.guests.simulation_iter().count()
        {
            for (p, g) in list.iter().zip(host.hub.guests.simulation_iter()) {
                if p.get().conn == Some(g.conn) {
                    p.set(building_player(g));
                }
            }
        }
    });
}

/// Kept at LAN action scope to preserve its published internals target path.
impl Game<'_> {
    pub(in crate::game) fn vehicle_has_other_viewers(
        container: Option<(i32, i32, i32)>,
        guests: &[sunrise_client::lan::guest::Guest],
        closing_conn: Option<u32>,
        key: (i32, i32, i32),
    ) -> bool {
        container == Some(key)
            || guests.iter().any(|g| {
                Some(g.conn) != closing_conn
                    && g.menu
                        .as_ref()
                        .is_some_and(|m| m.slots == Slots::Block(key))
            })
    }
}

/// ClientboundSwingAnimationPacket: the hand and the animation (its type and
/// base duration; each client applies the swinger's haste and fatigue).
pub(in crate::game) fn swing_packet(id: i32, hand: Hand, animation: SwingAnimation) -> C {
    C::SwingAnimation(play::SwingAnimation {
        id,
        hand: i32::from(hand == Hand::Off),
        kind: i32::from(animation.kind),
        duration: animation.duration,
    })
}

/// The block columns `Entity.isInWall` looks at: `AABB.ofSize` around the
/// eyes, 0.8 of the body's width across (`BlockPos.betweenClosedStream`).
pub(in crate::game) fn eye_columns(
    at: [f64; 3],
    body: Dimensions,
) -> [std::ops::RangeInclusive<i32>; 2] {
    let half = f64::from(body.width * 0.8) / 2.0;
    let range = |v: f64| (v - half).floor() as i32..=(v + half).floor() as i32;
    [range(at[0]), range(at[2])]
}

/// What `ServerPlayerGameMode.handleBlockBreakAction`'s STOP_DESTROY_BLOCK branch does with a
/// guest's STOP.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::game) enum StopDig {
    /// A block the last START did not name: the STOP is not this dig's.
    Other,
    /// Already broken, by the guest's own tick: only the crack goes.
    Gone,
    /// The dig the START counts is at least 70 % done: the block breaks outright.
    Break,
    /// Less than that: the server finishes it alone from that same START.
    Delay(u64),
}

/// The STOP branch's decision: a STOP is measured from the START that named its block
/// (`destroyPos`, which an abort or a STOP from out of reach leaves alone), whose ticks times
/// `getDestroyProgress` decide between the break and the delayed dig (#4124).
pub(in crate::game) fn stop_dig(
    play: &GuestPlay,
    pos: [i32; 3],
    air: bool,
    ticks: u64,
    per_tick: f32,
) -> StopDig {
    match play.dig_start_for(pos) {
        None => StopDig::Other,
        Some(_) if air => StopDig::Gone,
        Some(start) if per_tick * (ticks - start + 1) as f32 >= 0.7 => StopDig::Break,
        Some(start) => StopDig::Delay(start),
    }
}

/// ServerPlayer.swingAndResetAttackStrength resets only when a use starts a swing.
pub(in crate::game) fn reset_use_attack_strength(ticker: &mut u32, before: u32, after: u32) {
    if before != after {
        *ticker = 0;
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    use super::{eye_columns, reset_use_attack_strength};
    #[allow(unused_imports)]
    use crate::game::api_values::TryIntoApi;
    #[allow(unused_imports)]
    use sunrise_client::pose::Pose;

    pub(in crate::game) struct StopMenu(pub(in crate::game) Option<mod_api::GameMode>);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for StopMenu {
        fn container_opened(
            &self,
            host: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::ContainerOpenedEvent<'_>,
        ) -> mod_api::Flow {
            if let Some(mode) = self.0 {
                let player = host.players().acting_player().unwrap();
                assert!(player.set_game_mode(host, mode));
            }
            mod_api::Flow::Cancel
        }
    }

    #[test]
    fn use_swing_resets_attack_charge_only_when_started() {
        let mut ticker = 20;
        reset_use_attack_strength(&mut ticker, 3, 4);
        // A sword's attack speed is 1.6: vanilla's (ticker + 0.5) / 12.5.
        assert_eq!((ticker as f32 + 0.5) / 12.5, 0.04);

        ticker = 8;
        reset_use_attack_strength(&mut ticker, 4, 4);
        assert_eq!(ticker, 8); // No swing, or a swing rejected in its first half.
    }

    #[test]
    #[ignore = "daily"]
    fn menu_rules_match_guest_packets_and_prediction() {
        use mod_api::{Hooks, MenuPart, MenuSlotRulesEvent, ModEntry, Mods, Registrar};
        use std::sync::Arc;
        use sunrise_client::{
            chunk::Chunk,
            containers::{BlockEntity, ModBlockEntity, ModBlockEntityFields, ModMenuType},
            items::ItemStack,
            lan::menu::Slots,
            menu::{Input, Kind, Menu},
        };
        use sunrise_protocol::packets::play::{ContainerClick, Serverbound};

        struct Part;
        #[mod_api::sunrise_part]
        impl MenuPart for Part {
            fn menu_slot_rules(&self, _: &dyn mod_api::Host, event: &mut MenuSlotRulesEvent<'_>) {
                if mod_api::__private::item_key_id(event.stack.item()).unwrap()
                    == "minecraft:granite"
                    && event.stack.count() < 8
                {
                    event.may_place = false;
                }
                if event.slot == 0 {
                    if mod_api::__private::item_key_id(event.stack.item()).unwrap()
                        == "minecraft:diorite"
                        && event.stack.count() >= 8
                    {
                        event.max_stack = 3;
                    }
                    if matches!(
                        mod_api::__private::item_key_id(event.stack.item()).unwrap(),
                        "minecraft:andesite" | "minecraft:stone"
                    ) {
                        event.max_stack = 3;
                    }
                    if mod_api::__private::item_key_id(event.stack.item()).unwrap()
                        == "minecraft:dirt"
                    {
                        event.may_place = false;
                    }
                }
            }
        }
        struct Mod;
        #[mod_api::__subscriptions]
        impl Hooks for Mod {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.menu_part("minecraft:generic_9x3", Part);
                r.menu_part("test:menu", Part);
            }
        }
        let mods: Mods = vec![ModEntry::new("menu_rules", Box::new(Mod))].into();
        crate::game::api_values::install_parts(&mods).unwrap();
        crate::entry::install_client_hooks();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_guest_turn_tests::with_guests(|game| {
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let pos = (1, 64, 1);
                let stone = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                game.world.set(pos.0, pos.1, pos.2, stone).unwrap();
                let menu_type = Box::leak(Box::new(ModMenuType {
                    id: "test:menu",
                    owner: "test",
                    slots: 27,
                    player_inventory: true,
                    data_slots: 0,
                    owned_slots: None,
                    results: Vec::new(),
                    inputs: Vec::new(),
                    crafting: None,
                    links: None,
                }));
                for is_mod in [false, true] {
                    game.world.block_entities.insert(
                        pos,
                        if is_mod {
                            BlockEntity::Mod(Box::new(ModBlockEntity::new(
                                "test:block".into(),
                                ModBlockEntityFields {
                                    items: vec![None; 27],
                                    ..Default::default()
                                },
                            )))
                        } else {
                            BlockEntity::Chest(vec![None; 27])
                        },
                    );
                    let host = game.lan.as_mut().unwrap();
                    host.hub.open_container(
                        12,
                        "minecraft:generic_9x3",
                        sunrise_core::nbt::Tag::String("test".into()),
                        pos,
                        Slots::Block(pos),
                        vec![None; 27],
                        None,
                    );
                    host.hub.guests.update(0, |guest| {
                        let opened = guest.menu.as_mut().unwrap();
                        if is_mod {
                            opened.registry_id = "test:menu".into();
                            opened.mod_type = Some(menu_type);
                        }
                    });
                    let window = host.hub.guests[0].menu.as_ref().unwrap().window;
                    for (item, existing, expected) in [
                        ("minecraft:dirt", 0, 0),
                        ("minecraft:stone", 0, 3),
                        ("minecraft:diorite", 2, 3),
                    ] {
                        let stack = ItemStack::new(item, 8);
                        game.world.block_entities.get_mut(&pos).unwrap().slots_mut()[0] =
                            (existing > 0).then(|| ItemStack::new(item, existing));
                        let mut prediction = Menu::new(if is_mod {
                            Kind::Mod(menu_type)
                        } else {
                            Kind::Chest(3)
                        });
                        prediction.set_registry_id(Some(if is_mod {
                            "test:menu"
                        } else {
                            "minecraft:generic_9x3"
                        }));
                        prediction
                            .load_slot(0, (existing > 0).then(|| ItemStack::new(item, existing)));
                        prediction.carried = Some(stack.clone());
                        prediction
                            .click(&game.menu_env(), 0, 0, Input::Pickup)
                            .unwrap();
                        game.lan
                            .as_mut()
                            .unwrap()
                            .hub
                            .guests
                            .update(0, |guest| guest.carried = Some(stack));
                        game.lan_guest_packet(
                            12,
                            Serverbound::ContainerClick(ContainerClick {
                                container: window,
                                state: 0,
                                slot: 0,
                                button: 0,
                                input: 0,
                                changed: Vec::new(),
                                carried: None,
                            }),
                        );
                        let actual = game.world.block_entities.get(&pos).unwrap().slots()[0]
                            .as_ref()
                            .map_or(0, |s| s.count);
                        assert_eq!(actual, expected, "{item} mod={is_mod}");
                        assert_eq!(prediction.slot(0).map_or(0, |s| s.count), actual);
                        assert_eq!(
                            actual + prediction.carried.as_ref().map_or(0, |s| s.count),
                            existing + 8
                        );
                        assert_eq!(
                            prediction.carried.as_ref().map_or(0, |s| s.count),
                            game.lan.as_ref().unwrap().hub.guests[0]
                                .carried
                                .as_ref()
                                .map_or(0, |s| s.count)
                        );
                    }
                    for (item, existing, second, carried, creative, expected) in [
                        ("minecraft:granite", 0, 0, 8, false, (4, 4, 0)),
                        ("minecraft:andesite", 8, 0, 8, false, (3, 4, 9)),
                        ("minecraft:stone", 2, 0, 64, true, (3, 64, 0)),
                        ("minecraft:stone", 2, 63, 64, true, (3, 64, 62)),
                    ] {
                        let slots = game.world.block_entities.get_mut(&pos).unwrap().slots_mut();
                        slots[0] = (existing > 0).then(|| ItemStack::new(item, existing));
                        slots[1] = (second > 0).then(|| ItemStack::new(item, second));
                        let mut prediction = Menu::new(if is_mod {
                            Kind::Mod(menu_type)
                        } else {
                            Kind::Chest(3)
                        });
                        prediction.set_registry_id(Some(if is_mod {
                            "test:menu"
                        } else {
                            "minecraft:generic_9x3"
                        }));
                        prediction.load_slot(0, slots[0].clone());
                        prediction.load_slot(1, slots[1].clone());
                        prediction.carried =
                            Some(sunrise_client::items::ItemStack::new(item, carried));
                        prediction.creative = creative;
                        game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
                            guest.carried =
                                Some(sunrise_client::items::ItemStack::new(item, carried));
                            guest.game_mode = if creative {
                                sunrise_client::lan::guest::CREATIVE
                            } else {
                                0
                            };
                        });
                        let kind = if creative { 2 << 2 } else { 0 };
                        for (slot, button) in
                            [(-999, kind), (0, kind | 1), (1, kind | 1), (-999, kind | 2)]
                        {
                            prediction
                                .click(&game.menu_env(), slot, button, Input::QuickCraft)
                                .unwrap();
                            game.lan_guest_packet(
                                12,
                                Serverbound::ContainerClick(ContainerClick {
                                    container: window,
                                    state: 0,
                                    slot: slot as i16,
                                    button: button as i8,
                                    input: 5,
                                    changed: Vec::new(),
                                    carried: None,
                                }),
                            );
                        }
                        let slots = game.world.block_entities.get(&pos).unwrap().slots();
                        let result = (
                            slots[0].as_ref().map_or(0, |s| s.count),
                            slots[1].as_ref().map_or(0, |s| s.count),
                            game.lan.as_ref().unwrap().hub.guests[0]
                                .carried
                                .as_ref()
                                .map_or(0, |s| s.count),
                        );
                        let predicted = (
                            prediction.slot(0).map_or(0, |s| s.count),
                            prediction.slot(1).map_or(0, |s| s.count),
                            prediction.carried.as_ref().map_or(0, |s| s.count),
                        );
                        assert_eq!(result, expected, "{item} mod={is_mod}");
                        assert_eq!(predicted, expected, "{item} mod={is_mod}");
                        if !creative {
                            assert_eq!(result.0 + result.1 + result.2, existing + carried);
                        }
                    }
                    game.lan
                        .as_mut()
                        .unwrap()
                        .hub
                        .guests
                        .update(0, |guest| guest.game_mode = 0);
                    game.world.block_entities.get_mut(&pos).unwrap().slots_mut()[0] =
                        Some(sunrise_client::items::ItemStack::new("minecraft:dirt", 2));
                    let mut prediction = Menu::new(if is_mod {
                        Kind::Mod(menu_type)
                    } else {
                        Kind::Chest(3)
                    });
                    prediction.set_registry_id(Some(if is_mod {
                        "test:menu"
                    } else {
                        "minecraft:generic_9x3"
                    }));
                    prediction.load_slot(
                        0,
                        Some(sunrise_client::items::ItemStack::new("minecraft:dirt", 2)),
                    );
                    prediction.set_player_item(
                        9,
                        Some(sunrise_client::items::ItemStack::new("minecraft:dirt", 4)),
                    );
                    prediction
                        .click(&game.menu_env(), 27, 0, Input::QuickMove)
                        .unwrap();
                    game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
                        guest.inventory[9] =
                            Some(sunrise_client::items::ItemStack::new("minecraft:dirt", 4))
                    });
                    game.lan_guest_packet(
                        12,
                        Serverbound::ContainerClick(ContainerClick {
                            container: window,
                            state: 0,
                            slot: 27,
                            button: 0,
                            input: 1,
                            changed: Vec::new(),
                            carried: None,
                        }),
                    );
                    assert_eq!(
                        game.world.block_entities.get(&pos).unwrap().slots()[0]
                            .as_ref()
                            .unwrap()
                            .count,
                        2
                    );
                    assert_eq!(prediction.slot(0).unwrap().count, 2);
                    game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
                        guest.inventory.fill(None);
                        guest.inventory[0] =
                            Some(sunrise_client::items::ItemStack::new("minecraft:stone", 8));
                    });
                    game.lan_guest_packet(
                        12,
                        Serverbound::ContainerClick(ContainerClick {
                            container: window,
                            state: 0,
                            slot: 54,
                            button: 0,
                            input: 2,
                            changed: Vec::new(),
                            carried: None,
                        }),
                    );
                    let guest = &game.lan.as_ref().unwrap().hub.guests[0];
                    assert_eq!(guest.inventory[0].as_ref().unwrap().count, 8);
                    assert_eq!(guest.inventory.iter().flatten().count(), 1);
                    game.world.block_entities.get_mut(&pos).unwrap().slots_mut()[0] =
                        Some(sunrise_client::items::ItemStack::new("minecraft:dirt", 2));
                    game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
                        guest.inventory.fill(None);
                        guest.inventory[0] =
                            Some(sunrise_client::items::ItemStack::new("minecraft:stone", 8));
                    });
                    game.lan_guest_packet(
                        12,
                        Serverbound::ContainerClick(ContainerClick {
                            container: window,
                            state: 0,
                            slot: 0,
                            button: 0,
                            input: 2,
                            changed: Vec::new(),
                            carried: None,
                        }),
                    );
                    assert_eq!(
                        game.world.block_entities.get(&pos).unwrap().slots()[0]
                            .as_ref()
                            .unwrap()
                            .count,
                        3
                    );
                    let guest = &game.lan.as_ref().unwrap().hub.guests[0];
                    assert_eq!(guest.inventory[0].as_ref().unwrap().count, 5);
                    assert_eq!(guest.inventory[1].as_ref().unwrap().count, 2);
                    // A component-limited displaced stack fills to 16, then leaves 7 elsewhere.
                    let mut limited = ItemStack::new("minecraft:dirt", 8);
                    limited.data_mut().set_other(
                        "minecraft:max_stack_size",
                        Some(sunrise_core::nbt::Tag::Int(16)),
                    );
                    game.world.block_entities.get_mut(&pos).unwrap().slots_mut()[0] =
                        Some(limited.clone());
                    limited.count = 15;
                    game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
                        guest.inventory.fill(None);
                        guest.inventory[1] = Some(limited);
                        guest.selected = 1;
                        guest.inventory[0] =
                            Some(sunrise_client::items::ItemStack::new("minecraft:stone", 8));
                    });
                    game.lan_guest_packet(
                        12,
                        Serverbound::ContainerClick(ContainerClick {
                            container: window,
                            state: 0,
                            slot: 0,
                            button: 0,
                            input: 2,
                            changed: Vec::new(),
                            carried: None,
                        }),
                    );
                    let guest = &game.lan.as_ref().unwrap().hub.guests[0];
                    assert_eq!(guest.inventory[1].as_ref().unwrap().count, 16);
                    assert_eq!(
                        guest
                            .inventory
                            .iter()
                            .flatten()
                            .filter(|s| &*s.item == "minecraft:dirt")
                            .map(|s| s.count)
                            .sum::<u32>(),
                        23
                    );
                    game.guest_close_all(12);
                }
            })
        });
    }

    #[test]
    #[ignore = "daily"]
    fn crafting_guest_self_swap_keeps_eight_items() {
        use std::sync::Arc;
        use sunrise_client::chunk::Chunk;
        use sunrise_protocol::packets::play::{ContainerClick, Serverbound};
        super::super::native_guest_turn_tests::with_guests(|game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let table = game
                .pack
                .assets
                .registry
                .default_state("minecraft:crafting_table")
                .unwrap();
            game.world.set(1, 64, 1, table).unwrap();
            game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
                guest.inventory[0] =
                    Some(sunrise_client::items::ItemStack::new("minecraft:stone", 8));
            });
            game.lan_open_crafting(12, [1, 64, 1]);
            let window = game
                .lan
                .as_ref()
                .unwrap()
                .menus
                .get(&12)
                .unwrap()
                .crafting
                .table
                .as_ref()
                .unwrap()
                .0;
            game.lan_guest_packet(
                12,
                Serverbound::ContainerClick(ContainerClick {
                    container: window,
                    state: 0,
                    slot: 37,
                    button: 0,
                    input: 2,
                    changed: Vec::new(),
                    carried: None,
                }),
            );
            let guest = &game.lan.as_ref().unwrap().hub.guests[0];
            assert_eq!(guest.inventory[0].as_ref().unwrap().count, 8);
            assert_eq!(
                guest
                    .inventory
                    .iter()
                    .flatten()
                    .map(|s| s.count)
                    .sum::<u32>(),
                8
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn block_menu_handle_and_use_each_count_the_guest_once() {
        use crate::game::{Game, Hit, sculk::TEST_POSTED_EVENTS};
        use mod_api::{BlockUpdateFlags, GameMode};
        use std::sync::Arc;
        use sunrise_client::chunk::Chunk;
        super::super::native_guest_turn_tests::with_guests(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let level = game.level("minecraft:overworld").unwrap();
            let pos = (3, 64, 4);
            let at = [pos.0, pos.1, pos.2];
            let hit = Hit {
                pos: at,
                inside: false,
                face: [0, 1, 0],
                point: [3.5, 65.0, 4.5],
            };
            let use_menu = |game: &mut Game<'_>| {
                assert_eq!(
                    game.as_guest(12, |game| game.use_hand(Some(&hit)).unwrap()),
                    Some(true)
                );
            };
            let take_signals = |game: &mut Game<'_>| {
                let lids = std::mem::take(&mut game.server.lan_block_events)
                    .into_iter()
                    .filter_map(|(p, id, count, _)| (p == at && id == 1).then_some(count))
                    .collect::<Vec<_>>();
                let sounds = std::mem::take(&mut game.server.pending_sounds)
                    .into_iter()
                    .map(|sound| sound.event)
                    .collect::<Vec<_>>();
                let events = TEST_POSTED_EVENTS.with(|posted| {
                    std::mem::take(posted.borrow_mut().as_mut().unwrap())
                        .into_iter()
                        .map(|(event, _, _)| event)
                        .collect::<Vec<_>>()
                });
                (lids, sounds, events)
            };
            for block in ["chest", "ender_chest", "shulker_box"] {
                game.stop_audio_capture();
                let state = game
                    .pack
                    .assets
                    .registry
                    .default_state(&format!("minecraft:{block}"))
                    .unwrap();
                assert_eq!(
                    game.level_set_block(level, at, state, BlockUpdateFlags::NEIGHBORS),
                    Ok(true)
                );
                game.start_audio_capture();
                TEST_POSTED_EVENTS.with(|posted| *posted.borrow_mut() = Some(Vec::new()));
                let count = |game: &Game<'_>| {
                    if block == "shulker_box" {
                        game.be_anims.shulkers.get(&at).map_or(0, |s| s.open_count)
                    } else {
                        game.server.lid_openers.get(&pos).copied().unwrap_or(0)
                    }
                };
                // A shulker's veto can change the acting player to spectator, but
                // the use still spends the click and must never touch its opener count.
                let switch = (block == "shulker_box").then_some(GameMode::Spectator);
                mod_platform::testing::with_mods(
                    vec![mod_api::ModEntry::new(
                        "stop_menu",
                        Box::new(StopMenu(switch)),
                    )]
                    .into(),
                    || {
                        use_menu(game);
                        let guest = &game.lan.as_ref().unwrap().hub.guests[0];
                        assert!(guest.menu.is_none());
                        assert_eq!(guest.spectator(), switch.is_some());
                        assert_eq!(count(game), 0, "{block}");
                        assert_eq!(take_signals(game), (vec![], vec![], vec![]), "{block}");
                    },
                );
                if switch.is_some() {
                    // In the reverse direction the rejected spectator must not
                    // remove the other player's actual opener.
                    assert_eq!(game.open_block_menu(13, pos), super::MenuOpen::Opened);
                    assert_eq!(count(game), 1);
                    take_signals(game);
                    mod_platform::testing::with_mods(
                        vec![mod_api::ModEntry::new(
                            "stop_menu",
                            Box::new(StopMenu(Some(GameMode::Survival))),
                        )]
                        .into(),
                        || {
                            assert_eq!(
                                game.as_guest(12, |game| {
                                    game.spectator_open_menu(at);
                                    game.container == Some(pos)
                                }),
                                Some(true)
                            );
                            let guest = &game.lan.as_ref().unwrap().hub.guests[0];
                            assert!(!guest.spectator());
                            assert!(guest.menu.is_none());
                            assert_eq!(count(game), 1);
                            assert_eq!(take_signals(game), (vec![], vec![], vec![]));
                        },
                    );
                    game.guest_close_all(13);
                    assert_eq!(count(game), 0);
                    take_signals(game);
                }
                for direct in [false, true] {
                    if direct {
                        assert_eq!(game.open_block_menu(12, pos), super::MenuOpen::Opened);
                    } else {
                        use_menu(game);
                    }
                    assert!(game.lan.as_ref().unwrap().hub.guests[0].menu.is_some());
                    assert_eq!(count(game), 1, "{block}");
                    assert_eq!(
                        take_signals(game),
                        (
                            vec![1],
                            vec![format!("minecraft:block.{block}.open")],
                            vec!["minecraft:container_open"],
                        ),
                        "{block}"
                    );
                    game.guest_close_all(12);
                    assert_eq!(count(game), 0, "{block}");
                    assert_eq!(
                        take_signals(game),
                        (
                            vec![0],
                            vec![format!("minecraft:block.{block}.close")],
                            vec!["minecraft:container_close"],
                        ),
                        "{block}"
                    );
                }
                game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
                    guest.game_mode = 3;
                });
                assert_eq!(game.open_block_menu(12, pos), super::MenuOpen::Opened);
                assert_eq!(count(game), 0, "{block}");
                game.guest_close_all(12);
                assert_eq!(take_signals(game), (vec![], vec![], vec![]), "{block}");
                game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
                    guest.game_mode = 0;
                });
            }
            TEST_POSTED_EVENTS.with(|posted| *posted.borrow_mut() = None);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn block_part_use_item_on_consumes_once() {
        use super::super::native_entity_refs::tests::with_game;
        use mod_api::InteractionResult as Answer;
        use mod_api::{self, BlockPart, Hooks, Registrar, ServerHost, UseItemOnEvent};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering::SeqCst},
        };
        use sunrise_client::chunk::Chunk;

        #[derive(Clone)]
        struct BlockUse(Arc<AtomicUsize>);
        #[mod_api::sunrise_part]
        impl BlockPart for BlockUse {
            fn use_item_on(
                &self,
                _: &mut dyn ServerHost,
                _: &mut UseItemOnEvent<'_>,
            ) -> Option<Answer> {
                self.0.fetch_add(1, SeqCst);
                Some(Answer::Success)
            }
        }
        #[mod_api::__subscriptions]
        impl Hooks for BlockUse {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                let block_key = r.block_key("minecraft:lever");
                r.block_part(block_key, self.clone());
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "block_use",
            Box::new(BlockUse(calls.clone())),
        )]
        .into();
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let registry = game.pack.assets.registry;
                let stone = registry.default_state("minecraft:stone").unwrap();
                let lever = game
                    .with_props(
                        registry.default_state("minecraft:lever").unwrap(),
                        &[("face", "floor")],
                    )
                    .unwrap();
                game.world.set(0, 63, 4, stone).unwrap();
                game.world.set(0, 64, 4, lever).unwrap();
                let hit = super::super::Hit {
                    pos: [0, 64, 4],
                    face: [0, 1, 0],
                    point: [0.5, 65.0, 4.5],
                    inside: false,
                };
                assert!(game.use_hand(Some(&hit)).unwrap());
                assert_eq!(game.world.block(0, 64, 4), lever);
                assert_eq!(calls.load(SeqCst), 1);
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn replacing_empty_note_block_use_does_not_award_vanilla_stats() {
        use super::super::native_entity_refs::tests::with_game;
        use mod_api::InteractionResult as Answer;
        use mod_api::{self, BlockPart, Hooks, Registrar, ServerHost, UseWithoutItemEvent};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering::SeqCst},
        };
        use sunrise_client::chunk::Chunk;

        #[derive(Clone)]
        struct Replacement(Arc<AtomicBool>);
        #[mod_api::sunrise_part]
        impl BlockPart for Replacement {
            fn use_without_item(
                &self,
                _: &mut dyn ServerHost,
                _: &mut UseWithoutItemEvent<'_>,
            ) -> Option<Answer> {
                self.0.load(SeqCst).then_some(Answer::Success)
            }
        }
        #[mod_api::__subscriptions]
        impl Hooks for Replacement {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                let block_key = r.block_key("minecraft:note_block");
                r.block_part(block_key, self.clone());
            }
        }
        let replace = Arc::new(AtomicBool::new(true));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "empty_note_replacement",
            Box::new(Replacement(replace.clone())),
        )]
        .into();
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let note = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:note_block")
                    .unwrap();
                game.world.set(0, 64, 4, note).unwrap();
                let hit = super::super::Hit {
                    pos: [0, 64, 4],
                    face: [0, 1, 0],
                    point: [0.5, 65.0, 4.5],
                    inside: false,
                };
                assert!(game.use_hand(Some(&hit)).unwrap());
                assert_eq!(game.world.block(0, 64, 4), note);
                assert_eq!(game.stats.get("custom", "minecraft:tune_noteblock"), 0);

                // Delegating still runs the actual method and awards its stat.
                replace.store(false, SeqCst);
                assert!(game.use_hand(Some(&hit)).unwrap());
                assert_ne!(game.world.block(0, 64, 4), note);
                assert_eq!(game.stats.get("custom", "minecraft:tune_noteblock"), 1);
            });
        });
    }

    /// #4124: a STOP is measured from the START that named its block (`destroyPos`) — an abort, or
    /// a STOP from out of reach, ends the dig the guest's button held but leaves that clock — and
    /// breaks the block at 70 % of the dig, or the server finishes it alone from the same START.
    /// A guest that reaches a block only at a jump's apex keeps its dig this way, and no other
    /// block or new body inherits the clock.
    #[test]
    fn a_stop_after_a_fall_is_measured_from_the_starts_clock() {
        use super::{GuestPlay, Mining, StopDig, stop_dig};
        let pos = [12, 70, -5];
        let mut play = GuestPlay {
            digging: Some((pos, [0, 1, 0])),
            mining: Some(Mining {
                pos,
                progress: 0.5,
                per_tick: 0.0,
                ticks: 8,
                crack: None,
            }),
            destroy_pos: Some(pos),
            dig_start: 1000,
            ..GuestPlay::default()
        };
        // The landing: the dig the button held is gone, the clock the START set is not.
        play.dig_ended();
        assert!(play.digging.is_none() && play.mining.is_none());
        // 13 ticks of 0.05: 65 % of the dig, so the server finishes it from tick 1000 itself.
        assert_eq!(
            stop_dig(&play, pos, false, 1012, 0.05),
            StopDig::Delay(1000)
        );
        // 16 ticks: 80 %, the STOP breaks the block outright.
        assert_eq!(stop_dig(&play, pos, false, 1015, 0.05), StopDig::Break);
        // A block the START did not name never gets the clock, and a broken one only its crack.
        assert_eq!(
            stop_dig(&play, [12, 71, -5], false, 1015, 0.05),
            StopDig::Other
        );
        assert_eq!(stop_dig(&play, pos, true, 1015, 0.05), StopDig::Gone);
        // A respawn's new ServerPlayerGameMode starts from no clock at all.
        play.destroy_pos = None;
        assert_eq!(stop_dig(&play, pos, false, 1015, 0.05), StopDig::Other);
    }

    /// #3684: a player shrunk to `minecraft:scale` 0.1 who stands 0.05 from a
    /// wall has eyes 0.048 wide, which don't reach into it: they don't
    /// suffocate there, as a full-sized player 0.3 from the wall doesn't.
    #[test]
    fn a_small_player_checks_only_the_blocks_at_their_small_eyes() {
        let at = [205.95, 100.0, 530.5];
        assert_eq!(
            eye_columns(at, Pose::Standing.dimensions(0.1)),
            [205..=205, 530..=530]
        );
        assert_eq!(
            eye_columns(at, Pose::Standing.dimensions(1.0)),
            [205..=206, 530..=530]
        );
        assert_eq!(
            eye_columns([205.7, 100.0, 530.5], Pose::Standing.dimensions(1.0)),
            [205..=205, 530..=530]
        );
    }
}

/// A `ServerboundSelectBundleItemPacket` slot as the hub's `select_bundle_item` reads it.
/// With a crafting table or a station open (`own` slots of its own first), its
/// inventory part is the inventory menu's main inventory and hotbar from slot 9, in
/// the same order; its own slots hold no bundle the hub can reach.
pub(in crate::game) fn bundle_inventory_slot(slot: usize, own: Option<usize>) -> Option<usize> {
    match own {
        Some(own) => slot.checked_sub(own).filter(|s| *s < 36).map(|s| s + 9),
        None => Some(slot),
    }
}

/// Where the server aims a guest's attack or interact on a mob. The packet names only the
/// entity, so the server picked the mob's centre; a slab, trapdoor, stair or fence between the
/// eyes and that centre then hid a mob the guest's own crosshair hit through the gap (#5772).
/// This returns the centre when it is visible, else the visible point of the box nearest to it (the one nearer the eyes when two are equally near)
/// (`occluded` tells whether a block outline hides a point from the eyes); the centre again
/// when the whole box is hidden.
pub(in crate::game) fn visible_aim_point(
    eye: [f64; 3],
    lo: [f64; 3],
    hi: [f64; 3],
    occluded: impl Fn([f64; 3]) -> bool,
) -> [f64; 3] {
    let centre = [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0);
    if !occluded(centre) {
        return centre;
    }
    let margin = [0, 1, 2].map(|k| ((hi[k] - lo[k]) / 2.0).min(0.02));
    let across = |k: usize, steps: usize| {
        (0..=steps)
            .map(|i| {
                lo[k] + margin[k] + (hi[k] - lo[k] - 2.0 * margin[k]) * i as f64 / steps as f64
            })
            .collect::<Vec<f64>>()
    };
    let (xs, ys, zs) = (across(0, 2), across(1, 4), across(2, 2));
    let mut candidates = Vec::with_capacity(xs.len() * ys.len() * zs.len());
    for &x in &xs {
        for &y in &ys {
            for &z in &zs {
                candidates.push([x, y, z]);
            }
        }
    }
    let nearness = |p: &[f64; 3]| (0..3).map(|k| (p[k] - centre[k]).powi(2)).sum::<f64>();
    let from_eye = |p: &[f64; 3]| (0..3).map(|k| (p[k] - eye[k]).powi(2)).sum::<f64>();
    candidates.sort_by(|a, b| {
        nearness(a)
            .total_cmp(&nearness(b))
            .then(from_eye(a).total_cmp(&from_eye(b)))
            .then(a[1].total_cmp(&b[1]))
            .then(a[0].total_cmp(&b[0]))
            .then(a[2].total_cmp(&b[2]))
    });
    candidates
        .into_iter()
        .find(|p| !occluded(*p))
        .unwrap_or(centre)
}

/// The first target `(id, removed, bounds)` whose box meets the swept box,
/// skipping removed ones and the vehicle the player rides.
pub(in crate::game) fn first_spin_target(
    swept: Aabb,
    riding: Option<u32>,
    mut targets: impl Iterator<Item = (u32, bool, Aabb)>,
) -> Option<usize> {
    targets.position(|(id, removed, b)| !removed && Some(id) != riding && b.intersects(swept))
}

#[cfg(test)]
pub(in crate::game) mod hotbar_tests {
    use super::super::native_entity_refs::tests::with_game;
    use super::super::native_item_places::tests::{guest, open_to_lan};
    use mod_api::{
        Flow, Hooks, ModEntry, PlayerSwitchHotbarSlotPostEvent, PlayerSwitchHotbarSlotPreEvent,
        ServerHost,
    };
    use std::sync::{Arc, Mutex};
    use sunrise_protocol::packets::play;

    /// What the hooks heard, as (pre or post, uuid, entity id, old, new).
    type Heard = Arc<Mutex<Vec<(&'static str, u128, i32, i32, i32)>>>;

    /// Hears both hooks and refuses a switch to slot 8.
    struct Hotbar(Heard);
    #[mod_api::__subscriptions]
    impl Hooks for Hotbar {
        fn player_switch_hotbar_slot_pre(
            &self,
            _: &mut dyn ServerHost,
            e: &mut PlayerSwitchHotbarSlotPreEvent,
        ) -> Flow {
            let row = ("pre", e.player, e.player_entity_id, e.old_slot, e.new_slot);
            self.0.lock().unwrap().push(row);
            if e.new_slot == 8 {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }
        fn player_switch_hotbar_slot_post(
            &self,
            _: &mut dyn ServerHost,
            e: &mut PlayerSwitchHotbarSlotPostEvent,
        ) {
            let row = ("post", e.player, e.player_entity_id, e.old_slot, e.new_slot);
            self.0.lock().unwrap().push(row);
        }
    }

    /// `PlayerSwitchHotbarSlotEvent`: pre, then post once the slot changed, both naming the
    /// player by UUID and entity id; the same slot again fires neither, and a cancelled pre
    /// keeps the old slot with no post.
    #[test]
    #[ignore = "daily"]
    fn a_hotbar_switch_fires_pre_then_post_and_pre_can_keep_the_slot() {
        let heard = Heard::default();
        let mods = vec![ModEntry::new("hotbar", Box::new(Hotbar(heard.clone())))].into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                open_to_lan(game);
                game.lan.as_mut().unwrap().hub.guests.push(guest(1));
                let switch = |game: &mut super::Game<'_>, slot| {
                    game.lan_guest_packet(
                        1,
                        play::Serverbound::SetCarriedItem(play::SetCarriedItem { slot }),
                    );
                    game.lan.as_ref().unwrap().hub.guests[0].selected
                };
                assert_eq!(switch(game, 3), 3);
                assert_eq!(switch(game, 3), 3);
                assert_eq!(switch(game, 8), 3);
                assert_eq!(
                    *heard.lock().unwrap(),
                    [
                        ("pre", 123, 20, 0, 3),
                        ("post", 123, 20, 0, 3),
                        ("pre", 123, 20, 3, 8),
                    ]
                );
            });
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod respawn_position_tests {
    use super::super::native_entity_refs::tests::with_game;
    use super::super::native_item_places::tests::{guest, open_to_lan};
    use mod_api::{Hooks, LevelDimension, ModEntry, PlayerRespawnPositionEvent, ServerHost};
    use std::sync::{Arc, Mutex};
    use sunrise_client::chunk::Chunk;

    /// What the hook heard: the player's UUID, the original level and point,
    /// and whether the block was missing.
    type Heard = Arc<Mutex<Vec<(u128, LevelDimension, [f64; 3], bool)>>>;

    /// Moves every respawn ten blocks east, turns the player north, and
    /// keeps no respawn point for next time.
    struct East(Heard);
    #[mod_api::__subscriptions]
    impl Hooks for East {
        fn player_respawn_position(
            &self,
            _: &mut dyn ServerHost,
            e: &mut PlayerRespawnPositionEvent,
        ) {
            let row = (
                e.player.uuid(),
                e.original_level,
                e.original_position,
                e.missing_respawn_block,
            );
            self.0.lock().unwrap().push(row);
            e.position[0] += 10.0;
            e.yaw = 180.0;
            e.copy_original_spawn_position = false;
        }
    }

    /// `PlayerRespawnPositionEvent` on a server: a guest whose forced point
    /// needs no block respawns there, the mod hears that point with the
    /// player's handle, and its new point, facing and copy choice are what
    /// the player gets.
    #[test]
    #[ignore = "daily"]
    fn a_mod_moves_a_respawn_and_drops_the_point() {
        let heard = Heard::default();
        let mods = vec![ModEntry::new("east", Box::new(East(heard.clone())))].into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                open_to_lan(game);
                game.options.dedicated = Some(Vec::new());
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let mut player = guest(1);
                player.dead = true;
                player.spawn = Some([5.5, 64.0, 5.5]);
                game.lan.as_mut().unwrap().hub.guests.push(player);
                game.guest_respawn(1);
                let heard = heard.lock().unwrap();
                let [(uuid, level, original, missing)] = heard[..] else {
                    panic!("heard {heard:?}");
                };
                assert_eq!(
                    (uuid, level, missing),
                    (123, LevelDimension::Overworld, false)
                );
                let g = &game.lan.as_ref().unwrap().hub.guests[0];
                assert!(!g.dead);
                assert_eq!(g.position, [original[0] + 10.0, original[1], original[2]]);
                assert_eq!(g.yaw, 180.0);
                assert_eq!((g.spawn, &g.respawn_elsewhere), (None, &None));
            });
        });
    }

    /// Hears each respawn's facing and changes nothing.
    struct Listen(Arc<Mutex<Vec<([f64; 3], f32, f32)>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Listen {
        fn player_respawn_position(
            &self,
            _: &mut dyn ServerHost,
            e: &mut PlayerRespawnPositionEvent,
        ) {
            self.0.lock().unwrap().push((*e.position, e.yaw, e.pitch));
        }
    }

    /// A mod that only listens changes nothing: the guest respawns at its
    /// point facing the respawn direction, not the way it died, and keeps
    /// the point.
    #[test]
    #[ignore = "daily"]
    fn a_listening_mod_leaves_the_respawn_as_it_was() {
        let heard = Arc::default();
        let mods = vec![ModEntry::new(
            "listen",
            Box::new(Listen(Arc::clone(&heard))),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                open_to_lan(game);
                game.options.dedicated = Some(Vec::new());
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let mut player = guest(1);
                player.dead = true;
                (player.yaw, player.pitch) = (90.0, 30.0);
                player.spawn = Some([5.5, 64.0, 5.5]);
                game.lan.as_mut().unwrap().hub.guests.push(player);
                game.guest_respawn(1);
                let heard = heard.lock().unwrap();
                let [(found, yaw, pitch)] = heard[..] else {
                    panic!("heard {heard:?}");
                };
                assert_eq!((yaw, pitch), (0.0, 0.0), "told the respawn facing");
                let g = &game.lan.as_ref().unwrap().hub.guests[0];
                assert!(!g.dead);
                assert_eq!(g.position, found);
                assert_eq!((g.yaw, g.pitch), (0.0, 0.0));
                assert_eq!(g.spawn, Some([5.5, 64.0, 5.5]));
            });
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod pickup_xp_tests {
    use super::super::native_entity_refs::tests::with_game;
    use super::super::native_item_places::tests::{guest, open_to_lan};
    use sunrise_client::entities::ExperienceOrb;
    use sunrise_core::geometry::Vec3;

    /// Listens to `pickup_xp`, canceling it when told to.
    struct Listen(bool);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Listen {
        fn pickup_xp(
            &self,
            _: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::PickupXpEvent<'_>,
        ) -> mod_api::Flow {
            if self.0 {
                mod_api::Flow::Cancel
            } else {
                mod_api::Flow::Continue
            }
        }
    }

    /// A guest takes the orb it stands in: the orb's id left in the level and the
    /// guest's total experience after.
    fn take_orb() -> (bool, i32) {
        let mut out = (false, 0);
        with_game(|game| {
            open_to_lan(game);
            let mut g = guest(1);
            g.loaded = true;
            g.position = [0.5, 70.0, 0.5];
            game.lan.as_mut().unwrap().hub.guests.push(g);
            let p = Vec3::new(0.5, 70.0, 0.5);
            game.orbs.push(ExperienceOrb {
                size_answer: None,
                attachment_lifetime: Default::default(),
                rotation: [0.0, 0.0],
                join: Default::default(),
                id: 40,
                position: p,
                previous: p,
                velocity: Vec3::ZERO,
                value: 7,
                age: 0,
                ground: sunrise_world::ground_support::GroundState::from_ground_flag(true),
                count: 1,
                fire_ticks: 0,
                ticks: 0,
                air: 300,
                health: 5,
                no_gravity: true,
                following: None,
                fall_distance: 0.0,
            });
            game.guest_orbs(0);
            let left = game.orbs.iter().any(|o| o.id == 40);
            out = (left, game.lan.as_ref().unwrap().hub.guests[0].xp_total);
        });
        out
    }

    /// `PlayerXpEvent.PickupXp` through the shared `listened` probe: a pickup goes the
    /// same with no listener and with one that changes nothing, and a cancel keeps the orb.
    #[test]
    #[ignore = "daily"]
    fn pickup_xp_is_the_same_with_and_without_a_listener() {
        let none = take_orb();
        assert_eq!(none, (false, 7));
        let mods = vec![mod_api::ModEntry::new("listen", Box::new(Listen(false)))].into();
        mod_platform::testing::with_mods(mods, || assert_eq!(take_orb(), none));
        let mods = vec![mod_api::ModEntry::new("cancel", Box::new(Listen(true)))].into();
        mod_platform::testing::with_mods(mods, || assert_eq!(take_orb(), (true, 0)));
    }
}

#[cfg(test)]
pub(in crate::game) mod respawn_world_point_tests {
    #[allow(unused_imports)]
    use crate::game::{Audio, Game, Options, Progress, Settings, load_resources};
    #[allow(unused_imports)]
    use std::sync::{Arc, atomic::AtomicI32, mpsc};
    #[allow(unused_imports)]
    use sunrise_client::lan::{Joined, NetConfig, Out, PlayerSender, guest::Guest, hub::HubConfig};
    #[allow(unused_imports)]
    use sunrise_core::geometry::Vec3;
    #[allow(unused_imports)]
    use sunrise_protocol::packets::{configuration::ClientInformation, play::Clientbound};
}

#[cfg(test)]
pub(in crate::game) mod swing_echo_tests {
    use super::ItemStack;
    use crate::game::{Audio, Game, Options, Progress, Settings, load_resources};
    use std::sync::{Arc, atomic::AtomicI32, mpsc};
    use sunrise_client::chunk::Chunk;
    use sunrise_client::lan::{Joined, NetConfig, Out, PlayerSender, guest::Guest, hub::HubConfig};
    use sunrise_core::position::ChunkPos;
    use sunrise_protocol::packets::{
        configuration::ClientInformation,
        play::{self, Clientbound, Serverbound},
    };

    /// handleUseItemOn swings the player's arm, but sends the swing to the
    /// player's own client only when the client did not predict it: a block
    /// placed (SUCCESS) is the client's own swing, a bed's use
    /// (SUCCESS_SERVER) is not (#9887).
    #[test]
    #[ignore = "daily"]
    fn only_a_server_only_use_swings_back_to_the_player() {
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
            std::env::temp_dir().join(format!("sunrise-swing-echo-unit-{}", std::process::id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = Game::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap()
            .0;
        let mut chunk = Chunk::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 199, z, stone);
            }
        }
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("setblock 6 200 4 red_bed[facing=north,part=foot]"));
        assert!(game.run_command("setblock 6 200 3 red_bed[facing=north,part=head]"));
        game.lan_host(
            super::super::lan::LanSettings {
                port: 0,
                guest_commands: true,
                force_game_mode: false,
            },
            NetConfig {
                announce: false,
                ..NetConfig::loopback(0, "swing-echo-test".into(), 2)
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
                uuid: 331,
                name: "SwingGuest".into(),
                properties: Vec::new(),
                info: ClientInformation::new(2),
                brand: "test".into(),
                sender: PlayerSender::new(tx, Arc::default()),
                latency: Arc::new(AtomicI32::new(0)),
                address: "127.0.0.1:25565".parse().unwrap(),
                expected: false,
            },
            2,
            [4.5, 200.0, 4.5],
            2,
        );
        guest.dimension = "minecraft:overworld".into();
        guest.placed = true;
        guest.loaded = true;
        guest.inventory[0] = Some(ItemStack::new("minecraft:stone", 16));
        guest.selected = 0;
        let mut host = game.lan.take().unwrap();
        host.hub.guests.push(guest);
        let id = host.hub.guests[0].entity_id;
        let swings_back = || {
            let mut n = 0;
            for out in rx.try_iter() {
                let bodies = match out {
                    Out::Packet(body) => vec![body],
                    Out::Packets(bodies) => bodies,
                    Out::Disconnect(_) => panic!("unexpected disconnect"),
                };
                for body in bodies {
                    let mut r = sunrise_protocol::buf::Reader::new(&body);
                    let packet_id = r.varint().unwrap();
                    if let Ok(Clientbound::SwingAnimation(p)) =
                        Clientbound::decode(packet_id, r.rest())
                        && p.id == id
                    {
                        n += 1;
                    }
                }
            }
            n
        };
        let use_on = |pos: [i32; 3], cursor: [f32; 3], sequence: i32| {
            Serverbound::UseItemOn(play::UseItemOn {
                hand: 0,
                pos,
                face: 1,
                cursor,
                inside: false,
                world_border: false,
                sequence,
            })
        };
        game.lan = Some(host);
        // BlockItem.place: SUCCESS, the client's own swing.
        game.lan_guest_packet(2, use_on([4, 199, 6], [0.5, 1.0, 0.5], 1));
        assert_eq!(game.world.block(4, 200, 6).0, stone, "the stone is placed");
        assert_eq!(swings_back(), 0, "a placement is not swung back");
        // The swing ends before the next use can start one.
        for _ in 0..8 {
            game.lan_swings();
        }
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .update(0, |g| g.selected = 1);
        // BedBlock.useWithoutItem: SUCCESS_SERVER, the server's swing only.
        game.lan_guest_packet(2, use_on([6, 200, 4], [0.5, 0.5625, 0.5], 2));
        assert_eq!(swings_back(), 1, "a bed's use is swung back");
    }
}

/// `Entity.touchingUnloadedChunk`: the box at `at` inflated by one block
/// reaches a chunk that is not loaded (`Level.hasChunksAt` over the floor of
/// its minimum and the ceiling of its maximum, as section coordinates).
pub(in crate::game) fn touching_unloaded_chunk(
    at: [f64; 3],
    body: Dimensions,
    loaded: impl Fn(i32, i32) -> bool,
) -> bool {
    let half = f64::from(body.width) / 2.0 + 1.0;
    let section = |v: f64| (v as i32) >> 4;
    let (x0, x1) = (
        section((at[0] - half).floor()),
        section((at[0] + half).ceil()),
    );
    let (z0, z1) = (
        section((at[2] - half).floor()),
        section((at[2] + half).ceil()),
    );
    !(x0..=x1).all(|x| (z0..=z1).all(|z| loaded(x, z)))
}

#[cfg(test)]
pub(in crate::game) mod guest_close_listener_tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::game::{Audio, Options, Progress, Settings, load_resources};
    #[allow(unused_imports)]
    use std::sync::{Arc, atomic::AtomicI32, mpsc};
    #[allow(unused_imports)]
    use sunrise_client::{
        chunk::Chunk,
        containers::BlockEntity,
        lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig},
        sculk::Info,
    };
    #[allow(unused_imports)]
    use sunrise_core::position::ChunkPos;
    #[allow(unused_imports)]
    use sunrise_protocol::packets::configuration::ClientInformation;

    pub(in crate::game) fn selected(game: &Game<'_>) -> Option<Info> {
        let Some(BlockEntity::Sculk(sensor)) = game.world.block_entities.get(&(9, 100, 4)) else {
            panic!("actual command-created sensor must exist");
        };
        sensor.vibration.selector.current.map(|(info, _)| info)
    }
    pub(in crate::game) fn reset_listener(game: &mut Game<'_>) {
        assert!(game.run_command("setblock 9 100 4 air strict"));
        assert!(game.run_command("setblock 9 100 4 sculk_sensor strict"));
        game.sculk_prepare();
        game.sculk_sync_index();
        assert!(selected(game).is_none());
    }
    pub(in crate::game) fn open(game: &mut Game<'_>, conn: u32, sequence: i32) -> i32 {
        game.lan_guest_packet(
            conn,
            S::UseItemOn(play::UseItemOn {
                hand: 0,
                pos: [6, 100, 4],
                face: 1,
                cursor: [0.5, 1.0, 0.5],
                inside: false,
                world_border: false,
                sequence,
            }),
        );
        let guest = game.lan.as_mut().unwrap().hub.guest(conn).unwrap();
        assert!(guest.ack_up_to.get() >= sequence);
        assert_eq!(guest.health, 20.0);
        assert_eq!(guest.position, [4.5, 100.0, 4.5]);
        let menu = guest
            .menu
            .as_ref()
            .expect("actual packet must open the chest menu");
        assert_eq!(menu.pos, Some((6, 100, 4)));
        assert_eq!(menu.size, 27);
        menu.window
    }
    pub(in crate::game) fn close(game: &mut Game<'_>, conn: u32, window: i32) {
        game.lan_guest_packet(
            conn,
            S::ContainerClose(play::ContainerClose { container: window }),
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod recipe_check_budget_tests {
    use super::{GuestPlay, RECIPE_CHECK_COST};

    #[test]
    fn ten_checks_at_once_then_one_each_tick() {
        let mut play = GuestPlay::default();
        for _ in 0..10 {
            assert!(play.recipe_check_available());
            play.recipe_checks += RECIPE_CHECK_COST;
        }
        assert!(!play.recipe_check_available());
        for _ in 0..20 {
            play.decay_recipe_checks();
            assert!(play.recipe_check_available());
            play.recipe_checks += RECIPE_CHECK_COST;
            assert!(!play.recipe_check_available());
        }
        for _ in 0..20 {
            play.decay_recipe_checks();
        }
        assert_eq!(play.recipe_checks, 0);
    }
}

#[cfg(test)]
pub(in crate::game) mod item_on_block_cooldown_tests {
    use super::ItemStack;
    use crate::game::{Audio, Game, Options, Progress, Settings, load_resources};
    use std::sync::{Arc, atomic::AtomicI32, mpsc};
    use sunrise_client::chunk::Chunk;
    use sunrise_client::lan::{Joined, NetConfig, Out, PlayerSender, guest::Guest, hub::HubConfig};
    use sunrise_core::position::ChunkPos;
    use sunrise_protocol::packets::{
        configuration::ClientInformation,
        play::{self, Clientbound, Serverbound},
    };

    // Actual registered guest packet route, with real loaded block facts.
    fn placement_control(check_start: Option<bool>) {
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
            std::env::temp_dir().join(format!("sunrise-use-cooldown-unit-{}", std::process::id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = Game::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap()
            .0;
        let mut chunk = Chunk::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 199, z, stone);
            }
        }
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        game.lan_host(
            super::super::lan::LanSettings {
                port: 0,
                guest_commands: true,
                force_game_mode: false,
            },
            NetConfig {
                announce: false,
                ..NetConfig::loopback(0, "use-cooldown-test".into(), 2)
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
                uuid: 331,
                name: "SwingGuest".into(),
                properties: Vec::new(),
                info: ClientInformation::new(2),
                brand: "test".into(),
                sender: PlayerSender::new(tx, Arc::default()),
                latency: Arc::new(AtomicI32::new(0)),
                address: "127.0.0.1:25565".parse().unwrap(),
                expected: false,
            },
            2,
            [4.5, 200.0, 4.5],
            2,
        );
        guest.dimension = "minecraft:overworld".into();
        guest.placed = true;
        guest.loaded = true;
        guest.inventory[0] = Some(ItemStack::new("minecraft:stone", 16));
        guest.selected = 0;

        game.lan.as_mut().unwrap().hub.guests.push(guest);
        assert_eq!(game.lan.as_mut().unwrap().hub.guests[0].uuid, 331);
        assert!(
            game.lan.as_mut().unwrap().hub.guests[0].placed
                && game.lan.as_mut().unwrap().hub.guests[0].loaded
        );
        assert_eq!(game.lan.as_mut().unwrap().hub.guests[0].game_mode, 0);
        let mut planks = ItemStack::new("minecraft:oak_planks", 5);
        planks.data_mut().set_other(
            "minecraft:use_cooldown",
            Some(sunrise_core::nbt::Tag::compound([
                ("seconds", sunrise_core::nbt::Tag::Float(30.0f32.to_bits())),
                (
                    "cooldown_group",
                    sunrise_core::nbt::Tag::String("probe:group".into()),
                ),
            ])),
        );
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .update(0, |guest| guest.inventory[0] = Some(planks));
        let use_on = |x, sequence| {
            Serverbound::UseItemOn(play::UseItemOn {
                hand: 0,
                pos: [x, 199, 6],
                face: 1,
                cursor: [0.5, 1.0, 0.5],
                inside: false,
                world_border: false,
                sequence,
            })
        };
        assert!(game.world.is_air(game.world.block(5, 200, 6)));
        if check_start.is_none() {
            let mut stick = game.lan.as_mut().unwrap().hub.guests[0].inventory[0]
                .as_ref()
                .unwrap()
                .clone();
            stick.item = "minecraft:stick".into();
            stick.count = 1;
            game.lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .update(0, |guest| guest.inventory[0] = Some(stick));
            game.lan_guest_packet(2, use_on(5, 1));
            assert!(
                game.world.is_air(game.world.block(5, 200, 6)),
                "failed stick use"
            );
            assert!(
                !game.lan.as_mut().unwrap().play[&2]
                    .cooldowns
                    .is_on_cooldown("probe:group")
            );
            let mut planks = game.lan.as_mut().unwrap().hub.guests[0].inventory[0]
                .as_ref()
                .unwrap()
                .clone();
            planks.item = "minecraft:oak_planks".into();
            planks.count = 5;
            game.lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .update(0, |guest| guest.inventory[0] = Some(planks));
        }
        game.lan_guest_packet(2, use_on(5, 2));
        let oak = game
            .pack
            .assets
            .registry
            .default_state("minecraft:oak_planks")
            .unwrap();
        assert_eq!(game.world.block(5, 200, 6), oak, "actual first placement");
        assert_eq!(
            game.lan.as_mut().unwrap().hub.guests[0].inventory[0]
                .as_ref()
                .unwrap()
                .count,
            4
        );
        assert!(
            game.lan.as_mut().unwrap().play.contains_key(&2),
            "real acting guest context installed"
        );
        if check_start == Some(true) {
            assert!(
                game.lan.as_mut().unwrap().play[&2]
                    .cooldowns
                    .is_on_cooldown("probe:group"),
                "successful use starts the actual guest's group"
            );
            assert!(rx.try_iter().any(|out| {
                let bodies = match out { Out::Packet(body) => vec![body], Out::Packets(bodies) => bodies, Out::Disconnect(_) => panic!("disconnect") };
                bodies.iter().any(|body| {
                    let mut r = sunrise_protocol::buf::Reader::new(body);
                    matches!(Clientbound::decode(r.varint().unwrap(),r.rest()), Ok(Clientbound::Cooldown(p)) if p.group == "probe:group" && p.duration == 600)
                })
            }), "existing helper's actual cooldown packet");
        } else if check_start == Some(false) {
            game.lan_guest_packet(2, use_on(6, 2));
            assert!(
                game.world.is_air(game.world.block(6, 200, 6)),
                "same-group second placement is rejected"
            );
            assert_eq!(
                game.lan.as_mut().unwrap().hub.guests[0].inventory[0]
                    .as_ref()
                    .unwrap()
                    .count,
                4
            );
        }
    }
    #[test]
    #[ignore = "daily"]
    fn failed_stick_use_keeps_group_available_for_placement() {
        placement_control(None);
    }
    #[test]
    #[ignore = "daily"]
    fn actual_guest_placement_starts_its_cooldown() {
        placement_control(Some(true));
    }
    #[test]
    #[ignore = "daily"]
    fn actual_guest_second_placement_is_rejected() {
        placement_control(Some(false));
    }
}

#[cfg(test)]
#[path = "tests/aim_point_tests.rs"]
pub(in crate::game) mod aim_point_tests;

#[cfg(test)]
#[path = "tests/boat_holder_movement_tests.rs"]
pub(in crate::game) mod boat_holder_movement_tests;

#[cfg(test)]
#[path = "tests/bundle_slot_tests.rs"]
pub(in crate::game) mod bundle_slot_tests;

#[cfg(test)]
#[path = "tests/crack_feedback_tests.rs"]
pub(in crate::game) mod crack_feedback_tests;

#[cfg(test)]
#[path = "tests/menu_still_valid_tests.rs"]
pub(in crate::game) mod menu_still_valid_tests;

#[cfg(test)]
#[path = "tests/player_entity_tick_tests.rs"]
pub(in crate::game) mod player_entity_tick_tests;

#[cfg(test)]
#[path = "tests/projectile_lifecycle_tests.rs"]
pub(in crate::game) mod projectile_lifecycle_tests;

#[cfg(test)]
#[path = "tests/spin_target_tests.rs"]
pub(in crate::game) mod spin_target_tests;

#[cfg(test)]
#[path = "tests/spawn_protection_tests.rs"]
pub(in crate::game) mod spawn_protection_tests;

#[cfg(test)]
#[path = "tests/unloaded_chunk_tests.rs"]
pub(in crate::game) mod unloaded_chunk_tests;
