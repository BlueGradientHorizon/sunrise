#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::base::LevelView;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::base::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::ModBlockEntity;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// `ParticleEngine.clearParticles`: every particle and emitter goes before the next
    /// frame is drawn or the next particle tick runs, whichever comes first. It takes
    /// `&self`, so a screen or render hook can ask, as with `play_local_sound`.
    /// False on a server.
    pub(in crate::game) fn clear_particles(&self) -> bool {
        if !self.native_window() {
            return false;
        }
        self.client.mod_clear_particles.set(true);
        true
    }
    /// Where the mouse is, in GUI pixels.
    pub(in crate::game) fn cursor(&self) -> [f32; 2] {
        [self.client.cursor.0, self.client.cursor.1]
    }
    /// A small local vanilla particle; caller should rate-limit emission.
    pub(in crate::game) fn flame_particle(&mut self, at: [f64; 3], soul: bool) {
        use sunrise_client::particles::{ParticleOptions, ParticleType};
        if !self.native_window() || at.iter().any(|v| !v.is_finite()) {
            return;
        }
        let Some(p) = self.native_player() else {
            return;
        };
        if (at[0] - p.x).powi(2) + (at[1] - p.y).powi(2) + (at[2] - p.z).powi(2) > 1024.0 {
            return;
        }
        let kind = if soul {
            ParticleType::SoulFireFlame
        } else {
            ParticleType::Flame
        };
        self.particles
            .0
            .add(&ParticleOptions::simple(kind), at, [0.0, 0.01, 0.0]);
    }
    /// The mod whose own screen is open, if one is (Escape, or another screen
    /// replacing it, closes it).
    pub(in crate::game) fn mod_screen(&self) -> Option<&str> {
        self.client
            .native_mod_screen
            .as_deref()
            .filter(|_| self.client.screen == Screen::Mod)
    }
    /// Opens a screen of mod `mod_id`'s own through the normal screen-transition
    /// path (`Gui.setScreen`): `screen_opening` and `screen_closing` fire for the
    /// transition and `screen_init_post` runs once opened, so the mod mounts
    /// widgets in `screen_init_post`, draws in `native_screen_render_post`, and
    /// takes its clicks, scroll, keys, and typing there; the mouse is freed, and
    /// Escape closes it. Every mod's own screen has the `ScreenView.id` `mod`, so
    /// a mod's screen hooks check `mod_screen()` is its own before mounting or
    /// drawing. It doesn't pause a single-player game (as the inventory doesn't).
    /// Can replace any open vanilla screen, title screen, overlay (book, sign
    /// editor, dialog, or prompt), or another mod's screen, running the replaced
    /// screen's close cleanup once hooks allow the transition. Returns `true`
    /// when `mod_id`'s screen is open, and `false` when cancelled or redirected
    /// by `screen_opening`, while a world or connection is loading, or without a
    /// client window (a headless/dedicated server, the server thread, or
    /// guest-acting contexts). A gamepad can close it (B, Start) but not click on
    /// it: its clicks don't reach the mouse hooks.
    pub(in crate::game) fn mod_screen_open(&mut self, mod_id: &str) -> bool {
        let unavailable = self.renderer.0.is_none()
            || self.lan_guest_acting()
            || (self.player.is_none() && self.title.0.is_none())
            || self.remote_loading();
        let owner = if self.screen_over_playing() {
            None
        } else {
            self.mod_screen()
        };
        if let Some(open) = mod_screen_opens(self.client.screen, unavailable, owner, mod_id) {
            return open;
        }
        if self.screen_hooks_heard() {
            let decision = {
                let current = if self.screen_over_playing() {
                    self.observed_screen_view()
                        .unwrap_or_else(|| self.screen_change_view(self.client.screen))
                } else if self.client.screen == Screen::Playing
                    && let Some(title) = self.title.0.as_ref()
                {
                    let none = self.screen_change_view(Screen::Playing);
                    screen_view_with_id(&none, title.mode.id())
                } else {
                    self.screen_change_view(self.client.screen)
                };
                let new = screen_view_for_mod(&self.screen_change_view(Screen::Mod), mod_id);
                self.decide_screen_for_mod_open(&current, &new, current.id != "playing")
            };
            match decision {
                ScreenDecision::Keep => return false,
                ScreenDecision::Vanilla(target) => {
                    self.close_replaced_screen_for_mod();
                    self.open_screen_target(target);
                    return false;
                }
                ScreenDecision::ModScreen(other_mod) => {
                    self.close_replaced_screen_for_mod();
                    self.apply_screen(Screen::Mod, Some(other_mod));
                    return self.mod_screen() == Some(mod_id);
                }
                ScreenDecision::Open => {}
            }
        }
        self.close_replaced_screen_for_mod();
        self.apply_screen(Screen::Mod, Some(mod_id));
        self.mod_screen() == Some(mod_id)
    }
    /// Shows the panel beside container screens with these items (at most
    /// 8192), as the game's own slots in pages, with a search box; the
    /// player's clicks, searches and keys over items come back as
    /// `native_panel_event`. The panel is the last mod's to show one; empty
    /// items and no recipe page take yours away.
    pub(in crate::game) fn panel_items(&mut self, mod_id: &str, items: Vec<mod_api::Stack>) {
        self.client.panel.show_items(mod_id, items);
    }
    /// The particles alive in the client's level (`ParticleEngine.countParticles`): 0 once
    /// a clear is asked, and on a server.
    pub(in crate::game) fn particle_count(&self) -> u32 {
        if !self.native_window() || self.client.mod_clear_particles.get() {
            return 0;
        }
        u32::try_from(self.client_particle_count()).unwrap_or(u32::MAX)
    }
    /// Plays `sound` for this player only (`SoundPlay`'s event: `minecraft:ui.button.click`,
    /// or a mod's own from its `assets/<id>/sounds.json`): nothing is sent to the server, and
    /// other players don't hear it. It takes `&self`, so a screen, key or mouse hook can play
    /// one: the sound is queued and starts at the next audio update (once a client tick),
    /// `after` ticks counted from then (0 to 1200, a minute; `SoundEngine.playDelayed`). `SoundPlay::ui` plays it at the listener as an interface sound
    /// (`SimpleSoundInstance.forUI`); `SoundPlay::at` at a position in the client's level,
    /// fading with distance (`volume` past 1 carries farther, 16 blocks times the volume), as
    /// thunder heard after the flash or a blast heard when its shock wave arrives. Its
    /// `SoundRef` follows and controls this play: `state`, `stop` and `fade`. None, with nothing
    /// played, on a server or for a value out of range (`SoundPlay::effects` included, by
    /// `SoundEffects::valid`); an event the resources don't have plays nothing, as vanilla's,
    /// and its handle reads `Stopped` once the update has tried it. Its effects stay as given
    /// unless the sound ticks: a `play_sound` listener can change them on each live refresh.
    pub(in crate::game) fn play_local_sound(
        &self,
        sound: &SoundPlay,
        after: u32,
    ) -> Option<SoundRef> {
        if !self.native_window() || !local_sound_ok(sound, after) {
            return None;
        }
        Some(self.client.mod_sounds.queue(sound, after))
    }
}

#[cfg(test)]
#[path = "base_tests/screen_open_tests_client.rs"]
pub(in crate::game) mod screen_open_tests;

#[cfg(test)]
#[path = "base_tests/screen_widgets_tests_client.rs"]
pub(in crate::game) mod screen_widgets_tests;
