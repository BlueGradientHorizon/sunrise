//! Multiplayer: the game driven by a server connection instead of the local
//! simulation. Mirrors vanilla's ClientPacketListener (chunks, entities,
//! inventory, health, time, weather, chat, respawn and dimension changes),
//! LocalPlayer.sendPosition (movement packets) and MultiPlayerGameMode
//! (block breaking with sequence numbers, use-item, attacks, container
//! clicks). The player still moves with the local physics, as the vanilla
//! client predicts its own movement; the server stays authoritative for
//! everything else.
pub(in crate::game) mod selector_metadata;
#[cfg(test)]
use selector_metadata::hurt_sound_for;
pub(in crate::game) mod attack_target;
pub(in crate::game) mod fall_sound;
pub(in crate::game) mod general;
pub(in crate::game) mod interaction_actions;
pub(in crate::game) mod interaction_prediction;
#[allow(unused_imports)]
use inventory::{
    CREATIVE_SYNCED, creative_sync_slot, creative_sync_stacks, mark_synced, predict_clicks,
};
pub(in crate::game) mod recipe_properties;
pub(in crate::game) mod use_item;

pub(in crate::game) mod block_entity_data;
#[cfg(test)]
pub(in crate::game) mod block_entity_data_tests;
#[cfg(test)]
pub(in crate::game) mod claimed_item_use_tests;
#[cfg(test)]
pub(in crate::game) mod gamemode_actions_tests;
#[allow(unused_imports)]
use super::{Dim, Game, Hit, Screen};
#[cfg(test)]
use movement::MovePacket;
#[allow(unused_imports)]
#[cfg(test)]
// The move-packet tests reach it through `super::super`.
use movement::stepped_move;
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;
#[allow(unused_imports)]
#[cfg(test)]
use sunrise_client_lib::net::{Event, Session};
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_items::items::ItemStack;
#[allow(unused_imports)]
use sunrise_level::{chunk::Chunk, containers::BlockEntity};
#[allow(unused_imports)]
use sunrise_protocol::{
    item::Stack,
    packets::{
        Empty,
        play::{self, Clientbound as C, Serverbound as S},
    },
    registry::{Builtin, DimensionType, Dynamic},
    secure_chat::Signature,
    text::Language,
};
use sunrise_sim::first_person_sim::Hand;
#[allow(unused_imports)]
use sunrise_sim::{
    join_seq::JoinSeq,
    menu::Input,
    mobs::{Mob, MobType},
    motion::PlayerMotion,
    sprint::Keys,
};
#[allow(unused_imports)]
use {
    sunrise_core::nbt::Tag,
    sunrise_mod_runtime::attachments::{Holder, ResidentAttachments},
    sunrise_world::{
        registry::BlockStateId,
        stepped_interpolation::{Path, PosRot, SteppedInterpolation},
    },
};

pub(in crate::game) mod abilities;
pub(in crate::game) mod chat;
#[cfg(test)]
pub(in crate::game) mod chat_audit_tests;
pub(in crate::game) mod chat_received;
pub(in crate::game) mod commands;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game) use chat::Decoration;
pub(in crate::game) mod client_level_tick;
pub(in crate::game) mod effect;
pub(in crate::game) mod entity_data_schema;
pub(in crate::game) mod entity_section_steps;
pub(in crate::game) mod inventory;
pub(in crate::game) mod item_use;
pub(in crate::game) mod kept_columns;
pub(in crate::game) mod minecarts;
pub(in crate::game) mod motion;
pub(in crate::game) mod movement;
pub(in crate::game) mod packets_entity;
pub(in crate::game) mod packets_object;
pub(in crate::game) mod packets_player;
pub(in crate::game) mod packets_world;
pub(in crate::game) mod passengers;
pub(in crate::game) mod skin;
pub(in crate::game) mod status;
pub(in crate::game) mod tables;
pub(in crate::game) mod tick;
pub(in crate::game) mod ui;
#[path = "use.rs"]
pub(in crate::game) mod r#use;

#[allow(unused_imports)]
pub(in crate::game) use super::villager_client::builtin_registries;

pub(in crate::game) mod chat_queue;

/// A text component the server sent, for the HUD.
pub(in crate::game) fn remote_text(tag: &Tag) -> sunrise_game_data::text::Text {
    use sunrise_game_data::scoreboard::{comp_text, tag_to_json};
    comp_text(&tag_to_json(tag))
}

/// `Player.getVoicePitch`: a constant 1.0
/// (`javap -p -c -constants net.minecraft.world.entity.player.Player`:
/// `0: fconst_1  1: freturn`), so a player's own hurt and death sounds are
/// neither raised for a baby nor jittered - `LivingEntity.getVoicePitch`, which
/// the mobs port, does both. It takes no random, so it draws nothing from the
/// level random either.
pub(in crate::game) fn player_voice_pitch() -> f32 {
    1.0
}

/// The live entity a bound sound names; absence means that object was removed.
pub(in crate::game) fn entity_sound_state(
    id: i32,
    player: Option<(i32, [f64; 3], bool)>,
    mob: Option<sunrise_game_data::sound_source::SoundEntityState>,
    tracked: &HashMap<i32, crate::game::client_side::remote_client::Tracked>,
    dragon: Option<(i32, [f64; 3])>,
) -> Option<sunrise_game_data::sound_source::SoundEntityState> {
    use sunrise_game_data::sound_source::SoundEntityState;
    if let Some((player_id, position, silent)) = player
        && player_id == id
    {
        return Some(SoundEntityState {
            position,
            silent,
            removed: false,
        });
    }
    let mut state = mob.or_else(|| {
        tracked.get(&id).map(|t| SoundEntityState {
            position: t.position,
            silent: t.silent,
            removed: false,
        })
    })?;
    // The dragon's Mob is metadata storage; only Dragon follows its movement.
    if let Some((dragon_id, position)) = dragon
        && dragon_id == id
    {
        state.position = position;
    }
    Some(state)
}

/// `ClientboundOpenScreenPacket.title` as a container screen draws it: the
/// component's styles, and the `§` codes a literal holds (`StringDecomposer`).
pub(in crate::game) fn menu_title(tag: &Tag) -> sunrise_game_data::text::Text {
    remote_text(tag).resolve_legacy()
}

/// Smooth an entity toward a new position over three ticks (InterpolationHandler).
#[derive(Clone, Copy)]
/// `LinearInterpolationHandler`'s target: a position and rotation (yRot,
/// xRot) reached in `steps` ticks.
pub(in crate::game) struct Lerp {
    pub(in crate::game) target: [f64; 3],
    pub(in crate::game) rotation: Option<(f32, f32)>,
    pub(in crate::game) steps: u32,
}

/// `SteppedInterpolationHandler.doInterpolate`'s `setPos` and `setRot` on a
/// mob: its yaw kept continuous (the same turn as vanilla's, without the
/// wrap that would spin it in the frame between ticks), its pitch clamped.
pub(in crate::game) fn place_interpolated(m: &mut Mob, at: PosRot) {
    m.position = Vec3::new(at.position[0], at.position[1], at.position[2]);
    m.yaw += wrap_degrees(at.yaw - m.yaw);
    m.head_pitch = (at.pitch % 360.0).clamp(-90.0, 90.0);
}

/// Where a mob is, as its `SteppedInterpolationHandler` sees it.
pub(in crate::game) fn mob_pos_rot(m: &Mob) -> PosRot {
    PosRot {
        position: [m.position.x, m.position.y, m.position.z],
        yaw: m.yaw,
        pitch: m.head_pitch,
    }
}

/// The open menu name of a server's horse inventory, then its chest's
/// columns (the packet names no `MenuType`).
pub(in crate::game) const MOUNT_MENU: &str = "sunrise:mount";
/// ServerboundPlayerInputPacket's shift bit.
pub(in crate::game) const SHIFT_INPUT: u8 = 1 << 5;

pub(in crate::game) fn face_direction(face: [i32; 3]) -> u8 {
    match face {
        [0, -1, 0] => 0,
        [0, 1, 0] => 1,
        [0, 0, -1] => 2,
        [0, 0, 1] => 3,
        [-1, 0, 0] => 4,
        [1, 0, 0] => 5,
        _ => 1,
    }
}

/// A protocol number format as the scoreboard's.
pub(in crate::game) fn number_format(
    f: &play::NumberFormat,
) -> sunrise_game_data::scoreboard::NumberFormat {
    use sunrise_game_data::scoreboard::{NumberFormat, tag_to_json};
    match f {
        play::NumberFormat::Blank => NumberFormat::Blank,
        play::NumberFormat::Styled(t) => NumberFormat::Styled(tag_to_json(t)),
        play::NumberFormat::Fixed(t) => NumberFormat::Fixed(tag_to_json(t)),
    }
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    // remote_sound_name: remote/sync.rs

    // connect_remote_session: remote/session.rs

    // init_remote_for_replay: remote/session.rs

    // leave_server: remote/session.rs

    /// Their kind and place, for `status` (shared projectiles; client callers append `hooks_status`).
    pub(in crate::game) fn projectiles_status(&self) -> Vec<serde_json::Value> {
        use serde_json::json;
        let at = |p: sunrise_core::geometry::Vec3| [p.x, p.y, p.z];
        let mut out = Vec::new();
        for f in &self.fangs.list {
            out.push(
                json!({"id": f.id, "kind": "minecraft:evoker_fangs", "position": f.pos,
                "yaw": f.yaw, "started": f.started, "life": f.life}),
            );
        }
        for t in &self.tnts {
            out.push(json!({"id": t.id, "kind": "minecraft:tnt", "position": at(t.position), "fuse": t.fuse}));
        }
        for f in &self.falling {
            out.push(json!({"id": f.id, "kind": "minecraft:falling_block", "position": at(f.position), "velocity": at(f.velocity), "state": f.state.0}));
        }
        for b in &self.boats {
            out.push(json!({"id": b.id, "kind": "minecraft:boat", "position": at(b.position), "velocity": at(b.velocity)}));
        }
        for c in &self.carts {
            out.push(json!({"id": c.id, "kind": "minecraft:minecart", "position": at(c.position), "velocity": at(c.velocity)}));
        }
        for c in &self.crystals {
            out.push(json!({"id": c.id, "kind": "minecraft:end_crystal", "position": at(c.position), "show_bottom": c.show_bottom}));
        }
        for t in &self.tridents {
            out.push(json!({"id": t.id, "kind": "minecraft:trident", "position": at(t.position), "velocity": at(t.velocity), "on_fire": t.fire_ticks > 0}));
        }
        for r in &self.rockets {
            out.push(json!({"id": r.id, "kind": "minecraft:firework_rocket", "position": at(r.position), "velocity": at(r.velocity)}));
        }
        for p in &self.creatures.projectiles {
            out.push(json!({"id": p.id, "kind": format!("{:?}", p.kind).split([' ', '{']).next().unwrap_or(""), "position": at(p.position), "velocity": at(p.velocity),
                "player_owned": p.owner == Some(sunrise_sim::mobs::Who::Player),
                "on_fire": p.arrow.as_ref().is_some_and(|a| a.on_fire())}));
        }
        for (kind, p, _) in self.potions.thrown_entities() {
            out.push(json!({"kind": kind, "position": at(p)}));
        }
        for (id, p) in self.end.eye_positions() {
            out.push(json!({"id": id, "kind": "minecraft:eye_of_ender", "position": p}));
        }
        out
    }

    // move_entity: remote/motion.rs

    // remote_objective: remote/sync.rs

    // remote_teleport: remote/sync.rs

    // tick_remote and tick_remote_player: remote/tick.rs

    // tick_remote_guardian_eyes: remote/tick.rs

    // tick_remote_entities: remote/tick.rs

    // remote_mining: remote/use.rs

    // remote_use_block: remote/use.rs

    // remote_block_reacts: remote/use.rs
    // remote_block_template: remote/use.rs

    // remote_place_preview: remote/use.rs

    // remote_use_bucket: remote/use.rs

    // remote_start_held_use: remote/use.rs

    // remote_use_instrument: remote/use.rs

    // remote_use_hands: remote/use.rs

    // remote_complete_using: remote/use.rs

    // remote_send_carried: remote/sync.rs

    // remote_player_attack: remote/ui.rs

    // remote_key: remote/ui.rs

    // remote_click: remote/use.rs

    // remote_overlay: remote/ui.rs

    // remote_status: remote/status.rs
}

/// `Entity.lookAt`'s rotation from `from` towards `to`, as (yaw, pitch).
pub(in crate::game) fn look_at_rotation(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    use sunrise_core::geometry::trig::atan2;
    let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    let horizontal = (dx * dx + dz * dz).sqrt();
    // One multiply by `Mth.RAD_TO_DEG` on the double.
    let degrees = |r: f64| r * f64::from(sunrise_core::geometry::trig::RAD_TO_DEG);
    let pitch = wrap_degrees((-degrees(atan2(dy, horizontal))) as f32);
    let yaw = wrap_degrees(degrees(atan2(dz, dx)) as f32 - 90.0);
    (yaw, pitch)
}

/// `PositionMoveRotation.calculateAbsolute`'s pitch: the packet's pitch,
/// added to `source` when it is relative, clamped to +-90.
pub(in crate::game) fn position_packet_pitch(source: f32, change: f32, relative: bool) -> f32 {
    let offset = if relative { source } else { 0.0 };
    (offset + change).clamp(-90.0, 90.0)
}

/// `Mth.wrapDegrees`.
pub(in crate::game) fn wrap_degrees(a: f32) -> f32 {
    let a = a % 360.0;
    if a >= 180.0 {
        a - 360.0
    } else if a < -180.0 {
        a + 360.0
    } else {
        a
    }
}

/// `LivingEntity.tick`'s body target and `tickHeadTurn` for another player:
/// the body turns toward where it walks (backwards when walking back), or to
/// the look while the arm swings, and stays within 50 degrees of the look.
pub(in crate::game) fn remote_player_head_turn(m: &mut Mob) {
    let (dx, dz) = (m.position.x - m.previous.x, m.position.z - m.previous.z);
    let mut target = m.body_yaw;
    if (dx * dx + dz * dz) as f32 > 0.002_500_000_2 {
        let walk = (dz.atan2(dx) as f32).to_degrees() - 90.0;
        let facing = (wrap_degrees(m.yaw) - walk).abs();
        target = if 95.0 < facing && facing < 265.0 {
            walk - 180.0
        } else {
            walk
        };
    }
    if m.anim.attack_anim > 0.0 {
        target = m.yaw;
    }
    m.body_yaw += wrap_degrees(target - m.body_yaw) * 0.3;
    let head = wrap_degrees(m.yaw - m.body_yaw);
    if head.abs() > 50.0 {
        m.body_yaw += head - head.signum() * 50.0;
    }
}

/// ServerboundContainerClickPacket's `ContainerInput` ordinal.
pub(in crate::game) fn input_ordinal(input: Input) -> i32 {
    match input {
        Input::Pickup => 0,
        Input::QuickMove => 1,
        Input::Swap => 2,
        Input::Clone => 3,
        Input::Throw => 4,
        Input::QuickCraft => 5,
        Input::PickupAll => 6,
    }
}

pub(in crate::game) mod dry_level;
#[cfg(test)]
use dry_level::DryLevel;

pub(in crate::game) type DrySound = (
    sunrise_core::position::BlockPos,
    std::sync::Arc<str>,
    mod_api::SoundSource,
    f32,
    f32,
);
/// The blocks a use on the dry level set.
pub(in crate::game) type DryChanges = Vec<(sunrise_core::position::BlockPos, BlockStateId)>;
/// What a use on the dry level changed, and the sounds it played.
pub(in crate::game) type DryUse = (DryChanges, Vec<DrySound>);
/// Where `BlockItem.place` on the dry level placed, and the blocks it set.
pub(in crate::game) struct DryPlace {
    pub(in crate::game) pos: sunrise_core::position::BlockPos,
    pub(in crate::game) matches_candidate: bool,
    pub(in crate::game) changes: DryChanges,
}

/// The blocks a guest's hand flips that its use predicts (and hears at once): a wooden or
/// copper door, trapdoor or fence gate, a lever, a button or a cauldron, whose fill level
/// changes as the bucket, bottle or dye lands. An iron door or trapdoor has no hand use.
pub(in crate::game) fn hand_operated(
    behaviours: &sunrise_world::level::Behaviours,
    state: BlockStateId,
) -> bool {
    behaviours
        .data(state)
        .is(sunrise_world::level::classes::Class::FenceGate)
        || named_hand_operated(behaviours.table.name(state))
}

pub(in crate::game) fn named_hand_operated(name: &str) -> bool {
    let name = name.trim_start_matches("minecraft:");
    ((name.ends_with("_door") || name.ends_with("_trapdoor")) && !name.starts_with("iron_"))
        || name.ends_with("_button")
        || name == "lever"
        || name.ends_with("cauldron")
}

/// `InteractionHand` on the wire.
pub(in crate::game) fn hand_id(hand: Hand) -> i32 {
    match hand {
        Hand::Main => 0,
        Hand::Off => 1,
    }
}

/// Whether `Item.use` of `info` ends `Minecraft.startUseItem`'s click, so the
/// off hand gets no turn. `FireworkRocketItem.use` answers SUCCESS while the
/// player glides (the rocket boosts them) and PASS otherwise.
pub(in crate::game) fn use_ends_click(
    info: &sunrise_items::items::ItemInfo,
    fall_flying: bool,
) -> bool {
    item_has_use(info) || (fall_flying && &*info.id == "minecraft:firework_rocket")
}

/// Whether `Item.use` of this item answers SUCCESS on this client, which
/// swings (`MultiPlayerGameMode.useItem`'s `result.shouldSwing()`), rather
/// than CONSUME, which only ends the click: a bow, crossbow, trident or
/// spear starting its charge, `ItemUtils.startUsingInstantly`'s food, drink,
/// spyglass, shield and goat horn, and the ender eye's SUCCESS_SERVER (the
/// server's swing source, not this client's).
pub(in crate::game) fn remote_use_swings(held: &ItemStack, consumable: bool) -> bool {
    let id = &*held.item;
    !matches!(
        id,
        "minecraft:bow"
            | "minecraft:crossbow"
            | "minecraft:trident"
            | "minecraft:spyglass"
            | "minecraft:goat_horn"
            | "minecraft:ender_eye"
            | "minecraft:brush"
    ) && !consumable
        && sunrise_sim::spear_sim::Kinetic::of(id).is_none()
}

/// Whether an object interaction on the client predicts a hand swing (`InteractionResult.SUCCESS`).
/// Boarding a minecart answers CONSUME and does not swing.
pub(in crate::game) fn remote_object_swings(response: Option<bool>, is_boarding: bool) -> bool {
    response.unwrap_or(!is_boarding)
}

/// Whether right-clicking an item frame passes (`InteractionResult.PASS`).
/// A fixed frame or an empty frame when clicking with an empty hand passes.
pub(in crate::game) fn item_frame_interact_passes(
    fixed: bool,
    frame_has_item: bool,
    hand_has_item: bool,
) -> bool {
    fixed || (!frame_has_item && !hand_has_item)
}

/// Whether right-clicking a boat passes (`InteractionResult.PASS`) on a client.
/// `AbstractBoat.interact` passes when sneaking or out of control (60 ticks), and
/// answers SUCCESS otherwise without checking for room (only the server's
/// `startRiding` does). `AbstractChestBoat.interact` opens the chest instead of
/// passing, unless it is out of control, not sneaked at and has room.
pub(in crate::game) fn boat_interact_passes(
    chest: bool,
    sneaking: bool,
    out_of_control: bool,
    has_room: bool,
) -> bool {
    if chest {
        !sneaking && out_of_control && has_room
    } else {
        sneaking || out_of_control
    }
}

/// Whether `Item.use` usually acts on this item (eats, drinks, equips, raises,
/// throws, shoots, fills or places), which ends `Minecraft.startUseItem` at
/// its hand. Tools and materials pass, and the off hand gets its turn. The
/// server decides; this is the client's guess of whether to go on. A carrot or warped fungus
/// on a stick is not here: `FoodOnAStickItem.use` passes on a client, and only the server's
/// answer to a boost that can run swings the hand.
pub(in crate::game) fn item_has_use(info: &sunrise_items::items::ItemInfo) -> bool {
    let id = info.id.trim_start_matches("minecraft:");
    id.ends_with("_spear")
        || info.is("minecraft:fishing_rod") // a content mod's rod too
        || info.consumable.is_some()
        || (info.equip_slot.is_some() && info.swappable)
        || info.blocks_attacks.is_some()
        || sunrise_items::items::bucket_contents(&info.id).is_some()
        || id.ends_with("_boat")
        || id.ends_with("_raft")
        || id.ends_with("_bundle")
        || matches!(
            id,
            "bow"
                | "crossbow"
                | "trident"
                | "spyglass"
                | "snowball"
                | "egg"
                | "blue_egg"
                | "brown_egg"
                | "ender_pearl"
                | "ender_eye"
                | "wind_charge"
                | "splash_potion"
                | "lingering_potion"
                | "experience_bottle"
                | "bucket"
                | "glass_bottle"
                | "map"
                | "writable_book"
                | "written_book"
                | "goat_horn"
                | "bundle"
                | "brush"
                | "knowledge_book"
        )
}

/// The view distance a client asks for: its render distance, which Sunrise
/// lets go past vanilla's 32 (a vanilla server still caps it at its own).
pub(in crate::game) fn client_view_distance(render_distance: i32) -> i8 {
    render_distance.clamp(2, i32::from(i8::MAX)) as i8
}

/// `ClientPacketListener.handleBlockEntityData` loads the tag into the existing
/// block entity, so the state the client builds itself stays: copied from `old`
/// into `new`, the block entity decoded from the tag.
pub(in crate::game) fn keep_client_state(old: Option<&BlockEntity>, new: &mut BlockEntity) {
    match (old, new) {
        // The turn of a vault's item and a trial spawner's mob (VaultClientData).
        (Some(BlockEntity::Vault(old)), BlockEntity::Vault(new)) => {
            new.spin = old.spin;
            new.o_spin = old.o_spin;
        }
        (Some(BlockEntity::TrialSpawner(old)), BlockEntity::TrialSpawner(new)) => {
            new.spin = old.spin;
            new.o_spin = old.o_spin;
        }
        // A beacon's beam (`BeaconBlockEntity.loadAdditional` reads only the
        // powers): a fresh one draws nothing until the next tick walks it again.
        (Some(BlockEntity::Beacon(old)), BlockEntity::Beacon(new)) => {
            new.sections.clone_from(&old.sections);
            new.checking.clone_from(&old.checking);
            (new.last_check_y, new.levels) = (old.last_check_y, old.levels);
        }
        // `eruptionTick` is set once, when the level is set; an update tag leaves it.
        (Some(BlockEntity::PotentSulfur(old)), BlockEntity::PotentSulfur(new)) => {
            new.eruption_tick = old.eruption_tick;
        }
        _ => {}
    }
}

/// `LocalPlayer.onSyncedDataUpdated(DATA_LIVING_ENTITY_FLAGS)`: the server's
/// flags (1 using, 2 the off hand) start a use the client is not running
/// (`startUsingItem`, which needs an item in that hand), and stop one it is
/// (`stopUsingItem`). `using` is the slot in use and its ticks.
pub(in crate::game) fn synced_local_use(
    using: Option<(usize, u32)>,
    flags: i8,
    selected: usize,
    has_item: impl Fn(usize) -> bool,
) -> Option<(usize, u32)> {
    let server_using = flags & 1 != 0;
    match using {
        None if server_using => {
            let slot = if flags & 2 != 0 {
                crate::game::held_items::OFFHAND_SLOT
            } else {
                selected
            };
            has_item(slot).then_some((slot, 0))
        }
        Some(_) if !server_using => None,
        using => using,
    }
}

pub(in crate::game) fn object_skin_changed(
    published: &HashMap<String, String>,
    key: &str,
    skin: &str,
) -> bool {
    published.get(key).is_none_or(|previous| previous != skin)
}

/// `AttributeInstance.calculateValue` before the attribute's range: the base,
/// `ADD_VALUE`s, then `ADD_MULTIPLIED_BASE`s of that, then each
/// `ADD_MULTIPLIED_TOTAL`.
pub(in crate::game) fn snapshot_value(s: &play::AttributeSnapshot) -> f64 {
    let amounts = |op: i32| {
        s.modifiers
            .iter()
            .filter(move |m| m.operation == op)
            .map(|m| m.amount)
    };
    let base = s.base + amounts(0).sum::<f64>();
    let mut value = base + amounts(1).map(|a| base * a).sum::<f64>();
    for amount in amounts(2) {
        value *= 1.0 + amount;
    }
    value
}

/// What `ChatScreen.handleChatInput` sends of typed chat once `ClientChatEvent` (`ask`, with
/// the outgoing message and the cancel flag) had its say: nothing when canceled or emptied.
pub(in crate::game) fn chat_after_event(
    message: &str,
    ask: impl FnOnce(&mut String, &mut bool),
) -> Option<String> {
    let mut outgoing = message.to_owned();
    let mut cancel = false;
    ask(&mut outgoing, &mut cancel);
    (!cancel && !outgoing.is_empty()).then_some(outgoing)
}
/// `Entity.isOnFire` for the window's player: while the server's flag is set
/// the predicted tick's countdown never takes `fire_ticks` below 1.
pub(in crate::game) fn hold_server_fire(fire_ticks: i32, server_on_fire: bool) -> i32 {
    if server_on_fire {
        fire_ticks.max(1)
    } else {
        fire_ticks
    }
}

/// `Entity.isColliding`: whether the collision `shape` of a block at `pos`
/// meets `entity`'s box. Shapes are block-local, so they move onto the block
/// first (`VoxelShape.move`), and any box that overlaps is a collision
/// (`Shapes.joinIsNotEmpty` with `BooleanOp.AND`).
pub(in crate::game) fn shape_meets(
    shape: &sunrise_core::geometry::VoxelShape,
    pos: [i32; 3],
    entity: sunrise_core::geometry::Aabb,
) -> bool {
    let at = Vec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
    shape
        .boxes()
        .iter()
        .any(|b| b.move_by(at).intersects(entity))
}

/// `ClientLevel.syncBlockState`'s snap: a restored block that meets the player
/// sends it back to the position the prediction remembered, unless the player
/// was teleported after that prediction was made
/// (`lastTeleportSequence < sequence`: the remembered position is stale).
pub(in crate::game) fn snap_target(
    recorded: [f64; 3],
    last_teleport_sequence: i32,
    sequence: i32,
    meets: bool,
) -> Option<[f64; 3]> {
    (meets && last_teleport_sequence < sequence).then_some(recorded)
}

/// [`Remote::has_level`]: logged in (a Login arrived) and no reconfiguration
/// since (`reconfiguring` is set by StartConfiguration, cleared by the next
/// Login).
pub(in crate::game) fn client_has_level(logged_in: bool, reconfiguring: Option<u64>) -> bool {
    logged_in && reconfiguring.is_none()
}

// A packet's removals (`RemoveEntities`), apart: this file may not grow.
#[cfg(test)]
pub(in crate::game) mod interaction_event_tests;

#[cfg(test)]
pub(in crate::game) mod native_entity_tests;
#[cfg(test)]
pub(in crate::game) mod no_gravity_data_tests;

#[cfg(test)]
pub(in crate::game) mod camera_view_tests;
#[cfg(test)]
pub(in crate::game) mod effect_blend_tests;
#[cfg(test)]
pub(in crate::game) mod level_sound_tests;

#[cfg(test)]
pub(in crate::game) mod attachment_query_tests;
#[cfg(test)]
pub(in crate::game) mod client_camera_tests;
#[cfg(test)]
pub(in crate::game) mod passage_cross_tests;
#[cfg(test)]
pub(in crate::game) mod respawn_motion_tests;
#[cfg(test)]
pub(in crate::game) mod riptide_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    pub(in crate::game) mod client_explosion_tests {
        include!("client_explosion_tests.rs");
    }

    use super::super::api_values::TryIntoApi;

    pub(in crate::game) mod block_entity_state_tests {
        include!("block_entity_state_tests.rs");
    }
    pub(in crate::game) mod chat_decoration_tests {
        include!("chat_decoration_tests.rs");
    }
    pub(in crate::game) mod chat_event_tests {
        include!("chat_event_tests.rs");
    }
    pub(in crate::game) mod chat_window_and_input_tests {
        include!("chat_window_and_input_tests.rs");
    }
    pub(in crate::game) mod play_bundle_tests {
        include!("play_bundle_tests.rs");
    }
    pub(in crate::game) mod chunk_storage_tests {
        include!("chunk_storage_tests.rs");
    }
    pub(in crate::game) mod click_ending_tests {
        include!("click_ending_tests.rs");
    }
    pub(in crate::game) mod client_attributes_tests {
        include!("client_attributes_tests.rs");
    }
    pub(in crate::game) mod client_block_changed_tests {
        include!("client_block_changed_tests.rs");
    }
    pub(in crate::game) mod camera_draw_tests {
        include!("camera_draw_tests.rs");
    }
    pub(in crate::game) mod camera_parity_red_tests {
        include!("camera_parity_red_tests.rs");
    }
    pub(in crate::game) mod set_camera_kinds_tests {
        include!("set_camera_kinds_tests.rs");
    }
    pub(in crate::game) mod camera_death_tests {
        include!("camera_death_tests.rs");
    }
    pub(in crate::game) mod client_hud_camera_tests {
        include!("client_hud_camera_tests.rs");
    }
    pub(in crate::game) mod client_day_time_tests {
        include!("client_day_time_tests.rs");
    }
    pub(in crate::game) mod client_epochs_tests {
        include!("client_epochs_tests.rs");
    }
    pub(in crate::game) mod client_handles_tests {
        include!("client_handles_tests.rs");
    }
    pub(in crate::game) mod client_level_environment_tests {
        include!("client_level_environment_tests.rs");
    }
    pub(in crate::game) mod client_level_lifecycle_tests {
        include!("client_level_lifecycle_tests.rs");
    }
    pub(in crate::game) mod client_menu_tests {
        include!("client_menu_tests.rs");
    }
    pub(in crate::game) mod client_player_reads_tests {
        include!("client_player_reads_tests.rs");
    }
    pub(in crate::game) mod client_vanilla_reads_tests {
        include!("client_vanilla_reads_tests.rs");
    }
    pub(in crate::game) mod client_read_tests {
        include!("client_read_tests.rs");
    }
    pub(in crate::game) mod container_drop_tests {
        include!("container_drop_tests.rs");
    }
    pub(in crate::game) mod creative_sync_tests {
        include!("creative_sync_tests.rs");
    }
    pub(in crate::game) mod damage_sound_tests {
        include!("damage_sound_tests.rs");
    }
    pub(in crate::game) mod dig_abort_tests {
        include!("dig_abort_tests.rs");
    }
    pub(in crate::game) mod dimension_transition_tests {
        include!("dimension_transition_tests.rs");
    }
    pub(in crate::game) mod game_event_tests {
        include!("game_event_tests.rs");
    }
    pub(in crate::game) mod hand_operated_tests {
        include!("hand_operated_tests.rs");
    }
    pub(in crate::game) mod hand_swap_tests {
        include!("hand_swap_tests.rs");
    }
    pub(in crate::game) mod hanging_entity_age_tests {
        include!("hanging_entity_age_tests.rs");
    }
    pub(in crate::game) mod main_hand_tests {
        include!("main_hand_tests.rs");
    }
    pub(in crate::game) mod menu_click_order_tests {
        include!("menu_click_order_tests.rs");
    }
    pub(in crate::game) mod menu_generation_tests {
        include!("menu_generation_tests.rs");
    }
    pub(in crate::game) mod menu_packets_tests {
        include!("menu_packets_tests.rs");
    }
    pub(in crate::game) mod client_close_tests {
        include!("client_close_tests.rs");
    }
    pub(in crate::game) mod menu_title_tests {
        include!("menu_title_tests.rs");
    }
    pub(in crate::game) mod movement_snap_tests {
        include!("movement_snap_tests.rs");
    }
    pub(in crate::game) mod object_interactions_tests {
        include!("object_interactions_tests.rs");
    }
    pub(in crate::game) mod particle_group_tests {
        include!("particle_group_tests.rs");
    }
    pub(in crate::game) mod placement_preview_tests {
        include!("placement_preview_tests.rs");
    }
    pub(in crate::game) mod player_animation_tests {
        include!("player_animation_tests.rs");
    }
    pub(in crate::game) mod player_sleep_tests {
        include!("player_sleep_tests.rs");
    }
    pub(in crate::game) mod predicted_block_notice_tests {
        include!("predicted_block_notice_tests.rs");
    }
    pub(in crate::game) mod predicted_throw_tests {
        include!("predicted_throw_tests.rs");
    }
    pub(in crate::game) mod remote_columns_tests {
        include!("remote_columns_tests.rs");
    }
    pub(in crate::game) mod server_fire_flag_tests {
        include!("server_fire_flag_tests.rs");
    }
    pub(in crate::game) mod slot_mapping_tests {
        include!("slot_mapping_tests.rs");
    }
    pub(in crate::game) mod sound_entity_tests {
        include!("sound_entity_tests.rs");
    }

    #[test]
    #[ignore = "daily"]
    fn named_remote_players_are_told_one_by_one() {
        use mod_api::{
            EntityTickListenedEvent, EntityTickPostEvent, EntityTickPreEvent, Hooks, HostSide,
            ModEntry, Mods, Registrar,
        };
        use std::sync::{Arc, Mutex};
        type Calls = Arc<Mutex<Vec<(bool, Vec<u32>)>>>;
        struct Named(Calls);
        #[mod_api::__subscriptions]
        impl Hooks for Named {
            fn entity_tick_listened(&self, _: HostSide<'_>, event: &mut EntityTickListenedEvent) {
                event.listened = true;
            }
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.entity_tick_each("minecraft:player");
            }
            fn entity_tick_pre(&self, _: HostSide<'_>, event: &mut EntityTickPreEvent<'_>) {
                self.0.lock().unwrap().push((true, event.entities.to_vec()));
            }
            fn entity_tick_post(&self, _: HostSide<'_>, event: &mut EntityTickPostEvent<'_>) {
                self.0
                    .lock()
                    .unwrap()
                    .push((false, event.entities.to_vec()));
            }
        }
        let heard: Calls = Arc::default();
        let mods: Mods = vec![ModEntry::new(
            "remote-player-ticks",
            Box::new(Named(heard.clone())),
        )]
        .into();
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_client_game(|game| {
                let (client, _server) = sunrise_protocol::connection::Pipe::pair();
                game.connect_local(client, "avatar-tick-client").unwrap();
                let remote = game.side.remote.as_mut().unwrap();
                let kind = remote.player_type.as_ref().unwrap().clone();
                assert!(
                    !kind.flags.tick_alone,
                    "render model does not select tick routing"
                );
                for id in [66, 67] {
                    remote.level_players.insert(id, id as u128);
                    game.mobs.push(sunrise_sim::mobs::Mob::new(
                        id as u32,
                        kind.clone(),
                        sunrise_core::geometry::Vec3::ZERO,
                        0.0,
                    ));
                }
                game.tick_remote_entities(false);
            });
        });
        assert_eq!(
            *heard.lock().unwrap(),
            [
                (true, vec![66]),
                (false, vec![66]),
                (true, vec![67]),
                (false, vec![67])
            ]
        );
    }

    pub(in crate::game) mod spear_jab_tests {
        include!("spear_jab_tests.rs");
    }
    pub(in crate::game) mod client_admission_tests {
        include!("client_admission_tests.rs");
    }
    pub(in crate::game) mod tab_publication_tests {
        include!("tab_publication_tests.rs");
    }
    pub(in crate::game) mod thrown_item_swing_tests {
        include!("thrown_item_swing_tests.rs");
    }
    pub(in crate::game) mod client_emitters_tests {
        include!("client_emitters_tests.rs");
    }
    pub(in crate::game) mod tracking_state_tests {
        include!("tracking_state_tests.rs");
    }
    pub(in crate::game) mod use_flag_tests {
        include!("use_flag_tests.rs");
    }

    /// A client connecting through a local pipe and through configuration, as
    /// the client menu tests drive it: the server end's reader (what the
    /// client sends) and writer.
    pub(crate) fn connect_menu_test(
        game: &mut crate::game::ClientGame<'_>,
    ) -> (
        sunrise_protocol::connection::ConnReader,
        sunrise_protocol::connection::ConnWriter,
    ) {
        connect_play_test(game, |_, _| {})
    }

    pub(in crate::game) fn connect_play_test(
        game: &mut crate::game::ClientGame<'_>,
        configure: impl FnOnce(
            &mut sunrise_protocol::connection::ConnReader,
            &mut sunrise_protocol::connection::ConnWriter,
        ),
    ) -> (
        sunrise_protocol::connection::ConnReader,
        sunrise_protocol::connection::ConnWriter,
    ) {
        use sunrise_protocol::connection::{Connection, Pipe};
        game.options.execution_role = super::super::execution_role::ExecutionRole::Client;
        let (client, server) = Pipe::pair();
        game.connect_local(client, "MenuReadTest").unwrap();
        finish_play_test(game, Connection::local(server), configure)
    }

    pub(in crate::game) fn finish_play_test(
        game: &mut crate::game::ClientGame<'_>,
        server: sunrise_protocol::connection::Connection,
        configure: impl FnOnce(
            &mut sunrise_protocol::connection::ConnReader,
            &mut sunrise_protocol::connection::ConnWriter,
        ),
    ) -> (
        sunrise_protocol::connection::ConnReader,
        sunrise_protocol::connection::ConnWriter,
    ) {
        use sunrise_protocol::packets::{Empty, configuration as cfg, login};
        let (mut reader, mut writer) = server.split();
        reader
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        reader.recv().unwrap();
        reader.recv().unwrap();
        writer
            .send(
                &login::Clientbound::LoginFinished(login::LoginFinished {
                    profile: login::GameProfile {
                        uuid: 42,
                        name: "MenuReadTest".into(),
                        properties: vec![],
                    },
                    session: 1,
                })
                .encode(),
            )
            .unwrap();
        let (id, body) = reader.recv().unwrap();
        assert!(matches!(
            login::Serverbound::decode(id, &body).unwrap(),
            login::Serverbound::LoginAcknowledged(_)
        ));
        reader.recv().unwrap();
        reader.recv().unwrap();
        configure(&mut reader, &mut writer);
        writer
            .send(&cfg::Clientbound::FinishConfiguration(Empty).encode())
            .unwrap();
        game.acknowledge_test_configuration();
        let (id, body) = reader.recv().unwrap();
        assert!(matches!(
            cfg::Serverbound::decode(id, &body).unwrap(),
            cfg::Serverbound::FinishConfiguration(_)
        ));
        (reader, writer)
    }

    /// The test server offers its payload manifest and reads the client's offer.
    pub(in crate::game) fn offer_test_payloads(
        reader: &mut sunrise_protocol::connection::ConnReader,
        writer: &mut sunrise_protocol::connection::ConnWriter,
        manifest: &sunrise_protocol::payload_negotiation::Manifest,
    ) {
        use sunrise_protocol::{
            packets::configuration as cfg,
            payload_negotiation::{CHANNEL, Control},
        };
        writer
            .send(
                &cfg::Clientbound::CustomPayload(cfg::CustomPayload {
                    channel: CHANNEL.into(),
                    data: Control::Offer(manifest.clone()).encode().unwrap(),
                })
                .encode(),
            )
            .unwrap();
        let (id, body) = reader.recv().unwrap();
        assert!(
            matches!(cfg::Serverbound::decode(id, &body).unwrap(), cfg::Serverbound::CustomPayload(p) if p.channel == CHANNEL)
        );
    }

    /// Spawns the connected client in the overworld as entity 7, profile 42.
    pub(crate) fn spawn_menu_test(
        game: &mut crate::game::ClientGame<'_>,
    ) -> sunrise_protocol::packets::play::SpawnInfo {
        use crate::game::client_side::remote_client::RemoteDimensionEntry;
        use sunrise_client::level_loading::Reason;
        use sunrise_protocol::packets::play::SpawnInfo;
        let spawn = SpawnInfo {
            dimension_type: 0,
            dimension: "minecraft:overworld".into(),
            seed: 0,
            game_type: 0,
            previous_game_type: None,
            debug: false,
            flat: true,
            last_death: None,
            portal_cooldown: 0,
            sea_level: 63,
        };
        game.enter_remote_dimension(&spawn, RemoteDimensionEntry::Login, Reason::Other);
        let remote = game.side.remote.as_mut().unwrap();
        remote.logged_in = true;
        remote.entity_id = 7;
        remote.profile = 42;
        spawn
    }

    use super::{
        boat_interact_passes, chat_after_event, item_frame_interact_passes, remote_object_swings,
    };

    #[test]
    #[ignore = "daily"]
    fn block_alias_survival_consumption_and_client_prediction() {
        use crate::game::{Options, Progress, Settings, World, load_resources};
        use std::{path::PathBuf, sync::Arc};
        use sunrise_items::items::ItemStack;
        use sunrise_level::chunk::Chunk;
        use sunrise_mod_package::content::{Block, Content, Item};
        use sunrise_sim::mod_content::{ContentMod, build};
        let base = PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
        let root = std::env::temp_dir().join(format!("sunrise-alias-game-{}", std::process::id()));
        let mut content = Content::declared("ruby");
        content
            .declare(
                &[Block {
                    id: "ruby:frame".into(),
                    template: "minecraft:stone".into(),
                    item: false,
                    ..Default::default()
                }],
                &["empty", "filled"].map(|name| Item {
                    id: format!("ruby:{name}"),
                    template: "minecraft:stick".into(),
                    block: Some("ruby:frame".into()),
                    ..Default::default()
                }),
            )
            .unwrap();
        let mods = [ContentMod {
            id: "ruby".into(),
            content,
            jars: Vec::new(),
        }];
        let inventory = sunrise_mod_package::enabled::EnabledMods::new([(
            "ruby".to_owned(),
            mod_api::Side::Both,
        )]);
        build(&base, std::path::Path::new(""), &mods, &root, &inventory).unwrap();
        let (registry, _) = sunrise_world::level::data::load_registry_with_flags(&root).unwrap();
        crate::game::api_values::register_game_keys((0..registry.len()).map(|id| {
            registry
                .state(sunrise_world::registry::BlockStateId(id as u32))
                .unwrap()
                .name
                .clone()
        }));
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
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
            crate::game::profile_resolver,
        )
        .unwrap();
        game.world = World::new(game.res.facts().air, 1);
        for x in -1..=1 {
            for z in -1..=1 {
                game.world
                    .chunks
                    .insert((x, z), Arc::new(Chunk::empty(x, z)));
            }
        }
        game.sync_tick_chunks();
        game.set_mode_id(0);
        game.player = None;
        game.selected = 0;
        let target = game
            .pack
            .assets
            .registry
            .default_state("ruby:frame")
            .unwrap();
        for (i, name) in ["ruby:empty", "ruby:filled"].iter().enumerate() {
            let pos = [i as i32 * 2, 70, 0];
            let hit = crate::game::Hit {
                pos,
                face: [0, 1, 0],
                inside: false,
                point: [f64::from(pos[0]) + 0.5, 70.0, 0.5],
            };
            let stack = ItemStack::new(name, 2);
            assert_eq!(game.items().get(name).unwrap().block, Some(target));
            *game.held_mut() = Some(stack);
            assert!(game.use_hand(Some(&hit)).unwrap());
            assert_eq!(game.world.block(pos[0], pos[1], pos[2]), target);
            let held = game.held().unwrap();
            assert_eq!((&*held.item, held.count), (*name, 1));
        }
        let blocked = crate::game::Hit {
            pos: [0, 70, 0],
            face: [0, 1, 0],
            inside: false,
            point: [0.5, 71.0, 0.5],
        };
        game.set_mode_id(2);
        let before = game.held().cloned();
        game.use_hand(Some(&blocked)).unwrap();
        assert_eq!(
            game.held(),
            before.as_ref(),
            "adventure refusal keeps the alias stack"
        );
        drop(game);

        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::Client,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = crate::game::execution_role::ExecutionRole::Client;
        let mut client = crate::game::ClientGame::new(
            &loaded.resources,
            loaded.client_res.unwrap(),
            loaded.pack,
            loaded.client_pack.unwrap(),
            loaded.gui.unwrap(),
            options,
            settings,
            crate::game::Audio::silent(
                &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
            ),
            None,
            crate::game::server_side::integrated::serve,
            crate::game::server_side::integrated::run_component_harness_on_server,
            crate::game::session_verifier(),
            crate::game::profile_resolver,
        )
        .unwrap();
        let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
        client.connect_local(pipe, "AliasTest").unwrap();
        client.world = World::new(client.res.facts().air, 1);
        for x in -1..=1 {
            for z in -1..=1 {
                client
                    .world
                    .chunks
                    .insert((x, z), Arc::new(Chunk::empty(x, z)));
            }
        }
        client.set_local_mode(sunrise_command::args::GameMode::Survival);
        client.player = None;
        client.selected = 0;
        let target = client
            .pack
            .assets
            .registry
            .default_state("ruby:frame")
            .unwrap();
        for (i, name) in ["ruby:empty", "ruby:filled"].iter().enumerate() {
            let pos = [i as i32 * 2, 70, 0];
            let hit = crate::game::Hit {
                pos,
                face: [0, 1, 0],
                inside: false,
                point: [f64::from(pos[0]) + 0.5, 70.0, 0.5],
            };
            let stack = ItemStack::new(name, 2);
            assert_eq!(client.items().get(name).unwrap().block, Some(target));
            let preview = client
                .remote_can_place(&hit, &stack)
                .expect("client predicts the alias");
            assert_eq!([preview.pos.x, preview.pos.y, preview.pos.z], pos);
            assert_eq!(
                client.world.block(pos[0], pos[1], pos[2]),
                client.res.facts().air,
                "prediction is dry"
            );
        }
        drop(client);
        std::fs::remove_dir_all(root).unwrap();
    }

    use sunrise_core::nbt::Tag;
    use sunrise_items::items::ItemStack;
    use sunrise_level::containers::{Beacon, BlockEntity};

    /// #12040: a server's client moves at the speed the server's attributes give it, never its
    /// effects' own. Vanilla's server removes a Speed that runs out in ServerPlayer.doTick, after
    /// the tick's sendDirtyEntityData: ClientboundRemoveMobEffectPacket goes out at once and the
    /// movement speed without the effect's modifier a tick later, so for that tick the client still
    /// runs fast (a potion drunk starts the same way). A client a tick off is what GrimAC's
    /// Simulation flagged.
    #[test]
    #[ignore = "daily"]
    fn a_servers_client_takes_its_speed_from_the_attributes_not_the_effect() {
        use crate::game::{Audio, Options, Progress, Settings, load_resources};
        use sunrise_core::geometry::Vec3;
        use sunrise_protocol::packets::play::{
            AttributeModifier, AttributeSnapshot, Clientbound as C, RemoveMobEffect,
            UpdateAttributes, UpdateMobEffect,
        };
        use sunrise_sim::motion::PlayerMotion;
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::Client,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        let mut game = crate::game::ClientGame::new(
            &loaded.resources,
            loaded.client_res.unwrap(),
            loaded.pack,
            loaded.client_pack.unwrap(),
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
            crate::game::server_side::integrated::serve,
            crate::game::server_side::integrated::run_component_harness_on_server,
            crate::game::session_verifier(),
            crate::game::profile_resolver,
        )
        .unwrap();
        game.init_remote_for_replay().unwrap();
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
        let me = game.side.remote.as_ref().unwrap().entity_id;
        let speed = game.remote_effect_network_id("speed").unwrap();
        let attribute = {
            let p = game.player.as_ref().unwrap();
            let key = p.attributes.key("minecraft:movement_speed").unwrap();
            key.attribute().registry_id()
        };
        // Player.createAttributes: 0.1F.
        let walking = f64::from(0.1f32);
        // Speed II: 0.2 (a float) a level, multiplying the total.
        let amount = f64::from(0.2f32) * 2.0;
        let attributes = |sped: bool| {
            C::UpdateAttributes(UpdateAttributes {
                id: me,
                values: vec![AttributeSnapshot {
                    attribute,
                    base: walking,
                    modifiers: sped
                        .then(|| AttributeModifier {
                            id: "minecraft:effect.speed".into(),
                            amount,
                            operation: 2,
                        })
                        .into_iter()
                        .collect(),
                }],
            })
        };
        // Each step: the packets of one server tick, then a client tick.
        let mut step = |packets: Vec<C>| {
            for packet in packets {
                game.handle_packet(packet).unwrap();
            }
            game.tick_client_effects();
            game.player
                .as_mut()
                .unwrap()
                .attribute("minecraft:movement_speed")
                .unwrap()
        };
        let effect = C::UpdateMobEffect(UpdateMobEffect {
            id: me,
            effect: speed,
            amplifier: 1,
            duration: 60,
            flags: 2 | 4,
        });
        let fast = walking * (1.0 + amount);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let seen = [
            step(vec![effect]),
            step(vec![attributes(true)]),
            step(Vec::new()),
            step(vec![C::RemoveMobEffect(RemoveMobEffect {
                id: me,
                effect: speed,
            })]),
            step(vec![attributes(false)]),
        ];
        let expected = [walking, fast, fast, fast, walking];
        assert!(
            seen.iter().zip(expected).all(|(&s, e)| close(s, e)),
            "speed on each tick: {seen:?}, vanilla's client: {expected:?}"
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod boat_push_order_tests;
#[cfg(test)]
pub(in crate::game) mod item_use_tick_tests;
#[cfg(test)]
pub(in crate::game) mod movement_input_tests;
#[cfg(test)]
pub(in crate::game) mod swim_start_tests;
/// `Entity.baseTick`'s fluid update for a mob this window only displays (`fluids`, sampled before
/// the tick), with the splash it draws, and the block it stands on.
pub(in crate::game) fn client_fluid_step(
    m: &mut sunrise_sim::mobs::Mob,
    fluids: Option<sunrise_sim::mobs::living::Fluids>,
    standing: Option<String>,
    own: Option<i32>,
    fx: &mut Vec<sunrise_sim::mobs::MobEvent>,
) {
    if let Some(fluids) = fluids {
        let was = m.fluids.in_water;
        let first = m.first_tick;
        // Entity.baseTick saves the preceding eye sample before the new fluid update.
        m.was_eye_in_water = m.fluids.eye_in_water;
        m.fluids = fluids;
        // Entity.doWaterSplashEffect on the client copy. Particles
        // only: ClientLevel draws addParticle, and playSeededSound
        // drops Entity.playSound (the server broadcasts that).
        if m.fluids.in_water && !was && !first && Some(m.id as i32) != own {
            sunrise_sim::mobs::living::client_water_splash(m, fx);
        }
        m.first_tick = false;
    }
    if let Some(name) = standing {
        m.standing_on = Some(name);
    }
}

#[cfg(test)]
pub(in crate::game) mod frozen_screen_effects_tests;
#[cfg(test)]
pub(in crate::game) mod placement_prediction_tests;
#[cfg(test)]
#[path = "player_tick_tests.rs"]
pub(in crate::game) mod player_tick_tests;
#[cfg(test)]
pub(in crate::game) mod rider_tick_tests;
#[cfg(test)]
pub(in crate::game) mod smoke_source_tests;

#[allow(unused_imports)]
pub(in crate::game) use inventory::{
    inventory_screen_slot, inventory_screen_slot_with_type, remote_station_kind,
};
