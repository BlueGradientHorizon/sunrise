use super::{hasher, random_words, relative, vec3};
use crate::game::ServerGame;
use std::{
    collections::BTreeSet,
    fmt::Write as _,
    hash::{DefaultHasher, Hash, Hasher},
    mem::discriminant,
};
use sunrise_core::{
    geometry::Vec3,
    nbt::{Tag, numeric_int},
};
use sunrise_gameplay_common::game::{
    FallingBlock, boats_common::Boat, commands_common::Target, dragon_common::dragon_part_id,
    entity::Entity, fireworks::Rocket, fishing_common::State, general::GeneralRow,
    minecarts::Minecart, tnt_common::PrimedTnt,
};
use sunrise_items::{
    equipment::Equipment,
    item_nbt::item_to_nbt,
    items::{ItemStack, Items},
};
use sunrise_level::effects::{EffectInstance, PlayerEffects};
use sunrise_sim::{
    entities::{ArmorStand, ExperienceOrb, ItemEntity},
    join_seq::{JoinSeq, Joined},
    mobs::{Mob, leash::Holder},
    motion::PlayerMotion,
    projectiles::{Projectile, ProjectileKind},
    text_display_data::resident::{DisplayContents, DisplayState},
    thrown_trident::ThrownTrident,
};

#[derive(Default)]
pub(super) struct EntityGroups {
    pub(super) identity: DefaultHasher,
    pub(super) shared: DefaultHasher,
}

fn tag(h: &mut impl Hasher, value: &Tag) {
    match value {
        Tag::Compound(fields) => {
            value.id().hash(h);
            let mut entries: Vec<_> = fields.iter().collect();
            entries.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
            entries.len().hash(h);
            for (key, value) in entries {
                key.hash(h);
                tag(h, value);
            }
        }
        Tag::List(values) => {
            value.id().hash(h);
            values.len().hash(h);
            for value in values {
                tag(h, value);
            }
        }
        value => value.hash(h),
    }
}

fn optional_tag(h: &mut impl Hasher, value: Option<&Tag>) {
    value.is_some().hash(h);
    if let Some(value) = value {
        tag(h, value);
    }
}

fn stack(h: &mut impl Hasher, value: Option<&ItemStack>, items: &Items) {
    value.is_some().hash(h);
    if let Some(value) = value {
        tag(h, &item_to_nbt(value, items));
    }
}

fn vec_bits<const N: usize>(v: [f64; N]) -> [u64; N] {
    v.map(f64::to_bits)
}

fn rotation(v: [f32; 2]) -> [u32; 2] {
    v.map(f32::to_bits)
}

#[derive(Default)]
pub(super) struct Shared<'a> {
    uuid: Option<[i32; 4]>,
    fire: Option<i32>,
    air: Option<i32>,
    portal: Option<i32>,
    ground: Option<bool>,
    collisions: [Option<bool>; 4],
    name: Option<&'a Tag>,
    text: Option<&'a sunrise_game_data::text::Text>,
    literal_name: Option<&'a str>,
}

pub(super) trait SharedParts: Entity {
    fn target(&self) -> Target;
    fn shared(&self, _game: &ServerGame<'_>) -> Shared<'_> {
        Shared::default()
    }
}

macro_rules! shared_parts {
    ($ty:ty, $kind:ident $(, |$e:ident, $game:ident| $shared:expr)?) => {
        impl SharedParts for $ty {
            fn target(&self) -> Target {
                Target::$kind(self.id)
            }
            $(
                fn shared(&self, $game: &ServerGame<'_>) -> Shared<'_> {
                    let $e = self;
                    $shared
                }
            )?
        }
    };
}

shared_parts!(Mob, Mob, |e, _game| Shared {
    fire: Some(e.fire_ticks),
    air: Some(e.air),
    collisions: [None, None, Some(e.vertical_collision_below), None],
    text: e.custom_name_text.as_ref(),
    literal_name: e.custom_name.as_deref(),
    ..Shared::default()
});
shared_parts!(ItemEntity, Item, |e, _game| Shared {
    fire: Some(e.fire_ticks),
    collisions: [None, Some(e.vertical_collision), None, None],
    ..Shared::default()
});
shared_parts!(Boat, Boat, |e, _game| Shared {
    fire: Some(e.fire_ticks),
    ..Shared::default()
});
shared_parts!(Minecart, Cart);
shared_parts!(ArmorStand, Stand, |e, _game| Shared {
    fire: Some(e.fire_ticks),
    air: Some(e.air),
    text: e.custom_name.as_ref(),
    ..Shared::default()
});
shared_parts!(PrimedTnt, Tnt);
shared_parts!(FallingBlock, Falling, |e, _game| Shared {
    uuid: e.saved.as_ref().map(|s| s.0),
    fire: Some(e.fire_ticks),
    portal: Some(e.portal.cooldown),
    ..Shared::default()
});
shared_parts!(ExperienceOrb, Orb, |e, _game| Shared {
    fire: Some(e.fire_ticks),
    air: Some(e.air),
    ..Shared::default()
});
shared_parts!(Projectile, Projectile, |e, game| Shared {
    uuid: e.uuid,
    fire: Some(e.arrow.as_ref().map_or(e.fire_ticks, |a| a.fire_ticks)),
    air: Some(e.air_supply),
    portal: matches!(e.kind, ProjectileKind::EnderPearl).then(|| game.pearl_portal_cooldown(e.id)),
    ..Shared::default()
});
shared_parts!(Rocket, Rocket);
shared_parts!(ThrownTrident, Trident, |e, _game| Shared {
    fire: Some(e.fire_ticks),
    ..Shared::default()
});
shared_parts!(GeneralRow, General, |e, _game| Shared {
    uuid: Some(e.uuid),
    fire: Some(e.base.fire_ticks),
    ground: Some(e.on_ground),
    collisions: [
        Some(e.horizontal_collision),
        Some(e.vertical_collision),
        Some(e.vertical_collision_below),
        None,
    ],
    text: e.custom_name.as_ref(),
    ..Shared::default()
});

pub(super) fn typed_shared(game: &ServerGame<'_>, groups: &mut EntityGroups, e: &impl SharedParts) {
    let mut live = e.shared(game);
    live.ground = live
        .ground
        .or_else(|| e.ground_state().map(|g| g.on_ground));
    entity_shared(game, groups, e.id(), Some(e.target()), live);
}

fn entity_shared(
    game: &ServerGame<'_>,
    groups: &mut EntityGroups,
    id: u32,
    target: Option<Target>,
    live: Shared<'_>,
) {
    let uuid = live
        .uuid
        .or_else(|| game.common.persist.uuid_of(id))
        .or_else(|| {
            let Tag::IntArray(words) = game.saved_entity_compound(target?)?.get("UUID")? else {
                return None;
            };
            words.as_slice().try_into().ok()
        });
    (id, uuid).hash(&mut groups.identity);
    entity_values(game, &mut groups.shared, id, target, live);
}

fn entity_values(
    game: &ServerGame<'_>,
    h: &mut DefaultHasher,
    id: u32,
    target: Option<Target>,
    live: Shared<'_>,
) {
    let raw = target.and_then(|t| game.saved_entity_compound(t));
    let number = |key| raw.and_then(|tag| tag.get(key)).and_then(numeric_int);
    id.hash(h);
    live.fire.or_else(|| number("Fire")).hash(h);
    live.air.or_else(|| number("Air")).hash(h);
    live.portal
        .or_else(|| game.server.portals.entities.get(&id).map(|p| p.cooldown))
        .or_else(|| number("PortalCooldown"))
        .unwrap_or(0)
        .hash(h);
    live.ground
        .or_else(|| number("OnGround").map(|n| n != 0))
        .hash(h);
    live.collisions.hash(h);
    if let Some(target) = target {
        let tags: Vec<_> = game.saved_entity_tags(target).iter_utf16().collect();
        tags.hash(h);
    } else {
        0usize.hash(h);
    }
    let text_tag = live
        .text
        .map(|text| sunrise_game_data::scoreboard::json_to_tag(&text.component_view()));
    let literal_tag = live.literal_name.map(|name| Tag::String(name.into()));
    optional_tag(
        h,
        live.name
            .or(text_tag.as_ref())
            .or(literal_tag.as_ref())
            .or_else(|| raw.and_then(|tag| tag.get("CustomName"))),
    );
}

#[derive(Default)]
struct Group {
    hash: DefaultHasher,
    count: usize,
}

impl Group {
    fn row(&mut self, id: u32, join: JoinSeq) -> &mut DefaultHasher {
        self.count += 1;
        (id, join.is_joined().then(|| relative(join.get()))).hash(&mut self.hash);
        &mut self.hash
    }

    fn append(self, out: &mut String, name: &str) {
        write!(out, "{name}={}:{:016x} ", self.count, self.hash.finish()).unwrap();
    }
}

pub(super) fn level(game: &ServerGame<'_>, shared: &mut EntityGroups) -> String {
    use sunrise_gameplay_common::game::tables::GameItems as _;
    let mut out = String::new();
    let items = game.common.res.items();
    hangings(game, shared, items).append(&mut out, "hangings");
    decorations(game, shared, items, &mut out);
    transient(game, shared, items, &mut out);
    end_entities(game, shared, &mut out);
    seats(game).append(&mut out, "seats");
    players(game, items).append(&mut out, "players");
    out.pop();
    out
}

fn hangings(game: &ServerGame<'_>, shared: &mut EntityGroups, items: &Items) -> Group {
    let mut group = Group::default();
    for e in &game.common.paintings {
        let h = group.row(e.id, e.join);
        "painting".hash(h);
        (
            e.pos,
            e.facing,
            &e.variant,
            &e.asset,
            e.width,
            e.height,
            e.check_interval,
            e.ticks,
        )
            .hash(h);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Painting(e.id)),
            Shared::default(),
        );
    }
    for e in &game.common.item_frames {
        let h = group.row(e.id, e.join);
        "item_frame".hash(h);
        (
            e.pos,
            e.facing,
            e.glow,
            e.rotation,
            e.drop_chance.to_bits(),
            e.fixed,
            e.invisible,
            e.check_interval,
            e.ticks,
            random_words(&e.random),
        )
            .hash(h);
        stack(h, e.item.as_ref(), items);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Frame(e.id)),
            Shared {
                name: e.custom_name.as_ref(),
                ..Shared::default()
            },
        );
    }
    group
}

fn decorations(game: &ServerGame<'_>, shared: &mut EntityGroups, items: &Items, out: &mut String) {
    let mut cushions = Group::default();
    for e in &game.common.cushions {
        let h = cushions.row(e.id, e.join);
        vec3(h, e.position);
        (e.pos, e.yaw.to_bits(), e.color, e.check_interval).hash(h);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Cushion(e.id)),
            Shared {
                name: e.custom_name.as_ref(),
                ..Shared::default()
            },
        );
    }
    cushions.append(out, "cushions");

    let mut others = Group::default();
    for e in game.server.others.0.iter() {
        let h = others.row(e.id, e.join);
        e.kind.hash(h);
        vec3(h, e.position);
        (rotation([e.yaw, e.pitch]), e.knot_check).hash(h);
        if let Some(Tag::Compound(raw)) = game.common.persist.raw.get(&e.id) {
            let mut payload: Vec<_> = raw
                .iter()
                .filter(|(key, _)| {
                    ![
                        "UUID",
                        "Fire",
                        "Air",
                        "OnGround",
                        "PortalCooldown",
                        "Tags",
                        "CustomName",
                    ]
                    .iter()
                    .any(|name| key.0.iter().copied().eq(name.encode_utf16()))
                })
                .collect();
            payload.sort_unstable_by_key(|&(key, _)| key);
            payload.len().hash(h);
            for (key, value) in payload {
                key.hash(h);
                tag(h, value);
            }
        } else {
            0usize.hash(h);
        }
        e.display.is_some().hash(h);
        if let Some(display) = e.display.as_deref() {
            hash_display(h, display, items);
        }
        e.interaction
            .map(|i| {
                (
                    i.width.to_bits(),
                    i.height.to_bits(),
                    i.response,
                    i.attack.map(|a| (a.player, a.timestamp)),
                    i.interaction.map(|a| (a.player, a.timestamp)),
                )
            })
            .hash(h);
        e.eye.is_some().hash(h);
        if let Some(eye) = e.eye {
            vec3(h, eye.motion);
            (eye.life, eye.synced_fire, eye.has_visual_fire).hash(h);
        }
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Other(e.id)),
            Shared {
                fire: e.eye.map(|eye| eye.fire_ticks),
                ..Shared::default()
            },
        );
    }
    others.append(out, "other_entities");

    let mut knots = Group::default();
    for e in &game.common.resident_entities.knots {
        let h = knots.row(e.id, e.join);
        (e.fence, e.invulnerable, e.no_gravity, e.silent, e.glowing).hash(h);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Knot(e.id)),
            Shared {
                fire: Some(e.fire_ticks),
                ..Shared::default()
            },
        );
    }
    let mut fence_ids: Vec<_> = game.server.knot_ids.iter().collect();
    fence_ids.sort_unstable();
    fence_ids.hash(&mut knots.hash);
    for e in game
        .common
        .mobs
        .iter()
        .filter(|e| e.extra.leash.is_some() || e.extra.leash_pending.is_some())
    {
        (e.id, e.extra.leash_player, e.extra.leash_knot_id).hash(&mut knots.hash);
        hash_leash(&mut knots.hash, e.extra.leash, e.extra.leash_pending);
    }
    for e in game
        .common
        .boats
        .iter()
        .filter(|e| e.leash.is_some() || e.leash_pending.is_some())
    {
        (e.id, e.leash_player, e.leash_knot_id).hash(&mut knots.hash);
        hash_leash(&mut knots.hash, e.leash, e.leash_pending);
    }
    knots.append(out, "leash_knots");
}

fn hash_leash(
    h: &mut DefaultHasher,
    holder: Option<Holder>,
    pending: Option<sunrise_sim::mobs::leash::Pending>,
) {
    use sunrise_sim::mobs::leash::Pending;
    discriminant(&holder).hash(h);
    if let Some(holder) = holder {
        discriminant(&holder).hash(h);
        match holder {
            Holder::Player => {}
            Holder::Mob(id) | Holder::Boat(id) => id.hash(h),
            Holder::Knot(at) => at.hash(h),
        }
    }
    discriminant(&pending).hash(h);
    if let Some(pending) = pending {
        discriminant(&pending).hash(h);
        match pending {
            Pending::Uuid(uuid) => uuid.hash(h),
            Pending::Knot(at) => at.hash(h),
        }
    }
}

fn hash_display(h: &mut DefaultHasher, d: &DisplayState, items: &Items) {
    let t = d.transformation;
    for vector in [
        t.translation.as_slice(),
        &t.left_rotation,
        &t.scale,
        &t.right_rotation,
    ] {
        for value in vector {
            value.to_bits().hash(h);
        }
    }
    (
        d.billboard.name(),
        d.brightness,
        [
            d.view_range,
            d.shadow_radius,
            d.shadow_strength,
            d.width,
            d.height,
        ]
        .map(f32::to_bits),
        d.glow_color_override,
        d.interpolation_duration,
        d.start_interpolation,
        d.teleport_duration,
    )
        .hash(h);
    discriminant(&d.contents).hash(h);
    match &d.contents {
        DisplayContents::Block(state) => state.0.hash(h),
        DisplayContents::Item {
            stack: item,
            context,
        } => {
            context.name().hash(h);
            stack(h, item.as_deref(), items);
        }
        DisplayContents::Text(text) => {
            tag(h, &text.component.to_nbt());
            (
                text.line_width,
                text.background,
                text.opacity,
                text.shadow,
                text.see_through,
                text.default_background,
                text.alignment.name(),
            )
                .hash(h);
        }
    }
}

fn transient(game: &ServerGame<'_>, shared: &mut EntityGroups, items: &Items, out: &mut String) {
    let mut fangs = Group::default();
    for e in game.common.fangs.list.iter() {
        let h = fangs.row(e.id, e.join_seq());
        (
            vec_bits(e.pos),
            e.yaw.to_bits(),
            e.warmup,
            e.life,
            e.owner,
            e.started,
            e.synced_fire,
            e.has_visual_fire,
            random_words(&e.random),
        )
            .hash(h);
        vec3(h, e.motion);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Fangs(e.id)),
            Shared {
                fire: Some(e.fire_ticks),
                ..Shared::default()
            },
        );
    }
    fangs.append(out, "fangs");

    let mut lightning = Group::default();
    for e in game.common.bolts.iter() {
        let h = lightning.row(e.id, e.join_seq());
        (
            vec_bits(e.at),
            rotation(e.rotation),
            e.fall_distance.to_bits(),
            e.life,
            e.flashes,
            e.seed,
            e.visual_only,
            e.fires,
            e.cause,
            random_words(&e.random),
        )
            .hash(h);
        vec3(h, e.velocity);
        (
            &e.strike.converted,
            &e.strike.hit,
            &e.strike.hit_items,
            &e.strike.spared,
        )
            .hash(h);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Bolt(e.id)),
            Shared {
                fire: Some(e.fire_ticks),
                portal: Some(e.portal_cooldown),
                air: Some(e.air),
                ground: Some(e.on_ground),
                name: e.custom_name.as_ref(),
                ..Shared::default()
            },
        );
    }
    lightning.append(out, "lightning");

    let mut spawners = Group::default();
    for e in game.common.ominous_spawners.iter() {
        let h = spawners.row(e.id, e.join);
        (
            vec_bits(e.position),
            e.spawn_item_after,
            e.tick_count,
            e.synced_fire,
            e.has_visual_fire,
        )
            .hash(h);
        stack(h, Some(&e.item), items);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Spawner(e.id)),
            Shared {
                uuid: Some(e.uuid),
                fire: Some(e.fire_ticks),
                ..Shared::default()
            },
        );
    }
    spawners.append(out, "ominous_spawners");

    let mut hooks = Group::default();
    for (e, _) in game.fishing_hooks_here() {
        let h = hooks.row(e.id, e.join);
        vec3(h, e.position);
        vec3(h, e.velocity);
        discriminant(&e.state).hash(h);
        if let State::Hooked(id) = e.state {
            id.hash(h);
        }
        (
            e.caster,
            e.hooked_in,
            e.left_owner,
            rotation([e.yaw, e.pitch]),
            e.tick_count,
            e.life,
            e.nibble,
            e.time_until_lured,
            e.time_until_hooked,
            e.fish_angle.to_bits(),
            e.out_of_water,
            e.open_water,
        )
            .hash(h);
        (e.luck, e.lure, e.last_deflected_by, random_words(&e.random)).hash(h);
        e.medium
            .as_ref()
            .map(|medium| {
                (
                    medium.mod_id,
                    &medium.answered,
                    &medium.loot_table,
                    medium.lure_ticks,
                )
            })
            .hash(h);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Hook(e.id)),
            Shared {
                uuid: Some(e.uuid),
                fire: Some(e.base.fire_ticks),
                ground: Some(e.ground.on_ground),
                ..Shared::default()
            },
        );
    }
    hooks.append(out, "fishing_hooks");

    let mut potions = Group::default();
    for e in &game.common.potions.thrown {
        let h = potions.row(e.id, JoinSeq::UNJOINED);
        vec3(h, e.position);
        vec3(h, e.velocity);
        (
            e.age,
            e.thrower,
            e.guest,
            e.left_owner,
            e.last_deflected_by,
            random_words(&e.random),
        )
            .hash(h);
        stack(h, Some(&e.stack), items);
        entity_shared(game, shared, e.id, None, Shared::default());
    }
    potions.append(out, "thrown_potions");
}

fn end_entities(game: &ServerGame<'_>, shared: &mut EntityGroups, out: &mut String) {
    let mut eyes = Group::default();
    for e in &game.common.end.eyes {
        let h = eyes.row(e.id, JoinSeq::UNJOINED);
        (
            vec_bits(e.position.to_array()),
            vec_bits(e.velocity.to_array()),
            vec_bits(e.target.to_array()),
            e.life,
            e.survive,
        )
            .hash(h);
        entity_shared(game, shared, e.id, None, Shared::default());
    }
    eyes.append(out, "thrown_eyes");

    let mut crystals = Group::default();
    for e in &game.common.crystals {
        let h = crystals.row(e.id, e.join);
        vec3(h, e.position);
        (
            rotation(e.rotation),
            e.show_bottom,
            e.time,
            e.beam,
            e.invulnerable,
            e.fall.1.to_bits(),
        )
            .hash(h);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Crystal(e.id)),
            Shared {
                fire: Some(e.fire),
                ground: Some(e.fall.0),
                ..Shared::default()
            },
        );
    }
    crystals.append(out, "end_crystals");

    let mut dragon = Group::default();
    for e in game.common.dragon.iter() {
        let h = dragon.row(e.id, e.join);
        vec3(h, e.position);
        vec3(h, e.velocity);
        (
            rotation([e.yaw, e.pitch]),
            e.body_yaw.to_bits(),
            e.head_yaw.to_bits(),
            e.yaw_accel.to_bits(),
            e.health.to_bits(),
            e.hurt_time,
            e.death_time,
            e.death_canceled,
            e.in_wall,
            e.ticks,
            e.no_ai,
            random_words(&e.random),
        )
            .hash(h);
        (
            e.flame_count,
            e.flame_ended,
            e.sitting_damage.to_bits(),
            e.invulnerable,
            e.last_hurt.to_bits(),
            e.target,
            e.aggressive,
            e.left_handed,
            e.persistent,
        )
            .hash(h);
        hash_phase(h, &e.phase);
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Dragon(e.id)),
            Shared::default(),
        );
        for (i, part) in e.parts.iter().enumerate() {
            let id = dragon_part_id(e.id, i);
            let h = dragon.row(id, JoinSeq::UNJOINED);
            vec3(h, part.0);
            vec3(h, part.1);
            entity_shared(
                game,
                shared,
                id,
                Some(Target::DragonPart(id)),
                Shared {
                    uuid: Some(part.3),
                    ..Shared::default()
                },
            );
        }
    }
    dragon.append(out, "dragon");

    let mut clouds = Group::default();
    for e in game.common.breath_clouds.iter() {
        let h = clouds.row(e.id, e.join_seq());
        (
            vec_bits(e.at),
            vec_bits(e.motion),
            e.fall_distance.to_bits(),
            e.radius.to_bits(),
            e.first_tick,
            e.age,
            e.duration,
            e.from_dragon,
        )
            .hash(h);
        hash_victims(h, &e.victims);
        e.potion.is_some().hash(h);
        if let Some(potion) = &e.potion {
            let s = &potion.settings;
            (
                s.wait_time,
                s.reapplication_delay,
                s.radius_on_use.to_bits(),
                s.duration_on_use,
                s.duration_scale.to_bits(),
                potion.radius_per_tick.to_bits(),
                potion.color,
                potion.owner,
                potion.guest,
            )
                .hash(h);
            (&potion.effects, &potion.hidden, &potion.ambient).hash(h);
            hash_victims(h, &potion.victims);
            optional_tag(
                h,
                potion
                    .custom_particle
                    .as_ref()
                    .and_then(|p| p.vanilla_tag.as_ref()),
            );
        }
        entity_shared(
            game,
            shared,
            e.id,
            Some(Target::Cloud(e.id)),
            Shared {
                fire: Some(e.fire_ticks),
                ..Shared::default()
            },
        );
    }
    clouds.append(out, "breath_clouds");
}

fn hash_victims(
    h: &mut DefaultHasher,
    victims: &[(sunrise_gameplay_common::game::potions_common::Victim, i32)],
) {
    use sunrise_gameplay_common::game::{dragon_common::FightWho, potions_common::Victim};
    let mut victims: Vec<_> = victims
        .iter()
        .map(|&(who, until)| {
            let (kind, id) = match who {
                Victim::Mob(id) => (0, id),
                Victim::Player(FightWho::Host) => (1, 0),
                Victim::Player(FightWho::Guest(conn)) => (2, conn),
            };
            (kind, id, until)
        })
        .collect();
    victims.sort_unstable();
    victims.hash(h);
}

fn hash_phase(h: &mut DefaultHasher, phase: &sunrise_gameplay_common::game::dragon_common::Phase) {
    use sunrise_gameplay_common::game::dragon_common::{FightWho, Phase};
    phase.id().hash(h);
    let (target, path) = match phase {
        Phase::Holding {
            path,
            target,
            clockwise,
        } => {
            clockwise.hash(h);
            (target, path.as_ref())
        }
        Phase::Strafe {
            charge,
            path,
            target,
            clockwise,
            player,
        } => {
            (
                *charge,
                *clockwise,
                player.map(|who| match who {
                    FightWho::Host => None,
                    FightWho::Guest(conn) => Some(conn),
                }),
            )
                .hash(h);
            (target, path.as_ref())
        }
        Phase::Takeoff {
            first,
            path,
            target,
        } => {
            first.hash(h);
            (target, path.as_ref())
        }
        Phase::LandingApproach { path, target } => (target, path.as_ref()),
        Phase::Landing { target } | Phase::Hover { target } => (target, None),
        Phase::Charging { target, since } => {
            since.hash(h);
            (target, None)
        }
        Phase::Dying { target, time } => {
            time.hash(h);
            (target, None)
        }
        Phase::Scanning { time } => return time.hash(h),
        Phase::Attacking { ticks } | Phase::Flaming { ticks } => return ticks.hash(h),
    };
    target.is_some().hash(h);
    if let Some(target) = target {
        vec3(h, *target);
    }
    path.is_some().hash(h);
    if let Some(path) = path {
        (path.0.next, &path.0.nodes).hash(h);
    }
}

fn seats(game: &ServerGame<'_>) -> Group {
    let resident = game.common.mobs.iter();
    let retained = game
        .common
        .persist
        .off_level
        .values()
        .filter_map(|e| e.as_mob());
    let mobs = resident
        .chain(retained)
        .filter(|e| !e.removed)
        .filter_map(|e| Some((e.id, e.extra.mount?)));
    let rows = game
        .common
        .resident_entities
        .general
        .rows
        .iter()
        .filter_map(|e| Some((e.id, e.mount?)));
    let bolts = game
        .common
        .bolts
        .iter()
        .filter_map(|e| Some((e.id, e.mount?)));
    let host = game
        .common
        .player
        .iter()
        .filter_map(|p| Some((p.entity_id as u32, game.common.riding?)));
    let guests = level_guests(game).filter_map(|p| Some((p.entity_id as u32, p.vehicle?)));
    hash_seats(
        mobs.chain(rows).chain(bolts).chain(host).chain(guests),
        |vehicle| game.seated_passengers(vehicle),
    )
}

fn hash_seats(
    edges: impl IntoIterator<Item = (u32, u32)>,
    passengers: impl Fn(u32) -> Vec<(i64, u32, bool)>,
) -> Group {
    let mut edges: Vec<_> = edges.into_iter().collect();
    edges.sort_unstable();
    let vehicles: BTreeSet<_> = edges.iter().map(|&(_, vehicle)| vehicle).collect();
    let mut group = Group::default();
    for (rider, vehicle) in edges {
        vehicle.hash(group.row(rider, JoinSeq::UNJOINED));
    }
    vehicles.len().hash(&mut group.hash);
    for vehicle in vehicles {
        vehicle.hash(&mut group.hash);
        let seats = passengers(vehicle);
        seats.len().hash(&mut group.hash);
        for (_, rider, player) in seats {
            (rider, player).hash(&mut group.hash);
        }
    }
    group
}

fn level_guests<'a>(
    game: &'a ServerGame<'_>,
) -> impl Iterator<Item = &'a sunrise_sim::lan::guest::Guest> {
    let dim = game.common.dim.id();
    game.server
        .lan
        .iter()
        .flat_map(|host| host.hub.guests.iter())
        .filter(move |g| g.placed && g.dimension == dim)
}

fn players(game: &ServerGame<'_>, items: &Items) -> Group {
    let mut group = Group::default();
    let c = &game.common;
    if let Some(p) = &c.player {
        let id = p.entity_id as u32;
        let h = group.row(id, JoinSeq::UNJOINED);
        vec3(h, p.position);
        game.saved_player_uuid().hash(h);
        let food = &p.health.food;
        (
            rotation([c.yaw, c.pitch]),
            p.health.health().to_bits(),
            p.health.absorption().to_bits(),
            food.food,
            food.saturation.to_bits(),
            c.experience.level,
            c.experience.progress.to_bits(),
            c.experience.total,
            c.selected,
            c.mode_id(),
            c.riding,
        )
            .hash(h);
        Some(motion_values(p)).hash(h);
        player_items(
            h,
            (&c.inventory, &c.armor, c.offhand.as_ref(), &c.equipment),
            items,
        );
        effects(h, &c.potions.effects);
        entity_values(
            game,
            h,
            id,
            Some(Target::Player),
            Shared {
                fire: Some(p.fire_ticks),
                air: Some(p.air),
                portal: Some(c.portal_player.cooldown),
                ground: Some(p.ground.on_ground),
                collisions: player_collisions(p),
                ..Shared::default()
            },
        );
    }
    let mut guests: Vec<_> = level_guests(game).collect();
    guests.sort_unstable_by_key(|p| p.entity_id);
    for p in guests {
        let id = p.entity_id as u32;
        let h = group.row(id, JoinSeq::UNJOINED);
        p.uuid.hash(h);
        (
            vec_bits(p.position),
            rotation([p.yaw, p.pitch]),
            p.health.to_bits(),
            p.motion
                .as_deref()
                .map(|m| m.health.absorption().to_bits())
                .or_else(|| p.saved_absorption.map(f32::to_bits)),
            p.food,
            p.saturation.to_bits(),
            p.xp_level,
            p.xp_progress.to_bits(),
            p.xp_total,
            p.selected,
            p.game_mode,
            p.vehicle,
        )
            .hash(h);
        p.motion
            .as_deref()
            .map(motion_values)
            .or_else(|| {
                p.loaded_body.map(|b| {
                    (
                        vec_bits(b.velocity),
                        b.fall_distance.to_bits(),
                        b.food_exhaustion.to_bits(),
                        b.food_tick_timer,
                    )
                })
            })
            .hash(h);
        player_items(
            h,
            (&p.inventory, &p.armor, p.offhand.as_ref(), &p.equipment),
            items,
        );
        effects(h, &p.effects);
        let motion = p.motion.as_deref();
        let portal = game
            .server
            .lan
            .as_ref()
            .and_then(|host| host.play.get(&p.conn))
            .map_or(0, |play| play.portal.cooldown);
        entity_values(
            game,
            h,
            id,
            Some(Target::Guest(p.conn)),
            Shared {
                fire: motion
                    .map(|m| m.fire_ticks)
                    .or_else(|| p.loaded_body.map(|b| b.fire_ticks)),
                air: motion
                    .map(|m| m.air)
                    .or_else(|| p.loaded_body.map(|b| b.air)),
                portal: Some(portal),
                ground: Some(p.on_ground),
                collisions: motion.map_or([None; 4], player_collisions),
                ..Shared::default()
            },
        );
    }
    group
}

fn player_collisions(p: &PlayerMotion) -> [Option<bool>; 4] {
    [
        Some(p.collision.horizontal),
        Some(p.collision.vertical),
        Some(p.collision.below),
        Some(p.collision.minor),
    ]
}

fn motion_values(p: &PlayerMotion) -> ([u64; 3], u64, u32, i32) {
    (
        vec_bits([p.velocity.x, p.velocity.y, p.velocity.z]),
        p.fall_distance.to_bits(),
        p.health.food.exhaustion.to_bits(),
        p.health.food.timer,
    )
}

type PlayerItems<'a> = (
    &'a [Option<ItemStack>],
    &'a [Option<ItemStack>; 4],
    Option<&'a ItemStack>,
    &'a Equipment,
);

fn player_items(
    h: &mut DefaultHasher,
    (inventory, armor, offhand, extra): PlayerItems<'_>,
    items: &Items,
) {
    inventory.len().hash(h);
    for slot in inventory.iter().chain(armor).chain(extra.iter()) {
        stack(h, slot.as_ref(), items);
    }
    stack(h, offhand, items);
}

fn effects(h: &mut DefaultHasher, effects: &PlayerEffects) {
    effects.list.len().hash(h);
    for effect in &effects.list {
        let mut next = Some(effect);
        while let Some(EffectInstance {
            id,
            duration,
            amplifier,
            ambient,
            visible,
            show_icon,
            hidden,
            ..
        }) = next
        {
            (true, id, duration, amplifier, ambient, visible, show_icon).hash(h);
            next = hidden.as_deref();
        }
        false.hash(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seat_digest_preserves_boarding_order_but_ignores_edge_enumeration() {
        let digest = |edges: [(u32, u32); 3], order: [u32; 2]| {
            hash_seats(edges, |vehicle| match vehicle {
                10 => order.into_iter().map(|id| (0, id, false)).collect(),
                11 => vec![(0, 13, false)],
                _ => unreachable!(),
            })
            .hash
            .finish()
        };
        let edges = [(11, 10), (12, 10), (13, 11)];
        assert_eq!(
            digest(edges, [11, 12]),
            digest([edges[2], edges[0], edges[1]], [11, 12])
        );
        assert_ne!(digest(edges, [11, 12]), digest(edges, [12, 11]));
        assert_ne!(
            digest(edges, [11, 12]),
            digest([(11, 10), (12, 10), (13, 10)], [11, 12])
        );
    }

    #[test]
    fn compound_digest_ignores_map_order_but_preserves_list_order() {
        let digest = |tag_value: Tag| {
            let mut h = hasher();
            tag(&mut h, &tag_value);
            h.finish()
        };
        let list = || Tag::List(vec![Tag::Int(1), Tag::Int(2)]);
        let expected = digest(Tag::compound([("a", list()), ("z", Tag::Int(0))]));
        assert_eq!(
            expected,
            digest(Tag::compound([("z", Tag::Int(0)), ("a", list())]))
        );
        assert_ne!(
            expected,
            digest(Tag::compound([
                ("a", Tag::List(vec![Tag::Int(2), Tag::Int(1)])),
                ("z", Tag::Int(0)),
            ]))
        );
    }
}
