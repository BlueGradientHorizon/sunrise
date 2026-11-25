//! Minecarts (`AbstractMinecart` with `OldMinecartBehavior`): following
//! rails and slopes, powered and activator rails, coming off the track,
//! riding with the rider's push, pushing by players and other carts,
//! `MinecartItem` placement, `VehicleEntity` breaking, the chest, hopper,
//! furnace and TNT variants, saving, and `AbstractMinecartRenderer`'s old
//! rail-following render with the display block.
#[cfg(test)]
#[path = "minecarts/lifetime_tests.rs"]
pub(in crate::game) mod lifetime_tests;
#[path = "minecarts/save_lending.rs"]
pub(in crate::game) mod save_lending;
#[path = "minecarts/section_steps.rs"]
pub(in crate::game) mod section_steps;

#[allow(unused_imports)]
use super::{Game, Hit, InteractTarget, Keys, entity::Entity};
#[allow(unused_imports)]
use std::{collections::HashMap, sync::Arc};
#[allow(unused_imports)]
use sunrise_client::entity_rules::EntityRules;
#[allow(unused_imports)]
use sunrise_client::join_seq::{JoinList, ListStamp};
#[allow(unused_imports)]
use sunrise_client::mobs::{Mob, seen_grid::SeenGrid};
#[allow(unused_imports)]
use sunrise_client::{
    blocks::Fluid,
    containers::BlockEntity,
    items::{ItemStack, Items},
    mesher::Vertex,
    mobs::{Part, Species, Who, anim::Poses, riders},
    particles::{ParticleOptions, ParticleType},
    random_source,
    scoreboard::pushable_by,
    sound_engine::{BLOCKS, CartSound, PlayResult, SoundInstance, Sounds},
};
#[allow(unused_imports)]
use sunrise_core::{
    geometry::{Aabb, Vec3, trig},
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
use sunrise_world::registry::BlockStateId;
#[path = "minecarts/hit.rs"]
pub(in crate::game) mod hit;
#[allow(unused_imports)]
pub(in crate::game) use hit::CartHit;
#[allow(unused_imports)]
pub(in crate::game) use sunrise_gameplay::minecart_cells::CartCells;
#[allow(unused_imports)]
use sunrise_gameplay::minecart_seat::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::game) enum CartKind {
    Rideable,
    Chest,
    Hopper,
    Furnace,
    Tnt,
    /// `MinecartCommandBlock`: its `BaseCommandBlock` is `Minecart::command`,
    /// run by a powered activator rail (`game/administration/command_minecarts.rs`).
    CommandBlock,
    /// `MinecartSpawner`: a `BaseSpawner` that ticks where the cart is
    /// (`Minecart::spawner`). Only `/summon` makes one: there is no item.
    Spawner,
}
impl CartKind {
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id.strip_prefix("minecraft:")? {
            "minecart" => Self::Rideable,
            "chest_minecart" => Self::Chest,
            "hopper_minecart" => Self::Hopper,
            "furnace_minecart" => Self::Furnace,
            "tnt_minecart" => Self::Tnt,
            "command_block_minecart" => Self::CommandBlock,
            "spawner_minecart" => Self::Spawner,
            _ => return None,
        })
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Rideable => "minecraft:minecart",
            Self::Chest => "minecraft:chest_minecart",
            Self::Hopper => "minecraft:hopper_minecart",
            Self::Furnace => "minecraft:furnace_minecart",
            Self::Tnt => "minecraft:tnt_minecart",
            Self::CommandBlock => "minecraft:command_block_minecart",
            Self::Spawner => "minecraft:spawner_minecart",
        }
    }
    /// `AbstractMinecart.getDropItem` and `getPickResult`: the item a cart drops and
    /// is picked as (a spawner minecart's is a plain minecart: there is no item of its own).
    pub fn item(self) -> &'static str {
        match self {
            Self::Spawner => "minecraft:minecart",
            kind => kind.id(),
        }
    }
    pub(in crate::game) fn slots(self) -> usize {
        match self {
            Self::Chest => 27,
            Self::Hopper => 5,
            _ => 0,
        }
    }
}
#[derive(Clone, Debug)]
pub(in crate::game) struct Minecart {
    pub(in crate::game) attachment_lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
    /// When it joined its list (`sunrise_client::join_seq`); set by the list.
    pub(in crate::game) join: sunrise_client::join_seq::JoinSeq,
    pub id: u32,
    pub kind: CartKind,
    pub position: Vec3,
    pub previous: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub previous_yaw: f32,
    pub pitch: f32,
    pub previous_pitch: f32,
    /// Minecart's client-only accumulated turn and the passenger's eased turn.
    pub rotation_offset: f32,
    pub player_rotation_offset: f32,
    pub passenger_yaw_delta: f32,
    /// Experimental track steps, drained by the server entity tracker.
    pub lerp_steps: Vec<sunrise_protocol::packets::play::MinecartStep>,
    pub flipped: bool,
    /// `AbstractMinecart.onRails`: transitions into rails need a position snap.
    pub(in crate::game) on_rails: bool,
    pub(in crate::game) ground: sunrise_world::ground_support::GroundState,
    pub(in crate::game) fall_distance: f64,
    /// The squared speed `MinecartTNT.causeFallDamage` blows up with, set by
    /// a landing of three blocks or more and taken by the cart's next tick
    /// `Entity.stuckSpeedMultiplier`: set by a cobweb the cart's box is in,
    /// scaled into its next move.
    pub(in crate::game) stuck: Vec3,
    pub hurt_time: i32,
    pub hurt_dir: i32,
    pub damage: f32,
    /// `Entity.noGravity` (`NoGravity` in its save).
    pub no_gravity: bool,
    /// `Entity.ticksFrozen` (`TicksFrozen` in its save): powder snow builds
    /// it up (`InsideBlockEffectType.FREEZE`), a cart never melts it.
    pub ticks_frozen: i32,
    pub items: Vec<Option<ItemStack>>,
    /// Furnace minecarts: fuel ticks and `MinecartFurnace.push`. Its y part
    /// stays between ticks (it sets the push's length), only saving drops it.
    pub fuel: i32,
    pub push: Vec3,
    /// TNT minecarts: ticks until the explosion, -1 when not primed.
    pub fuse: i32,
    /// TNT minecarts: `explosionPowerBase` and `explosionSpeedFactor`, the
    /// blast's power at rest and its gain with the cart's speed.
    pub explosion_power: f32,
    pub explosion_speed_factor: f32,
    /// Hopper minecarts: an unpowered activator rail enables them.
    pub enabled: bool,
    /// Chest minecarts: the loot table rolled into `items` on load, saved
    /// again while the items are still that roll.
    pub loot: Option<sunrise_client::save::PendingContainerLoot>,
    /// `Entity.firstTick`: not ticked yet (saved as `HasTicked`).
    pub first_tick: bool,
    /// `Entity.isInWater` (`wasTouchingWater`): `AbstractMinecart.tick`
    /// updates it after the move (`updateFluidInteraction`), so the move
    /// goes by where the cart was a tick before, and a new cart's first
    /// move is a dry one.
    pub in_water: bool,
    /// Command block minecarts: the cart's `BaseCommandBlock` (command,
    /// success count, last output, output tracking). The block-only flags of
    /// `CommandBlock` stay unset.
    pub command: sunrise_client::command_block::CommandBlock,
    /// `Entity.tickCount`, which `MinecartCommandBlock.activateMinecart`
    /// measures its `ACTIVATION_DELAY` against.
    pub tick_count: i32,
    /// `MinecartCommandBlock.lastActivated`.
    pub last_activated: i32,
    /// An activator rail asked for the command during this tick; it runs once
    /// the cart is back in the level (`command_minecarts.rs`).
    pub command_due: bool,
    /// Spawner minecarts: the cart's `BaseSpawner` (`Delay`, `SpawnData`,
    /// `SpawnPotentials` and the rest as a spawner block saves them).
    pub spawner: Option<Box<sunrise_client::mobs::spawner::Spawner>>,
    /// The synched `DATA_ID_COMMAND_NAME` and `DATA_ID_LAST_OUTPUT`, which
    /// follow the command block only at `onUpdated`.
    pub synced_command: String,
    pub synced_output: Option<sunrise_core::nbt::Tag>,
    /// `AbstractMinecart.customDisplayBlockState` (`DisplayState`): the block
    /// state's NBT form, resolved against the registry where it draws.
    pub custom_display: Option<sunrise_core::nbt::Tag>,
    /// `AbstractMinecart.customDisplayOffset` (`DisplayOffset`), in sixteenths.
    pub custom_offset: Option<i32>,
    pub random: LegacyRandom,
    /// What `entity_size` answered at its last size refresh (`EntityEvent.Size`): its box takes
    /// the answer while it holds for its vanilla size.
    pub(in crate::game) size_answer: Option<sunrise_client::pose::SizeAnswer>,
}
/// `EntityType.MINECART`'s `sized(0.98F, 0.7F)`: float dimensions, widened.
pub(in crate::game) const WIDTH: f64 = 0.98_f32 as f64;
pub(in crate::game) const HEIGHT: f64 = 0.7_f32 as f64;
/// `EntityEvent.TNT_PRIME`. `MinecartTNT.primeFuse` broadcasts it;
/// `MinecartTNT.handleEntityEvent` answers by calling `primeFuse`.
pub(in crate::game) const TNT_PRIME_EVENT: i8 = 70;
/// `AbstractMinecart.getDefaultDisplayOffset` (6), in sixteenths of a block, as
/// `MinecartChest` (8) and `MinecartHopper` (1) override it.
pub(in crate::game) fn default_display_offset(kind: CartKind) -> f64 {
    match kind {
        CartKind::Chest => 8.0,
        CartKind::Hopper => 1.0,
        CartKind::Rideable
        | CartKind::Furnace
        | CartKind::Tnt
        | CartKind::CommandBlock
        | CartKind::Spawner => 6.0,
    }
}
/// `AbstractMinecart.getDisplayOffset`: the saved `DisplayOffset`, else the default.
pub(in crate::game) fn display_offset(c: &Minecart) -> f64 {
    c.custom_offset
        .map_or_else(|| default_display_offset(c.kind), f64::from)
}

/// World-space envelope of the cart model and its optional displayed block,
/// based on the same root position and display offset used by the renderer.
pub(in crate::game) fn cart_render_bounds(position: Vec3, offset: f64, has_display: bool) -> Aabb {
    let (bottom, top) = if has_display {
        let display_shift = 0.75 * (offset - 8.0) / 16.0;
        (
            (position.y - 0.5).min(position.y + 0.375 - 0.4 + display_shift),
            (position.y + 0.9).max(position.y + 0.375 + 0.4 + display_shift + 0.9),
        )
    } else {
        (position.y - 0.5, position.y + 0.9)
    };
    Aabb::new(
        Vec3::new(position.x - 1.0, bottom, position.z - 1.0),
        Vec3::new(position.x + 1.0, top, position.z + 1.0),
    )
}
/// A display state that is air shows nothing.
pub(in crate::game) fn shown_unless_air(
    state: Option<BlockStateId>,
    is_air: impl Fn(BlockStateId) -> bool,
) -> Option<BlockStateId> {
    state.filter(|&s| !is_air(s))
}
/// `readAdditionalSaveData`'s `input.read("DisplayState", CODEC)` is empty
/// when the state does not decode: the cart forgets it (and no longer saves it).
pub(in crate::game) fn forget_unreadable_display(
    c: &mut Minecart,
    resolves: impl Fn(&sunrise_core::nbt::Tag) -> bool,
) {
    if c.custom_display.as_ref().is_some_and(|t| !resolves(t)) {
        c.custom_display = None;
    }
}

/// The carts' and mobs' boxes for one tick's cart pushes (`push_from_cart`,
/// `pickup_from_cart`), so a cart reads its neighbours instead of every cart and
/// every mob of the level. Both answer in slot order, as the scans did.
///
/// The cart grid follows each ticked cart ([`CartIndex::moved`]); it is
/// rebuilt once the list's length changed (a cart gone or added), and dropped
/// after a command, which can move or replace any cart
/// ([`CartIndex::forget_carts`]). The mob grid holds until the mob
/// list's next write ([`ListStamp`]). A native mod's hook can move either kind
/// unseen, so with mods loaded both scans stay.
#[derive(Default)]
pub(in crate::game) struct CartIndex {
    pub(in crate::game) carts: Option<(SeenGrid, Vec<u32>)>,
    pub(in crate::game) mobs: Option<(ListStamp, SeenGrid)>,
    pub(in crate::game) near: Vec<u32>,
}
/// Where the cart `id` is in `carts`: at `hint` (where it was a moment ago,
/// the usual case while carts tick in order) or found by a scan. A world
/// with hundreds of mineshaft chest carts asked this of every cart several
/// times a tick.
pub(in crate::game) fn cart_at(carts: &[Minecart], id: u32, hint: usize) -> Option<usize> {
    if carts.get(hint).is_some_and(|c| c.id == id) {
        return Some(hint);
    }
    carts.iter().position(|c| c.id == id)
}
/// `AbstractMinecart.push(entity)` for a non-cart `entity` at `feet`:
/// the push the cart loses (the entity gets a quarter of it back).
pub(in crate::game) fn cart_push_from(feet: Vec3, cart: Vec3) -> Option<(f64, f64)> {
    let (xa, za) = (feet.x - cart.x, feet.z - cart.z);
    let dd = xa * xa + za * za;
    if dd < f64::from(1.0e-4f32) {
        return None;
    }
    let d = dd.sqrt();
    let pow = (1.0 / d).min(1.0);
    let k = f64::from(0.1f32) * 0.5;
    Some((xa / d * pow * k, za / d * pow * k))
}
/// `MinecartFurnace.calculateNewPushAlong`: project the push onto movement.
pub(in crate::game) fn push_along(push: Vec3, movement: Vec3) -> Vec3 {
    if push.horizontal_distance_squared() > 1.0e-4 && movement.horizontal_distance_squared() > 0.001
    {
        push.projected_on(movement).normalize().scale(push.length())
    } else {
        push
    }
}
/// The minecart's passenger attachment minus Player.DEFAULT_VEHICLE_ATTACHMENT.
pub(in crate::game) const SEAT: f64 = PASSENGER_Y - 0.6;
/// The placeholder container key under which an open cart's items live.
pub(in crate::game) const CART_KEY_Y: i32 = sunrise_client::lan::menu::VEHICLE_KEY_Y;

/// `OldMinecartBehavior.pushAndPickupEntities`: living entities the cart
/// offers `startRiding` instead of `Entity.push`. Players click to board;
/// iron golems always push; a cart that already has a rider is `isVehicle`.
pub(in crate::game) fn pickup_instead_of_push(
    species: Species,
    is_passenger: bool,
    cart_is_vehicle: bool,
) -> bool {
    !cart_is_vehicle && !is_passenger && species != Species::IronGolem
}

/// `EntitySelector.pushableBy` asks `isPushable`: `Shulker` takes no push
/// (`Mob::pushable` leaves it out) but still counts, so a cart offers it
/// `startRiding` like any other living entity.
pub(in crate::game) fn offered_pickup(pushable: bool, species: Species, alive: bool) -> bool {
    pushable || species == Species::Shulker && alive
}

/// `Shulker.push(Entity)` is empty, so a shulker that is not picked up (it
/// already rides something) neither moves nor slows the cart.
pub(in crate::game) fn takes_push_from_cart(species: Species) -> bool {
    species != Species::Shulker
}

/// `Entity.canRide` for a cart: the boarding cooldown, and a warden or the
/// wither never rides (`canRide` false).
pub(in crate::game) fn can_ride_cart(species: Species, boarding_cooldown: i32) -> bool {
    boarding_cooldown <= 0 && !matches!(species, Species::Warden | Species::Wither)
}

/// `Entity.positionRider` on an `AbstractMinecart`: the seat offset
/// (`cart_seat_offset`) above the cart.
pub(in crate::game) fn cart_seat(cart: Vec3, seat_offset: f64) -> Vec3 {
    Vec3::new(cart.x, cart.y + seat_offset, cart.z)
}

/// A general rider's seat on a cart: the cart's passenger attachment with no vehicle attachment of its own.
pub(in crate::game) fn cart_general_seat(cart: Vec3) -> Vec3 {
    cart_seat(cart, PASSENGER_Y)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Shape {
    NorthSouth,
    EastWest,
    AscendingEast,
    AscendingWest,
    AscendingNorth,
    AscendingSouth,
    SouthEast,
    SouthWest,
    NorthWest,
    NorthEast,
}
impl Shape {
    pub(in crate::game) fn parse(s: &str) -> Self {
        match s {
            "east_west" => Self::EastWest,
            "ascending_east" => Self::AscendingEast,
            "ascending_west" => Self::AscendingWest,
            "ascending_north" => Self::AscendingNorth,
            "ascending_south" => Self::AscendingSouth,
            "south_east" => Self::SouthEast,
            "south_west" => Self::SouthWest,
            "north_west" => Self::NorthWest,
            "north_east" => Self::NorthEast,
            _ => Self::NorthSouth,
        }
    }
    pub(in crate::game) fn is_slope(self) -> bool {
        matches!(
            self,
            Self::AscendingEast | Self::AscendingWest | Self::AscendingNorth | Self::AscendingSouth
        )
    }
    /// `AbstractMinecart.EXITS`.
    pub(in crate::game) fn exits(self) -> ([i32; 3], [i32; 3]) {
        let (w, e, n, s) = ([-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]);
        let below = |v: [i32; 3]| [v[0], -1, v[2]];
        match self {
            Self::NorthSouth => (n, s),
            Self::EastWest => (w, e),
            Self::AscendingEast => (below(w), e),
            Self::AscendingWest => (w, below(e)),
            Self::AscendingNorth => (n, below(s)),
            Self::AscendingSouth => (below(n), s),
            Self::SouthEast => (s, e),
            Self::SouthWest => (s, w),
            Self::NorthWest => (n, w),
            Self::NorthEast => (n, e),
        }
    }
}
pub(in crate::game) struct Rail {
    pub(in crate::game) name: Arc<str>,
    pub(in crate::game) shape: Shape,
    pub(in crate::game) powered: bool,
}
impl Minecart {
    /// `Entity.onGround`.
    pub(in crate::game) fn on_ground(&self) -> bool {
        self.ground.on_ground
    }
    pub fn new(id: u32, kind: CartKind, position: Vec3) -> Self {
        Self {
            size_answer: None,
            join: Default::default(),
            attachment_lifetime: Default::default(),
            id,
            kind,
            position,
            previous: position,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            previous_yaw: 0.0,
            pitch: 0.0,
            previous_pitch: 0.0,
            rotation_offset: 0.0,
            player_rotation_offset: 0.0,
            passenger_yaw_delta: 0.0,
            lerp_steps: Vec::new(),
            flipped: false,
            on_rails: false,
            ground: Default::default(),
            fall_distance: 0.0,
            stuck: Vec3::ZERO,
            hurt_time: 0,
            hurt_dir: 1,
            damage: 0.0,
            no_gravity: false,
            ticks_frozen: 0,
            items: vec![None; kind.slots()],
            fuel: 0,
            push: Vec3::ZERO,
            fuse: -1,
            explosion_power: DEFAULT_EXPLOSION_POWER,
            explosion_speed_factor: DEFAULT_EXPLOSION_SPEED_FACTOR,
            enabled: true,
            loot: None,
            first_tick: true,
            in_water: false,
            command: sunrise_client::command_block::CommandBlock::default(),
            tick_count: 0,
            last_activated: 0,
            command_due: false,
            // A fresh `BaseSpawner`, as `MinecartSpawner`'s constructor makes.
            spawner: (kind == CartKind::Spawner).then(Default::default),
            synced_command: String::new(),
            synced_output: None,
            custom_display: None,
            custom_offset: None,
            random: random_source::entity(),
        }
    }
    /// `EntityType.MINECART & co`'s `getDimensions()`: width, height and eye height.
    pub(in crate::game) const DIMENSIONS: [f32; 3] = [0.98, 0.7, 0.595];
    pub fn bounds(&self) -> Aabb {
        self.bounds_at(self.position)
    }
    /// The box of this cart were its position `p`.
    pub(in crate::game) fn bounds_at(&self, p: Vec3) -> Aabb {
        let w = WIDTH / 2.0;
        let vanilla = Aabb::new(
            Vec3::new(p.x - w, p.y, p.z - w),
            Vec3::new(p.x + w, p.y + HEIGHT, p.z + w),
        );
        sunrise_client::pose::SizeAnswer::sized_box(self.size_answer, Self::DIMENSIONS, p, vanilla)
    }
}
/// `AbstractMinecartContainer.applyNaturalSlowdown`'s keep factor, in the
/// float arithmetic vanilla uses: 0.98 plus a thousandth per empty step of the
/// comparator signal (none while a loot table is still unrolled), times 0.95
/// in water.
pub(in crate::game) fn container_keep(signal: i32, loot_pending: bool, in_water: bool) -> f64 {
    let mut keep = 0.98_f32;
    if !loot_pending {
        keep += (15 - signal) as f32 * 0.001_f32;
    }
    if in_water {
        keep *= 0.95_f32;
    }
    f64::from(keep)
}
#[cfg_attr(test, derive(Clone))]
pub(in crate::game) struct CartModels {
    pub models: HashMap<CartKind, Part>,
}
impl CartModels {
    pub fn load(document: Option<&sunrise_client::mobs::EntityModelDocument>) -> Self {
        let kinds = [
            CartKind::Rideable,
            CartKind::Chest,
            CartKind::Hopper,
            CartKind::Furnace,
            CartKind::Tnt,
            CartKind::CommandBlock,
            CartKind::Spawner,
        ];
        let models = kinds
            .into_iter()
            .filter_map(|k| document?.model(&format!("{}#main", k.id())).map(|m| (k, m)))
            .collect();
        Self { models }
    }
}
impl Game<'_> {
    /// `getCurrentBlockPosOrRailBelow`.
    pub(in crate::game) fn rail_block(&self, p: Vec3, improved: bool) -> [i32; 3] {
        let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let below = if improved {
            (p.y - 0.1 - f64::from(1.0e-5_f32)).floor() as i32
        } else {
            y - 1
        };
        if self.tagged_rail(self.world.block(x, below, z)) {
            [x, below, z]
        } else {
            [x, y, z]
        }
    }
    /// `OldMinecartBehavior.getPos`: the point on the rail under (x, y, z), on the track the
    /// rail lays for `cart`.
    pub(in crate::game) fn rail_pos(
        &self,
        cart: Option<u32>,
        x: f64,
        y: f64,
        z: f64,
    ) -> Option<[f64; 3]> {
        let [xt, yt, zt] = self.rail_block(Vec3::new(x, y, z), self.minecart_improvements());
        let rail = self.rail(cart, xt, yt, zt)?;
        let (e0, e1) = rail.shape.exits();
        let (fx, fy, fz) = (f64::from(xt), f64::from(yt), f64::from(zt));
        let x0 = fx + 0.5 + f64::from(e0[0]) * 0.5;
        let y0 = fy + 0.0625 + f64::from(e0[1]) * 0.5;
        let z0 = fz + 0.5 + f64::from(e0[2]) * 0.5;
        let x1 = fx + 0.5 + f64::from(e1[0]) * 0.5;
        let y1 = fy + 0.0625 + f64::from(e1[1]) * 0.5;
        let z1 = fz + 0.5 + f64::from(e1[2]) * 0.5;
        let (xd, yd, zd) = (x1 - x0, (y1 - y0) * 2.0, z1 - z0);
        let progress = if xd == 0.0 {
            z - fz
        } else if zd == 0.0 {
            x - fx
        } else {
            ((x - x0) * xd + (z - z0) * zd) * 2.0
        };
        let mut out = [x0 + xd * progress, y0 + yd * progress, z0 + zd * progress];
        if yd < 0.0 {
            out[1] += 1.0;
        } else if yd > 0.0 {
            out[1] += 0.5;
        }
        Some(out)
    }
    /// `AbstractMinecart`: `minecart_improvements` picks the smoother
    /// physics.
    pub(in crate::game) fn minecart_improvements(&self) -> bool {
        self.enabled_feature_ids()
            .iter()
            .any(|f| f == "minecraft:minecart_improvements")
    }
    /// `positionRider` for the mobs in minecarts: each sits on the cart's
    /// passenger attachment, less its own vehicle attachment.
    pub(in crate::game) fn seat_cart_mobs(&mut self, riding: &super::passengers::Riders) {
        for i in 0..self.carts.len() {
            let id = self.carts[i].id;
            let Some(js) = riding.get(&id) else {
                continue;
            };
            let (pos, prev) = (self.carts[i].position, self.carts[i].previous);
            let cart_box = self.carts[i].bounds();
            let seats: Vec<(usize, f64, f64)> = js
                .iter()
                .map(|&j| {
                    let m = &self.mobs[j];
                    // Minecart's box is 0.98 wide.
                    let attachment = riders::vehicle_attachment(m, 0.98);
                    (j, attachment, cart_seat_offset(m.species(), attachment))
                })
                .collect();
            for (j, attachment, offset) in seats {
                let m = &mut self.mobs[j];
                m.seated(id, attachment, cart_box);
                sunrise_client::mobs::living::teleports::set_pos(m, cart_seat(pos, offset));
                m.previous = cart_seat(prev, offset);
                m.velocity = Vec3::ZERO;
                m.fall_distance = 0.0;
                m.ground.on_ground = false;
                m.extra.passenger = true;
                m.ai.nav.stop();
            }
        }
    }
    /// Refuse a removed target before carrying its owning slot into a tick clone.
    pub(in crate::game) fn store_cart_back(&mut self, index: usize, cart: Minecart) -> bool {
        !self
            .persist
            .removals
            .contains(&super::commands::Target::Cart(cart.id))
            && self.carts.store_back(index, cart)
    }

    /// `getDismountLocationForPassenger`: beside the cart's motion direction,
    /// else on top.
    pub(in crate::game) fn dismount_cart(&mut self, i: usize) {
        let id = self.carts[i].id;
        if self.riding == Some(id) && !self.native_may_leave(id) {
            return;
        }
        self.riding = None;
        if self.dismount_in_portal(self.carts[i].position) {
            return;
        }
        let target = self.cart_player_dismount_target(i);
        let from = self.player.as_ref().map(|player| player.position);
        if let Some(p) = self.player.as_mut() {
            p.position = target;
            p.velocity = Vec3::ZERO;
        }
        section_steps::tell_player(self, from.map(|from| (from, target)));
    }
    /// Where the cart at `i` lets a player off.
    pub(in crate::game) fn cart_player_dismount_target(&self, i: usize) -> Vec3 {
        let c = &self.carts[i];
        let (bx, by, bz) = (
            c.position.x.floor() as i32,
            c.position.y.floor() as i32,
            c.position.z.floor() as i32,
        );
        let mut target = None;
        let face = cart_motion_face(c.yaw, c.flipped, self.minecart_improvements());
        'search: for dy in [0, 1, -1] {
            for (dx, dz) in dismount_offsets(face) {
                let (x, y, z) = (bx + dx, by + dy, bz + dz);
                let below = self.res.facts().get(self.world.block(x, y - 1, z));
                let here = self.res.facts().get(self.world.block(x, y, z));
                let head = self.res.facts().get(self.world.block(x, y + 1, z));
                if (below.full_collision || below.legacy_solid)
                    && !here.full_collision
                    && !head.full_collision
                    && here.fluid == Fluid::None
                {
                    target = Some(Vec3::new(
                        f64::from(x) + 0.5,
                        f64::from(y),
                        f64::from(z) + 0.5,
                    ));
                    break 'search;
                }
            }
        }
        let top = c.bounds().max_position().y;
        target.unwrap_or(Vec3::new(c.position.x, top, c.position.z))
    }
    /// The slots of the cart `id`, for a command: `getChestVehicleItem` and
    /// `setChestVehicleItem` first unpack its loot table with no player, so
    /// no `player_generates_container_loot` fires later. While its menu is
    /// open the live slots are the placeholder copy (`open_cart_container`).
    pub(in crate::game) fn edit_cart_slots<R>(
        &mut self,
        id: u32,
        edit: impl FnOnce(&mut Vec<Option<ItemStack>>) -> R,
    ) -> Option<R> {
        let i = self.carts.iter().position(|c| c.id == id)?;
        self.unpack_loot(super::container_loot::LootTarget::Cart(id), None);
        match self
            .world
            .block_entities
            .get_mut(&(i32::MAX - 15, CART_KEY_Y, id as i32))
        {
            Some(BlockEntity::Chest(open)) => Some(edit(open)),
            _ => Some(self.carts.update(i, |c| edit(&mut c.items))),
        }
    }
    /// Closing the cart's container open under `key` copies the items back.
    pub(in crate::game) fn close_cart_container(&mut self, key: (i32, i32, i32)) {
        if key.1 != CART_KEY_Y {
            return;
        }
        self.store_cart_items(key);
        let guest_viewers = self
            .lan
            .as_ref()
            .is_some_and(|host| Self::vehicle_has_other_viewers(None, &host.hub.guests, None, key));
        if !guest_viewers {
            self.world.block_entities.remove(&key);
        }
    }
    /// A chest or hopper minecart or a chest boat is about to be removed
    /// (`Containers.dropContents` / `AbstractChestBoat.remove`): what an open
    /// menu edited goes back to the vehicle, so it is what drops or travels
    /// with it, and the menu's stand-in leaves the level. A menu that is
    /// still open over it then has nothing left to take.
    pub(in crate::game) fn release_vehicle_container(&mut self, id: u32) {
        let key = (i32::MAX - 15, CART_KEY_Y, id as i32);
        self.store_cart_items(key);
        self.world.block_entities.remove(&key);
        if self.container == Some(key) {
            self.container = None;
        }
    }
    /// The menu of a chest or hopper minecart or a chest boat works on the
    /// vehicle's own items (`ContainerEntity`): the placeholder under `key`
    /// that the menu edits is copied back to the vehicle.
    pub(in crate::game) fn store_cart_items(&mut self, key: (i32, i32, i32)) {
        if key.1 != CART_KEY_Y {
            return;
        }
        let Some(BlockEntity::Chest(items)) = self.world.block_entities.get(&key) else {
            return;
        };
        let items = items.clone();
        if let Some(i) = self.carts.iter().position(|c| c.id as i32 == key.2) {
            self.carts.update(i, |c| c.items = items);
        } else if let Some(i) = self.boats.iter().position(|b| b.id as i32 == key.2) {
            self.boats.update(i, |b| {
                b.items = items;
            });
        }
    }
    /// The open cart's kind when a cart container is shown.
    pub(in crate::game) fn open_cart_kind(&self) -> Option<CartKind> {
        let key = self.container.filter(|k| k.1 == CART_KEY_Y)?;
        // A chest boat's chest shows like a chest minecart's.
        self.carts
            .iter()
            .find(|c| c.id as i32 == key.2)
            .map(|c| c.kind)
            .or_else(|| {
                self.boats
                    .iter()
                    .any(|b| b.chest && b.id as i32 == key.2)
                    .then_some(CartKind::Chest)
            })
    }
    /// `Containers.dropContents(level, entity, container)` at a vehicle's
    /// position `at`: each slot (an empty one too) from a spot in its block,
    /// in pieces of 10 to 30 with the level random's scatter, no pickup
    /// delay. `AbstractChestBoat.remove` and `AbstractMinecartContainer.remove`
    /// spill this way for a destroying removal (kill or discard), and
    /// `chestVehicleDestroyed` again for the emptied container.
    pub(in crate::game) fn drop_vehicle_contents(
        &mut self,
        at: [f64; 3],
        items: &[Option<ItemStack>],
    ) {
        for stack in items {
            let count = stack.as_ref().map_or(0, |s| s.count);
            let pieces = sunrise_world::level::removal::drop_item_stack_at(
                &mut self.world.random,
                at,
                count,
            );
            if let Some(stack) = stack {
                for piece in pieces {
                    let piece_stack = ItemStack {
                        count: piece.count,
                        ..stack.clone()
                    };
                    self.spawn_item(piece_stack, piece.pos, piece.motion, 0);
                }
            }
        }
    }
    /// The seed vanilla picks a block state model's weighted alternatives with outside the
    /// world (`BlockModelResolver.MODEL_SEED`).
    pub(in crate::game) const BLOCK_MODEL_SEED: i64 = 42;
}
/// Saving and loading.
impl Minecart {
    pub fn to_nbt(
        &self,
        uuid: [i32; 4],
        raw: Option<&sunrise_core::nbt::Tag>,
        items: &crate::items::Items,
    ) -> sunrise_core::nbt::Tag {
        use sunrise_core::nbt::{JavaString, Tag};
        let mut tag = sunrise_client::entity_nbt::vehicle_to_nbt(
            self.kind.id(),
            self.position,
            self.velocity,
            [self.yaw, self.pitch],
            (self.ground.on_ground, self.fall_distance),
            uuid,
            raw,
        );
        sunrise_client::entity_nbt::write_saved_flags(
            &mut tag,
            self.ground.on_ground,
            self.no_gravity,
        );
        if let Tag::Compound(map) = &mut tag {
            // ContainerEntity.addChestVehicleSaveData belongs only to the
            // container subclasses, never to a plain or specialized cart.
            if self.kind.slots() == 0 {
                map.remove(&JavaString::from("Items"));
            }
            if self.kind == CartKind::CommandBlock {
                for key in ["LastOutput", "LastExecution"] {
                    map.remove(&JavaString::from(key));
                }
            }
            map.remove(&JavaString::from("DisplayOffset"));
            map.remove(&JavaString::from("DisplayState"));
            map.remove(&JavaString::from("explosion_power"));
            map.remove(&JavaString::from("explosion_speed_factor"));
            super::boats::put_ticks_frozen(map, self.ticks_frozen);
            let mut put = |k: &str, v: Tag| {
                map.insert(JavaString::from(k), v);
            };
            put("FlippedRotation", Tag::Byte(i8::from(self.flipped)));
            // AbstractMinecart saves firstTick as HasTicked. A cart never burns:
            // Entity.move sets -getFireImmuneTicks(), 0 for all but players.
            put("HasTicked", Tag::Byte(i8::from(self.first_tick)));
            put("Fire", Tag::Short(0));
            // addAdditionalSaveData: the offset only off its default, the state only when set.
            if let Some(offset) = self.custom_offset
                && f64::from(offset) != default_display_offset(self.kind)
            {
                put("DisplayOffset", Tag::Int(offset));
            }
            if let Some(state) = &self.custom_display {
                put("DisplayState", state.clone());
            }
            match self.kind {
                CartKind::Chest | CartKind::Hopper => {
                    put(
                        "Items",
                        sunrise_client::save::encode_items(&self.items, items),
                    );
                    if self.kind == CartKind::Hopper {
                        put("Enabled", Tag::Byte(i8::from(self.enabled)));
                    }
                }
                CartKind::Furnace => {
                    put("PushX", Tag::Double(self.push.x.to_bits()));
                    put("PushZ", Tag::Double(self.push.z.to_bits()));
                    put("Fuel", Tag::Short(self.fuel as i16));
                }
                // MinecartTNT.addAdditionalSaveData: the powers only off their defaults.
                CartKind::Tnt => {
                    put("fuse", Tag::Int(self.fuse));
                    for (key, value, default) in [
                        (
                            "explosion_power",
                            self.explosion_power,
                            DEFAULT_EXPLOSION_POWER,
                        ),
                        (
                            "explosion_speed_factor",
                            self.explosion_speed_factor,
                            DEFAULT_EXPLOSION_SPEED_FACTOR,
                        ),
                    ] {
                        if value != default {
                            put(key, Tag::Float(value.to_bits()));
                        }
                    }
                }
                // BaseCommandBlock.save (its CustomName shares the entity's key).
                CartKind::CommandBlock => {
                    for (key, value) in self.command.save() {
                        if !matches!(key, "CustomName" | "powered" | "conditionMet" | "auto") {
                            put(key, value);
                        }
                    }
                }
                // BaseSpawner.save: the delays, ranges, SpawnData and SpawnPotentials.
                CartKind::Spawner => {
                    if let Some(spawner) = &self.spawner {
                        for (key, value) in
                            sunrise_client::mobs::spawner::SpawnerExt::save(&**spawner)
                        {
                            put(key, value);
                        }
                    }
                }
                CartKind::Rideable => {}
            }
            map.remove(&JavaString::from("LootTable"));
            map.remove(&JavaString::from("LootTableSeed"));
            if let Some(pending) = &self.loot {
                let items = map.get(&"Items".into()).cloned();
                pending.write_tags(map);
                if self.items.iter().any(Option::is_some) {
                    map.extend(items.map(|items| ("Items".into(), items)));
                }
            }
        }
        tag
    }
    pub fn from_nbt(tag: &sunrise_core::nbt::Tag, id: u32, kind: CartKind) -> Option<Self> {
        let (position, _, _) = sunrise_client::entity_nbt::boat_from_nbt(tag)?;
        let mut c = Self::new(id, kind, position);
        c.load(tag)?;
        Some(c)
    }
    /// `Entity.load` on this cart: the saved fields are read over the live
    /// ones, the rest (riders, lerp, hurt state) stays; its kind never changes.
    /// `None`, with nothing changed, when the compound has no position.
    pub fn load(&mut self, tag: &sunrise_core::nbt::Tag) -> Option<()> {
        use sunrise_core::nbt::Tag;
        let (position, velocity, yaw) = sunrise_client::entity_nbt::boat_from_nbt(tag)?;
        let kind = self.kind;
        let c = self;
        c.position = position;
        c.previous = position;
        c.velocity = velocity;
        c.yaw = yaw;
        c.previous_yaw = yaw;
        c.no_gravity = sunrise_client::entity_nbt::flag(tag, "NoGravity");
        c.ground.on_ground = sunrise_client::entity_nbt::flag(tag, "OnGround");
        c.pitch = sunrise_client::entity_nbt::rotation(tag)[1];
        c.previous_pitch = c.pitch;
        // ValueInput's numeric getters take any NumericTag: getIntOr is its
        // intValue, getShortOr the low 16 bits of it, getBooleanOr
        // `byteValue() != 0` (the low 8), getDoubleOr its doubleValue.
        let int = |k: &str| {
            tag.get(k)
                .and_then(sunrise_client_core::nbt_path::int_value)
        };
        let flag = |k: &str, default: bool| int(k).map_or(default, |v| v as i8 != 0);
        let double = |k: &str| match tag.get(k) {
            Some(Tag::Byte(v)) => f64::from(*v),
            Some(Tag::Short(v)) => f64::from(*v),
            Some(Tag::Int(v)) => f64::from(*v),
            Some(Tag::Long(v)) => *v as f64,
            Some(Tag::Float(v)) => f64::from(f32::from_bits(*v)),
            Some(Tag::Double(v)) => f64::from_bits(*v),
            _ => 0.0,
        };
        // Entity.load: OnGround and fall_distance, as saved.
        (c.ground.on_ground, c.fall_distance) = sunrise_client::entity_nbt::ground_from_nbt(tag);
        c.ticks_frozen = super::boats::saved_ticks_frozen(tag);
        c.flipped = flag("FlippedRotation", false);
        c.first_tick = flag("HasTicked", false);
        if kind.slots() > 0 {
            c.loot = sunrise_client::save::PendingContainerLoot::from_tag(tag);
            c.items = if c.loot.is_some() {
                vec![None; kind.slots()]
            } else {
                sunrise_client::save::decode_items(tag.get("Items"), kind.slots())
            };
        }
        c.enabled = flag("Enabled", true);
        c.fuel = int("Fuel").map_or(0, |v| i32::from(v as i16));
        // readAdditionalSaveData: the saved push has no y.
        c.push = Vec3::new(double("PushX"), 0.0, double("PushZ"));
        c.fuse = int("fuse").unwrap_or(-1);
        if kind == CartKind::Tnt {
            // MinecartTNT.readAdditionalSaveData: getFloatOr, kept to 0..128.
            let float = |k: &str, default: f32| match tag.get(k) {
                Some(Tag::Float(v)) => f32::from_bits(*v),
                Some(Tag::Double(v)) => f64::from_bits(*v) as f32,
                Some(Tag::Long(v)) => *v as f32,
                Some(other) => {
                    sunrise_client_core::nbt_path::int_value(other).map_or(default, |v| v as f32)
                }
                None => default,
            };
            c.explosion_power = float("explosion_power", DEFAULT_EXPLOSION_POWER).clamp(0.0, 128.0);
            c.explosion_speed_factor =
                float("explosion_speed_factor", DEFAULT_EXPLOSION_SPEED_FACTOR).clamp(0.0, 128.0);
        }
        // readAdditionalSaveData: DisplayState, DisplayOffset (default offset when absent).
        c.custom_display = tag.get("DisplayState").cloned();
        c.custom_offset = int("DisplayOffset");
        if kind == CartKind::CommandBlock {
            c.command = sunrise_client::command_block::CommandBlock::load(tag);
            // MinecartCommandBlock.readAdditionalSaveData sets the synched command.
            // A changed value invokes onSyncedDataUpdated -> setCommand, resetting successes.
            if c.synced_command != c.command.command {
                c.synced_command.clone_from(&c.command.command);
                c.command.set_command(&c.synced_command);
            }
            c.sync_command_output();
        }
        // MinecartSpawner.readAdditionalSaveData: BaseSpawner.load.
        if kind == CartKind::Spawner {
            c.spawner = Some(Box::new(sunrise_client::mobs::spawner::SpawnerExt::load(
                tag,
            )));
        }
        Some(())
    }
}

/// `MinecartTNT` default explosion powers.
pub(in crate::game) const DEFAULT_EXPLOSION_POWER: f32 = 4.0;
pub(in crate::game) const DEFAULT_EXPLOSION_SPEED_FACTOR: f32 = 1.0;

/// `VehicleEntity.hurtServer`'s destroy test: a creative player, damage past
/// 40, or (`MinecartTNT.shouldSourceDestroy`) fire or an explosion hitting a
/// TNT cart.
pub(in crate::game) fn cart_is_destroyed(creative: bool, damage: f32, ignites_tnt: bool) -> bool {
    creative || damage > 40.0 || ignites_tnt
}

/// `MinecartTNT.tick`'s collision detonation: a cart still moving at a tenth
/// of a block a tick or more when a block stops it blows up.
pub(in crate::game) fn tnt_ignites_by_collision(
    horizontal_collision: bool,
    speed_sqr: f64,
) -> bool {
    horizontal_collision && speed_sqr >= 0.009_999_999_776_482_582
}

/// NewMinecartBehavior's `(float) (Math.atan2(y, x) * 180.0 / Math.PI)` before
/// the cast: HotSpot's atan2, multiplied then divided in Java's order.
pub(in crate::game) fn java_degrees(y: f64, x: f64) -> f64 {
    sunrise_core::jmath::atan2(y, x) * 180.0 / std::f64::consts::PI
}

impl Game<'_> {
    /// `MinecartTNT.primeFuse`: fuse 80, then `Level.broadcastEntityEvent`
    /// (`EntityEvent.TNT_PRIME`, 70) and, unless the cart is silent,
    /// `SoundEvents.TNT_PRIMED` on `SoundSource.BLOCKS`. A client that
    /// receives 70 only sets the fuse (`handleEntityEvent`).
    pub(in crate::game) fn prime_cart(&mut self, c: &mut Minecart) {
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::TNT_EXPLODES)
        {
            return;
        }
        c.fuse = 80;
        let at = [c.position.x, c.position.y, c.position.z];
        let id = c.id as i32;
        self.lan_entity_event(id, TNT_PRIME_EVENT, at);
        if !self.entity_silent(c.id) {
            self.server_play_sound_on(BLOCKS, "minecraft:entity.tnt.primed", at, 1.0, 1.0);
        }
    }
}

#[cfg(test)]
#[path = "minecarts/minecart_tnt_tests.rs"]
pub(in crate::game) mod minecart_tnt_tests;

/// `MinecartFurnace.addFuel`: Java int arithmetic, so a fuel a mod set near
/// `i32::MAX` wraps negative and takes more.
pub(in crate::game) fn furnace_fuel_after(fuel: i32) -> Option<i32> {
    let next = fuel.wrapping_add(3600);
    (next <= 32000).then_some(next)
}

/// `MinecartFurnace.getMaxSpeed`: half the behavior's speed, three quarters
/// in water.
pub(in crate::game) fn furnace_speed(c: &Minecart, base: f64) -> f64 {
    if c.kind == CartKind::Furnace {
        base * if c.in_water { 0.75 } else { 0.5 }
    } else {
        base
    }
}

/// Local geometry with atlas coordinates resolved for both CPU and GPU consumers.
#[derive(Clone, Copy)]
pub(in crate::game) struct PreparedModelFace {
    pub(in crate::game) positions: [[f32; 3]; 4],
    pub(in crate::game) uv: [[f32; 2]; 4],
    pub(in crate::game) facing: sunrise_assets::model::Direction,
    pub(in crate::game) tint_index: i32,
    /// The side that hides this face when a neighbour covers it (`ModelQuad::cull`).
    pub(in crate::game) cull: Option<sunrise_assets::model::Direction>,
    pub(in crate::game) light_emission: u8,
}

impl PreparedModelFace {
    pub(in crate::game) fn new(
        quad: &sunrise_assets::bake::ModelQuad,
        sprite: sunrise_client::hud::Rect,
    ) -> Self {
        Self {
            positions: quad.positions,
            uv: quad.uv.map(|uv| {
                [
                    sprite.u0 + uv[0] * (sprite.u1 - sprite.u0),
                    sprite.v0 + uv[1] * (sprite.v1 - sprite.v0),
                ]
            }),
            facing: quad.facing,
            tint_index: quad.tint_index,
            cull: quad.cull,
            light_emission: quad.light_emission,
        }
    }

    pub(in crate::game) fn positions(&self) -> [[f32; 3]; 4] {
        self.positions
    }
}

/// Shared transform, shading and light path for block-state and prepared standalone faces.
pub(in crate::game) fn emit_model_faces(
    faces: impl IntoIterator<Item = (PreparedModelFace, u32)>,
    m: glam::Mat4,
    diffuse: bool,
    tint: impl Fn(i32) -> Option<u32>,
    out: &mut Vec<Vertex>,
) {
    use sunrise_assets::model::Direction as D;
    let mut normals = sunrise_client::item_render::FaceNormals::new(m);
    for (quad, light) in faces {
        let shade: u32 = match quad.facing {
            D::Up => 255,
            D::Down => 127,
            D::North | D::South => 204,
            D::West | D::East => 153,
        };
        let mut color = if diffuse {
            u32::MAX
        } else {
            0xFF00_0000 | shade << 16 | shade << 8 | shade
        };
        if let Some(t) = tint(quad.tint_index) {
            color =
                sunrise_client::hud_layout::multiply(color, sunrise_client::mobs::render::abgr(t));
        }
        let normal = if diffuse {
            let n = normals
                .face(quad.facing)
                .to_array()
                .map(|v| (v.clamp(-1.0, 1.0) * 127.0) as i8);
            [n[0], n[1], n[2], 127]
        } else {
            [0; 4]
        };
        let light = if diffuse {
            let emission = u32::from(quad.light_emission) * 16;
            (light & 0xFF) | (light >> 8 & 0xFF).max(emission) << 8
        } else {
            light
        };
        for k in 0..4 {
            let uv = quad.uv[k];
            out.push(Vertex {
                normal,
                pos: m
                    .transform_point3(glam::Vec3::from_array(quad.positions[k]))
                    .to_array(),
                uv,
                layer: 0,
                color,
                light,
                glint_uv: [0.0, 0.0],
            });
        }
    }
}

/// Vanilla multiplies tint, alpha included, only when `tintIndex != -1`.
pub(in crate::game) fn scale_tint_alpha<'f>(
    vertices: &mut [Vertex],
    faces: impl IntoIterator<Item = &'f PreparedModelFace>,
    tint: u32,
) {
    let alpha = tint >> 24;
    if alpha == 255 {
        return;
    }
    let (quads, _) = vertices.as_chunks_mut::<4>();
    for (face, quad) in faces.into_iter().zip(quads) {
        if face.tint_index < 0 {
            continue;
        }
        for vertex in quad {
            let scaled = (vertex.color >> 24) * alpha / 255;
            vertex.color = vertex.color & 0x00FF_FFFF | scaled << 24;
        }
    }
}

#[cfg(test)]
#[path = "minecarts/tests.rs"]
pub(in crate::game) mod tests;
sunrise_client::impl_joined!(Minecart);
sunrise_mod_runtime::impl_attachment_lifetime!(Minecart);

#[cfg(test)]
pub(in crate::game) mod tnt_impact_tests {
    use super::{CartKind, Minecart};
    use crate::game::native_entity_refs::tests::with_game;
    use sunrise_core::geometry::Vec3;

    /// Ticks a moving TNT cart until it stops or blows up; true when it
    /// blew up.
    fn runs_into_the_wall(rails: bool, at: Vec3, velocity: Vec3) -> bool {
        let mut result = false;
        with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            let registry = game.pack.assets.registry;
            let stone = registry.default_state("minecraft:stone").unwrap();
            let rail = registry.default_state("minecraft:rail").unwrap();
            let mut east_west = registry.state(rail).unwrap().clone();
            east_west
                .properties
                .insert("shape".into(), "east_west".into());
            let rail = registry.id(&east_west).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    game.world.set(x, 63, z, stone).unwrap();
                }
            }
            for z in 0..16 {
                game.world.set(9, 64, z, stone).unwrap();
                game.world.set(9, 65, z, stone).unwrap();
            }
            if rails {
                for x in 1..9 {
                    game.world.set(x, 64, 8, rail).unwrap();
                }
            }
            let mut cart = Minecart::new(1, CartKind::Tnt, at);
            cart.velocity = velocity;
            for _ in 0..40 {
                if !game.tick_cart(&mut cart, [0.0; 2], 0, &mut super::CartIndex::default()) {
                    result = true;
                    break;
                }
            }
        });
        result
    }

    #[test]
    #[ignore = "daily"]
    fn a_powered_rail_pushes_a_resting_cart_away_from_soul_sand() {
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        let registry = game.pack.assets.registry;
        let rail = registry.default_state("minecraft:powered_rail").unwrap();
        let mut state = registry.state(rail).unwrap().clone();
        state.properties.insert("shape".into(), "east_west".into());
        state.properties.insert("powered".into(), "true".into());
        let rail = registry.id(&state).unwrap();
        game.world.set(5, 64, 8, rail).unwrap();
        for (block, pushed) in [
            ("minecraft:soul_sand", true),
            ("minecraft:stone", true),
            ("minecraft:oak_slab", false),
        ] {
            let id = registry.default_state(block).unwrap();
            game.world.set(4, 64, 8, id).unwrap();
            let push = game.redstone_direction([5, 64, 8], super::Shape::EastWest);
            assert_eq!(push.x == 1.0, pushed, "{block}");
        }
    }

    #[test]
    #[ignore = "daily"]
    fn a_tnt_cart_stopped_head_on_by_a_wall_does_not_blow_up() {
        // OldMinecartBehavior.moveAlongTrack: the move zeroes the blocked x
        // and the track's re-pointing scales what is left, so MinecartTNT
        // reads no speed at the wall.
        assert!(!runs_into_the_wall(
            true,
            Vec3::new(3.5, 64.0, 8.5),
            Vec3::new(0.4, 0.0, 0.0)
        ));
    }

    #[test]
    #[ignore = "daily"]
    fn a_tnt_cart_off_its_rails_glancing_off_a_wall_blows_up() {
        // comeOffTrack keeps the delta the move resolved: the wall zeroes x,
        // z goes on at over a tenth of a block a tick.
        assert!(runs_into_the_wall(
            false,
            Vec3::new(8.3, 64.0, 4.5),
            Vec3::new(0.4, 0.0, 0.4)
        ));
    }

    #[test]
    #[ignore = "daily"]
    fn a_cart_on_a_slope_presses_a_plate_at_its_temporary_position() {
        // OldMinecartBehavior.moveAlongTrack places the cart at the rail's top
        // height, moves it horizontally (Entity.move records the step) and settles
        // it on the slope; the tick's applyEffectsFromBlocks still sweeps the step.
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        let registry = game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let plate = registry
            .default_state("minecraft:oak_pressure_plate")
            .unwrap();
        let rail = registry.default_state("minecraft:rail").unwrap();
        let mut slope = registry.state(rail).unwrap().clone();
        slope
            .properties
            .insert("shape".into(), "ascending_east".into());
        let rail = registry.id(&slope).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                game.world.set(x, 63, z, stone).unwrap();
            }
        }
        game.world.set(5, 64, 8, rail).unwrap();
        game.world.set(6, 64, 8, stone).unwrap();
        game.world.set(6, 65, 8, plate).unwrap();
        let mut cart = Minecart::new(1, CartKind::Rideable, Vec3::new(5.5, 64.5, 8.5));
        cart.velocity = Vec3::new(0.4, 0.0, 0.0);
        game.carts.push(cart.clone());
        assert!(game.tick_cart(&mut cart, [0.0; 2], 0, &mut super::CartIndex::default()));
        let powered = registry
            .state(game.world.block(6, 65, 8))
            .unwrap()
            .properties
            .get("powered")
            .cloned();
        assert_eq!(
            powered.as_deref(),
            Some("true"),
            "{:?}",
            registry.state(game.world.block(6, 65, 8))
        );
    }

    /// A cart blocked by a fence on a slope sits below the plate on the fence,
    /// but `AbstractMinecart.move` sweeps its blocks while it is still at the
    /// rail's top height, so the plate clocks on and off.
    #[test]
    #[ignore = "daily"]
    fn a_cart_resting_against_a_fence_keeps_toggling_the_plate_above_it() {
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        let registry = game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let fence = registry.default_state("minecraft:oak_fence").unwrap();
        let plate = registry
            .default_state("minecraft:oak_pressure_plate")
            .unwrap();
        let rail = registry.default_state("minecraft:rail").unwrap();
        let mut slope = registry.state(rail).unwrap().clone();
        slope
            .properties
            .insert("shape".into(), "ascending_east".into());
        let slope = registry.id(&slope).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                game.world.set(x, 62, z, stone).unwrap();
            }
        }
        game.world.set(0, 64, 8, fence).unwrap();
        game.world.set(0, 65, 8, plate).unwrap();
        game.world.set(1, 63, 8, stone).unwrap();
        game.world.set(1, 64, 8, slope).unwrap();
        game.world.set(2, 64, 8, stone).unwrap();
        game.world.set(2, 65, 8, rail).unwrap();
        game.carts.push(Minecart::new(
            1,
            CartKind::Rideable,
            Vec3::new(1.9, 64.9, 8.5),
        ));
        // Scheduled ticks and carts only run in lit, surrounded chunks near a
        // player.
        game.settings.simulation_distance = 4;
        game.settings.render_distance = 12;
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(8.5, 70.0, 12.5))
                .unwrap(),
        );
        for cx in -2..=2 {
            for cz in -2..=2 {
                if (cx, cz) != (0, 0) {
                    let mut chunk = sunrise_client::chunk::Chunk::empty(cx, cz);
                    chunk.lit = true;
                    game.world
                        .chunks
                        .insert((cx, cz), std::sync::Arc::new(chunk));
                    game.block_ticks
                        .add_empty_chunk(sunrise_core::position::ChunkPos::new(cx, cz))
                        .unwrap();
                }
            }
        }
        let mut center = (**game.world.chunks.get(&(0, 0)).unwrap()).clone();
        center.lit = true;
        game.world
            .chunks
            .insert((0, 0), std::sync::Arc::new(center));
        game.update_chunk_tickets();
        let mut edges = 0;
        let mut last = false;
        for _ in 0..200 {
            game.world.game_time += 1;
            game.tick_carts_now();
            game.tick_blocks().unwrap();
            let on = registry
                .state(game.world.block(0, 65, 8))
                .unwrap()
                .properties
                .get("powered")
                .is_some_and(|v| v == "true");
            if on != last {
                edges += 1;
                last = on;
            }
        }
        assert!(edges >= 4, "the plate changed {edges} times in 200 ticks");
    }
}

#[cfg(test)]
pub(in crate::game) mod cart_push_tests {
    use super::{CartKind, Minecart};
    use sunrise_client::{items::ItemStack, save::PendingContainerLoot};
    use sunrise_core::geometry::Vec3;

    /// Two chest carts pushed apart (`OldMinecartBehavior.pushAndPickupEntities`):
    /// the push runs on copies of the ticked cart, and the one still holding
    /// its mineshaft roll keeps the roll, so it is saved as its loot table.
    #[test]
    #[ignore = "daily"]
    fn a_pushed_chest_cart_keeps_its_pending_loot() {
        super::super::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            for x in 4..13 {
                for z in 4..13 {
                    game.world.set(x, 63, z, stone).unwrap();
                }
            }
            let items = vec![Some(ItemStack::new("minecraft:rail", 12)); 27];
            let loot = PendingContainerLoot {
                table: sunrise_core::nbt::Tag::String(
                    "minecraft:chests/abandoned_mineshaft".into(),
                ),
                seed: Some(sunrise_core::nbt::Tag::Long(42)),
            };
            let mut rolled = Minecart::new(1, CartKind::Chest, Vec3::new(8.3, 64.0, 8.5));
            rolled.items = items;
            rolled.loot = Some(loot.clone());
            game.carts.push(rolled);
            // Facing along X (yaw 0) and 0.4 apart on X: the old behaviour pushes.
            game.carts
                .push(Minecart::new(2, CartKind::Chest, Vec3::new(8.7, 64.0, 8.5)));
            for _ in 0..5 {
                game.tick_carts();
            }
            let cart = |id| game.carts.iter().find(|c| c.id == id).unwrap();
            assert!(
                cart(2).position.x - cart(1).position.x > 0.4,
                "the carts were pushed apart: {} {}",
                cart(1).position.x,
                cart(2).position.x
            );
            assert_eq!(cart(1).loot.as_ref(), Some(&loot));
            assert_eq!(cart(2).loot, None);
        });
    }

    /// `OldMinecartBehavior.pushAndPickupEntities` for carts: a moving rideable
    /// cart only pushes what `EntitySelector.pushableBy(cart)` allows, by its
    /// own team's collision rule and the target's; a still cart keeps vanilla's
    /// unfiltered minecart push.
    #[test]
    #[ignore = "daily"]
    fn a_moving_carts_team_rules_which_carts_it_pushes() {
        use crate::game::{Game, commands::Target};
        use sunrise_client::scoreboard::CollisionRule;

        /// Puts a cart on a team of its own, as `/team join` would.
        fn on_a_team(game: &mut Game<'_>, id: u32, rule: CollisionRule) {
            let name = game.holder_name(Target::Cart(id)).expect("a cart's UUID");
            let team = format!("cart-{id}");
            game.score.board.add_team(&team).collision_rule = rule;
            game.score.board.add_to_team(&name, &team);
        }
        /// Pushes with cart `pusher` moving at `velocity`, and returns the
        /// target's velocity afterwards.
        fn push_apart(game: &mut Game<'_>, pusher: usize, target: usize, velocity: Vec3) -> Vec3 {
            game.carts.update(target, |cart| cart.velocity = Vec3::ZERO);
            game.carts.update(pusher, |cart| cart.velocity = velocity);
            let mut index = super::CartIndex::default();
            game.push_from_cart(pusher, &mut index);
            game.carts[target].velocity
        }

        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        // Cart 2 moves east, into cart 1's box; yaw 0 faces east, along the push.
        game.carts.push(Minecart::new(
            1,
            CartKind::Rideable,
            Vec3::new(8.5, 64.0, 8.5),
        ));
        game.carts.push(Minecart::new(
            2,
            CartKind::Rideable,
            Vec3::new(8.1, 64.0, 8.5),
        ));
        let east = Vec3::new(0.2, 0.0, 0.0);
        // No teams: the moving cart pushes the still one.
        assert_ne!(
            push_apart(&mut game, 1, 0, east),
            Vec3::ZERO,
            "the moving cart pushes the still one"
        );
        // The target's `never` rule holds the push back.
        on_a_team(&mut game, 1, CollisionRule::Never);
        assert_eq!(
            push_apart(&mut game, 1, 0, east),
            Vec3::ZERO,
            "the target's collisionRule never keeps it off"
        );
        // The moving cart's own rule does too.
        game.score.board.remove_team("cart-1");
        on_a_team(&mut game, 2, CollisionRule::Never);
        assert_eq!(
            push_apart(&mut game, 1, 0, east),
            Vec3::ZERO,
            "the moving cart's collisionRule never keeps it off"
        );
        // Allies with `pushOwnTeam` do not push each other.
        game.score.board.remove_team("cart-2");
        on_a_team(&mut game, 1, CollisionRule::PushOwnTeam);
        let name = game.holder_name(Target::Cart(2)).expect("a cart's UUID");
        game.score.board.add_to_team(&name, "cart-1");
        assert_eq!(
            push_apart(&mut game, 1, 0, east),
            Vec3::ZERO,
            "pushOwnTeam keeps allies apart"
        );
        // A still cart is vanilla's else branch: no team filter at all.
        assert_ne!(
            push_apart(&mut game, 1, 0, Vec3::ZERO),
            Vec3::ZERO,
            "a still cart pushes the minecart in its box whatever the teams"
        );
    }

    /// `EntitySelector.pushableBy(cart)` for mobs: a moving rideable cart on a
    /// `never` team neither boards nor pushes the cow in its path.
    #[test]
    #[ignore = "daily"]
    fn a_moving_carts_team_keeps_it_from_boarding_a_cow() {
        use crate::game::commands::Target;
        use sunrise_client::{mobs::Mob, scoreboard::CollisionRule};

        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        let kind = game.res.mob_types().get("minecraft:cow").cloned().unwrap();
        let mut cart = Minecart::new(2, CartKind::Rideable, Vec3::new(8.5, 64.0, 8.5));
        cart.velocity = Vec3::new(0.2, 0.0, 0.0);
        game.carts.push(cart);
        let at = Vec3::new(8.5, 64.0, 8.5);
        game.mobs.push(Mob::new(7, kind, at, 0.0));
        let mut index = super::CartIndex::default();
        game.push_from_cart(0, &mut index);
        assert_eq!(
            game.mobs[0].extra.mount,
            Some(2),
            "the cow boards the empty moving cart"
        );
        game.mobs[0].extra.mount = None;
        game.mobs[0].velocity = Vec3::ZERO;
        // The cart's own `never` rule: no boarding and no push.
        let name = game.holder_name(Target::Cart(2)).expect("a cart's UUID");
        game.score.board.add_team("cart-2").collision_rule = CollisionRule::Never;
        game.score.board.add_to_team(&name, "cart-2");
        game.carts
            .update(0, |cart| cart.velocity = Vec3::new(0.2, 0.0, 0.0));
        let mut index = super::CartIndex::default();
        game.push_from_cart(0, &mut index);
        assert_eq!(
            game.mobs[0].extra.mount, None,
            "the cart's collisionRule never keeps the cow off"
        );
        assert_eq!(
            game.mobs[0].velocity,
            Vec3::ZERO,
            "and the cow is not pushed either"
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod container_drop_tests {
    use super::super::boats::Boat;
    use super::{CartHit, CartKind, Minecart};
    use std::sync::Arc;
    use sunrise_client::items::ItemStack;
    use sunrise_core::geometry::Vec3;
    use sunrise_core::random::{LegacyRandom, RandomSource};

    /// Advances `r` past the draws a spill takes from the level random, as
    /// vanilla's `Containers.dropContents` takes them: for each slot
    /// (`Containers.dropItemStack`) its spot, then for each pile the split
    /// size and the triangle motion. Every count here is at most 10, so a
    /// slot with items is a single pile.
    fn vanilla_spill_draws(r: &mut LegacyRandom, counts: &[u32]) {
        for &count in counts {
            for _ in 0..3 {
                r.next_double();
            }
            if count > 0 {
                let _ = r.next_int_bound(21);
                for _ in 0..3 {
                    r.triangle(0.0, 0.114_850_001_711_398_36);
                }
            }
        }
    }

    /// A chest boat or chest minecart broken with entity_drops on spills its
    /// slots in `remove` and then the emptied container again in
    /// `chestVehicleDestroyed`; the hull item takes the item entity's own
    /// random. The level random must end where vanilla's does.
    #[test]
    #[ignore = "daily"]
    fn a_broken_chest_vehicle_draws_the_level_random_like_vanilla() {
        let mut slots = vec![None; 27];
        slots[0] = Some(ItemStack::new("minecraft:diamond", 5));
        slots[4] = Some(ItemStack::new("minecraft:dirt", 8));
        let mut counts = vec![0; 27];
        counts[0] = 5;
        counts[4] = 8;
        for boat in [true, false] {
            crate::game::native_entity_refs::tests::with_game(|game| {
                assert!(
                    game.ui
                        .rules
                        .bool_at(sunrise_client::gamerules::vanilla::ENTITY_DROPS)
                );
                let at = Vec3::new(8.5, 64.0, 8.5);
                if boat {
                    let mut b = Boat::new(1, Arc::from("oak"), at, 0.0);
                    b.chest = true;
                    b.items = slots.clone();
                    game.boats.push(b);
                } else {
                    let mut c = Minecart::new(1, CartKind::Chest, at);
                    c.items = slots.clone();
                    game.carts.push(c);
                }
                game.world.random.set_seed(1234);
                if boat {
                    assert_eq!(game.hurt_boat_result(0, 100.0, false), Some(true));
                    assert!(game.boats.is_empty());
                } else {
                    assert!(game.hurt_cart_result(0, 100.0, false, false, CartHit::default()));
                    assert!(game.carts.is_empty());
                }
                let mut expected = LegacyRandom::new(1234);
                vanilla_spill_draws(&mut expected, &counts);
                vanilla_spill_draws(&mut expected, &[0; 27]);
                assert_eq!(
                    game.world.random.next_long(),
                    expected.next_long(),
                    "boat: {boat}"
                );
                // The two piles and the hull item.
                assert_eq!(game.dropped.len(), 3, "boat: {boat}");
            });
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod rejected_passenger_tests {
    use super::*;
    use crate::game::{commands::Target, native_entity_refs::tests::with_game};
    use sunrise_core::nbt::Tag;

    fn pig(uuid: i32, children: Vec<Tag>) -> Tag {
        Tag::compound([
            ("id", Tag::String("minecraft:pig".into())),
            ("UUID", Tag::IntArray(vec![0, 2175, 0, uuid])),
            (
                "Pos",
                Tag::List(vec![
                    Tag::Double(8.0f64.to_bits()),
                    Tag::Double(64.0f64.to_bits()),
                    Tag::Double(8.0f64.to_bits()),
                ]),
            ),
            ("Passengers", Tag::List(children)),
        ])
    }

    #[test]
    #[ignore = "daily"]
    fn rejected_cart_passenger_occupies_capacity_physics_pickup_and_interaction() {
        with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            let cart = 5000;
            game.carts.push(Minecart::new(
                cart,
                CartKind::Rideable,
                Vec3::new(8.0, 64.0, 8.0),
            ));
            game.load_mob(pig(1, vec![]), None).unwrap();
            let ghost = game
                .load_mob(pig(1, vec![pig(2, vec![])]), Some(cart))
                .unwrap();
            assert!(game.persist.off_level.contains_key(&ghost));
            let candidate = game.load_mob(pig(3, vec![]), None).unwrap();
            assert!(game.has_seated_passengers(cart));
            let mut copy = game.carts[0].clone();
            copy.velocity = Vec3::new(1.0, 0.0, 0.0);
            assert_eq!(
                game.natural_slowdown(&mut copy, game.has_seated_passengers(cart), 0.96)
                    .x,
                0.997
            );
            assert!(!game.start_riding_target(Target::Mob(candidate), Target::Cart(cart), false));
            game.carts
                .update(0, |c| c.velocity = Vec3::new(0.2, 0.0, 0.0));
            game.pickup_from_cart(0, &mut CartIndex::default());
            assert_eq!(game.mob_object(candidate).unwrap().extra.mount, None);
            assert!(!game.interact_cart(Some(InteractTarget {
                id: cart as i32,
                location: [0.0; 3]
            })));
            assert_eq!(game.riding, None);
            let mut camel = pig(4, vec![]);
            if let Tag::Compound(c) = &mut camel {
                c.insert("id".into(), Tag::String("minecraft:camel".into()));
            }
            let camel = game.load_mob(camel, None).unwrap();
            game.load_mob(pig(1, vec![]), Some(camel)).unwrap();
            game.load_mob(pig(1, vec![]), Some(camel)).unwrap();
            game.load_mob(pig(1, vec![]), Some(camel)).unwrap();
            assert_eq!(game.seated_passengers(camel).len(), 3);
            assert!(!game.start_riding_target(Target::Mob(candidate), Target::Mob(camel), false));
            let mut active = game.carts[0].clone();
            active.position = Vec3::new(40.0, 70.0, 40.0);
            active.yaw = 90.0;
            active.flipped = true;
            game.activate_cart(&mut active, true);
            let rider = game.mob_object(ghost).unwrap();
            assert_eq!(rider.extra.mount, None);
            assert_eq!(rider.position, Vec3::new(40.0, 70.0 + HEIGHT, 40.0));
            assert_eq!(rider.previous, rider.position);
            let leaf = game.mob_passenger_ids(ghost)[0];
            assert_eq!(game.mob_object(leaf).unwrap().position.x, 40.0);
            assert!(game.mob_object(leaf).unwrap().position.y >= 70.0);
            assert_eq!(game.carts[0].position, Vec3::new(8.0, 64.0, 8.0));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn cart_destruction_ejects_rejected_rider_and_snaps_frozen_descendant() {
        with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            game.carts.push(Minecart::new(
                5000,
                CartKind::Rideable,
                Vec3::new(8.0, 70.0, 8.0),
            ));
            game.load_mob(pig(1, vec![]), None).unwrap();
            let ghost = game
                .load_mob(pig(1, vec![pig(2, vec![])]), Some(5000))
                .unwrap();
            let leaf = game.mob_passenger_ids(ghost)[0];
            assert!(game.mobs_below_off_level().contains(&leaf));
            assert!(game.kill_vehicle_object(crate::game::commands::Target::Cart(5000)));
            let rider = game.mob_object(ghost).unwrap();
            assert_eq!(rider.extra.mount, None);
            assert!(!rider.extra.passenger);
            assert_eq!(rider.position.y, 70.0);
            assert_eq!(rider.position, rider.previous);
            assert_eq!(game.mob_object(leaf).unwrap().extra.mount, Some(ghost));
            assert!(game.mob_object(leaf).unwrap().position.y >= 70.0);
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod hook_mutation_regressions {
    use super::*;
    use std::sync::{Arc, Mutex};
    use sunrise_client::mobs::Mob;

    struct MutateRider {
        discard: bool,
        heard: Arc<Mutex<Vec<(u32, bool)>>>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for MutateRider {
        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            self.heard
                .lock()
                .unwrap()
                .push((event.entity, event.mounting));
            if !event.mounting && event.entity == 72 {
                if self.discard {
                    assert!(host.entity(72).unwrap().discard(host));
                } else {
                    let later = host.entity(73).unwrap();
                    let replacement = host.entity(74).unwrap();
                    assert!(later.start_riding(host, replacement, true).is_ok());
                }
            }
            mod_api::Flow::Continue
        }
    }
    #[test]
    #[ignore = "daily"]
    fn activator_preserves_a_later_passengers_callback_reparenting() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "mutate_rider",
            Box::new(MutateRider {
                discard: false,
                heard: heard.clone(),
            }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.carts
                    .push(Minecart::new(71, CartKind::Rideable, Vec3::ZERO));
                let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                for id in [72, 73, 74] {
                    let mut mob = Mob::new(id, kind.clone(), Vec3::ZERO, 0.0);
                    mob.extra.mount = (id != 74).then_some(71);
                    mob.extra.vehicle_order = i64::from(id);
                    game.mobs.push(mob);
                }
                let mut active = game.carts[0].clone();
                game.activate_cart(&mut active, true);
                assert_eq!(game.mob_object(73).unwrap().extra.mount, Some(74));
                assert_eq!(
                    heard
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|&&(id, mounting)| id == 73 && !mounting)
                        .count(),
                    1,
                    "only the callback's deliberate reparenting dismounts this rider"
                );
            });
        });
    }
    #[test]
    #[ignore = "daily"]
    fn activator_does_not_snap_a_rider_discarded_by_its_callback() {
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "mutate_rider",
            Box::new(MutateRider {
                discard: true,
                heard: Arc::default(),
            }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.carts
                    .push(Minecart::new(71, CartKind::Rideable, Vec3::ZERO));
                let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                let original = Vec3::new(2.0, 60.0, 3.0);
                let mut rider = Mob::new(72, kind.clone(), original, 0.0);
                rider.extra.mount = Some(71);
                game.mobs.push(rider);
                let mut child = Mob::new(73, kind, original, 0.0);
                child.extra.mount = Some(72);
                game.mobs.push(child);
                let mut active = game.carts[0].clone();
                active.position = Vec3::new(40.0, 70.0, 40.0);
                game.activate_cart(&mut active, true);
                assert!(game.mob_object(72).unwrap().removed);
                assert_eq!(game.mob_object(72).unwrap().position, original);
                assert_eq!(game.mob_object(73).unwrap().position, original);
            });
        });
    }
    struct DiscardDamagedVehicle(Arc<Mutex<usize>>);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for DiscardDamagedVehicle {
        fn vehicle_damage(
            &self,
            host: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::VehicleDamageEvent<'_>,
        ) -> mod_api::Flow {
            assert!(host.entity(71).unwrap().discard(host));
            mod_api::Flow::Continue
        }
        fn vehicle_destroy(
            &self,
            _: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::VehicleDestroyEvent<'_>,
        ) {
            *self.0.lock().unwrap() += 1;
        }
    }
    #[test]
    #[ignore = "daily"]
    fn a_damage_callback_discard_does_not_also_destroy_or_drop_the_vehicle() {
        for cart in [false, true] {
            let destroyed = Arc::new(Mutex::new(0));
            let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
                "discard_damaged",
                Box::new(DiscardDamagedVehicle(destroyed.clone())),
            )]
            .into();
            mod_platform::testing::with_mods(mods, || {
                super::super::native_entity_refs::tests::with_game(|game| {
                    game.options.dedicated = Some(Vec::new());
                    game.ui
                        .rules
                        .set("entity_drops", sunrise_client::gamerules::Value::Bool(true));
                    if cart {
                        game.carts
                            .push(Minecart::new(71, CartKind::Rideable, Vec3::ZERO));
                        game.hurt_cart(
                            0,
                            5.0,
                            false,
                            false,
                            crate::game::minecarts::CartHit::default(),
                        );
                    } else {
                        game.boats.push(super::super::boats::Boat::new(
                            71,
                            "oak".into(),
                            Vec3::ZERO,
                            0.0,
                        ));
                        game.hurt_boat(0, 5.0, false);
                    }
                    assert_eq!(*destroyed.lock().unwrap(), 0);
                    assert!(
                        game.dropped.is_empty(),
                        "discard must not create the vehicle item"
                    );
                    game.native_removals_flush();
                    assert_eq!(*destroyed.lock().unwrap(), 0);
                    assert!(game.dropped.is_empty());
                    assert!(game.entity(71).is_none());
                });
            });
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tnt_cart_power_tests {
    use super::*;

    #[test]
    fn tnt_cart_power_round_trips_live_state_and_clears_stale_defaults() {
        use sunrise_core::nbt::{Tag, parse_snbt};
        let raw = parse_snbt("{Pos:[1d,200d,1d],explosion_power:200f,explosion_speed_factor:-2d}")
            .unwrap()
            .0;
        let mut cart = Minecart::from_nbt(&raw, 1, CartKind::Tnt).unwrap();
        assert_eq!(cart.explosion_power, 128.0);
        assert_eq!(cart.explosion_speed_factor, 0.0);
        let saved = cart.to_nbt([0; 4], Some(&raw), &Default::default());
        assert_eq!(
            saved.get("explosion_power"),
            Some(&Tag::Float(128.0f32.to_bits()))
        );
        assert_eq!(
            saved.get("explosion_speed_factor"),
            Some(&Tag::Float(0.0f32.to_bits()))
        );
        cart.explosion_power = DEFAULT_EXPLOSION_POWER;
        cart.explosion_speed_factor = DEFAULT_EXPLOSION_SPEED_FACTOR;
        let saved = cart.to_nbt([0; 4], Some(&raw), &Default::default());
        assert!(saved.get("explosion_power").is_none());
        assert!(saved.get("explosion_speed_factor").is_none());
        let restored = Minecart::from_nbt(&saved, 2, CartKind::Tnt).unwrap();
        assert_eq!(restored.explosion_power, DEFAULT_EXPLOSION_POWER);
        assert_eq!(
            restored.explosion_speed_factor,
            DEFAULT_EXPLOSION_SPEED_FACTOR
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod cart_cost_bench {
    use super::{CartKind, Minecart};
    use crate::game::game_tests::tests::server_game;
    use sunrise_core::geometry::Vec3;

    /// Times `tick_carts` with many parked carts and some moving ones:
    /// `CARTS_PARKED` (default 600) carts each on its own rail piece, and
    /// `CARTS_MOVING` (default 100) on powered tracks. Prints the ms per tick.
    #[test]
    #[ignore = "a timing, needs prepared resources in SUNRISE_TEST_RESOURCES"]
    fn tick_carts_with_many_parked_carts() {
        let n = |key: &str, default: usize| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let (parked, moving) = (n("CARTS_PARKED", 600), n("CARTS_MOVING", 100));
        let mut resources = None;
        let mut game = server_game(&mut resources);
        game.settings.simulation_distance = 6;
        game.settings.render_distance = 12;
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(8.5, 80.0, 12.5))
                .unwrap(),
        );
        for cx in -3..=3 {
            for cz in -3..=3 {
                if (cx, cz) != (0, 0) {
                    let mut chunk = sunrise_client::chunk::Chunk::empty(cx, cz);
                    chunk.lit = true;
                    game.world
                        .chunks
                        .insert((cx, cz), std::sync::Arc::new(chunk));
                    game.block_ticks
                        .add_empty_chunk(sunrise_core::position::ChunkPos::new(cx, cz))
                        .unwrap();
                }
            }
        }
        game.update_chunk_tickets();
        let registry = game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let rail = registry.default_state("minecraft:rail").unwrap();
        let powered = registry.default_state("minecraft:powered_rail").unwrap();
        let with = |base: sunrise_world::registry::BlockStateId, shape: &str, on: bool| {
            let mut s = registry.state(base).unwrap().clone();
            s.properties.insert("shape".into(), shape.into());
            if on {
                s.properties.insert("powered".into(), "true".into());
            }
            registry.id(&s).unwrap()
        };
        let (rail_ns, rail_ew) = (
            with(rail, "north_south", false),
            with(powered, "east_west", true),
        );
        for x in -30..46 {
            for z in -30..46 {
                game.world.set(x, 63, z, stone).unwrap();
                game.world.set(x, 69, z, stone).unwrap();
            }
        }
        let mut id = 1;
        'park: for x in (-28..44).step_by(2) {
            for z in (-28..44).step_by(2) {
                if id as usize > parked {
                    break 'park;
                }
                game.world.set(x, 64, z, rail_ns).unwrap();
                let at = Vec3::new(f64::from(x) + 0.5, 64.0, f64::from(z) + 0.5);
                game.carts.push(Minecart::new(id, CartKind::Rideable, at));
                id += 1;
            }
        }
        for lane in 0..10 {
            let z = -28 + lane * 6;
            for x in -30..46 {
                game.world.set(x, 70, z, rail_ew).unwrap();
            }
            for k in 0..moving.div_ceil(10) {
                if id as usize > parked + moving {
                    break;
                }
                let at = Vec3::new(-29.5 + f64::from(k as i32) * 3.0, 70.0, f64::from(z) + 0.5);
                let mut c = Minecart::new(id, CartKind::Rideable, at);
                c.velocity = Vec3::new(0.3, 0.0, 0.0);
                game.carts.push(c);
                id += 1;
            }
        }
        let chicken = game
            .res
            .mob_types()
            .get("minecraft:chicken")
            .unwrap()
            .clone();
        for k in 0..n("MOBS", 3000) {
            let (x, z) = (
                -30.0 + (k % 76) as f64 + 0.5,
                -30.0 + ((k / 76) % 76) as f64 + 0.5,
            );
            game.mobs.push(sunrise_client::mobs::Mob::new(
                1000 + k as u32,
                chicken.clone(),
                Vec3::new(x, 75.0, z),
                0.0,
            ));
        }
        let total = game.carts.len();
        let (warm, ticks) = (5, n("TICKS", 40));
        let mut spent = std::time::Duration::ZERO;
        for t in 0..warm + ticks {
            game.world.game_time += 1;
            // Mobs move every tick on a live server: the list is written.
            if !game.mobs.is_empty() {
                let _ = &mut game.mobs[0];
            }
            let start = std::time::Instant::now();
            game.tick_carts();
            if t >= warm {
                spent += start.elapsed();
            }
        }
        eprintln!(
            "tick_carts: {total} carts ({parked} parked, {moving} moving): {:.3} ms/tick",
            spent.as_secs_f64() * 1e3 / ticks as f64
        );
    }
}

#[cfg(test)]
#[path = "minecarts/minecart_riders_tests.rs"]
pub(in crate::game) mod minecart_riders_tests;
