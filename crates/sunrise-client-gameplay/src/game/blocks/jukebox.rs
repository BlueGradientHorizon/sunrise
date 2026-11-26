//! Client jukebox song playback, Now Playing overlay, and level-event handlers.
use crate::game::ClientGame;
#[allow(unused_imports)]
pub(in crate::game) use sunrise_gameplay_common::game::jukebox_common::{
    jukebox_playable, jukebox_signals, song_of_record,
};
use sunrise_game_data::text::Text;
use sunrise_level::containers::BlockEntity;

#[cfg(feature = "client")]
impl ClientGame<'_> {
    /// Level event 1010: the song from the jukebox, and the "Now Playing" overlay.
    pub(in crate::game::client_side) fn start_song(&mut self, pos: [i32; 3], song: &str) {
        assert!(
            !self.is_logical_server(),
            "start_song called on server role"
        );
        self.stop_sound(pos);
        let Some(s) = self.items().songs.get(song).cloned() else {
            return;
        };
        let id = self.client.audio.0.play_jukebox_song(&s.sound, pos);
        self.client
            .jukebox_sounds
            .insert((pos[0], pos[1], pos[2]), id);
        let description =
            sunrise_game_data::component::to_text(&s.description, &mut |k| Text::literal(k));
        let message = Text::translate("record.nowPlaying", &[description]);
        self.say_system_now(&message);
        self.client.titles.set_overlay(message, true);
        self.update_song_volumes();
        self.common.notify_record_playing(pos, true);
    }

    /// `LevelEventHandler` 1010 on a client of a server.
    pub(in crate::game) fn jukebox_song_event(&mut self, pos: [i32; 3], data: i32) {
        if let Some(song) =
            jukebox_song_name(self.client.remote.as_ref().map(|r| &*r.registries), data)
        {
            self.start_song(pos, &song);
        }
    }

    /// `LevelEventHandler` 1011: `stopJukeboxSongAndNotifyNearby`.
    pub(in crate::game) fn jukebox_stop_event(&mut self, pos: [i32; 3]) {
        self.stop_sound(pos);
        self.common.notify_record_playing(pos, false);
    }

    pub(in crate::game) fn stop_sound(&mut self, pos: [i32; 3]) {
        assert!(
            !self.is_logical_server(),
            "stop_sound called on server role"
        );
        if let Some(id) = self.client.jukebox_sounds.remove(&(pos[0], pos[1], pos[2])) {
            self.client.audio.0.stop_and_release(id);
        }
    }

    /// Let go of songs that ended or whose jukebox stopped.
    pub(in crate::game::client_side) fn update_song_volumes(&mut self) {
        assert!(
            !self.is_logical_server(),
            "update_song_volumes called on server role"
        );
        let sounds: Vec<_> = self
            .client
            .jukebox_sounds
            .iter()
            .map(|(k, id)| (*k, *id))
            .collect();
        for (k, id) in sounds {
            let playing = matches!(self.world.block_entities.get(&k), Some(BlockEntity::Jukebox(j)) if j.playing);
            if !keeps_song(
                self.client.audio.0.engine.is_active(id),
                self.client.remote.is_some(),
                playing,
            ) {
                self.client.audio.0.stop_and_release(id);
                self.client.jukebox_sounds.remove(&k);
            }
        }
    }
}

/// The song a level event 1010's data names in the server's `jukebox_song` registry.
pub(in crate::game::client_side) fn jukebox_song_name(
    registries: Option<&sunrise_protocol::registry::Dynamic>,
    id: i32,
) -> Option<String> {
    registries?
        .name("minecraft:jukebox_song", id)
        .map(str::to_owned)
}

/// Whether a jukebox song keeps playing.
pub(in crate::game::client_side) fn keeps_song(
    active: bool,
    server_client: bool,
    jukebox_playing: bool,
) -> bool {
    active && (server_client || jukebox_playing)
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod tests {
    use super::{jukebox_song_name, keeps_song};
    use sunrise_protocol::registry::Dynamic;

    #[test]
    fn a_server_clients_jukebox_song_comes_from_level_event_1010() {
        let mut registries = Dynamic::default();
        registries.registries.insert(
            "minecraft:jukebox_song".into(),
            ["minecraft:11", "minecraft:13", "minecraft:cat"]
                .map(|n| (n.to_owned(), None))
                .to_vec(),
        );
        assert_eq!(
            jukebox_song_name(Some(&registries), 2).as_deref(),
            Some("minecraft:cat")
        );
        assert_eq!(jukebox_song_name(Some(&registries), 3), None);
        assert_eq!(jukebox_song_name(Some(&registries), -1), None);
        assert_eq!(jukebox_song_name(None, 2), None);
        assert!(keeps_song(true, true, false));
        assert!(!keeps_song(false, true, false));
        assert!(keeps_song(true, false, true));
        assert!(!keeps_song(true, false, false));
    }
}

use sunrise_gameplay_common::game::tables::GameItems as _;
