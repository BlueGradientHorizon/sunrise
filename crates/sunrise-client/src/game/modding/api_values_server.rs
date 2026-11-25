use super::api_values::{TryIntoApi, TryIntoEngine};
use sunrise_items::items::ItemStack as EngineStack;

impl super::Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn api_grindstone_place(
        &mut self,
        player: u128,
        entity: i32,
        pos: [i32; 3],
        inputs: [Option<&EngineStack>; 3],
        cancel: &mut bool,
    ) -> crate::menu::GrindstonePlaceEdits {
        use mod_api::item::menu::{GrindstonePlaceEdits, GrindstonePlaceInputs};

        let Ok([top, bottom, vanilla_output]) = inputs.try_into_api() else {
            return Default::default();
        };
        let inputs = GrindstonePlaceInputs {
            top: top.as_ref(),
            bottom: bottom.as_ref(),
            vanilla_output: vanilla_output.as_ref(),
        };
        let mut edits = GrindstonePlaceEdits {
            output: None,
            xp: -1,
        };
        let tx_depth = self.transactions.len();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.native_grindstone_place(player, entity, pos, &inputs, &mut edits, cancel);
        }))
        .is_err();
        self.abort_abandoned_transactions(tx_depth);
        if panicked {
            log::error!("grindstone_place: listener panicked at {pos:?}");
            *cancel = false;
            return Default::default();
        }
        if *cancel {
            return Default::default();
        }
        let xp = edits.xp.max(-1);
        let output = match edits.output.try_into_engine(self.items()) {
            Ok(output) => output,
            Err(err) => {
                log::warn!(
                    "grindstone_place: ignoring invalid output replacement at {pos:?}: {err:?}"
                );
                None
            }
        };
        crate::menu::GrindstonePlaceEdits { output, xp }
    }
    pub(in crate::game) fn api_grindstone_take(
        &mut self,
        player: u128,
        entity: i32,
        pos: [i32; 3],
        take: &crate::menu::PendingGrindstoneTake,
        cancel: &mut bool,
    ) -> crate::menu::GrindstoneTakeEdits {
        use mod_api::item::menu::{GrindstoneTakeEdits, GrindstoneTakeInputs};

        let initial_xp = if take.xp >= 0 {
            take.xp
        } else {
            let amount = take.vanilla_amount(&self.menu_env());
            self.journal_level_random();
            crate::stations::grindstone_xp_roll(amount, &mut self.world.random) as i32
        };
        let Ok([top, bottom]) = [take.top.as_ref(), take.bottom.as_ref()].try_into_api() else {
            return crate::menu::GrindstoneTakeEdits::initial(initial_xp);
        };
        let inputs = GrindstoneTakeInputs {
            top: top.as_ref(),
            bottom: bottom.as_ref(),
        };
        let mut edits = GrindstoneTakeEdits {
            xp: initial_xp,
            new_top: None,
            new_bottom: None,
        };
        let tx_depth = self.transactions.len();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.native_grindstone_take(player, entity, pos, &inputs, &mut edits, cancel);
        }))
        .is_err();
        self.abort_abandoned_transactions(tx_depth);
        if panicked {
            log::error!("grindstone_take: listener panicked at {pos:?}");
            *cancel = false;
            return crate::menu::GrindstoneTakeEdits::initial(initial_xp);
        }
        if *cancel {
            return crate::menu::GrindstoneTakeEdits::initial(initial_xp);
        }
        let new_top = match edits.new_top.try_into_engine(self.items()) {
            Ok(stack) => stack,
            Err(err) => {
                log::warn!("grindstone_take: ignoring invalid top remainder at {pos:?}: {err:?}");
                None
            }
        };
        let new_bottom = match edits.new_bottom.try_into_engine(self.items()) {
            Ok(stack) => stack,
            Err(err) => {
                log::warn!(
                    "grindstone_take: ignoring invalid bottom remainder at {pos:?}: {err:?}"
                );
                None
            }
        };
        crate::menu::GrindstoneTakeEdits {
            xp: edits.xp,
            new_top,
            new_bottom,
        }
    }
}

impl super::Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game::server_side) fn api_block_entity_update_tag(
        pos: [i32; 3],
        entity: &crate::containers::ModBlockEntity,
        tag: &mut Option<sunrise_core::nbt::Compound>,
    ) {
        if !mod_api::__private::has_part_hook(mod_api::HookId::BlockEntityUpdateTag) {
            return;
        }
        if let Ok(api) = entity.try_into_api() {
            crate::game::native_hooks::native_block_entity_update_tag(pos, &api, tag);
        }
    }
}
