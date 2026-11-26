#[cfg(test)]
pub(in crate::game) mod tests {
    use crate::game::client_side::native_hooks_client::client_options::{
        gui_options_snapshot, resolved_gui_scale,
    };

    #[test]
    fn gui_snapshot_tracks_advanced_tooltip_toggle() {
        let mut settings = sunrise_client::settings::Settings::default();
        for enabled in [false, true, false] {
            settings.file.set(
                "advancedItemTooltips",
                sunrise_client::options_file::V::Bool(enabled),
            );
            assert_eq!(
                gui_options_snapshot(&settings).advanced_item_tooltips,
                enabled
            );
        }
    }

    /// The snapshot carries the Damage Tilt option as the player sets it, off and full included.
    #[test]
    fn gui_snapshot_follows_the_damage_tilt_option() {
        let mut settings = sunrise_client::settings::Settings::default();
        assert_eq!(gui_options_snapshot(&settings).damage_tilt_strength, 1.0);
        for strength in [0.0, 0.35, 1.0] {
            settings.file.set(
                "damageTiltStrength",
                sunrise_client::options_file::V::Double(strength),
            );
            assert_eq!(
                gui_options_snapshot(&settings).damage_tilt_strength,
                strength
            );
        }
    }

    /// The literal `Window.calculateScale` outcomes the issue asks for: the
    /// option is a maximum, and the 320x240 floor wins over it.
    #[test]
    fn gui_scale_resolves_the_option_and_the_window() {
        assert_eq!(resolved_gui_scale(Some([1280, 720]), 0, false), Some(3));
        assert_eq!(resolved_gui_scale(Some([1920, 1080]), 0, false), Some(4));
        assert_eq!(resolved_gui_scale(Some([1920, 1080]), 2, false), Some(2));
        assert_eq!(resolved_gui_scale(Some([1280, 720]), 5, false), Some(3));
        assert_eq!(resolved_gui_scale(Some([320, 240]), 0, false), Some(1));
        // The Unicode font raises an odd resolved scale by one.
        assert_eq!(resolved_gui_scale(Some([1920, 1080]), 3, true), Some(4));
    }

    /// A window the renderer did not resize (an undrawable size keeps its previous
    /// configuration) resolves from the renderer's 1280x720, not the window's own
    /// 1920x1080, which alone would give 4.
    #[test]
    fn gui_scale_follows_the_size_the_renderer_applies() {
        assert_eq!(resolved_gui_scale(Some([1280, 720]), 0, false), Some(3));
        assert_eq!(resolved_gui_scale(Some([1920, 1080]), 0, false), Some(4));
    }

    #[test]
    fn gui_scale_needs_a_window() {
        assert_eq!(resolved_gui_scale(None, 0, false), None);
        assert_eq!(resolved_gui_scale(None, 4, true), None);
    }
}
