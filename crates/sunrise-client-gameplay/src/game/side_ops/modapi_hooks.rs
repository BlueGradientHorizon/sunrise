#[cfg(feature = "client")]
#[allow(unused_variables)]
impl SideOps for crate::game::client_side::ClientState {
    type Game<'g> = crate::game::ClientGame<'g>;

    fn side_has_remote(&self) -> bool {
        self.remote.is_some()
    }

    fn side_fire_pending_block_entity_loads(game: &mut Self::Game<'_>) {
        {
            game.client_fire_pending_block_entity_loads();
        }
    }
    fn side_creative_holders(game: &Self::Game<'_>) -> Option<Holders> {
        game.client_creative_holders()
    }
    fn side_creative_catalog<'__side_g>(
        game: &'__side_g Self::Game<'_>,
    ) -> &'__side_g Arc<Catalog> {
        game.client_creative_catalog()
    }
    fn side_reset_creative_screen(game: &mut Self::Game<'_>) {
        {
            game.client_reset_creative_screen();
        }
    }
    fn side_ask_swim_eligibility(game: &Self::Game<'_>, check: SwimmingCheck) -> Option<bool> {
        game.client_ask_swim_eligibility(check)
    }
    fn side_acting_id(game: &mut Self::Game<'_>) -> u128 {
        game.client_acting_id()
    }
    fn side_clicker(game: &mut Self::Game<'_>) -> (u128, i32) {
        game.client_clicker()
    }
    fn side_acting_or_local_player(game: &Self::Game<'_>) -> Option<PlayerRef> {
        game.client_acting_or_local_player()
    }
    fn side_chunks_loaded(game: &mut Self::Game<'_>) {
        {
            game.client_chunks_loaded();
        }
    }
    fn side_chunks_unloaded(game: &mut Self::Game<'_>, unloaded: &[Arc<Chunk>]) {
        {
            game.client_chunks_unloaded(unloaded);
        }
    }
    fn side_apply_after_use_components(game: &mut Self::Game<'_>, stack: &ItemStack) {
        {
            game.client_apply_after_use_components(stack);
        }
    }
}
use mod_api::PlayerRef;
use mod_api::SwimmingCheck;
use std::sync::Arc;
use sunrise_gameplay_common::game::side_ops::modapi_hooks::SideOps;
use sunrise_items::creative_catalog::Catalog;
use sunrise_items::creative_catalog::Holders;
use sunrise_items::items::ItemStack;
use sunrise_level::chunk::Chunk;
