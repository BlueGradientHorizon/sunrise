#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::DISMOUNT_POSES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::DismountWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::GameDismountWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::Mount;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::MountState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::Rider;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::on_top;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::start_riding_jump;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::steering_item;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    blocks::Fluid,
    first_person::Hand,
    hud::HudBatch,
    items::{EntityTypeTags, ItemStack},
    mobs::{Mob, MobEvent, Species, Who, equine},
    motion::{MotionWorld, PlayerMotion},
    pose::{Dimensions, Pose},
    sprint::Keys,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3, VoxelShape},
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::living_air::escape_vector;

impl Game<'_, crate::game::client_side::ClientState> {
    /// The jump bar when it is the contextual bar.
    pub(in crate::game) fn draw_jump_bar(&self, batch: &mut HudBatch, w: i32, h: i32) {
        let Some(i) = self.mount_index() else {
            return;
        };
        let m = &self.mobs[i];
        // JumpableVehicleBar: `getJumpCooldown`, which only the camel's and
        // the nautilus' dash have; a horse always shows the progress.
        let cooldown = i32::from(
            self.mount.jump_ticks < 0
                && (matches!(m.species(), Species::Camel | Species::CamelHusk)
                    || sunrise_client::mobs::nautilus::is_nautilus(m.species())),
        );
        let draws = sunrise_client::hud_overlays::jump_bar(w, h, cooldown, self.mount.jump_scale);
        sunrise_client::hud_layout::paint(batch, &self.gui.atlas, &draws);
    }
    /// `willPrioritizeJumpInfo`: charging a jump or recharging.
    pub(in crate::game) fn mount_jumping(&self) -> bool {
        self.mount.jump_scale > 0.0 || self.mount.jump_ticks < 0
    }
    pub(in crate::game) fn client_own_passenger_index(&self, _vehicle: u32) -> usize {
        self.remote.as_ref().map_or(0, |r| r.passenger_index)
    }
}
