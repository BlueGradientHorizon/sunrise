//! `MinecartCommandBlock`: its `BaseCommandBlock` run by a powered activator
//! rail (`performCommand`, `CloseableCommandBlockSource`), the entity data
//! that carries its command and last output to clients, and the edit screen's
//! `ServerboundSetCommandMinecartPacket` with `handleSetCommandMinecart`.
use super::{Game, command_blocks::CommandBlockScreen, minecarts::CartKind};
use sunrise_client_core::text::Text;
use sunrise_level::command_block::Mode;
use sunrise_protocol::packets::play::{self, Serverbound as S};

impl Game<'_, crate::game::client_side::ClientState> {
    /// `LocalPlayer.openMinecartCommandBlock`: the editor over the cart's
    /// command (`MinecartCommandBlockEditScreen`).
    pub(in crate::game::client_side) fn open_cart_command_block(&mut self, id: u32) {
        let Some(block) = self
            .carts
            .iter()
            .find(|c| c.id == id && c.kind == CartKind::CommandBlock)
            .map(|c| c.command.clone())
        else {
            return;
        };
        self.open_edit_screen((0, 0, 0), Some(id), block, Mode::Redstone, false);
    }
    /// Edit screen's Done for a cart: updates local track flag and sends `SetCommandMinecart`.
    pub(in crate::game::client_side) fn set_command_minecart(
        &mut self,
        id: u32,
        s: &CommandBlockScreen,
    ) {
        debug_assert!(
            !self.is_logical_server(),
            "set_command_minecart called on Server role"
        );
        if !self.can_use_game_master_blocks() {
            self.client_system_message(Text::translate("advMode.notAllowed", &[]));
            return;
        }
        let command = s.command.value().to_owned();
        let track_output = s.track_output;
        // The output button set the cart's own flag as it was pressed.
        self.edit_command_cart(id, |c| c.command.track_output = track_output);
        self.send(S::SetCommandMinecart(play::SetCommandMinecart {
            entity: id as i32,
            command,
            track_output,
        }));
        if !track_output {
            self.edit_command_cart(id, |c| c.command.last_output = None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::minecarts::Minecart;
    use sunrise_core::geometry::Vec3;

    #[test]
    #[ignore = "daily"]
    fn scoped_cart_commands_keep_tick_and_self_removal_rules() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.world.game_time = 10;
            game.carts
                .push(Minecart::new(41, CartKind::CommandBlock, Vec3::ZERO));
            game.carts
                .push(Minecart::new(42, CartKind::Rideable, Vec3::ZERO));
            game.carts.update(0, |c| c.command.set_command("Searge"));
            assert!(game.perform_cart_command(41));
            assert_eq!(game.carts[0].command.success_count, 1);
            game.carts.update(0, |c| c.command.set_command(""));
            assert!(game.perform_cart_command(41));
            assert_eq!(game.carts[0].command.success_count, 0);
            assert_eq!(game.carts[0].command.last_execution, 10);
            assert!(!game.perform_cart_command(41));
            assert!(!game.perform_cart_command(42));
            assert!(!game.perform_cart_command(99));
            game.world.game_time = 11;
            game.carts.update(0, |c| c.command.set_command("kill @s"));
            assert!(game.perform_cart_command(41));
            assert_eq!(game.carts.len(), 1);
            assert_eq!(game.carts[0].id, 42);
            assert_eq!(game.carts[0].command.success_count, 0);
        });
    }
}
