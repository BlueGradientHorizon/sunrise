#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::jukebox::jukebox_playable;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::jukebox::jukebox_signals;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::jukebox::jukebox_song_name;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::jukebox::keeps_song;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::jukebox::song_of_record;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::jukebox::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    block_entity_item,
    containers::{BlockEntity, Jukebox},
    items::ItemStack,
    mobs::{Species, Who},
    particles::{ParticleOptions, ParticleType},
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::level::flags as level_flags;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn start_song_server(&mut self, pos: [i32; 3], song: &str) {
        if self.audio_capture_active() {
            self.server.lan_songs.push((pos, Some(song.to_owned())));
        }
    }
    /// Level event 1011: the song ends, here and for the guests.
    pub(in crate::game) fn stop_song(&mut self, pos: [i32; 3]) {
        self.stop_sound(pos);
        self.notify_record_playing(pos, false);
        if self.audio_capture_active() {
            self.server.lan_songs.push((pos, None));
        }
    }
}
