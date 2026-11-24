#[allow(unused_imports)]
pub(in crate::game::server_side) use super::super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{CartKind, Minecart};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::game_tests::tests::server_game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::minecart_tnt_tests::server;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::minecart_tnt_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::items::ItemStack;

/// A lone TNT cart at (0.5, 64, 0.5); a silent one keeps `Silent:1b` where
/// `Entity.saveWithoutId` does.
pub(in crate::game) fn tnt_cart(game: &mut Game<'_>, id: u32, silent: bool) {
    game.carts.clear();
    game.carts
        .push(Minecart::new(id, CartKind::Tnt, Vec3::new(0.5, 64.0, 0.5)));
    game.persist.raw.remove(&id);
    game.server.lan_entity_events.clear();
    if silent {
        let (tag, _) = sunrise_core::nbt::parse_snbt("{Silent:1b}").unwrap();
        game.persist.raw.insert(id, tag);
    }
}
