use super::*;
use mod_api::{
    ClientAudioOptions, ClientChatOptions, ClientGraphicsOptions, ClientGuiOptions,
    ClientInputOptions,
};

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl Game<'_, crate::game::client_side::ClientState> {
    /// The GUI scale in effect (`Window.getGuiScale`): the scale the window
    /// applies from the GUI Scale option and its size, at least 1; none on a
    /// server or when no window exists. `gui_options().gui_scale` is only the
    /// option, where 0 means automatic.
    pub(in crate::game) fn gui_scale(&self) -> Option<u32> {
        // The size the renderer applies, as the drawing code reads it: it keeps its previous
        // configuration when the window reports a size it cannot draw at.
        let framebuffer = self
            .side
            .window
            .0
            .as_ref()
            .and(self.side.renderer.0.as_ref())
            .map(|r| [r.config.width, r.config.height]);
        resolved_gui_scale(
            framebuffer,
            self.side.settings.gui_scale,
            self.side.settings.force_unicode,
        )
    }
    /// `Window.getWidth` and `getHeight`: the window's framebuffer pixels, the size
    /// `window_resize` reports; none on a server or when no window exists.
    pub(in crate::game) fn window_size(&self) -> Option<[u32; 2]> {
        let size = self.side.window.0.as_ref()?.inner_size();
        Some([size.width, size.height])
    }
    /// A snapshot of the current graphics preferences.
    pub(in crate::game) fn graphics_options(&self) -> ClientGraphicsOptions {
        let s = &self.side.settings;
        ClientGraphicsOptions {
            fov: s.fov,
            render_distance: s.render_distance,
            simulation_distance: s.simulation_distance,
            brightness: s.gamma as f32,
            max_fps: s.max_fps,
            vsync: s.vsync,
            fullscreen: s.fullscreen,
            view_bobbing: s.view_bobbing,
            smooth_lighting: s.smooth_lighting,
            particles: s.particles,
            entity_distance: s.entity_distance as f32,
            biome_blend: s.biome_blend,
        }
    }
    /// A snapshot of the current GUI and accessibility preferences.
    pub(in crate::game) fn gui_options(&self) -> ClientGuiOptions {
        gui_options_snapshot(&self.side.settings)
    }
    /// A snapshot of the current chat preferences.
    pub(in crate::game) fn chat_options(&self) -> ClientChatOptions {
        let s = &self.side.settings;
        ClientChatOptions {
            visibility: s.chat_visibility,
            colors: s.chat_colors,
            links: s.chat_links,
            opacity: s.chat_opacity as f32,
            scale: s.chat_scale as f32,
            width: s.chat_width as f32,
            height_focused: s.chat_height_focused as f32,
            height_unfocused: s.chat_height_unfocused as f32,
            line_spacing: s.chat_line_spacing as f32,
            delay: s.chat_delay,
        }
    }
    /// A snapshot of the current audio preferences.
    pub(in crate::game) fn audio_options(&self) -> ClientAudioOptions {
        ClientAudioOptions {
            master_volume: self.side.settings.master_volume as f32,
        }
    }
    /// A snapshot of the current input preferences.
    pub(in crate::game) fn input_options(&self) -> ClientInputOptions {
        let settings = &self.side.settings;
        ClientInputOptions {
            sensitivity: settings.sensitivity as f32,
            invert_x: settings.invert_x,
            invert_y: settings.invert_y,
            mouse_wheel_sensitivity: settings.mouse_wheel_sensitivity as f32,
            discrete_scroll: settings.discrete_scroll,
            toggle_crouch: settings.toggle_crouch,
            toggle_sprint: settings.toggle_sprint,
            auto_jump: settings.auto_jump,
        }
    }
}

/// Convert live client settings into the API-owned GUI snapshot.
pub(in crate::game) fn gui_options_snapshot(
    s: &sunrise_client::settings::Settings,
) -> ClientGuiOptions {
    ClientGuiOptions {
        gui_scale: s.gui_scale,
        attack_indicator: s.attack_indicator,
        main_hand_left: s.main_hand_left,
        reduced_debug_info: s.reduced_debug_info,
        advanced_item_tooltips: s.file.bool("advancedItemTooltips"),
        high_contrast: s.high_contrast,
        show_subtitles: s.show_subtitles,
        notification_display_time: s.notification_display_time as f32,
        text_background_opacity: s.text_background_opacity as f32,
        menu_blur: s.menu_blur,
        // The factor the engine's own hurt tilt reads (`fov.rs`, `bob_hurt`).
        damage_tilt_strength: s.file.double("damageTiltStrength"),
    }
}

/// `Window.getGuiScale` for a framebuffer (`None` without a window): the
/// window's own `GuiSize` resolution, the same one it draws with.
pub(in crate::game) fn resolved_gui_scale(
    framebuffer: Option<[u32; 2]>,
    gui_scale: u32,
    unicode: bool,
) -> Option<u32> {
    framebuffer.map(|size| sunrise_client::window::GuiSize::new(size, gui_scale, unicode).scale)
}
