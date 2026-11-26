//! Boats and rafts (`AbstractBoat`): water/land/air status, floating,
//! paddling controls, collisions, riding (`positionRider`, `clampRotation`,
//! dismounting), `BoatItem` placement, `VehicleEntity.hurtServer` breaking,
//! saving and `AbstractBoatRenderer` with animated paddles.
pub(in crate::game) mod below_world;
#[cfg(test)]
pub(in crate::game) mod friction_tests;
#[cfg(test)]
pub(in crate::game) mod lifetime_tests;
#[cfg(test)]
pub(in crate::game) mod paddle_tests;

use super::{Game, InteractTarget};
use crate::game::boats_common::{
    BUBBLE_DOWN_EVENT, BUBBLE_TIME, BUBBLE_UP_EVENT, Boat, WIDTH, reached_cells,
};
use std::sync::Arc;
use sunrise_core::nbt::Tag;
use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
    random::RandomSource,
};
use sunrise_items::{blocks::Fluid, items::ItemStack};
use sunrise_sim::{
    mobs::{
        leash::{Holder, Pending},
        riders,
    },
    motion::RiddenBoat,
};

/// The snowflake `PowderSnowBlock.entityInside` sends for an entity whose known movement is
/// `known`: none unless it moved horizontally and a coin falls so, then `Mth.randomBetween(-1, 1)`
/// over 12 sideways and 0.05 up.
pub(in crate::game::server_side) fn snowflake_velocity(
    known: Vec3,
    random: &mut impl RandomSource,
) -> Option<[f64; 3]> {
    if (known.x == 0.0 && known.z == 0.0) || !random.next_boolean() {
        return None;
    }
    let xa = (random.next_float() * 2.0 - 1.0) * 0.083_333_336_f32;
    let za = (random.next_float() * 2.0 - 1.0) * 0.083_333_336_f32;
    Some([f64::from(xa), f64::from(0.05_f32), f64::from(za)])
}
/// `Entity.load`'s `Fire` (a short; 0 when absent).
pub(in crate::game::server_side) fn saved_fire(tag: &Tag) -> i32 {
    i32::from(sunrise_sim::save::number(tag.get("Fire")).unwrap_or(0) as i16)
}
impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn record_boat_lid_movement(&mut self, i: usize, delta: Vec3) {
        self.record_driven_boat_lid_movement(i, delta);
    }

    /// `Entity.lavaHurt`'s sound once the hurt went through.
    pub(in crate::game) fn lava_hurt_sound(&mut self, at: [f64; 3]) {
        let pitch = 2.0 + self.world.random.next_float() * 0.4;
        self.server_play_sound("minecraft:entity.generic.burn", at, 0.4, pitch);
    }

    pub(in crate::game) fn server_tick_boat_leash(&mut self, b: &mut Boat) {
        self.tick_boat_leash(b);
    }

    pub(in crate::game) fn server_boat_can_simulate_movement(&self, id: u32) -> bool {
        self.side.lan.as_ref().is_none_or(|host| {
            !host
                .hub
                .guests
                .iter()
                .any(|guest| guest.vehicle == Some(id))
        })
    }
    /// Every boat but the ridden one (which ticks with its rider).
    pub(in crate::game) fn tick_boats(&mut self) {
        self.vehicles_ticking(true);
        self.tick_boats_now();
        self.vehicles_ticking(false);
    }
    pub(in crate::game::server_side) fn tick_boats_now(&mut self) {
        // ServerLevel.tick: only entities in entity-ticking chunks tick.
        let ticking = self.entity_ticking_area();
        let mut round = self.native_tick_round();
        // A server player's boat is driven by its client (lan_vehicles.rs).
        let driven = self.guest_vehicles();
        // EntityTickEvent.Post comes once AbstractBoat.tick is over, after the
        // passes of `boat_server_passes` (lava, ejection, bubbles, contacts),
        // and before the next boat ticks, as `ServerLevel.tickNonPassenger`.
        // A pass (lava breaks a boat) or a mod's hook may remove boats: the next one is found

        // after this one by its id, so none is skipped or ticked twice.
        let mut i = 0;
        while i < self.boats.len() {
            let id = self.boats[i].id;

            'boat: {
                let p = self.boats[i].position;
                if Some(id) == self.riding
                    || driven.contains(&id)
                    || !ticking(p.x.floor() as i32, p.z.floor() as i32)
                {
                    // Its rider's client moves it, but the server still wears
                    // off its damage and then burns it.
                    if ticking(p.x.floor() as i32, p.z.floor() as i32) {
                        if driven.contains(&id) {
                            self.boats.update(i, |b| b.wear_off_hurt());
                        }
                        if self.boat_fire_tick(i) || self.boat_below_world(i) {
                            break 'boat;
                        }
                    }
                    // The lead still ticks too.
                    if ticking(p.x.floor() as i32, p.z.floor() as i32) {
                        self.with_entity_held_out(id, |game| {
                            let mut b = game.boats[i].clone();
                            game.tick_boat_leash(&mut b);
                            game.store_boat_back(i, b);
                        });
                    }
                    self.boat_server_passes(id, &driven, &ticking, &round);
                    break 'boat;
                }
                let kind = if round.listening() {
                    self.boats[i].entity_type()
                } else {
                    String::new()
                };
                if !self.native_tick_pre_one(&mut round, id, &kind) {
                    self.boat_server_passes(id, &driven, &ticking, &round);
                    break 'boat;
                }
                // The hook may have removed boats.
                let Some(i) = Some(i)
                    .filter(|&i| self.boats.get(i).is_some_and(|b| b.id == id))
                    .or_else(|| self.boats.iter().position(|b| b.id == id))
                else {
                    break 'boat;
                };
                self.boats.update(i, |b| b.wear_off_hurt());
                // Entity.baseTick burns it before it moves; the fire may break it.
                if !self.boat_fire_tick(i) && !self.boat_below_world(i) {
                    let section_step = self.with_entity_held_out(id, |game| {
                        let b = game.boats[i].clone();
                        let from = b.position;
                        let b = game.tick_boat(b, false);
                        let to = b.position;
                        game.store_boat_back(i, b).then_some((from, to))
                    });
                    if let Some((from, to)) = section_step {
                        self.entity_section_step(id, [from.x, from.y, from.z], [to.x, to.y, to.z]);
                    }
                    self.boat_server_passes(id, &driven, &ticking, &round);
                }
                self.native_tick_post_one(&round, id, &kind);
            }
            i = match self.boats.get(i) {
                Some(b) if b.id == id => i + 1,
                _ => self
                    .boats
                    .iter()
                    .position(|b| b.id == id)
                    .map_or(i, |k| k + 1),
            };
        }
    }
    /// The rest of `AbstractBoat.tick` on the server, for every boat, the
    /// client-driven ones too: `applyEffectsFromBlocks` (lava breaks a boat
    /// at once), the passengers of one under water too long get out, and it
    /// picks up or pushes what it touches.
    pub(in crate::game::server_side) fn boat_server_passes(
        &mut self,
        id: u32,
        driven: &[u32],
        ticking: &impl Fn(i32, i32) -> bool,
        round: &super::native_ticks::TickRound,
    ) {
        let Some(i) = self.boats.iter().position(|b| b.id == id) else {
            return;
        };
        let p = self.boats[i].position;
        if !ticking(p.x.floor() as i32, p.z.floor() as i32) || round.canceled(id) {
            return;
        }
        if Some(id) != self.riding && self.boat_lava_tick(i) {
            return;
        }
        if self.boat_fire_passes(i) {
            return;
        }
        self.boat_powder_snow_snowflakes(i);
        // `AbstractBoat.tick` counts submersion on the server even when
        // its controlling client owns the boat's movement. Its
        // `applyEffectsFromBlocks` still runs here: a cobweb or powder snow
        // resets the fall of the boat the guest drives.
        if driven.contains(&id) {
            let mut b = self.boats[i].clone();
            self.boat_step_on(&mut b);
            self.boat_stuck_in_blocks(&mut b);
            self.boats.update(i, |boat| {
                boat.stuck = b.stuck;
                boat.fall_distance = b.fall_distance;
                boat.velocity = b.velocity;
                boat.fire_ticks = b.fire_ticks;
            });
            self.refresh_boat_status(i);
        }
        if self.boats[i].out_of_control >= 60.0 {
            self.eject_boat_riders(i);
        }
        self.boat_bubble_tick(i);
        self.boat_contacts(i);
    }
    /// `AbstractBoat.onAboveBubbleColumn`: on the server (this window
    /// simulates the boat) the column is remembered, the countdown starts and
    /// the splash's game event goes to the listeners (its sound and particle
    /// are the client's).
    pub(in crate::game::server_side) fn boat_above_bubble_column(
        &mut self,
        i: usize,
        drag_down: bool,
    ) {
        self.boats.update(i, |b| {
            b.above_bubble_column = true;
            b.bubble_down = drag_down;
            if b.bubble_time == 0 {
                b.bubble_time = BUBBLE_TIME;
            }
        });
        if !self.boat_splash_roll(i) {
            return;
        }
        let (id, p) = (self.boats[i].id, self.boats[i].position);
        // Entity.gameEvent(SPLASH, getControllingPassenger()): the window's
        // own player when it drives the boat.
        let ctx = if self.riding == Some(id) && self.player.is_some() {
            self.player_context(None)
        } else {
            self.who_context(None, None)
        };
        self.post_game_event("minecraft:splash", [p.x, p.y, p.z], ctx);
    }
    /// `Entity.ejectPassengers`: the mobs get out here, this window's player
    /// gets out, and a server's player is asked to
    /// (`guest_vehicle_tick` lets it out through its client).
    pub(in crate::game::server_side) fn eject_boat_riders(&mut self, i: usize) {
        self.eject_boat_mobs(i);
        let id = self.boats[i].id;
        if self.riding == Some(id) && self.player.is_some() {
            self.dismount(i);
        } else if self.guest_rides(id) {
            self.boats.update(i, |b| b.eject_riders = true);
        }
    }
    /// `AbstractBoat.tickBubbleColumn`: on the server, a boat that stays over
    /// a column for `BUBBLE_TIME` ticks is thrown up or pulled down (a down
    /// column lets the riders out first), and every client is told.
    pub(in crate::game::server_side) fn tick_boat_bubble_column(&mut self, i: usize) {
        if let Some(down) = self.boats.update(i, Boat::tick_bubble_countdown) {
            let (id, p) = (self.boats[i].id, self.boats[i].position);
            let event = if down {
                BUBBLE_DOWN_EVENT
            } else {
                BUBBLE_UP_EVENT
            };
            if down {
                self.eject_boat_riders(i);
            }
            self.lan_entity_event(id as i32, event, [p.x, p.y, p.z]);
            self.handle_bubble_column_effect(i, down);
        }
    }
    /// `AbstractBoat.tick`: the end of it on the server for boat `i`, the two
    /// `applyEffectsFromBlocks` that reach bubble columns (the lava is
    /// `boat_lava_tick`'s), then `tickBubbleColumn`; and the wobble when this
    /// window is its own client.
    pub(in crate::game::server_side) fn boat_bubble_tick(&mut self, i: usize) {
        let id = self.boats[i].id;
        // An ejection nobody took (its player left the boat first) is void.
        if !self.guest_rides(id) {
            self.boats.update(i, |b| b.eject_riders = false);
        }
        let touches = self.boat_bubble_columns(i);
        // AbstractBoat.tick calls applyEffectsFromBlocks twice.
        for pass in 0..2 {
            if pass == 1 {
                self.boat_second_step(i);
            }
            for &column in &touches {
                if column.surface {
                    self.boat_above_bubble_column(i, column.drag);
                } else {
                    self.boat_inside_bubble_column(i, column);
                }
            }
        }
        self.tick_boat_bubble_column(i);
        if self.options.execution_role.is_client() {
            self.boats.update(i, Boat::tick_bubble_wobble);
        }
    }
    /// `PowderSnowBlock.entityInside` for a boat: the powder snow cells its box, and the box it
    /// had before its move, reach into (`Entity.checkInsideBlocks` along the move).
    pub(in crate::game::server_side) fn boat_powder_snow_cells(&self, b: &Boat) -> Vec<[i32; 3]> {
        self.boat_reached_cells(b, |p| self.is_powder_snow(p))
    }
    /// The cells a boat's box, and the box it had before its move, reach into
    /// (`Entity.checkInsideBlocks` along the move), in x, y, z order.
    pub(in crate::game::server_side) fn boat_reached_cells(
        &self,
        b: &Boat,
        is: impl Fn([i32; 3]) -> bool,
    ) -> Vec<[i32; 3]> {
        let mut cells = reached_cells(b.bounds(), &is);
        if b.previous != b.position {
            for p in reached_cells(b.bounds_at(b.previous), &is) {
                if !cells.contains(&p) {
                    cells.push(p);
                }
            }
        }
        cells
    }
    /// `BaseFireBlock.fireDamage` of a fire block at `p`.
    pub(in crate::game::server_side) fn fire_damage_at(&self, p: [i32; 3]) -> Option<f32> {
        match &*self
            .res
            .facts()
            .get(self.world.block(p[0], p[1], p[2]))
            .name
        {
            "minecraft:fire" => Some(1.0),
            "minecraft:soul_fire" => Some(2.0),
            _ => None,
        }
    }
    /// `Entity.baseTick` on the server: a burning boat takes 1 fire damage
    /// every 20 ticks (none in lava) and burns a tick down. True when the fire
    /// broke it.
    pub(in crate::game::server_side) fn boat_fire_tick(&mut self, i: usize) -> bool {
        if self.boats[i].fire_ticks <= 0 {
            return false;
        }
        let burn = self.boats[i].fire_ticks % 20 == 0 && !self.in_lava(self.boats[i].bounds());
        self.boats.update(i, |b| b.fire_ticks -= 1);
        burn && self.burn_boat(i, 1.0, "minecraft:on_fire")
    }
    /// The fire of the two `applyEffectsFromBlocks` in `AbstractBoat.tick`.
    /// `Entity.checkInsideBlocks` meets the blocks of the box the boat started
    /// its move in first, then the ones it reached, each block once; in each
    /// step fire blocks light it (`BaseFireBlock.fireIgnite`, a cooldown tick
    /// at a time) and each burns it, then a burning boat melts the powder snow
    /// (with `mob_griefing`) and water and powder snow put it out. Rain puts
    /// it out too, and a boat neither burning nor lit has its cooldown
    /// cleared. True when the fire broke it.
    pub(in crate::game::server_side) fn boat_fire_passes(&mut self, i: usize) -> bool {
        let b = &self.boats[i];
        let mut steps = vec![b.bounds_at(b.previous)];
        if b.previous != b.position {
            steps.push(b.bounds());
        }
        // `Entity.collidedWithFluid` sweeps the box along the whole move:
        // water reaches it down to the lower of its two bottoms.
        let low = b.previous.y.min(b.position.y);
        let (at, top) = (b.position, b.bounds().max_position().y);
        let rain = self.in_rain(at, top);
        for _ in 0..2 {
            let before = self.boats[i].fire_ticks;
            let mut in_snow = false;
            let mut seen = Vec::new();
            for &bounds in &steps {
                let cells = reached_cells(bounds, |p| !seen.contains(&p));
                seen.extend_from_slice(&cells);
                let fires: Vec<f32> = cells
                    .iter()
                    .filter_map(|&p| self.fire_damage_at(p))
                    .collect();
                if !fires.is_empty() {
                    self.boats.update(i, |b| {
                        if b.fire_ticks < 0 {
                            b.fire_ticks += 1;
                        }
                        if b.fire_ticks >= 0 {
                            // igniteForSeconds(8)
                            b.fire_ticks = b.fire_ticks.max(160);
                        }
                    });
                    for &damage in &fires {
                        if self.burn_boat(i, damage, "minecraft:in_fire") {
                            return true;
                        }
                    }
                }
                let water = cells.iter().any(|&p| self.water_reaches(p, low));
                let snow: Vec<[i32; 3]> = cells
                    .iter()
                    .copied()
                    .filter(|&p| self.is_powder_snow(p))
                    .collect();
                in_snow |= !snow.is_empty();
                if self.boats[i].fire_ticks > 0
                    && self
                        .ui
                        .rules
                        .bool_at(sunrise_game_data::gamerules::vanilla::MOB_GRIEFING)
                {
                    for &cell in &snow {
                        self.powder_snow_melt(BlockPos::new(cell[0], cell[1], cell[2]));
                    }
                }
                if water || !snow.is_empty() {
                    // clearFire
                    self.boats.update(i, |b| b.fire_ticks = b.fire_ticks.min(0));
                }
            }
            let now = self.boats.update(i, |b| {
                if in_snow {
                    b.ticks_frozen = (b.ticks_frozen + 1)
                        .min(sunrise_sim::motion::PlayerMotion::TICKS_REQUIRED_TO_FREEZE);
                }
                if rain {
                    b.fire_ticks = b.fire_ticks.min(0);
                }
                let now = b.fire_ticks;
                if now <= 0 && now <= before {
                    b.fire_ticks = 0;
                }
                now
            });
            if before > 0 && now <= 0 {
                self.extinguish_sound([at.x, at.y, at.z]);
            }
        }
        false
    }
    /// Whether the water at cell `p` reaches a box whose bottom is `low`
    /// (`Entity.collidedWithFluid`).
    pub(in crate::game::server_side) fn water_reaches(&self, p: [i32; 3], low: f64) -> bool {
        self.fluid_height_of(p[0], p[1], p[2], Some(Fluid::Water))
            .is_some_and(|(height, _)| low + 1.0e-5 < f64::from(p[1]) + f64::from(height))
    }
    /// `VehicleEntity.hurtServer` for fire (`inFire`, `onFire`), which an
    /// `Invulnerable` boat ignores. True when the boat broke.
    pub(in crate::game::server_side) fn burn_boat(
        &mut self,
        i: usize,
        damage: f32,
        damage_type: &str,
    ) -> bool {
        let id = self.boats[i].id;
        let invulnerable = self.saved_refuses_damage(id, damage_type, false);
        let kind = self.boats[i].entity_type();
        let source = sunrise_sim::health::HurtSource {
            damage_type,
            causing: 0,
            direct: 0,
            position: None,
        };
        !self.native_nonliving_invulnerable(id, &kind, source, invulnerable)
            && self.hurt_boat(i, damage, false)
    }
    /// `PowderSnowBlock.entityInside`'s `ServerLevel.sendParticles`: a boat that moved
    /// horizontally in powder snow throws snowflakes the players around see. `AbstractBoat.tick`
    /// calls `applyEffectsFromBlocks` twice.
    pub(in crate::game::server_side) fn boat_powder_snow_snowflakes(&mut self, i: usize) {
        let b = &self.boats[i];
        let cells = self.boat_powder_snow_cells(b);
        if cells.is_empty() {
            return;
        }
        let known = b.position.subtract(b.previous);
        let (x, z) = (b.position.x, b.position.z);
        for _ in 0..2 {
            for cell in &cells {
                if let Some(velocity) = snowflake_velocity(known, &mut self.world.random) {
                    self.lan_snowflake([x, f64::from(cell[1] + 1), z], velocity);
                }
            }
        }
    }
    /// `LavaFluid.entityInside` (`lavaHurt`) from the two
    /// `applyEffectsFromBlocks` calls in `AbstractBoat.tick`: twice 40
    /// damage, so lava breaks a boat on its first tick inside, and the fire
    /// `lavaIgnite` lights never gets to burn it. True when the boat broke.
    pub(in crate::game::server_side) fn boat_lava_tick(&mut self, i: usize) -> bool {
        if !self.in_lava(self.boats[i].bounds()) {
            return false;
        }
        for _ in 0..2 {
            let p = self.boats[i].position;
            let id = self.boats[i].id;
            let kind = self.boats[i].entity_type();
            let vanilla = self.saved_refuses_damage(id, "minecraft:lava", false);
            let broke =
                !self.refuses_lava_hurt(id, &kind, vanilla) && self.hurt_boat(i, 4.0, false);
            self.lava_hurt_sound([p.x, p.y, p.z]);
            if broke {
                return true;
            }
        }
        false
    }
    /// Whether a vehicle's box, deflated by 1e-5, reaches into lava below its
    /// surface (`Entity.collidedWithFluid`).
    pub(in crate::game) fn in_lava(&self, bb: Aabb) -> bool {
        let (lo, hi) = (bb.min_position(), bb.max_position());
        let e = 1.0e-5;
        for x in (lo.x + e).floor() as i32..=(hi.x - e).floor() as i32 {
            for y in (lo.y + e).floor() as i32..=(hi.y - e).floor() as i32 {
                for z in (lo.z + e).floor() as i32..=(hi.z - e).floor() as i32 {
                    if let Some((height, _)) = self.fluid_height_of(x, y, z, Some(Fluid::Lava))
                        && lo.y + e < f64::from(y) + f64::from(height)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
    /// `AbstractBoat.modifyPassengerFluidInteractionBox`: the ridden boat's box
    /// and whether it is underwater, for `Entity.getFluidInteractionBox`. A boat
    /// keeps only the part of the passenger's box above its own top, so a boat
    /// bobbing on water never sinks the passenger's eyes or body.
    pub(in crate::game::server_side) fn riding_boat_fluid_box(&self) -> Option<RiddenBoat> {
        self.boats
            .iter()
            .find(|b| Some(b.id) == self.riding)
            .map(|b| RiddenBoat {
                under_water: b.under_water(),
                max_y: b.bounds().max_position().y,
            })
    }
    /// `getDismountLocationForPassenger`: in front of the player if there is
    /// floor there, else on top of the boat.
    pub(in crate::game::server_side) fn dismount(&mut self, i: usize) {
        let id = self.boats[i].id;
        if self.riding == Some(id) && !self.native_may_leave(id) {
            return;
        }
        self.riding = None;
        // `AbstractBoat.tick`'s non-authoritative branch: the copy of a boat
        // that no player of this side drives drops its cached motion
        // (`setDeltaMovement(Vec3.ZERO)`, `deltaRotation = 0`) and is not
        // moved, so a boat is at a standstill when its rider gets out instead
        // of coasting on the velocity of the ride that just ended.
        self.boats.update(i, |b| b.velocity = Vec3::ZERO);
        self.boats.update(i, |b| b.delta_rotation = 0.0);
        if self.dismount_in_portal(self.boats[i].position) {
            return;
        }
        let target = self.boat_player_dismount_target(i, self.yaw);
        if let Some(p) = self.player.as_mut() {
            p.position = target;
            p.velocity = Vec3::ZERO;
        }
    }
    /// Where the boat at `i` lets a player facing `yaw` off.
    pub(in crate::game::server_side) fn boat_player_dismount_target(
        &self,
        i: usize,
        yaw: f32,
    ) -> Vec3 {
        // The player's bbWidth, a float.
        self.boat_dismount_target(&self.boats[i], f64::from(0.6_f32), yaw)
    }
    /// `AbstractBoat.getDismountLocationForPassenger` for a passenger `width` wide (its float `getBbWidth()`,
    /// widened) facing `yaw`: the escape vector from a box `getBbWidth() * Mth.SQRT_OF_TWO` wide, in float.
    pub(in crate::game) fn boat_dismount_target(&self, b: &Boat, width: f64, yaw: f32) -> Vec3 {
        let sqrt_of_two = 2.0_f64.sqrt() as f32;
        let escape = sunrise_world::living_air::escape_vector(
            f64::from(WIDTH as f32 * sqrt_of_two),
            width,
            yaw,
        );
        let (x, z) = (b.position.x + escape.x, b.position.z + escape.z);
        let top = b.bounds().max_position().y;
        let (bx, by, bz) = (x.floor() as i32, top.floor() as i32, z.floor() as i32);
        let below = self.res.facts().get(self.world.block(bx, by - 1, bz));
        let here = self.res.facts().get(self.world.block(bx, by, bz));
        if below.fluid != Fluid::Water
            && !here.full_collision
            && (below.full_collision || below.legacy_solid)
        {
            Vec3::new(x, f64::from(by), z)
        } else {
            Vec3::new(b.position.x, top, b.position.z)
        }
    }
    /// `Entity.setYRot` on a boat just made (its old rotation moves with it).
    pub(in crate::game::server_side) fn set_boat_yaw(&mut self, id: u32, yaw: f32) {
        if let Some(i) = self.boats.iter().position(|b| b.id == id) {
            self.boats.update(i, |b| {
                b.yaw = yaw;
                b.previous_yaw = yaw;
            });
        }
    }
    /// `BoatItem.use`: aim at a block or the water surface.
    pub(in crate::game::server_side) fn place_boat(&mut self, stack: &ItemStack) -> bool {
        let Some(mut boat) = self.boat_to_place(stack) else {
            return false;
        };
        self.side.next_entity += 1;
        boat.id = self.side.next_entity;
        let key = (
            boat.position.x.floor() as i32 >> 4,
            boat.position.z.floor() as i32 >> 4,
        );
        self.world.modified.insert(key);
        let (id, kind, yaw) = (boat.id, boat.entity_type(), boat.yaw);
        self.boats.push(boat);
        self.apply_item_stack_config(super::commands::Target::Boat(id), &kind, stack, true);
        // BoatItem.use: setYRot(player.getYRot()) after the stack's config.
        self.set_boat_yaw(id, yaw);
        if self.side.serving.is_some() || self.options.dedicated.is_some() {
            self.native_vehicle_create(
                self.boats.last().unwrap().entity_type().as_str(),
                [
                    self.boats.last().unwrap().position.x,
                    self.boats.last().unwrap().position.y,
                    self.boats.last().unwrap().position.z,
                ],
            );
        }
        // BoatItem.use: ENTITY_PLACE at the hit (the boat's position), by the player.
        let at = self
            .boats
            .last()
            .map(|b| [b.position.x, b.position.y, b.position.z]);
        if let Some(at) = at {
            let ctx = self.player_context(None);
            self.post_game_event("minecraft:entity_place", at, ctx);
        }
        if !self.creative {
            self.consume_held();
        }
        self.award_stat("used", &stack.item, 1);
        self.swing_arm();
        true
    }
    /// `AbstractBoat.interact`: board the boat under the crosshair, or the one
    /// a guest's interact packet named.
    pub(in crate::game::server_side) fn interact_boat(
        &mut self,
        target: Option<InteractTarget>,
    ) -> bool {
        let Some((i, d)) = (match target {
            Some(t) => crate::game::item_actions::named_entity(&self.boats, t.id, |b| b.id)
                .map(|i| (i, 0.0)),
            None => self.boat_hit(),
        }) else {
            return false;
        };
        let sneaking = self.server_acting_sneak();
        // Only a client's own pick is stopped by a block: the range check passed.
        if target.is_none() && !self.before_block(d) {
            return false;
        }
        // AbstractBoat.interact starts with Entity.interact: the lead.
        if self.interact_boat_leash(i) {
            self.swing_arm();
            return true;
        }
        // ChestBoat.interact: sneaking (or a full seat) opens the chest.
        if self.boats[i].chest && (sneaking || self.riding.is_some() || !self.boat_has_room(i)) {
            self.open_chest_boat_container(i);
            self.swing_arm();
            return true;
        }
        if sneaking
            || self.boats[i].out_of_control >= 60.0
            || self.riding.is_some()
            || self.ridden_by_guest(self.boats[i].id)
            || !self.boat_has_room(i)
        {
            return false;
        }
        let id = self.boats[i].id;
        self.start_riding_target(
            super::commands::Target::Player,
            super::commands::Target::Boat(id),
            false,
        );
        true
    }
    /// `ContainerEntity.interactWithContainerVehicle` (`AbstractChestBoat`):
    /// the chest's 27 slots go under a placeholder key that the menu edits; a
    /// server hands them to the player as a `ChestMenu` (`as_guest` ->
    /// `lan_open_container`). `AbstractChestBoat.interact` fires
    /// `CONTAINER_OPEN` and angers the piglins that see it.
    pub(in crate::game::server_side) fn open_chest_boat_container(&mut self, i: usize) {
        let b = &self.boats[i];
        let key = (i32::MAX - 15, super::minecarts::CART_KEY_Y, b.id as i32);
        let at = [b.position.x, b.position.y, b.position.z];
        let items = b.items.clone();
        // `startOpen` shares one container, it does not reset it: a stand-in
        // another player's menu is still editing over stays as it is.
        if !self.world.block_entities.contains_key(&key) {
            self.world
                .block_entities
                .insert(key, sunrise_level::containers::BlockEntity::Chest(items));
        }
        self.container = Some(key);
        self.server_set_acting_use(false);
        let ctx = self.player_context(None);
        self.post_game_event("minecraft:container_open", at, ctx);
        self.anger_piglins(true);
    }
    /// `AbstractChestBoat.openCustomInventoryScreen` (`HasCustomInventoryScreen`),
    /// the inventory key while riding a chest boat: its chest opens, not the
    /// player's inventory.
    pub(in crate::game::server_side) fn open_ridden_chest_boat(&mut self) -> bool {
        let Some(id) = self.riding else {
            return false;
        };
        let Some(i) = self.boats.iter().position(|b| b.id == id) else {
            return false;
        };
        if !self.boats[i].chest {
            return false;
        }
        self.open_chest_boat_container(i);
        true
    }
    /// `VehicleEntity.hurtServer` from the player's attack.
    pub(in crate::game::server_side) fn attack_boat(&mut self) -> bool {
        let Some((i, d)) = self.boat_hit() else {
            return false;
        };
        if !self.before_block(d) || self.mob_hit().is_some_and(|m| m.1 < d) {
            return false;
        }
        self.strike_boat(i);
        self.swing_arm();
        true
    }
    /// The accepted boat target, shared by aimed attacks and headless parity.
    pub(in crate::game) fn strike_boat(&mut self, i: usize) {
        let area = self.boats[i].bounds();
        // Player.attack's full damage against the boat's entity type.
        let (kind, id) = (self.boats[i].entity_type(), self.boats[i].id as i32);
        let (damage, scale, _, magic) = self.melee_damage(&kind, false, Some(id as u32));
        let sweep = self.sweep_before_hit(&kind, false);
        let chest = self.boats[i].chest;
        let attacker = self.own_entity_id();
        let source = sunrise_sim::health::HurtSource {
            damage_type: "minecraft:player_attack",
            causing: attacker,
            direct: attacker,
            position: None,
        };
        let vanilla =
            self.saved_refuses_damage(id as u32, "minecraft:player_attack", self.creative);
        let invulnerable = crate::game::entity_server::damage_level_of!(
            self,
            self.side.combat.0.attack(),
            no_push,
            None
        )
        .invulnerability_check(
            sunrise_sim::health::Hurting::Entity(id, &kind, source),
            vanilla,
        );
        let hurt = if invulnerable {
            None
        } else {
            self.hurt_boat_result(i, damage, self.creative)
        };
        // AbstractChestBoat.destroy -> ContainerEntity.chestVehicleDestroyed:
        // with entity_drops on, a player whose hit breaks a chest boat (not a
        // creative discard) angers the piglins that see it.
        if chest
            && hurt == Some(true)
            && !self.creative
            && self
                .ui
                .rules
                .bool_at(sunrise_game_data::gamerules::vanilla::ENTITY_DROPS)
        {
            self.anger_piglins(true);
        }
        self.vehicle_attack_sounds(area, sweep, magic.then_some(id));
        if damage > 0.0
            && let Some(destroyed) = hurt
        {
            let (knockback, _) = self.extra_knockback(&kind, scale);
            if knockback > 0.0 {
                // Player.causeExtraKnockback uses Entity.push for a boat,
                // not LivingEntity.knockback (no resistance or velocity halving).
                if !destroyed {
                    let (x, z) = self.extra_knockback_direction();
                    self.boats
                        .update(i, |b| b.velocity.x += f64::from(-(x as f32) * knockback));
                    self.boats.update(i, |b| b.velocity.y += 0.1);
                    self.boats
                        .update(i, |b| b.velocity.z += f64::from(-(z as f32) * knockback));
                }
                self.special_attack_slowdown(knockback);
            }
            self.remember_player_target(None);
            self.special_attack_exhaustion();
        }
        // Player.onAttack: the hit starts the attack strength again.
        self.side.attack_ticks = 0;
    }
    /// `VehicleEntity.hurtServer`: wobble, add the damage and break past 40
    /// (`creative`: a creative player's hit discards it straight away).
    /// `AbstractChestBoat.remove` drops the chest contents for every removal
    /// reason that destroys the boat (DISCARDED included), while the boat item
    /// itself only drops on the death path (`VehicleEntity.destroy`), not when
    /// a creative hit discards it. True when the boat was removed.
    pub(in crate::game) fn hurt_boat(&mut self, i: usize, damage: f32, creative: bool) -> bool {
        self.hurt_boat_result(i, damage, creative).unwrap_or(false)
    }
    /// Whether a spectator must be refused this boat's menu.
    pub(in crate::game) fn boat_loot_pending(&self, id: u32) -> bool {
        matches!(
            self.persist.raw.get(&id),
            Some(sunrise_core::nbt::Tag::Compound(map)) if map.contains_key(&"LootTable".into())
        )
    }

    /// Existing boat access routes all use the shared unpack operation.
    pub(in crate::game) fn unpack_boat_loot(&mut self, i: usize, player: bool) {
        let player = player.then(|| self.native_acting_player()).flatten();
        self.unpack_loot(
            crate::game::container_loot_level::LootTarget::Boat(self.boats[i].id),
            player,
        );
    }

    pub(in crate::game) fn hurt_boat_result(
        &mut self,
        i: usize,
        mut damage: f32,
        creative: bool,
    ) -> Option<bool> {
        let id = self.boats[i].id;
        if self
            .persist
            .destroying_vehicles
            .contains(&super::commands::Target::Boat(id))
            || self
                .persist
                .preparing_vehicle_removals
                .contains(&super::commands::Target::Boat(id))
        {
            return Some(true);
        }
        let mut cancel = false;
        self.native_vehicle_damage("boat", &mut damage, &mut cancel);
        if cancel {
            return None;
        }
        let Some(i) = self.boats.iter().position(|b| {
            b.id == id
                && !self
                    .persist
                    .removals
                    .contains(&crate::game::commands::Target::Boat(id))
        }) else {
            return Some(true);
        };
        let (destroyed, at) = self.boats.update(i, |b| {
            b.hurt_dir = -b.hurt_dir;
            b.hurt_time = 10;
            b.damage += damage * 10.0;
            (
                creative || b.damage > 40.0,
                [b.position.x, b.position.y, b.position.z],
            )
        });
        if destroyed {
            self.native_vehicle_destroy("boat", at);
            let Some(b) = self.boats.iter().find(|b| b.id == id) else {
                return Some(true);
            };
            let item = b.entity_type();
            let slots = b.items.len();
            if !self.kill_vehicle_object(super::commands::Target::Boat(id)) {
                return Some(true);
            }
            // `VehicleEntity.destroy(level, dropItem)`: `kill` (the boat's own
            // ENTITY_DIE after that removal), then the boat item on the death
            // path only, and only with `entity_drops` on, by `spawnAtLocation`
            // (an ItemEntity's own random motion).
            if !creative {
                self.entity_killed_event(at, false);
            }
            // `VehicleEntity.destroy(level, dropItem)` drops the boat item on
            // the death path only, and only with `entity_drops` on.
            if !creative
                && self
                    .ui
                    .rules
                    .bool_at(sunrise_game_data::gamerules::vanilla::ENTITY_DROPS)
            {
                self.spawn_item_own(
                    ItemStack {
                        item: Arc::from(item.as_str()),
                        count: 1,
                        damage: 0,
                        extra: None,
                    },
                    at,
                    10,
                );
                // AbstractChestBoat.destroy: `chestVehicleDestroyed` drops the
                // emptied chest again, which only draws each slot's spot.
                if slots != 0 {
                    self.drop_vehicle_contents(at, &vec![None; slots]);
                }
            }
        }
        Some(destroyed)
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use crate::game::boats_common::{
        Status, powder_snow_stuck, registered_boat, saved_ticks_frozen,
    };

    #[test]
    fn boat_admission_uses_registered_entity_ids() {
        let registry = sunrise_game_data::registries::Registries {
            entity_types: vec![
                "minecraft:bamboo_chest_raft".into(),
                "minecraft:oak_boat".into(),
                "minecraft:pig".into(),
            ],
            ..Default::default()
        };
        assert_eq!(
            registered_boat("minecraft:oak_boat", &registry),
            Some(("oak", false))
        );
        assert_eq!(
            registered_boat("minecraft:bamboo_chest_raft", &registry),
            Some(("bamboo", true))
        );
        assert_eq!(registered_boat("minecraft:bamboo_boat", &registry), None);
        assert_eq!(registered_boat("minecraft:pig", &registry), None);
    }

    fn boat() -> Boat {
        Boat::new(1, Arc::from("oak"), Vec3::ZERO, 0.0)
    }

    /// A boat the client takes over while it eases toward the server's position goes on
    /// easing there and reports that spot (the handler is the entity's, not the driver's).
    #[test]
    #[ignore = "daily"]
    fn a_boat_taken_over_mid_way_goes_on_easing() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let mut b = Boat::new(9, "oak".into(), Vec3::new(0.5, 64.0, 2.5), 0.0);
            b.go_on_easing(Vec3::new(0.5, 64.0, 3.5), (0.0, 0.0), 2);
            assert_eq!(b.client_position().0, Vec3::new(0.5, 64.0, 3.5));
            game.interpolate_driven_boat(&mut b);
            assert!((b.position.z - 3.0).abs() < 1e-9, "{}", b.position.z);
            game.interpolate_driven_boat(&mut b);
            assert!((b.position.z - 3.5).abs() < 1e-9, "{}", b.position.z);
            assert!(b.lerp.is_none());
        });
    }

    /// The boat this client drives eases to a server teleport over three ticks
    /// (LinearInterpolationHandler), and until it gets there it reports the spot it is
    /// going to (getClientPositionAndRotation).
    #[test]
    #[ignore = "daily"]
    fn a_driven_boat_eases_to_a_teleport_and_reports_where_it_is_going() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let mut b = Boat::new(9, "oak".into(), Vec3::new(0.5, 64.0, 2.5), 0.0);
            b.interpolate_to(Vec3::new(0.5, 64.0, 6.5), 90.0, 0.0);
            let mut steps = Vec::new();
            for _ in 0..3 {
                game.interpolate_driven_boat(&mut b);
                steps.push((b.position.z, b.yaw, b.client_position()));
            }
            let expected = [
                (2.5 + 4.0 / 3.0, 30.0),
                (2.5 + 8.0 / 3.0, 60.0),
                (6.5, 90.0),
            ];
            for (i, ((z, yaw, _), (want_z, want_yaw))) in steps.iter().zip(expected).enumerate() {
                assert!((z - want_z).abs() < 1e-9, "step {i}: z {z}");
                assert!((yaw - want_yaw).abs() < 1e-4, "step {i}: yaw {yaw}");
            }
            assert_eq!(steps[0].2, (Vec3::new(0.5, 64.0, 6.5), 90.0, 0.0));
            assert_eq!(steps[1].2, (Vec3::new(0.5, 64.0, 6.5), 90.0, 0.0));
            assert_eq!(steps[2].2, (b.position, b.yaw, b.pitch));
            assert!(b.lerp.is_none());
        });
    }

    #[test]
    #[ignore = "daily"]
    fn destruction_and_underwater_ejection_snap_rejected_passenger_descendants() {
        for broken in [false, true] {
            super::super::native_entity_refs::tests::with_game(|game| {
                let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                let mut boat = Boat::new(71, "oak".into(), Vec3::new(10.0, 200.0, 20.0), 0.0);
                boat.status = Status::UnderWater;
                boat.out_of_control = 60.0;
                game.boats.push(boat);
                let mut rider =
                    sunrise_sim::mobs::Mob::new(72, kind.clone(), Vec3::new(5.0, 190.0, 6.0), 0.0);
                rider.extra.mount = Some(71);
                rider.extra.passenger = true;
                game.persist
                    .off_level
                    .insert(72, crate::game::entity_memory::OffLevel::Mob(rider));
                let mut child = sunrise_sim::mobs::Mob::new(73, kind, Vec3::ZERO, 0.0);
                child.extra.mount = Some(72);
                child.age = 19;
                game.mobs.push(child);
                if broken {
                    assert!(game.hurt_boat(0, 5.0, true));
                    assert!(game.boats.is_empty());
                } else {
                    // The actual ejection entry used by AbstractBoat.tick at 60 submerged ticks.
                    game.eject_boat_riders(0);
                    assert_eq!(game.boats.len(), 1);
                }
                let rider = game.mob_object(72).unwrap();
                assert_eq!(rider.extra.mount, None);
                assert!(!rider.extra.passenger);
                assert_eq!(rider.extra.boarding_cooldown, 60);
                // Removed vehicle: retain x/z and clamp y to the boat's y.
                // Live underwater boat over air: getDismountLocation's box-top fallback.
                let expected = if broken {
                    Vec3::new(5.0, 200.0, 6.0)
                } else {
                    Vec3::new(10.0, 200.5625, 20.0)
                };
                assert_eq!(rider.position, expected);
                let child = game.mob_object(73).unwrap();
                assert_eq!(child.position.x, expected.x);
                assert_eq!(child.position.z, expected.z);
                assert!((child.position.y - expected.y - 0.86875).abs() < 1e-6);
                assert_eq!(child.previous, child.position);
                assert_eq!(child.age, 19);
                assert_eq!(child.extra.mount, Some(72));
            });
        }
    }

    /// `AbstractBoat.tickBubbleColumn` on a client: the wobble grows while the
    /// bubble time runs and fades after.
    #[test]
    fn the_wobble_grows_while_the_bubble_time_runs_and_fades_after() {
        let mut b = boat();
        b.bubble_time = BUBBLE_TIME;
        for _ in 0..30 {
            b.tick_bubble_wobble();
        }
        // 0.05 a tick, capped at 1.
        assert_eq!(b.bubble_multiplier, 1.0);
        // 10 * sin(0.5 * tickCount) * multiplier.
        let expected = 10.0 * (0.5 * 30.0f64).sin() as f32;
        assert!((b.bubble_angle - expected).abs() < 1.0e-4);
        // The render angle lerps between the last two ticks.
        let mid = b.bubble_angle(0.5);
        assert!((mid - (b.bubble_angle_old + b.bubble_angle) / 2.0).abs() < 1.0e-5);
        // Once the bubble time is 0 the multiplier fades by 0.1 a tick.
        b.bubble_time = 0;
        for _ in 0..4 {
            b.tick_bubble_wobble();
        }
        assert!((b.bubble_multiplier - 0.6).abs() < 1.0e-5);
        for _ in 0..10 {
            b.tick_bubble_wobble();
        }
        assert_eq!(b.bubble_multiplier, 0.0);
        assert_eq!(b.bubble_angle, 0.0);
    }

    /// `AbstractBoat.tickBubbleColumn` on the server: a boat held over a
    /// column fires on the 60th tick, with the column's direction, then the
    /// time restarts.
    #[test]
    fn the_bubble_countdown_fires_every_sixty_ticks_over_a_column() {
        for down in [false, true] {
            let mut b = boat();
            let mut fired = Vec::new();
            for tick in 1..=130 {
                // What boat_above_bubble_column does each tick.
                b.above_bubble_column = true;
                b.bubble_down = down;
                if b.bubble_time == 0 {
                    b.bubble_time = BUBBLE_TIME;
                }
                if let Some(d) = b.tick_bubble_countdown() {
                    fired.push((tick, d));
                }
                assert!(!b.above_bubble_column);
            }
            assert_eq!(fired, vec![(60, down), (120, down)]);
        }
    }

    /// Leaving the column resets the time: nothing fires, and a boat with no
    /// time left does nothing over a column until the column sets it.
    #[test]
    fn leaving_the_column_resets_the_bubble_countdown() {
        let mut b = boat();
        b.bubble_time = BUBBLE_TIME;
        for _ in 0..30 {
            b.above_bubble_column = true;
            assert_eq!(b.tick_bubble_countdown(), None);
        }
        assert_eq!(b.bubble_time, 30);
        // A tick away from the column.
        assert_eq!(b.tick_bubble_countdown(), None);
        assert_eq!(b.bubble_time, 0);
        b.above_bubble_column = true;
        assert_eq!(b.tick_bubble_countdown(), None);
        assert!(b.above_bubble_column);
    }

    #[test]
    fn a_boat_that_never_saw_a_column_does_not_wobble() {
        let mut b = boat();
        for _ in 0..10 {
            b.tick_bubble_wobble();
        }
        assert_eq!(b.bubble_angle, 0.0);
        assert_eq!(b.bubble_angle(0.5), 0.0);
    }

    /// #8156: the powder snow cells a boat's box reaches into, its box deflated by 1e-5.
    #[test]
    fn a_boat_reaches_the_powder_snow_cells_its_box_touches() {
        let b = Boat::new(1, "oak".into(), Vec3::new(10.5, 64.0, 10.5), 0.0);
        let snow = |cells: &'static [[i32; 3]]| move |p: [i32; 3]| cells.contains(&p);
        // Inside, a corner cell, and a cell the box only ends against.
        assert_eq!(
            reached_cells(b.bounds(), snow(&[[10, 64, 10]])),
            vec![[10, 64, 10]]
        );
        assert_eq!(
            reached_cells(b.bounds(), snow(&[[9, 64, 11], [12, 64, 10], [10, 65, 10]])),
            vec![[9, 64, 11]],
            "the cell beyond the edge and the one above the box are not reached"
        );
        let edge = Boat::new(1, "oak".into(), Vec3::new(11.3125, 64.0, 10.5), 0.0);
        assert!(reached_cells(edge.bounds(), snow(&[[12, 64, 10]])).is_empty());
        // x, y, z order.
        let all = reached_cells(b.bounds(), |_| true);
        assert_eq!(all.len(), 9);
        assert_eq!((all[0], all[8]), ([9, 64, 9], [11, 64, 11]));
    }

    /// `Entity.move`: a boat made stuck goes its velocity times the multiplier once, and
    /// loses its velocity; one that is not goes as it did.
    #[test]
    fn a_stuck_boat_moves_a_part_of_its_velocity_once() {
        let mut b = Boat::new(1, "oak".into(), Vec3::ZERO, 0.0);
        b.velocity = Vec3::new(1.0, 0.5, 2.0);
        assert_eq!(b.take_movement(), Vec3::new(1.0, 0.5, 2.0));
        assert_eq!(b.velocity, Vec3::new(1.0, 0.5, 2.0));
        b.stuck = powder_snow_stuck();
        let moved = b.take_movement();
        let slow = f64::from(0.9_f32);
        assert_eq!(moved, Vec3::new(slow, 0.75, 2.0 * slow));
        assert_eq!((b.velocity, b.stuck), (Vec3::ZERO, Vec3::ZERO));
        b.velocity = Vec3::new(0.0, 0.1, 1.0);
        assert_eq!(b.take_movement(), Vec3::new(0.0, 0.1, 1.0), "spent");
    }

    /// `Entity.saveWithoutId` writes `TicksFrozen` only while positive; `Entity.load` reads it.
    #[test]
    fn a_boat_saves_and_loads_ticks_frozen() {
        let mut b = Boat::new(
            1,
            std::sync::Arc::from("oak"),
            Vec3::new(0.0, 64.0, 0.0),
            0.0,
        );
        assert_eq!(
            b.to_nbt([0; 4], None, &Default::default())
                .get("TicksFrozen"),
            None
        );
        b.ticks_frozen = 6;
        let saved = b.to_nbt([0; 4], None, &Default::default());
        assert_eq!(saved.get("TicksFrozen"), Some(&Tag::Int(6)));
        assert_eq!(saved_ticks_frozen(&saved), 6);
        b.ticks_frozen = 0;
        assert_eq!(
            b.to_nbt([0; 4], Some(&saved), &Default::default())
                .get("TicksFrozen"),
            None
        );
    }
    /// `PowderSnowBlock.entityInside`'s snowflake: only for an entity that moved sideways, and
    /// then only when a coin falls so.
    #[test]
    fn a_boat_moving_sideways_in_powder_snow_throws_snowflakes_now_and_then() {
        use sunrise_core::random::LegacyRandom;
        // Still or only rising: no coin is drawn.
        let mut random = LegacyRandom::new(7);
        let mut untouched = LegacyRandom::new(7);
        assert_eq!(
            snowflake_velocity(Vec3::new(0.0, 0.4, 0.0), &mut random),
            None
        );
        assert_eq!(random.next_int(), untouched.next_int());
        let known = Vec3::new(0.1, 0.0, 0.0);
        // One stream, as the server's: about half the calls draw a snowflake. (Seeds a step
        // apart would not do, their first coin is nearly always the same.)
        let mut stream = LegacyRandom::new(42);
        let flakes: Vec<[f64; 3]> = (0..200)
            .filter_map(|_| snowflake_velocity(known, &mut stream))
            .collect();
        assert!(
            (60..=140).contains(&flakes.len()),
            "about half the seeds: {}",
            flakes.len()
        );
        let limit = f64::from(0.083_333_336_f32);
        for [x, y, z] in flakes {
            assert!(x.abs() <= limit && z.abs() <= limit);
            assert_eq!(y, f64::from(0.05_f32));
        }
        // Either horizontal axis is enough.
        assert!(
            (0..64)
                .any(|_| { snowflake_velocity(Vec3::new(0.0, 0.0, -0.1), &mut stream).is_some() })
        );
    }
}
#[cfg(test)]
pub(in crate::game) mod cobweb_tests {
    use super::Boat;
    use crate::game::game_tests::tests::server_game;
    use std::sync::Arc;
    use sunrise_core::geometry::Vec3;
    use sunrise_level::chunk::Chunk;

    /// `AbstractBoat.tick`'s `applyEffectsFromBlocks` calls `WebBlock.entityInside`, which
    /// makes the boat stuck: its next move goes a quarter of the speed across and a
    /// twentieth down, its velocity is spent and its fall is reset. A boat outside the web
    /// moves as it did and keeps falling.
    #[test]
    #[ignore = "daily"]
    fn a_cobweb_slows_a_boat_and_resets_its_fall() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        let web = game
            .pack
            .assets
            .registry
            .default_state("minecraft:cobweb")
            .unwrap();
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.world.set(8, 64, 8, web).unwrap();
        let boat = |x: f64| {
            let mut b = Boat::new(1, Arc::from("oak"), Vec3::new(x, 64.0, 8.5), 0.0);
            b.velocity = Vec3::new(1.0, 0.0, 0.0);
            b.fall_distance = 5.0;
            b
        };
        let (mut in_web, mut outside) = (boat(8.5), boat(12.5));
        // The tick in the web sets the multiplier and resets the fall.
        in_web = game.tick_boat(in_web, false);
        outside = game.tick_boat(outside, false);
        let (web_fall, free_fall) = (in_web.fall_distance, outside.fall_distance);
        // The next move spends the multiplier.
        let (web_from, free_from) = (in_web.position, outside.position);
        in_web = game.tick_boat(in_web, false);
        outside = game.tick_boat(outside, false);
        let (web_moved, free_moved) = (
            in_web.position.subtract(web_from),
            outside.position.subtract(free_from),
        );
        assert!(
            free_moved.x > 0.5,
            "the boat outside the web moves: {free_moved:?}"
        );
        assert_eq!(
            web_moved.x,
            free_moved.x * 0.25,
            "a quarter of the speed across"
        );
        assert_eq!(web_moved.z, 0.0);
        assert!(
            (web_moved.y - free_moved.y * f64::from(0.05_f32)).abs() < 1e-12,
            "a twentieth down: {} against {}",
            web_moved.y,
            free_moved.y
        );
        assert_eq!(
            in_web.velocity,
            Vec3::ZERO,
            "the multiplier and velocity are spent"
        );
        assert_eq!(web_fall, 0.0, "makeStuckInBlock resets the fall");
        assert!(
            free_fall > 5.0,
            "a boat outside the web keeps falling: {free_fall}"
        );
    }

    /// A boat whose box reaches a cobweb and, after the move, powder snow takes
    /// the multiplier of the block vanilla's traversal reaches last: the one
    /// after the move, not the one it leaves.
    #[test]
    #[ignore = "daily"]
    fn a_boat_leaving_a_cobweb_for_powder_snow_takes_the_powder_snow() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        let (web, snow) = {
            let registry = &game.pack.assets.registry;
            (
                registry.default_state("minecraft:cobweb").unwrap(),
                registry.default_state("minecraft:powder_snow").unwrap(),
            )
        };
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.world.set(8, 64, 8, web).unwrap();
        game.world.set(10, 64, 8, snow).unwrap();
        let mut boat = Boat::new(1, Arc::from("oak"), Vec3::new(8.5, 64.0, 8.5), 0.0);
        // Far enough in one move to leave the cobweb's cell and reach the snow's.
        boat.velocity = Vec3::new(2.0, 0.0, 0.0);
        boat = game.tick_boat(boat, false);
        // The same boat without the stored multiplier: the next move is the full one.
        let mut free = boat.clone();
        free.stuck = Vec3::ZERO;
        let (from, free_from) = (boat.position, free.position);
        boat = game.tick_boat(boat, false);
        free = game.tick_boat(free, false);
        let (moved, free_moved) = (
            boat.position.subtract(from),
            free.position.subtract(free_from),
        );
        assert!(
            free_moved.x > 1.0,
            "the boat outside the blocks moves: {free_moved:?}"
        );
        assert!(
            (moved.x - free_moved.x * f64::from(0.9_f32)).abs() < 1e-12,
            "powder snow's across multiplier: {} against {}",
            moved.x,
            free_moved.x
        );
        assert!(
            (moved.y - free_moved.y * 1.5).abs() < 1e-12,
            "powder snow's down multiplier: {} against {}",
            moved.y,
            free_moved.y
        );
    }
}
