//! Sunrise Studio, the replay editor: its free camera, camera path, timeline and top bar, and the
//! `/studio freecam|keyframe|path` commands. It builds on the
//! platform's replay recording and playback (`replay.rs`).

use super::{Game, Screen, packs};
use std::{path::PathBuf, time::Duration};
use sunrise_client::{
    packet_source::PacketSource,
    studio::{ReplayDriver, TimelineAction},
    text::Text,
};

/// Studio's state in a `Game`: the replay editor's free camera, timeline and top bar. Studio is a
/// Sunrise extra (docs/layers.md: the replay editor) on top of the platform's replay
/// (`replay.rs`); the engine only asks it whether the free camera flies.
#[derive(Default)]
pub(in crate::game) struct Studio {
    pub(in crate::game) freecam: sunrise_client::studio::StudioFreecam,
    pub(in crate::game) timeline: sunrise_client::studio::TimelineUi,
    pub(in crate::game) topbar: sunrise_client::studio::TopBarUi,
    pub(in crate::game) rmb_down: bool,
    pub(in crate::game) camera_track_dirty: bool,
}

/// Studio's editor runs. It is a Sunrise extra (docs/layers.md: the replay editor), so a build
/// without `sunrise-extras` (sunrise-plain) plays replays without it: no free camera, path,
/// timeline or top bar, and no editor commands or keys.
pub(in crate::game) const EDITOR: bool = cfg!(feature = "sunrise-extras");

impl Studio {
    /// The free camera flies, apart from the player.
    pub(in crate::game::client_side) fn flying(&self) -> bool {
        self.freecam.active
    }
}

pub const HELP: &[&str] = &[
    "studio record start [NAME]     start recording clientbound packets into .mcpr archive",
    "studio record stop             stop recording and finalize .mcpr archive",
    "studio record status           query recording status (active, elapsed, packets, bytes)",
    "studio replay open PATH        open .mcpr recording archive for playback",
    "studio replay pause            pause replay playback",
    "studio replay resume           resume replay playback",
    "studio replay speed MULTIPLIER set replay playback speed (0.01..20.0)",
    "studio replay seek MS          seek replay playback to timestamp in milliseconds",
    "studio replay status           query replay playback status",
    "studio replay close            close active replay session",
    "studio freecam <on|off>        enable or disable the Studio free camera",
    "studio keyframe <add|remove INDEX|clear> edit the replay camera path",
    "studio path <lock|unlock>      play or stop the camera path",
];

impl Game<'_, crate::game::client_side::ClientState> {
    /// Studio is up (its bars, its camera path, its mouse): a replay plays or the free camera flies.
    pub(in crate::game::client_side) fn studio_up(&self) -> bool {
        self.replay.playing() || self.side.studio.flying()
    }

    /// Studio's editor words of `/studio` (`freecam`, `keyframe`, `path`).
    pub(in crate::game::client_side) fn studio_editor_command(&mut self, words: &[&str]) -> bool {
        match words[0] {
            "freecam" => match words.get(1).copied() {
                Some("on") => {
                    if self.replay.driver.is_none() {
                        self.client_system_message(Text::translate(
                            "sunrise.studio.freecam.replay_required",
                            &[],
                        ));
                    } else if let Some((position, yaw, pitch)) = self.camera_view(1.0) {
                        self.side.studio.freecam.activate(
                            position,
                            yaw,
                            pitch,
                            self.settings.fov.clamp(1.0, 179.0),
                        );
                        self.capture(true);
                        self.client_system_message(Text::translate(
                            "sunrise.studio.freecam.enabled",
                            &[],
                        ));
                    } else {
                        self.client_system_message(Text::translate(
                            "sunrise.studio.freecam.unavailable",
                            &[],
                        ));
                    }
                }
                Some("off") => {
                    self.side.studio.freecam.deactivate();
                    self.capture(false);
                    self.client_system_message(Text::translate(
                        "sunrise.studio.freecam.disabled",
                        &[],
                    ));
                }
                _ => {
                    self.client_system_message(Text::translate("sunrise.studio.freecam.usage", &[]))
                }
            },
            "keyframe" => {
                let Some(driver) = self.replay.driver.as_ref() else {
                    self.client_system_message(Text::translate(
                        "sunrise.studio.replay.inactive",
                        &[],
                    ));
                    return true;
                };
                let time_ms = driver.position_ms();
                match words.get(1).copied() {
                    Some("add") => {
                        if !self.side.studio.freecam.active
                            && let Some((position, yaw, pitch)) = self.camera_view(1.0)
                        {
                            self.side.studio.freecam.activate(
                                position,
                                yaw,
                                pitch,
                                self.settings.fov.clamp(1.0, 179.0),
                            );
                        }
                        self.capture(true);
                        self.side.studio.freecam.add_keyframe_at_current(time_ms);
                        self.side.studio.camera_track_dirty = true;
                        self.client_system_message(Text::tr(
                            "sunrise.studio.keyframe.added",
                            &[&time_ms.to_string()],
                        ));
                    }
                    Some("clear") => {
                        self.side.studio.freecam.track.clear();
                        self.side.studio.camera_track_dirty = true;
                        self.client_system_message(Text::translate(
                            "sunrise.studio.keyframe.cleared",
                            &[],
                        ));
                    }
                    Some("remove") => {
                        let Some(index) =
                            words.get(2).and_then(|value| value.parse::<usize>().ok())
                        else {
                            self.client_system_message(Text::translate(
                                "sunrise.studio.keyframe.remove_usage",
                                &[],
                            ));
                            return true;
                        };
                        if index >= self.side.studio.freecam.track.keyframes.len() {
                            self.client_system_message(Text::translate(
                                "sunrise.studio.keyframe.index_invalid",
                                &[],
                            ));
                            return true;
                        }
                        self.side.studio.freecam.track.keyframes.remove(index);
                        self.side.studio.camera_track_dirty = true;
                        self.client_system_message(Text::tr(
                            "sunrise.studio.keyframe.removed",
                            &[&index.to_string()],
                        ));
                    }
                    _ => self.client_system_message(Text::translate(
                        "sunrise.studio.keyframe.usage",
                        &[],
                    )),
                }
            }
            "path" => match words.get(1).copied() {
                Some("lock") if !self.side.studio.freecam.track.is_empty() => {
                    self.side.studio.freecam.active = true;
                    self.side.studio.freecam.locked_to_path = true;
                    self.capture(true);
                    self.client_system_message(Text::translate("sunrise.studio.path.locked", &[]));
                }
                Some("unlock") => {
                    self.side.studio.freecam.locked_to_path = false;
                    self.client_system_message(Text::translate(
                        "sunrise.studio.path.unlocked",
                        &[],
                    ));
                }
                _ => self.client_system_message(Text::translate("sunrise.studio.path.usage", &[])),
            },
            _ => {}
        }
        true
    }

    /// Apply a menu action from the Studio top bar.
    pub(in crate::game::client_side) fn handle_studio_topbar_action(
        &mut self,
        action: sunrise_client::studio::TopBarAction,
    ) {
        use sunrise_client::studio::TopBarAction;
        match action {
            TopBarAction::None => {}
            TopBarAction::OpenPauseMenu => {
                self.side.pause_focus = None;
                self.side.screen = Screen::Paused;
                self.side.pause_menu = true;
                self.release_mappings_on_screen_open();
                self.commit_screen_transition();
                self.capture(false);
            }
            TopBarAction::ExitStudio => {
                if self.replay.driver.is_some() {
                    self.close_studio_replay();
                } else {
                    self.side.studio.freecam.deactivate();
                    self.capture(false);
                }
            }
            TopBarAction::OpenRecordingsFolder => {
                let directory = PathBuf::from("recordings");
                if let Err(error) = std::fs::create_dir_all(&directory) {
                    log::warn!("Studio: could not create recordings folder: {error}");
                } else {
                    packs::open_folder(&directory);
                }
            }
            TopBarAction::ResetCameraToPlayer => {
                let (position, yaw, pitch) = if let Some(player) = &self.player {
                    (
                        glam::DVec3::new(
                            player.position.x,
                            player.position.y + 1.62,
                            player.position.z,
                        ),
                        self.yaw,
                        self.pitch,
                    )
                } else {
                    (glam::DVec3::ZERO, 0.0, 0.0)
                };
                self.side
                    .studio
                    .freecam
                    .activate(position, yaw, pitch, self.settings.fov);
            }
            TopBarAction::ToggleLockToPath => {
                self.side.studio.freecam.locked_to_path =
                    !self.side.studio.freecam.locked_to_path;
            }
            TopBarAction::ToggleFreecam => {
                if self.side.studio.freecam.active {
                    self.side.studio.freecam.deactivate();
                    self.capture(false);
                } else {
                    let (position, yaw, pitch) =
                        self.camera_view(0.0)
                            .unwrap_or((glam::DVec3::ZERO, 0.0, 0.0));
                    self.side
                        .studio
                        .freecam
                        .activate(position, yaw, pitch, self.settings.fov);
                    self.capture(true);
                }
            }
            TopBarAction::AddKeyframe => {
                self.side
                    .studio
                    .freecam
                    .add_keyframe_at_current(self.side.studio.timeline.current_time_ms as u32);
                self.side.studio.camera_track_dirty = true;
            }
            TopBarAction::AddMarker => self.add_studio_marker(),
            TopBarAction::ClearKeyframes => {
                self.side.studio.freecam.track.keyframes.clear();
                self.side.studio.timeline.selected_keyframe = None;
                self.side.studio.camera_track_dirty = true;
            }
            TopBarAction::ToggleTimeline => {
                self.side.studio.timeline.visible = !self.side.studio.timeline.visible;
            }
            TopBarAction::SplitClip => {
                self.side
                    .studio
                    .timeline
                    .split_at(self.side.studio.timeline.current_time_ms);
            }
        }
    }

    /// Apply an action from the replay timeline HUD.
    pub(in crate::game::client_side) fn handle_studio_timeline_action(
        &mut self,
        action: sunrise_client::studio::TimelineAction,
    ) {
        use sunrise_client::studio::TimelineAction;
        match action {
            TimelineAction::PauseForSeek => {
                self.side.studio.timeline.playing = false;
                if let Some(replay) = self.replay.driver.as_mut() {
                    replay.set_paused(true);
                }
            }
            TimelineAction::Seek(time_ms) => {
                let time_ms = if self.replay.standalone {
                    time_ms
                } else {
                    let current = self
                        .replay
                        .driver
                        .as_ref()
                        .map_or(0, |driver| driver.position_ms())
                        as u64;
                    time_ms.max(current)
                };
                self.side.studio.timeline.current_time_ms = time_ms;
                if let Err(error) = self.seek_studio_replay(time_ms.min(u32::MAX as u64) as u32) {
                    log::warn!("Studio timeline seek failed: {error}");
                }
            }
            TimelineAction::TogglePlay => {
                self.side.studio.timeline.playing = !self.side.studio.timeline.playing;
                if let Some(replay) = self.replay.driver.as_mut() {
                    replay.set_paused(!self.side.studio.timeline.playing);
                }
            }
            TimelineAction::StepBack => {
                let time_ms = self
                    .side
                    .studio
                    .timeline
                    .current_time_ms
                    .saturating_sub(1000);
                self.handle_studio_timeline_action(TimelineAction::Seek(time_ms));
            }
            TimelineAction::StepForward => {
                let time_ms = self
                    .side
                    .studio
                    .timeline
                    .current_time_ms
                    .saturating_add(1000)
                    .min(self.side.studio.timeline.duration_ms);
                self.handle_studio_timeline_action(TimelineAction::Seek(time_ms));
            }
            TimelineAction::SeekStart => {
                self.handle_studio_timeline_action(TimelineAction::Seek(0))
            }
            TimelineAction::SeekEnd => self.handle_studio_timeline_action(TimelineAction::Seek(
                self.side.studio.timeline.duration_ms,
            )),
            TimelineAction::SetSpeed(speed) => {
                self.side.studio.timeline.speed = speed;
                if let Some(replay) = self.replay.driver.as_mut() {
                    replay.set_speed(speed);
                }
            }
            TimelineAction::ToggleFreecam => {
                if self.side.studio.freecam.active {
                    self.side.studio.freecam.deactivate();
                } else {
                    let (position, yaw, pitch) =
                        self.camera_view(0.0)
                            .unwrap_or((glam::DVec3::ZERO, 0.0, 0.0));
                    self.side
                        .studio
                        .freecam
                        .activate(position, yaw, pitch, self.settings.fov);
                }
            }
            TimelineAction::AddCameraKeyframe => {
                self.side
                    .studio
                    .freecam
                    .add_keyframe_at_current(self.side.studio.timeline.current_time_ms as u32);
                self.side.studio.camera_track_dirty = true;
            }
            TimelineAction::RemoveCameraKeyframe(index) => {
                if index < self.side.studio.freecam.track.keyframes.len() {
                    self.side.studio.freecam.track.keyframes.remove(index);
                    self.side.studio.camera_track_dirty = true;
                }
                self.side.studio.timeline.selected_keyframe = None;
            }
            TimelineAction::SelectKeyframe(index) => {
                self.side.studio.timeline.selected_keyframe = Some(index);
            }
            TimelineAction::KeyframeMoved => {
                self.side.studio.camera_track_dirty = true;
            }
            TimelineAction::SplitClip(time_ms) => self.side.studio.timeline.split_at(time_ms),
            TimelineAction::None => {}
        }
    }

    /// Add a bookmark at the replay playhead and save it in the `.mcpr` archive.
    pub(in crate::game::client_side) fn add_studio_marker(&mut self) {
        let Some(replay) = self
            .replay
            .driver
            .as_mut()
            .and_then(|driver| driver.as_any_mut().downcast_mut::<ReplayDriver>())
        else {
            return;
        };
        let label = Text::translate(
            "sunrise.studio.timeline.marker",
            &[Text::literal((replay.markers().len() + 1).to_string())],
        )
        .plain();
        match replay.add_marker(label) {
            Ok(_) => self.side.studio.timeline.markers = replay.markers().to_vec(),
            Err(error) => log::warn!("Studio: could not save replay marker: {error}"),
        }
    }

    /// Saves the free camera's path into the replay playing, when it changed.
    pub(in crate::game::client_side) fn save_studio_camera_track(&mut self) -> std::io::Result<()> {
        if self.side.studio.camera_track_dirty
            && let Some(driver) = self
                .replay
                .driver
                .as_mut()
                .and_then(|driver| driver.as_any_mut().downcast_mut::<ReplayDriver>())
        {
            driver.save_camera_track(&self.side.studio.freecam.track)?;
            self.side.studio.camera_track_dirty = false;
        }
        Ok(())
    }

    /// A replay opened: the timeline shows it and the free camera takes its saved path.
    pub(in crate::game::client_side) fn studio_replay_opened(&mut self, driver: &ReplayDriver) {
        self.side.studio.timeline.duration_ms = u64::from(driver.duration_ms());
        self.side.studio.timeline.current_time_ms = 0;
        self.side.studio.timeline.playing = !driver.is_paused();
        self.side.studio.timeline.markers = driver.markers().to_vec();
        self.side.studio.freecam.track = driver.camera_track().cloned().unwrap_or_default();
        self.side.studio.camera_track_dirty = false;
    }

    /// The replay moved on: a camera locked to its path follows it, and the timeline shows where
    /// it is.
    pub(in crate::game::client_side) fn studio_replay_advanced(&mut self) {
        if self.side.studio.freecam.locked_to_path
            && let Some(position) = self
                .replay
                .driver
                .as_ref()
                .map(|replay| replay.position_ms())
        {
            self.side.studio.freecam.update_spline_playback(position);
        }
        if let Some((position, duration, paused, markers)) =
            self.replay.driver.as_ref().map(|replay| {
                let markers = replay
                    .as_any()
                    .downcast_ref::<ReplayDriver>()
                    .map_or_else(Vec::new, |driver| driver.markers().to_vec());
                (
                    replay.position_ms(),
                    replay.duration_ms(),
                    replay.is_paused(),
                    markers,
                )
            })
        {
            self.side.studio.timeline.current_time_ms = u64::from(position);
            self.side.studio.timeline.duration_ms = u64::from(duration);
            self.side.studio.timeline.playing = !paused;
            self.side.studio.timeline.markers = markers;
        }
    }

    /// The replay closes: its camera path is saved (a failure is only logged).
    pub(in crate::game::client_side) fn studio_replay_closing(&mut self) {
        if let Err(error) = self.save_studio_camera_track() {
            log::warn!("Studio: could not save camera path: {error}");
        }
        self.side.studio.camera_track_dirty = false;
    }

    /// The replay closed: the timeline empties and the free camera lands.
    pub(in crate::game::client_side) fn studio_replay_closed(&mut self) {
        self.side.studio.timeline.current_time_ms = 0;
        self.side.studio.timeline.playing = false;
        self.side.studio.timeline.markers.clear();
        self.side.studio.timeline.split_points.clear();
        self.side.studio.timeline.selected_keyframe = None;
        self.side.studio.freecam.deactivate();
    }
}

/// What the engine's frame, render and window code hand to Studio.
impl Game<'_, crate::game::client_side::ClientState> {
    /// The top bar and the timeline over the HUD, while Studio is up in play.
    pub(in crate::game::client_side) fn draw_studio_bars(&mut self, batch: &mut super::HudBatch) {
        if !EDITOR || self.ui.hide_gui || self.side.screen != Screen::Playing || !self.studio_up()
        {
            return;
        }
        self.side.studio.topbar.render(
            batch,
            &self.gui.atlas,
            self.side.cursor,
            self.side.studio.freecam.locked_to_path,
        );
        self.side.studio.timeline.render(
            batch,
            &self.gui.atlas,
            self.side.cursor,
            &self.side.studio.freecam,
        );
    }

    /// The camera path among the debug lines, while Studio is up.
    pub(in crate::game::client_side) fn studio_camera_path(
        &self,
        gizmos: &mut sunrise_client::gizmos::Gizmos,
    ) {
        if EDITOR && self.studio_up() {
            sunrise_client::studio::render_camera_path(
                &self.side.studio.freecam.track,
                gizmos,
                None,
                100,
            );
        }
    }

    /// The pointer moved while the timeline's playhead or a keyframe is dragged: Studio took it.
    pub(in crate::game::client_side) fn studio_mouse_dragged(&mut self, width: f32) -> bool {
        if !EDITOR
            || self.side.screen != Screen::Playing
            || !(self.side.studio.timeline.dragging_playhead
                || self.side.studio.timeline.dragging_keyframe.is_some())
        {
            return false;
        }
        let action = self.side.studio.timeline.handle_mouse_drag(
            self.side.cursor,
            width,
            &mut self.side.studio.freecam,
        );
        if matches!(action, TimelineAction::KeyframeMoved) {
            self.side.studio.camera_track_dirty = true;
        }
        true
    }

    /// A mouse button while Studio is up: true when Studio took it (its bars, the right button's
    /// mouse look, every click while a replay plays).
    pub(in crate::game::client_side) fn studio_mouse_input(
        &mut self,
        pressed: bool,
        button: winit::event::MouseButton,
    ) -> bool {
        if !EDITOR {
            return self.side.screen == Screen::Playing && self.replay.playing();
        }
        if !pressed && button == winit::event::MouseButton::Left {
            let action = self.side.studio.timeline.handle_mouse_up();
            if !matches!(action, TimelineAction::None) {
                self.handle_studio_timeline_action(action);
            }
        }
        if (self.side.studio.freecam.active || self.replay.driver.is_some())
            && button == winit::event::MouseButton::Right
        {
            if let Some(renderer) = &self.side.renderer.0 {
                let batch = super::HudBatch::new(
                    [renderer.config.width, renderer.config.height],
                    self.settings.gui_scale_inputs(),
                );
                if self.side.studio.timeline.visible
                    && self.side.cursor.1 >= batch.height - self.side.studio.timeline.height()
                {
                    if pressed
                        && let Some(index) = self.side.studio.timeline.keyframe_at_cursor(
                            self.side.cursor,
                            batch.width,
                            batch.height,
                            &self.side.studio.freecam,
                        )
                    {
                        if index < self.side.studio.freecam.track.keyframes.len() {
                            self.side.studio.freecam.track.keyframes.remove(index);
                            self.side.studio.camera_track_dirty = true;
                        }
                        self.side.studio.timeline.selected_keyframe = None;
                    }
                    return true;
                }
            }
            if pressed {
                self.side.studio.topbar.close_dropdown();
                self.side.studio.rmb_down = true;
                self.capture(true);
                return true;
            } else if self.side.studio.rmb_down {
                self.side.studio.rmb_down = false;
                self.capture(false);
                return true;
            }
        }
        if pressed
            && button == winit::event::MouseButton::Left
            && self.side.screen == Screen::Playing
            && (self.replay.driver.is_some() || self.side.studio.freecam.active)
        {
            let size = self
                .side.renderer
                .0
                .as_ref()
                .map(|renderer| [renderer.config.width, renderer.config.height])
                .unwrap_or([1, 1]);
            let batch = super::HudBatch::new(size, self.settings.gui_scale_inputs());
            let had_open_menu = self.side.studio.topbar.open_menu.is_some();
            let topbar_action = self.side.studio.topbar.handle_mouse_click(
                self.side.cursor,
                batch.width,
                batch.height,
            );
            if !matches!(topbar_action, sunrise_client::studio::TopBarAction::None) {
                self.handle_studio_topbar_action(topbar_action);
                return true;
            }
            if had_open_menu || self.side.cursor.1 <= self.side.studio.topbar.height {
                return true;
            }
            if self.side.studio.timeline.visible {
                let action = self.side.studio.timeline.handle_mouse_down(
                    self.side.cursor,
                    batch.width,
                    batch.height,
                    &self.side.studio.freecam,
                );
                if !matches!(action, TimelineAction::None) {
                    self.handle_studio_timeline_action(action);
                    return true;
                }
                if self.side.cursor.1 >= batch.height - self.side.studio.timeline.height() {
                    return true;
                }
            }
        }
        self.side.screen == Screen::Playing && self.replay.driver.is_some()
    }

    /// The free camera flies by the movement keys, once per frame.
    pub(in crate::game::client_side) fn studio_fly(&mut self, elapsed: Duration) {
        if !self.side.studio.flying()
            || self.side.screen != Screen::Playing
            || self.dialog_open()
        {
            return;
        }
        use winit::keyboard::KeyCode;
        let held = |key| f64::from(u8::from(self.side.keys.raw(&key)));
        let forward = held(KeyCode::KeyW) - held(KeyCode::KeyS);
        let strafe = held(KeyCode::KeyD) - held(KeyCode::KeyA);
        let vertical = held(KeyCode::KeyE) - held(KeyCode::KeyQ) + held(KeyCode::Space);
        let fast = self.side.keys.raw(&KeyCode::ControlRight);
        self.side.studio.freecam.update_flight_motion(
            forward,
            strafe,
            vertical,
            fast,
            elapsed.as_secs_f64().min(0.1),
        );
    }

    /// A key pressed while a replay plays: Studio's shortcuts, and nothing else reaches play.
    pub(in crate::game::client_side) fn studio_key(&mut self, key: winit::keyboard::KeyCode) {
        use winit::keyboard::KeyCode;
        match key {
            KeyCode::Escape => {
                self.close_studio_replay();
            }
            KeyCode::KeyP => {
                self.side.pause_focus = None;
                self.side.screen = Screen::Paused;
                self.side.pause_menu = true;
                self.release_mappings_on_screen_open();
                self.commit_screen_transition();
                self.capture(false);
            }
            // The rest are the editor's.
            _ if !EDITOR => {}
            KeyCode::KeyF => self.handle_studio_timeline_action(TimelineAction::ToggleFreecam),
            KeyCode::KeyT => {
                self.side.studio.timeline.visible = !self.side.studio.timeline.visible
            }
            KeyCode::KeyM => self.add_studio_marker(),
            KeyCode::Home => self.handle_studio_topbar_action(
                sunrise_client::studio::TopBarAction::ResetCameraToPlayer,
            ),
            KeyCode::KeyL => {
                self.side.studio.freecam.locked_to_path =
                    !self.side.studio.freecam.locked_to_path;
            }
            KeyCode::KeyK => self.handle_studio_timeline_action(TimelineAction::AddCameraKeyframe),
            KeyCode::Delete => {
                if let Some(index) = self.side.studio.timeline.selected_keyframe {
                    self.handle_studio_timeline_action(TimelineAction::RemoveCameraKeyframe(index));
                }
            }
            _ => {}
        }
    }
}
