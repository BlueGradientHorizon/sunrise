

use super::world_loading;
use sunrise_game_data::Result;

#[cfg(test)]
impl crate::game::ServerGame<'_> {
    #[cfg(test)]
    pub(in crate::game) fn load_world(
        &mut self,
        request: world_loading::WorldRequest,
    ) -> Result<()> {
        let prepared = self.prepare_world_and_rules(request)?;
        self.finish_open_world(prepared).map(|_| ())
    }
}