//! Common `AttachmentHost` impl and shared level-weather helpers across server and client roles.
use super::{Dim, Game};
use mod_api::AttachmentAccess;
use sunrise_client::world::World;

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

impl Game<'_> {
    /// `Level.getSkyDarken`: the same dimension, time and weather calculation used by gameplay.
    pub(in crate::game) fn sky_darken_in(&self, world: &World, dimension: Dim) -> i32 {
        let environment = match self
            .remote
            .as_ref()
            .and_then(|r| r.level_environment.as_deref())
            .filter(|_| dimension == self.dim)
        {
            Some(own) => own,
            None => match dimension {
                Dim::Overworld => self.res.environment(),
                Dim::Extra(_) => sunrise_client::environment::Environment::for_level(
                    dimension.id(),
                    self.res.environment(),
                ),
                Dim::Nether => self.res.nether_environment(),
                Dim::End => self.res.end_environment(),
            },
        };
        let (rain, thunder) = weather_for_level(
            world.shape,
            dimension == Dim::End,
            self.world.weather.levels(1.0),
        );
        let day_time = self.command_clock_time(&self.level_clock_name(dimension));
        sunrise_client::environment::sky_darken(environment, day_time, rain, thunder)
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
