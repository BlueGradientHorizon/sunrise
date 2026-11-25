use super::*;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn tick_weather_effects(&mut self) {
        if self.flash > 0 {
            self.flash -= 1;
        }
        // The rain sounds come with the rain splashes (`tick_particles`).
    }
}
