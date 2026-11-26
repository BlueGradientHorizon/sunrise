//! `Game` as the `MobLevel` mob families run on: each service forwards to the game's own method.
use super::Game;
use sunrise_gameplay::mob_level::{MobLevel, MobParts};
use sunrise_sim::entities::ItemEntity;

impl Game<'_, crate::game::server_side::ServerState> {
    /// Notify tracking clients of a server-side mob item pickup without client particle bookkeeping.
    pub(in crate::game) fn server_mob_take_item(&mut self, item: &ItemEntity, collector: u32) {
        if let Some(host) = &self.lan {
            host.hub.take_item(
                item.id as i32,
                collector as i32,
                item.stack.count as i32,
                [item.position.x, item.position.y, item.position.z],
            );
        }
    }
}

impl MobLevel for Game<'_, crate::game::server_side::ServerState> {
    fn parts(&mut self) -> MobParts<'_> {
        MobParts {
            mobs: &mut self.mobs,
            dropped: &mut self.dropped,
            next_entity: self.side.next_entity,
            ticked: &self.creatures.ticked,
            items: world_items!(self),
            facts: self.res.facts(),
            world: &mut self.world,
        }
    }
    fn mob_griefing(&self) -> bool {
        self.ui
            .rules
            .bool_at(sunrise_game_data::gamerules::vanilla::MOB_GRIEFING)
    }
    fn entity_grief(&self, entity: u32, rule: bool) -> bool {
        Game::entity_grief(self, entity, rule)
    }
    fn spawn_item_own(
        &mut self,
        stack: sunrise_items::items::ItemStack,
        position: [f64; 3],
        pickup_delay: u32,
    ) -> [f64; 3] {
        Game::spawn_item_own(self, stack, position, pickup_delay)
    }
    fn just_spawned_item(&mut self) -> Option<&mut ItemEntity> {
        Game::just_spawned_item(self)
    }
    fn mob_uuid(&mut self, id: u32) -> [i32; 4] {
        Game::mob_uuid(self, id)
    }
    fn play_sound(&mut self, event: &str, at: [f64; 3], volume: f32, pitch: f32) {
        self.server_play_sound(event, at, volume, pitch);
    }
    fn on_item_pickup(&mut self, mob_index: usize, item: &ItemEntity) {
        self.mob_item_pickup_trigger(mob_index, item);
    }
    fn take(&mut self, collector: u32, item: ItemEntity) {
        self.server_mob_take_item(&item, collector);
    }
    fn discard_item(&mut self, index: usize) -> ItemEntity {
        self.remove_dropped(index)
    }
}
