//! The HUD-render gates `Hud`/`GameRenderer` apply before drawing.

pub(super) use sunrise_client_gameplay::hud_gates::{block_outline_drawn, debug_crosshair_drawn};
pub(super) use sunrise_gameplay::game_master_blocks::is_game_master_block;
