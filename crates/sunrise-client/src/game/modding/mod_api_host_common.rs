//! Common `AttachmentHost` impl and shared level-weather helpers across server and client roles.
use super::Game;
use mod_api::AttachmentAccess;

pub(in crate::game) fn weather_for_level(
    shape: sunrise_client::chunk::LevelShape,
    is_end: bool,
    levels: (f32, f32),
) -> (f32, f32) {
    if shape.sky_light && !shape.has_ceiling && !is_end {
        levels
    } else {
        (0.0, 0.0)
    }
}

impl mod_api::AttachmentHost for Game<'_> {
    fn validate_attachment(
        &self,
        namespace: &str,
        name: &str,
        sync: bool,
    ) -> Result<(), mod_api::AttachmentError> {
        mod_api::__private::validate_attachment_use(namespace, name, sync)
    }

    fn attachments(
        &mut self,
        holder: mod_api::Holder,
        access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        self.side_attachment_map(holder, access)
    }
}
