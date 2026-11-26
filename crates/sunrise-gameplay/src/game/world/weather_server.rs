impl crate::game::ServerGame<'_> {
    pub(in crate::game) fn tick_weather_effects(&mut self) {
        if self.common.flash > 0 {
            self.common.flash -= 1;
        }
        // The rain sounds come with the rain splashes (`tick_particles`).
    }
}
