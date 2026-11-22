//! A server's spectator camera (`ClientPacketListener.handleSetCamera`,
//! `Minecraft.cameraEntity`): `/spectate` or a spectator's click (sent as
//! `MultiPlayerGameMode.spectate`) attaches the view to another entity. The
//! view follows that entity while the player stands still and sends no moves
//! (`LocalPlayer.isControlledCamera`); the server keeps its player on the
//! camera and lets go when the player sneaks.
#[cfg(test)]
#[path = "tests/camera_mount_tests.rs"]
mod camera_mount_tests;
#[cfg(test)]
#[path = "tests/remote_camera_tests.rs"]
mod camera_tests;

use super::{Game, remote::Remote};
use std::collections::HashMap;
use sunrise_client::{
    camera::{AvatarBob, CameraEntity, FovPlayer, HurtBob, LivingCamera, UseItem},
    entities::{ArmorStand, ItemFrame, Painting},
    mobs::{Mob, Species, render, villager},
    motion::PlayerMotion,
};
use sunrise_world::collision_context::EntityContextKind;

/// Hurt and death timers carried by a living camera entity for `GameRenderer.bobHurt`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::game) struct CameraHurt {
    pub hurt_time: i32,
    pub hurt_duration: i32,
    pub hurt_dir: f32,
    pub death_time: Option<i32>,
}

impl CameraHurt {
    pub(in crate::game) fn bob(self, partial: f32) -> HurtBob {
        HurtBob {
            hurt_time: self.hurt_time as f32 - partial,
            hurt_duration: self.hurt_duration,
            hurt_dir: self.hurt_dir,
            death_time: self.death_time.map(|t| t as f32 + partial),
        }
    }
}

/// Single resolved camera entity (`Minecraft.getCameraEntity`), live or detached.
#[derive(Clone, Debug)]
pub(in crate::game) struct ResolvedCameraEntity {
    pub entity_id: u32,
    pub camera: CameraEntity,
    pub is_local_player: bool,
    pub is_player: bool,
    pub is_spectator: bool,
    pub is_sleeping: bool,
    pub blocks_sky: bool,
    pub hurt: Option<CameraHurt>,
    pub avatar_bob: Option<AvatarBob>,
    pub fov_player: Option<FovPlayer>,
    pub has_player_passenger: bool,
    pub collision_kind: EntityContextKind,
    pub post_effect: Option<&'static str>,
}

/// The entity `Hud.getCameraPlayer` finds: the local player, another player (its index in
/// `mobs`), or none because the camera is not a player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum CameraPlayer {
    Own,
    Other(usize),
    Absent,
}

/// `Minecraft.setCameraEntity` from a ClientboundSetCameraPacket: the player's
/// own id puts the view back on the player; an id the level does not hold
/// changes nothing (`ClientboundSetCameraPacket.getEntity` is null).
pub(super) fn camera_after_packet(
    current: Option<i32>,
    player_id: i32,
    camera_id: i32,
    in_level: bool,
) -> Option<i32> {
    if camera_id == player_id {
        None
    } else if in_level {
        Some(camera_id)
    } else {
        current
    }
}

/// `LivingEntity.isSleeping` with `getBedOrientation` as its `toYRot`: a villager, player or
/// mannequin lying in a bed, whose facing `sleep_pose` keeps in `ints[1]`.
fn sleeping_view(m: &Mob) -> Option<Option<f32>> {
    if !(m.species() == Species::Villager || render::avatar(m)) || m.extra.ints[1] == 0 {
        return None;
    }
    Some(villager::bed_orientation(m).map(|d| match d {
        mod_api::Direction::South => 0.0,
        mod_api::Direction::West => 90.0,
        mod_api::Direction::North => 180.0,
        _ => 270.0,
    }))
}

/// `Camera.alignWithEntity`'s view of a mob or another player: its
/// interpolated position, `getViewYRot`/`getViewXRot` and eye height, with
/// its scale and camera distance for a detached view. A living entity's
/// `getViewYRot` is its head's yaw (`yHeadRot`), not its body's `yRot`.
pub(super) fn mob_camera_entity(m: &Mob, (scale, camera_distance): (f32, f64)) -> CameraEntity {
    CameraEntity {
        position: [m.position.x, m.position.y, m.position.z],
        previous: [m.previous.x, m.previous.y, m.previous.z],
        x_rot: m.head_pitch,
        y_rot: m.head_yaw,
        x_rot_o: m.previous_head_pitch,
        y_rot_o: m.previous_head_yaw,
        eye_height: m.eye_height() as f32,
        living: Some(LivingCamera {
            scale,
            camera_distance,
            sleeping: sleeping_view(m),
            dead: (m.health <= 0.0).then_some(m.death_time),
        }),
        mount: None,
    }
}

/// `Camera.alignWithEntity`'s view of an armor stand, the usual camera of a
/// server's cutscene or menu scene. `ArmorStand.setYHeadRot` keeps its head
/// on `yRot`, so its view turns with its yaw; its eyes are its dimensions'
/// (`getDimensionsMarker`): none for a marker, `BABY_DIMENSIONS`' 0.9875 for
/// a small stand, the type's 1.7775 otherwise.
#[cfg(test)]
pub(super) fn stand_camera_entity(a: &ArmorStand) -> CameraEntity {
    stand_camera_entity_with(a, (1.0, 4.0))
}

pub(super) fn stand_camera_entity_with(
    a: &ArmorStand,
    (scale, camera_distance): (f32, f64),
) -> CameraEntity {
    let eye_height = if a.marker {
        0.0
    } else {
        a.probe_dimensions().2 as f32 * scale
    };
    CameraEntity {
        position: [a.position.x, a.position.y, a.position.z],
        previous: [a.previous.x, a.previous.y, a.previous.z],
        x_rot: a.pitch,
        y_rot: a.yaw,
        x_rot_o: a.pitch,
        y_rot_o: a.previous_yaw,
        eye_height,
        living: Some(LivingCamera {
            scale,
            camera_distance,
            sleeping: None,
            dead: None,
        }),
        mount: None,
    }
}

/// `Camera.alignWithEntity`'s view of a boat, which is not living (no
/// `getScale`, no camera distance): its interpolated position and `Entity`'s
/// view rotation, with `EntityType.BOAT`'s `eyeHeight(0.5625F)`.
pub(super) fn boat_camera_entity(b: &super::boats::Boat) -> CameraEntity {
    CameraEntity {
        position: [b.position.x, b.position.y, b.position.z],
        previous: [b.previous.x, b.previous.y, b.previous.z],
        x_rot: b.pitch,
        y_rot: b.yaw,
        x_rot_o: b.pitch,
        y_rot_o: b.previous_yaw,
        eye_height: 0.5625_f32,
        living: None,
        mount: None,
    }
}
/// `Camera.alignWithEntity`'s view of an item frame: a hanging entity sits at
/// the centre of its box, the block's centre pulled 0.46875 out of the wall
/// (`HangingEntity.recalculateBoundingBox` of `ItemFrame.createBoundingBox`),
/// does not move, and has `EntityType.ITEM_FRAME`'s zero eye height.
/// `ItemFrame.setDirection` turns it: `get2DDataValue() * 90` in yaw (south 0,
/// west 1, north 2, east 3), or `-90 * getAxisDirection().getStep()` in pitch
/// on the floor and ceiling.
pub(super) fn frame_camera_entity(f: &ItemFrame) -> CameraEntity {
    let step = f.step().map(f64::from);
    let at: [f64; 3] = std::array::from_fn(|i| f64::from(f.pos[i]) + 0.5 - step[i] * 0.46875);
    let (y_rot, x_rot) = match f.facing {
        0 => (0.0, 90.0),
        1 => (0.0, -90.0),
        2 => (180.0, 0.0),
        4 => (90.0, 0.0),
        5 => (270.0, 0.0),
        _ => (0.0, 0.0),
    };
    CameraEntity {
        position: at,
        previous: at,
        x_rot,
        y_rot,
        x_rot_o: x_rot,
        y_rot_o: y_rot,
        eye_height: 0.0,
        living: None,
        mount: None,
    }
}
/// `Camera.alignWithEntity`'s view of a painting: a hanging entity sits at the
/// centre of its box (`HangingEntity.recalculateBoundingBox`), faces
/// `get2DDataValue() * 90`, and keeps `EntityType.PAINTING`'s `0.425F` eyes.
pub(super) fn painting_camera_entity(p: &Painting) -> CameraEntity {
    let (lo, hi) = p.bounds();
    let at = std::array::from_fn(|i| (lo[i] + hi[i]) * 0.5);
    let y_rot = f32::from(p.facing & 3) * 90.0;
    CameraEntity {
        position: at,
        previous: at,
        x_rot: 0.0,
        y_rot,
        x_rot_o: 0.0,
        y_rot_o: y_rot,
        eye_height: 0.425,
        living: None,
        mount: None,
    }
}
/// `Camera.alignWithEntity`'s view of primed TNT: its interpolated position
/// and `Entity`'s view rotation (priming never changes it), with
/// `EntityType.TNT`'s `eyeHeight(0.15F)`.
pub(super) fn tnt_camera_entity(t: &super::tnt::PrimedTnt) -> CameraEntity {
    CameraEntity {
        position: [t.position.x, t.position.y, t.position.z],
        previous: [t.previous.x, t.previous.y, t.previous.z],
        x_rot: t.rotation[1],
        y_rot: t.rotation[0],
        x_rot_o: t.rotation[1],
        y_rot_o: t.rotation[0],
        eye_height: 0.15,
        living: None,
        mount: None,
    }
}

/// The entity lists a server's camera can name (`Level.getEntity`): the kinds
/// this client can follow.
pub(super) struct CameraEntities<'a> {
    pub mobs: &'a [Mob],
    pub stands: &'a [ArmorStand],
    pub boats: &'a [super::boats::Boat],
    pub frames: &'a [ItemFrame],
    pub tnts: &'a [super::tnt::PrimedTnt],
    /// `Entity.getVehicle` of every entity (the `SetPassengers` graph).
    pub vehicles: &'a HashMap<u32, super::remote_vehicles::PassengerSeat>,
    /// The local player's id and body, when a camera entity can ride it.
    pub player: Option<(u32, &'a PlayerMotion)>,
    /// A mob's `getScale` and `CAMERA_DISTANCE` for a detached view.
    pub living: Box<dyn Fn(&Mob) -> (f32, f64) + 'a>,
    /// An armor stand's `getScale` and `CAMERA_DISTANCE` for a detached view.
    pub stand_living: Box<dyn Fn(&ArmorStand) -> (f32, f64) + 'a>,
}

impl CameraEntities<'_> {
    /// The id is one of the kinds this client can draw the view out of.
    pub(super) fn holds(&self, id: u32) -> bool {
        self.mobs.iter().any(|m| m.id == id)
            || self.stands.iter().any(|a| a.id == id)
            || self.boats.iter().any(|b| b.id == id)
            || self.frames.iter().any(|f| f.id == id)
            || self.tnts.iter().any(|t| t.id == id)
    }
    /// `Camera.alignWithEntity`: the view out of the entity, `None` when the
    /// level does not hold it.
    #[cfg(test)]
    pub(super) fn view(&self, id: u32) -> Option<CameraEntity> {
        let mut entity = self.entity_view(id)?;
        entity.mount = self.living_vehicle(id);
        Some(entity)
    }
    /// `Camera.alignWithEntity`: `isPassenger() && getVehicle() instanceof LivingEntity`, the
    /// vehicle's scale and camera distance.
    fn living_vehicle(&self, id: u32) -> Option<(f32, f64)> {
        let vehicle = self.vehicles.get(&id)?.vehicle;
        if let Some(m) = self.mobs.iter().find(|m| m.id == vehicle) {
            return Some((self.living)(m));
        }
        if let Some(a) = self.stands.iter().find(|a| a.id == vehicle) {
            return Some((self.stand_living)(a));
        }
        let (_, p) = self.player.filter(|(player, _)| *player == vehicle)?;
        let attribute =
            |name: &str, default: f64| p.attributes.computed_value_of(name).unwrap_or(default);
        Some((
            attribute("minecraft:scale", 1.0) as f32,
            attribute("minecraft:camera_distance", 4.0),
        ))
    }
    #[cfg(test)]
    fn entity_view(&self, id: u32) -> Option<CameraEntity> {
        if let Some(m) = self.mobs.iter().find(|m| m.id == id) {
            return Some(mob_camera_entity(m, (self.living)(m)));
        }
        if let Some(a) = self.stands.iter().find(|a| a.id == id) {
            return Some(stand_camera_entity_with(a, (self.stand_living)(a)));
        }
        if let Some(b) = self.boats.iter().find(|b| b.id == id) {
            return Some(boat_camera_entity(b));
        }
        if let Some(f) = self.frames.iter().find(|f| f.id == id) {
            return Some(frame_camera_entity(f));
        }
        self.tnts.iter().find(|t| t.id == id).map(tnt_camera_entity)
    }
}

/// `GameRenderer.checkEntityPostEffect`: the chain a spectated creeper, spider or
/// enderman puts on the screen, or one a mod registered for its own entity type
/// (`Hooks::register_entity_spectator_shaders`).
pub(super) fn entity_post_effect(kind: &str) -> Option<&'static str> {
    sunrise_client_render::spectator_shaders::chain(kind)
}

impl Game<'_> {
    /// `ClientPacketListener.handleSetCamera`.
    pub(in crate::game) fn remote_set_camera(&mut self, camera_id: i32) {
        let id = camera_id as u32;
        let in_level = self.remote.as_ref().is_some_and(|r| {
            self.camera_entities(r).holds(id) || r.tracked_entity(camera_id).is_some()
        });
        let player = self
            .remote
            .as_ref()
            .is_some_and(|r| r.entity_id == camera_id);
        if let Some(r) = self.remote.as_mut() {
            let next = camera_after_packet(r.camera, r.entity_id, camera_id, in_level);
            if in_level || player {
                r.camera = next;
                r.detached_camera = None;
            }
        }
        // Minecraft.setCameraEntity, for an entity the level holds.
        if in_level || player {
            self.check_entity_post_effect(self.camera_type.is_first_person());
        }
    }
    /// `GameRenderer.checkEntityPostEffect` for the camera entity, or for none
    /// (`first_person` false: the third-person view clears it).
    pub(super) fn check_entity_post_effect(&mut self, first_person: bool) {
        self.spectated_post_effect = self
            .camera_subject()
            .filter(|_| first_person)
            .and_then(|c| c.post_effect);
    }
    /// The entity a server attached the view to, when it is not the player.
    pub(in crate::game) fn remote_camera_id(&self) -> Option<u32> {
        self.remote.as_ref()?.camera.map(|id| id as u32)
    }
    /// `LocalPlayer.isControlledCamera`: the view is the player's own.
    pub(super) fn controls_camera(&self) -> bool {
        self.remote_camera_id().is_none()
    }
    /// `Minecraft.getCameraEntity`: local player, live tracked entity, or detached snapshot.
    pub(in crate::game) fn camera_subject(&self) -> Option<ResolvedCameraEntity> {
        if let Some(r) = self.remote.as_ref()
            && let Some(id) = r.camera
        {
            if let Some(detached) = r
                .detached_camera
                .as_deref()
                .filter(|d| d.entity_id == id as u32)
            {
                return Some(detached.clone());
            }
            return self.live_entity_camera_subject(r, id);
        }
        self.local_player_camera_subject()
    }
    /// `Camera.getCameraEntityPartialTicks`: `1.0` when frozen unless player or player-ridden.
    pub(in crate::game) fn camera_partial_tick(&self) -> f64 {
        let entity_frozen = !self.clock.runs_normally()
            && self
                .camera_subject()
                .is_some_and(|c| !c.is_player && !c.has_player_passenger);
        self.clock.camera_fraction_for(entity_frozen)
    }
    /// `GameRenderer.shouldRenderBlockOutline`: requires a player camera entity plus HUD/mode gates.
    pub(in crate::game) fn should_render_block_outline(&self) -> bool {
        let adventure = self.mode == sunrise_client::command::args::GameMode::Adventure;
        let can_build = self.may_build() || adventure;
        self.camera_subject().is_some_and(|c| c.is_player)
            && super::hud_render::block_outline_drawn(
                self.ui.hide_gui,
                self.entity_targeted(),
                can_build,
                self.shows_panorama(),
            )
    }
    /// Camera entity feet position (`cameraEntity.position()`).
    pub(in crate::game) fn camera_entity_position(&self) -> Option<[f64; 3]> {
        self.camera_subject().map(|c| c.camera.position)
    }
    /// Camera entity eye position (`cameraEntity.getEyePosition()`).
    pub(in crate::game) fn camera_eye_position(&self) -> Option<[f64; 3]> {
        self.camera_subject().map(|c| {
            let [x, y, z] = c.camera.position;
            [x, y + f64::from(c.camera.eye_height), z]
        })
    }
    /// `AbstractBedBlock.getBedOrientation` as its `toYRot`; `None` for any non-bed block.
    fn bed_orientation_yaw(&self, head: [i32; 3]) -> Option<f32> {
        let state = self.world.block(head[0], head[1], head[2]);
        let entry = self.pack.assets.registry.state(state)?;
        if !entry.name.ends_with("_bed") || !entry.properties.contains_key("part") {
            return None;
        }
        match entry.properties.get("facing").map(String::as_str) {
            Some("south") => Some(0.0),
            Some("west") => Some(90.0),
            Some("north") => Some(180.0),
            Some("east") => Some(270.0),
            _ => None,
        }
    }
    /// `LivingEntity.isSleeping` + `getBedOrientation` for any live mob or remote player.
    fn mob_sleeping_view(&self, m: &Mob) -> Option<Option<f32>> {
        if let Some(head) = m.extra.sleeping_pos {
            return Some(self.bed_orientation_yaw(head));
        }
        sleeping_view(m)
    }
    /// `AbstractClientPlayer.getFieldOfViewModifier` inputs for a spectated remote player.
    fn remote_fov_player(&self, m: &Mob) -> FovPlayer {
        let movement_speed = self
            .motion
            .attributes
            .id("minecraft:movement_speed")
            .and_then(|attr| self.remote_attribute(m.id as i32, attr))
            .unwrap_or_else(|| {
                if m.attrs.speed > 0.0 {
                    m.attrs.speed
                } else {
                    0.1
                }
            });
        let using = (m.extra.using_item > 0).then(|| {
            let slot = if m.extra.using_off_hand {
                sunrise_items::equipment::OFFHAND
            } else {
                sunrise_items::equipment::MAINHAND
            };
            let item = match m.extra.slot_stack(slot).map(|s| &*s.item) {
                Some("minecraft:bow") => UseItem::Bow,
                Some("minecraft:spyglass") => UseItem::Spyglass,
                _ => UseItem::Other,
            };
            (item, (m.extra.using_item - 1).max(0))
        });
        FovPlayer {
            flying: false,
            walking_speed: 0.1,
            movement_speed,
            using,
        }
    }
    fn local_player_camera_subject(&self) -> Option<ResolvedCameraEntity> {
        let p = self.player.as_ref()?;
        let scale = p
            .attributes
            .computed_value_of("minecraft:scale")
            .unwrap_or(1.0) as f32;
        let distance = p
            .attributes
            .computed_value_of("minecraft:camera_distance")
            .unwrap_or(4.0);
        let sleeping = self.sleeping.map(|head| self.bed_orientation_yaw(head));
        let mount = self
            .riding
            .and_then(|id| self.mobs.iter().find(|m| m.id == id))
            .map(|m| self.living_camera_values(m));
        let ride_turn = self
            .riding
            .and_then(|id| self.boats.iter().find(|b| b.id == id))
            .map_or_else(
                || {
                    self.riding
                        .and_then(|id| self.carts.iter().find(|c| c.id == id))
                        .map_or(0.0, |c| c.passenger_yaw_delta)
                },
                |b| b.delta_rotation,
            );
        let dead = self.death_time();
        let camera = CameraEntity {
            position: [p.position.x, p.position.y, p.position.z],
            previous: [
                self.previous_position.x,
                self.previous_position.y,
                self.previous_position.z,
            ],
            x_rot: self.pitch,
            y_rot: self.yaw,
            x_rot_o: self.pitch,
            y_rot_o: self.yaw - ride_turn,
            eye_height: p.dimensions.eye_height,
            living: Some(LivingCamera {
                scale,
                camera_distance: distance,
                sleeping,
                dead,
            }),
            mount,
        };
        Some(ResolvedCameraEntity {
            entity_id: self.own_entity_id() as u32,
            camera,
            is_local_player: true,
            is_player: true,
            is_spectator: self.own_player_is_spectator(),
            is_sleeping: self.sleeping.is_some(),
            blocks_sky: self.potions.effects.get("blindness").is_some()
                || self.potions.effects.get("darkness").is_some(),
            hurt: Some(CameraHurt {
                hurt_time: p.health.hurt_animation_time,
                hurt_duration: p.health.hurt.hurt_duration,
                hurt_dir: p.health.hurt_dir,
                death_time: dead,
            }),
            avatar_bob: Some(self.avatar),
            fov_player: self.fov_player(),
            has_player_passenger: false,
            collision_kind: EntityContextKind::Player,
            post_effect: None,
        })
    }
    fn living_vehicle_for(&self, remote: &Remote, id: u32) -> Option<(f32, f64)> {
        self.camera_entities(remote).living_vehicle(id)
    }
    fn entity_has_player_passenger(&self, remote: &Remote, target: u32) -> bool {
        let rides_target = |mut rider: u32| -> bool {
            for _ in 0..16 {
                let next = if rider == remote.entity_id as u32 {
                    self.riding
                        .or_else(|| remote.passenger_vehicles.get(&rider).map(|s| s.vehicle))
                } else {
                    remote.passenger_vehicles.get(&rider).map(|s| s.vehicle)
                };
                match next {
                    Some(v) if v == target => return true,
                    Some(v) => rider = v,
                    None => return false,
                }
            }
            false
        };
        if self.player.is_some() && rides_target(remote.entity_id as u32) {
            return true;
        }
        remote
            .level_players
            .keys()
            .any(|&pid| rides_target(pid as u32))
    }
    fn live_entity_camera_subject(&self, remote: &Remote, id: i32) -> Option<ResolvedCameraEntity> {
        let uid = id as u32;
        let mount = self.living_vehicle_for(remote, uid);
        let has_player_passenger = self.entity_has_player_passenger(remote, uid);
        if let Some(m) = self.mobs.iter().find(|m| m.id == uid) {
            let is_player = &*m.kind.id == "minecraft:player";
            let mut camera = mob_camera_entity(m, self.living_camera_values(m));
            if let Some(living) = &mut camera.living {
                living.sleeping = self.mob_sleeping_view(m);
            }
            camera.mount = mount;
            let is_sleeping = camera.living.is_some_and(|l| l.sleeping.is_some());
            let dead = camera.living.and_then(|l| l.dead);
            return Some(ResolvedCameraEntity {
                entity_id: uid,
                camera,
                is_local_player: false,
                is_player,
                is_spectator: is_player && m.extra.spectator,
                is_sleeping,
                blocks_sky: false,
                hurt: Some(CameraHurt {
                    hurt_time: m.hurt_time,
                    hurt_duration: 10,
                    hurt_dir: 0.0,
                    death_time: dead,
                }),
                avatar_bob: is_player.then_some(m.extra.avatar),
                fov_player: is_player.then(|| self.remote_fov_player(m)),
                has_player_passenger,
                collision_kind: if is_player {
                    EntityContextKind::Player
                } else {
                    EntityContextKind::Living
                },
                post_effect: entity_post_effect(&m.kind.id),
            });
        }
        if let Some(a) = self.armor_stands.iter().find(|a| a.id == uid) {
            let mut camera = stand_camera_entity_with(a, self.stand_camera_values(a));
            camera.mount = mount;
            return Some(ResolvedCameraEntity {
                entity_id: uid,
                camera,
                is_local_player: false,
                is_player: false,
                is_spectator: false,
                is_sleeping: false,
                blocks_sky: false,
                hurt: Some(CameraHurt::default()),
                avatar_bob: None,
                fov_player: None,
                has_player_passenger,
                collision_kind: EntityContextKind::Living,
                post_effect: entity_post_effect("minecraft:armor_stand"),
            });
        }
        if let Some(i) = self.interactions.get(&id) {
            let camera = CameraEntity {
                position: [i.position.x, i.position.y, i.position.z],
                previous: [i.previous.x, i.previous.y, i.previous.z],
                x_rot: i.pitch,
                y_rot: i.yaw,
                x_rot_o: i.previous_pitch,
                y_rot_o: i.previous_yaw,
                eye_height: i.eye_height(),
                living: None,
                mount,
            };
            return Some(ResolvedCameraEntity {
                entity_id: uid,
                camera,
                is_local_player: false,
                is_player: false,
                is_spectator: false,
                is_sleeping: false,
                blocks_sky: false,
                hurt: None,
                avatar_bob: None,
                fov_player: None,
                has_player_passenger,
                collision_kind: EntityContextKind::Other,
                post_effect: entity_post_effect("minecraft:interaction"),
            });
        }
        if let Some(b) = self.boats.iter().find(|b| b.id == uid) {
            let kind = remote
                .entities
                .get(&id)
                .map_or_else(|| "minecraft:oak_boat".to_owned(), |t| t.kind.clone());
            let mut camera = boat_camera_entity(b);
            camera.mount = mount;
            let post_effect = entity_post_effect(&kind);
            return Some(ResolvedCameraEntity {
                entity_id: uid,
                camera,
                is_local_player: false,
                is_player: false,
                is_spectator: false,
                is_sleeping: false,
                blocks_sky: false,
                hurt: None,
                avatar_bob: None,
                fov_player: None,
                has_player_passenger,
                collision_kind: EntityContextKind::Other,
                post_effect,
            });
        }
        if let Some(c) = self.carts.iter().find(|c| c.id == uid) {
            let kind = remote
                .entities
                .get(&id)
                .map_or_else(|| "minecraft:minecart".to_owned(), |t| t.kind.clone());
            let camera = CameraEntity {
                position: [c.position.x, c.position.y, c.position.z],
                previous: [c.previous.x, c.previous.y, c.previous.z],
                x_rot: c.pitch,
                y_rot: c.yaw,
                x_rot_o: c.previous_pitch,
                y_rot_o: c.previous_yaw,
                eye_height: self.type_eye_height(&kind) as f32,
                living: None,
                mount,
            };
            let post_effect = entity_post_effect(&kind);
            return Some(ResolvedCameraEntity {
                entity_id: uid,
                camera,
                is_local_player: false,
                is_player: false,
                is_spectator: false,
                is_sleeping: false,
                blocks_sky: false,
                hurt: None,
                avatar_bob: None,
                fov_player: None,
                has_player_passenger,
                collision_kind: EntityContextKind::Other,
                post_effect,
            });
        }
        if let Some(f) = self.item_frames.iter().find(|f| f.id == uid) {
            let kind = if f.glow {
                "minecraft:glow_item_frame"
            } else {
                "minecraft:item_frame"
            };
            let mut camera = frame_camera_entity(f);
            camera.mount = mount;
            return Some(ResolvedCameraEntity {
                entity_id: uid,
                camera,
                is_local_player: false,
                is_player: false,
                is_spectator: false,
                is_sleeping: false,
                blocks_sky: false,
                hurt: None,
                avatar_bob: None,
                fov_player: None,
                has_player_passenger,
                collision_kind: EntityContextKind::Other,
                post_effect: entity_post_effect(kind),
            });
        }
        if let Some(p) = self.paintings.iter().find(|p| p.id == uid) {
            let mut camera = painting_camera_entity(p);
            camera.mount = mount;
            return Some(ResolvedCameraEntity {
                entity_id: uid,
                camera,
                is_local_player: false,
                is_player: false,
                is_spectator: false,
                is_sleeping: false,
                blocks_sky: false,
                hurt: None,
                avatar_bob: None,
                fov_player: None,
                has_player_passenger,
                collision_kind: EntityContextKind::Other,
                post_effect: entity_post_effect("minecraft:painting"),
            });
        }
        if let Some(t) = self.tnts.iter().find(|t| t.id == uid) {
            let mut camera = tnt_camera_entity(t);
            camera.mount = mount;
            return Some(ResolvedCameraEntity {
                entity_id: uid,
                camera,
                is_local_player: false,
                is_player: false,
                is_spectator: false,
                is_sleeping: false,
                blocks_sky: false,
                hurt: None,
                avatar_bob: None,
                fov_player: None,
                has_player_passenger,
                collision_kind: EntityContextKind::Other,
                post_effect: entity_post_effect("minecraft:tnt"),
            });
        }
        let tracked = remote.entities.get(&id)?;
        let camera = CameraEntity {
            position: tracked.position,
            previous: tracked.previous,
            x_rot: tracked.pitch,
            y_rot: tracked.yaw,
            x_rot_o: tracked.previous_pitch,
            y_rot_o: tracked.previous_yaw,
            eye_height: self.type_eye_height(&tracked.kind) as f32,
            living: None,
            mount,
        };
        let post_effect = entity_post_effect(&tracked.kind);
        let collision_kind = if tracked.kind == "minecraft:item" {
            EntityContextKind::Item
        } else {
            EntityContextKind::Other
        };
        Some(ResolvedCameraEntity {
            entity_id: uid,
            camera,
            is_local_player: false,
            is_player: false,
            is_spectator: false,
            is_sleeping: false,
            blocks_sky: false,
            hurt: None,
            avatar_bob: None,
            fov_player: None,
            has_player_passenger,
            collision_kind,
            post_effect,
        })
    }
    /// `Hud.getCameraPlayer`: whose hotbar, bars and vehicle the HUD shows.
    pub(in crate::game) fn camera_player(&mut self) -> CameraPlayer {
        let Some(id) = self.remote_camera_id() else {
            return CameraPlayer::Own;
        };
        if self
            .remote
            .as_ref()
            .is_some_and(|r| r.detached_camera.is_some())
        {
            return CameraPlayer::Absent;
        }
        match self.mob_index.find(&self.mobs, id) {
            Some(i) if &*self.mobs[i].kind.id == "minecraft:player" => CameraPlayer::Other(i),
            _ => CameraPlayer::Absent,
        }
    }
    /// The view of the entity a server attached the camera to, live or detached.
    pub(super) fn spectated_camera_snapshot(&self) -> Option<CameraEntity> {
        self.camera_subject()
            .filter(|c| !c.is_local_player)
            .map(|c| c.camera)
    }
    /// The level's entities a server camera may name.
    fn camera_entities<'a>(&'a self, remote: &'a Remote) -> CameraEntities<'a> {
        CameraEntities {
            mobs: &self.mobs,
            stands: &self.armor_stands,
            boats: &self.boats,
            frames: &self.item_frames,
            tnts: &self.tnts,
            vehicles: &remote.passenger_vehicles,
            player: self.player.as_ref().map(|p| (remote.entity_id as u32, p)),
            living: Box::new(|m| self.living_camera_values(m)),
            stand_living: Box::new(|a| self.stand_camera_values(a)),
        }
    }
    /// Returns synced or type-default scale and camera distance through LivingEntity.sanitizeScale.
    pub(in crate::game) fn living_camera_values(&self, m: &Mob) -> (f32, f64) {
        let catalog = &self.motion.attributes;
        let value = |name: &str, default: f64| {
            catalog
                .id(name)
                .and_then(|attribute| self.remote_attribute(m.id as i32, attribute))
                .or_else(|| m.kind.attributes.get(name).copied())
                .unwrap_or(default)
        };
        let raw_scale = value("minecraft:scale", 1.0) as f32;
        let scale = match m.species() {
            Species::HappyGhast => raw_scale.min(1.0),
            Species::Shulker => raw_scale.min(3.0),
            _ => raw_scale,
        };
        (scale, value("minecraft:camera_distance", 4.0))
    }
    /// The scale and camera distance `Camera.setup` reads from an armor stand.
    fn stand_camera_values(&self, a: &ArmorStand) -> (f32, f64) {
        let catalog = &self.motion.attributes;
        let value = |name: &str, default: f64| {
            catalog
                .id(name)
                .and_then(|attribute| self.remote_attribute(a.id as i32, attribute))
                .unwrap_or(default)
        };
        (
            value("minecraft:scale", 1.0) as f32,
            value("minecraft:camera_distance", 4.0),
        )
    }
    /// The entity under a spectator's crosshair (`Minecraft.hitResult` as an
    /// `EntityHitResult`): the nearest mob or object before any block.
    pub(super) fn spectator_click_target(&self) -> Option<i32> {
        let mob = self.mob_hit().map(|(i, d, _)| (self.mobs[i].id as i32, d));
        let object = self.remote_object_hit(false).map(|h| (h.id, h.distance));
        [mob, object]
            .into_iter()
            .flatten()
            .filter(|(_, d)| self.before_block(*d))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
    /// `LevelRenderer`: the camera entity is not drawn from its own eyes, but a sleeping one is.
    pub(super) fn hides_as_camera(&self, id: u32) -> bool {
        self.camera_type.is_first_person()
            && self
                .remote
                .as_ref()
                .is_some_and(|r| r.detached_camera.is_none() && r.camera == Some(id as i32))
            && !self
                .mobs
                .iter()
                .any(|m| m.id == id && self.mob_sleeping_view(m).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::{camera_after_packet, entity_post_effect, stand_camera_entity};
    use sunrise_client::entities::ArmorStand;
    use sunrise_core::geometry::Vec3;

    /// A server's camera on an armor stand looks out of the stand's eyes
    /// (EntityType.ARMOR_STAND `eyeHeight(1.7775F)`, `BABY_DIMENSIONS`
    /// 0.9875F for a small one, nothing for a marker), along its yaw.
    #[test]
    fn an_armor_stand_holds_the_camera_at_its_eyes() {
        let mut stand = ArmorStand::new(7, Vec3::new(10.5, 64.0, -3.5), 90.0);
        stand.yaw = 135.0;
        stand.pitch = -20.0;
        let view = stand_camera_entity(&stand);
        assert_eq!(view.position, [10.5, 64.0, -3.5]);
        assert_eq!(view.eye_height, 1.7775f32);
        assert_eq!((view.y_rot_o, view.y_rot), (90.0, 135.0));
        assert_eq!(view.x_rot, -20.0);
        stand.small = true;
        assert_eq!(stand_camera_entity(&stand).eye_height, 0.9875f32);
        stand.marker = true;
        assert_eq!(stand_camera_entity(&stand).eye_height, 0.0);
    }

    /// Only a creeper, spider or enderman has a spectator chain.
    #[test]
    fn spectated_entities_pick_their_chain() {
        assert_eq!(
            entity_post_effect("minecraft:creeper"),
            Some("minecraft:creeper")
        );
        assert_eq!(
            entity_post_effect("minecraft:spider"),
            Some("minecraft:spider")
        );
        assert_eq!(
            entity_post_effect("minecraft:enderman"),
            Some("minecraft:invert")
        );
        assert_eq!(
            entity_post_effect("minecraft:cave_spider"),
            Some("minecraft:spider")
        );
        assert_eq!(entity_post_effect("minecraft:cow"), None);
    }

    /// The camera goes to a known entity, back to the player on its own id,
    /// and stays where it is for an entity the level does not hold.
    #[test]
    fn set_camera_follows_vanilla() {
        assert_eq!(camera_after_packet(None, 5, 9, true), Some(9));
        assert_eq!(camera_after_packet(Some(9), 5, 5, false), None);
        assert_eq!(camera_after_packet(Some(9), 5, 12, false), Some(9));
        assert_eq!(camera_after_packet(None, 5, 12, false), None);
    }
}
