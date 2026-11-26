//! The engine side of `rotate`: the callback every template placement turns its states through,
//! asking the parts of each state's block on the worldgen workers and on the server thread. It
//! reads what the server published (`native_worldgen_parts`), never a game thread's tables.
use super::api_values::block_state::Index;
pub use super::native_worldgen_parts;
pub use mod_api::{BlockState, HookId, StructureRotation};
pub use sunrise_world::registry::BlockStateId as EngineState;
pub use sunrise_world::worldgen::structure::Rotation;

/// The engine's `rotate` callback: the state the parts of `state`'s block place at `pos` for
/// `state` turned by `rotation`, starting from the game's `table`. Nothing is read or converted
/// while no part answers the hook; a refused answer or a panicking part keeps `table`.
pub fn rotate(
    state: EngineState,
    pos: [i32; 3],
    rotation: Rotation,
    table: EngineState,
) -> EngineState {
    if !mod_api::__private::has_part_hook(HookId::Rotate) {
        return table;
    }
    let Some(index) = native_worldgen_parts::index() else {
        return table;
    };
    if !index.worldgen_listens(HookId::Rotate, state) {
        return table;
    }
    let (Ok(asked), Ok(mut rotated)) = (index.to_api(state), index.to_api(table)) else {
        return table;
    };
    let rotation = match rotation {
        Rotation::None => StructureRotation::None,
        Rotation::Clockwise90 => StructureRotation::Clockwise90,
        Rotation::Clockwise180 => StructureRotation::Clockwise180,
        Rotation::Counterclockwise90 => StructureRotation::Counterclockwise90,
    };
    if let Err(error) = sunrise_level::crash::contain(|| {
        native_worldgen_parts::with_host(|host| {
            mod_api::world::rotation::native_rotate(host, asked, pos, rotation, &mut rotated);
        });
    }) {
        log::warn!("rotate callback failed: {error}; the template places the game's state");
        return table;
    }
    turned(&index, asked, rotated).unwrap_or(table)
}

/// The parts' answer for `state`, as the engine places it: None for a state of another block or
/// one the game's registry does not have, naming the mods whose parts answer for `state`'s
/// block.
fn turned(index: &Index, state: BlockState, answer: BlockState) -> Option<EngineState> {
    let refused = |why: &str| {
        log::warn!(
            "rotate: a part of {} answered {why}; the template places the game's state",
            mod_api::__private::worldgen_part_mods(HookId::Rotate, state).join(", ")
        );
        None
    };
    if answer.block() != state.block() {
        return refused("a state of another block");
    }
    match index.to_engine(answer) {
        Ok(placed) => Some(placed),
        Err(error) => refused(&format!(
            "a block state this game does not have ({error:?})"
        )),
    }
}
