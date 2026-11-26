//! Read-only block, tag and fluid queries for native mods.
use crate::game::CommonGame as Game;
use mod_api::BlockKey;
use mod_api::{FluidKey, FluidState, LevelRef, LightLayer, UnresolvedFluid};
use sunrise_core::geometry::Direction;
pub use sunrise_items::items::Items;
pub use sunrise_level::world::World;
pub use sunrise_world::{
    fluid::{FluidHeightWorld, FluidKind, FluidState as EngineFluidState},
    level::{Behaviours, fluid_can_be_replaced_with},
    registry::{BlockRegistry, BlockStateId},
};

/// Shared by the level handle and the restricted block-hook view.
pub fn block_face_sturdy(
    world: &World,
    b: &Behaviours,
    binding: &sunrise_world::block_collision::BlockCollisionRegistry<'_>,
    pos: [i32; 3],
    face: mod_api::BlockFace,
    support: mod_api::SupportType,
) -> Option<bool> {
    if !world.is_loaded_at(pos[0], pos[1], pos[2]) {
        return None;
    }
    let state = world.block(pos[0], pos[1], pos[2]);
    let direction = block_face_direction(face);
    if !b.has_cached_support(state) || binding.publishes_shapes(state) {
        use sunrise_level::collisions::SceneCollisions;
        use sunrise_world::{block_support::SupportType, dynamic_support::DynamicSupport};
        let mut scene = SceneCollisions {
            world,
            binding,
            subject: None,
        };
        let support = match support {
            mod_api::SupportType::Full => SupportType::Full,
            mod_api::SupportType::Center => SupportType::Center,
            mod_api::SupportType::Rigid => SupportType::Rigid,
            _ => unreachable!("mod_api::SupportType: unhandled variant"),
        };
        // A state that may publish takes its block entity's shape, else its declared one.
        if binding.publishes_shapes(state) {
            let shape = binding.collision_shape(&mut scene, state, pos).ok()?;
            return support.is_supporting(&shape, direction).ok();
        }
        let dynamic = DynamicSupport::from_state(binding.registry().state(state)?).ok()??;
        return dynamic
            .is_face_sturdy(&mut scene, pos, direction, support)
            .ok();
    }
    Some(match support {
        mod_api::SupportType::Full => b.is_face_sturdy(state, direction),
        mod_api::SupportType::Center => b.is_face_sturdy_center(state, direction),
        mod_api::SupportType::Rigid => b.is_face_sturdy_rigid(state, direction),
        _ => unreachable!("mod_api::SupportType: unhandled variant"),
    })
}

/// Read the published sky light without requiring a block or a resident column.
pub fn can_see_sky(world: &World, pos: [i32; 3]) -> bool {
    world.shape.sky_light && world_brightness(world, LightLayer::Sky, pos) >= 15
}

thread_local! {
    // Like the key-to-state mapping, this snapshot belongs to the thread resolving the world.
    static BLOCK_TAGS: std::cell::RefCell<std::collections::HashMap<String, std::collections::HashSet<BlockKey>>> = Default::default();
}

/// Bind the world's merged tags alongside its block mappings, including the resource-only case.
/// Never cache on Items: its string tables also serve tools and client network snapshots.
pub fn prepare_block_tags(
    tags: &std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> std::collections::HashMap<String, std::collections::HashSet<BlockKey>> {
    tags.iter()
        .map(|(tag, members)| {
            let mut set = std::collections::HashSet::<BlockKey>::default();
            for name in members {
                if let Some(block) = mod_api::__private::block_key(name) {
                    set.insert(block);
                }
            }
            (tag.clone(), set)
        })
        .collect()
}

pub fn install_block_tags(
    tags: std::collections::HashMap<String, std::collections::HashSet<BlockKey>>,
) {
    BLOCK_TAGS.with(|snapshot| *snapshot.borrow_mut() = tags);
}

#[cfg(any(test, feature = "test-support"))]
pub fn installed_block_tag_snapshot()
-> std::collections::HashMap<String, std::collections::HashSet<BlockKey>> {
    BLOCK_TAGS.with(|snapshot| snapshot.borrow().clone())
}

/// One availability check, one tag-name lookup and one API key membership test.
/// An unavailable owning-thread mapping is not a missing tag.
pub fn block_in_tag(block: BlockKey, tag: &str) -> Option<bool> {
    super::api_values::default_state(block)?;
    let tag = sunrise_command::args::normalized(tag);
    Some(BLOCK_TAGS.with(|snapshot| {
        snapshot
            .borrow()
            .get(tag.as_ref())
            .is_some_and(|set| set.contains(&block))
    }))
}

/// `BlockState.getFluidState`. None for an id no state has.
pub(in crate::game) fn state_fluid(
    behaviours: &Behaviours,
    registry: &BlockRegistry,
    state: BlockStateId,
) -> Option<FluidState> {
    registry.state(state)?;
    api_fluid_state(behaviours.fluid(state))
}

/// Convert the engine's state using the load-owned kind-to-key mapping.
pub fn api_fluid_state(state: EngineFluidState) -> Option<FluidState> {
    Some(mod_api::__private::fluid_state(
        match state.kind().vanilla_index() {
            Some(index) => mod_api::__private::world_fluid_key(usize::from(index))?,
            None => mod_api::__private::fluid_key(state.kind().name())?,
        },
        state.amount(),
        state.falling(),
        state.is_source(),
    ))
}

/// `FluidState.is(TagKey)` over the merged fluid tags. The empty fluid is in none.
pub fn fluid_in_tag(items: &Items, fluid: &str, tag: &str) -> bool {
    items.fluid_in_tag(
        &sunrise_command::args::normalized(tag),
        &sunrise_command::args::normalized(fluid),
    )
}

/// `FluidState.getShape`'s one box for `fluid` with `above` over it: up to a full block under
/// the same fluid (`FluidState.getHeight`), else to the fluid's own height. None for no fluid.
pub fn fluid_shape(fluid: EngineFluidState, above: Option<EngineFluidState>) -> Option<[f64; 6]> {
    if fluid.kind() == FluidKind::Empty {
        return None;
    }
    let height = if above.is_some_and(|above| above.kind().same_family(fluid.kind())) {
        1.0
    } else {
        f64::from(fluid.own_height())
    };
    Some([0.0, 0.0, 0.0, 1.0, height, 1.0])
}

/// The fluid a registry id names, with or without `minecraft:`.
fn fluid_kind(fluid: &str) -> Option<FluidKind> {
    FluidKind::from_name(&sunrise_command::args::normalized(fluid))
}

/// The world fluid a registered key names. None for a tank-only fluid, which has no world
/// form, and for a key the registry doesn't hold.
pub fn engine_fluid_kind(fluid: FluidKey) -> Option<FluidKind> {
    FluidKind::from_name(mod_api::__private::fluid_id(fluid)?)
}

/// The blocks around one position of a level, as `FluidState.getHeight` reads them: an unloaded
/// block is empty and has no sturdy underside.
struct FluidColumn<'a, 'g, S: crate::game::Side> {
    game: &'a Game<'g, S>,
    level: LevelRef,
}
impl<S: crate::game::Side> FluidColumn<'_, '_, S> {
    fn fluid_at(&self, pos: [i32; 3]) -> EngineFluidState {
        self.game
            .level_block_state(self.level, pos)
            .map_or(EngineFluidState::EMPTY, |state| {
                self.game.res.behaviours().fluid(state)
            })
    }
}
impl<S: crate::game::Side> FluidHeightWorld for FluidColumn<'_, '_, S> {
    type Error = std::convert::Infallible;
    fn fluid(&mut self, pos: [i32; 3]) -> Result<EngineFluidState, Self::Error> {
        Ok(self.fluid_at(pos))
    }
    fn sturdy_down(&mut self, pos: [i32; 3]) -> Result<bool, Self::Error> {
        Ok(self
            .game
            .level_block_state(self.level, pos)
            .is_some_and(|state| {
                self.game
                    .res
                    .behaviours()
                    .is_face_sturdy(state, Direction::Down)
            }))
    }
}

impl crate::game::CommonGame<'_> {
    /// The saved name for an engine-issued unresolved fluid.
    pub(in crate::game) fn unresolved_fluid_id(&self, fluid: UnresolvedFluid) -> String {
        mod_api::__private::unresolved_fluid_name(fluid)
    }

    /// Whether the resolved block is in the world's merged block tag, on a client the tag its
    /// server sent. Unknown tags return false; unavailable tags or owning-thread block mappings
    /// return None.
    pub(in crate::game) fn block_in_tag(&self, block: BlockKey, tag: &str) -> Option<bool> {
        self.native_tag_items()?;
        block_in_tag(block, tag)
    }

    /// The outline of `state` as `BlockState.getShape` answers it with an empty getter at the
    /// origin (`EmptyBlockGetter.INSTANCE`, `BlockPos.ZERO`): its boxes, each `[min x, y, z,
    /// max x, y, z]` in the block's own space, with no random offset and none of a world's
    /// context (a state that reads its neighbours or block entity answers what its state alone
    /// gives). Empty for a state with no outline, such as air. None for an id no state has.
    pub(in crate::game) fn state_outline(&self, state: BlockStateId) -> Option<Vec<[f64; 6]>> {
        self.pack.assets.registry.state(state)?;
        Some(self.res.facts().get(state).outline.to_vec())
    }

    /// The fluid state `state` holds (`BlockState.getFluidState`): water for a waterlogged state,
    /// the empty fluid for most. None for an id no state has.
    pub fn state_fluid_state(&self, state: BlockStateId) -> Option<FluidState> {
        state_fluid(self.res.behaviours(), self.pack.assets.registry, state)
    }

    /// Whether `fluid` is in the fluid tag `tag` (`FluidState.is(TagKey)`, `minecraft:water` or
    /// `water`), over the merged fluid tags, on a client those its server sent. An unknown tag
    /// is false. None as for `block_in_tag`.
    pub(in crate::game) fn fluid_in_tag(&self, fluid: &str, tag: &str) -> Option<bool> {
        Some(fluid_in_tag(self.native_tag_items()?, fluid, tag))
    }

    /// The fluid's shape at `pos` (`FluidState.getShape`), block-local: none for no fluid, else
    /// the floor to `FluidState.getHeight`, a full block under the same fluid and the fluid's
    /// own height otherwise. None as for `level_block_state`; an unloaded block above is no
    /// fluid.
    pub(in crate::game) fn level_fluid_shape(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<Vec<[f64; 6]>> {
        let behaviours = self.res.behaviours();
        let fluid = behaviours.fluid(self.level_block_state(level, pos)?);
        let above = self
            .level_block_state(level, [pos[0], pos[1] + 1, pos[2]])
            .map(|state| behaviours.fluid(state));
        Some(fluid_shape(fluid, above).into_iter().collect())
    }

    /// `FluidState.getHeight(level, pos)` of the fluid at `pos`. None for a stale handle or a
    /// position that isn't loaded, as `level_block_state`.
    pub(in crate::game) fn level_fluid_height(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<f32> {
        let mut column = FluidColumn { game: self, level };
        self.level_block_state(level, pos)?;
        let Ok(height) = column.fluid_at(pos).height_at(&mut column, pos);
        Some(height)
    }

    /// The surface height the camera's immersion test reads at `pos`
    /// (`FluidState::camera_height_at`). None as for `level_fluid_height`.
    pub(in crate::game) fn level_fluid_height_for_camera(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<f32> {
        let mut column = FluidColumn { game: self, level };
        self.level_block_state(level, pos)?;
        let Ok(height) = column.fluid_at(pos).camera_height_at(&mut column, pos);
        Some(height)
    }

    /// `FluidState.canBeReplacedWith(level, pos, fluid, direction)` for the fluid at `pos`. False
    /// for a `fluid` with no world form; None as for `level_fluid_height`.
    pub(in crate::game) fn level_fluid_can_be_replaced_with(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        fluid: FluidKey,
        direction: Direction,
    ) -> Option<bool> {
        let mut column = FluidColumn { game: self, level };
        self.level_block_state(level, pos)?;
        let here = column.fluid_at(pos);
        let Some(other) = engine_fluid_kind(fluid) else {
            return Some(false);
        };
        Some(fluid_can_be_replaced_with(here, other, direction, || {
            let Ok(height) = here.height_at(&mut column, pos);
            height
        }))
    }

    /// Whether `fluid` and `other` are the same fluid (`Fluid.isSame`): a source and its
    /// flowing form are, so `minecraft:water` and `minecraft:flowing_water` are the same, water
    /// and lava not. Names are fluid ids, with or without `minecraft:`; false when either
    /// names no fluid.
    pub(in crate::game) fn fluid_is_same(&self, fluid: &str, other: &str) -> bool {
        match (fluid_kind(fluid), fluid_kind(other)) {
            (Some(fluid), Some(other)) => fluid.same_family(other),
            _ => false,
        }
    }

    /// The bucket item that picks `fluid` up (`Fluid.getBucket`), `minecraft:water_bucket` for
    /// water and flowing water. None for `minecraft:empty`, whose bucket is air, and for a name
    /// no fluid has.
    pub(in crate::game) fn fluid_bucket(&self, fluid: &str) -> Option<mod_api::ItemKey> {
        mod_api::__private::item_key(sunrise_world::level::fluid_bucket(fluid_kind(fluid)?)?)
    }

    /// The sound event a bucket plays as it picks `fluid` up (`Fluid.getPickupSound`),
    /// `minecraft:item.bucket.fill_lava` for lava and flowing lava. None for
    /// `minecraft:empty`, which has none, and for a name no fluid has.
    pub(in crate::game) fn fluid_pickup_sound(&self, fluid: &str) -> Option<&'static str> {
        sunrise_world::level::fluid_pickup_sound(fluid_kind(fluid)?)
    }

    /// The sound event a bucket plays as it empties `fluid` (`BucketItem.playEmptySound`),
    /// `minecraft:item.bucket.empty_lava` for lava and flowing lava. None for
    /// `minecraft:empty`, for a tank-only fluid and for a name no fluid has.
    pub(in crate::game) fn fluid_empty_sound(&self, fluid: &str) -> Option<&'static str> {
        sunrise_world::level::fluid_empty_sound(fluid_kind(fluid)?)
    }
}

/// The stored light of `layer` at `pos` in `world`, as the game's own light query reads it.
pub(in crate::game) fn world_brightness(world: &World, layer: LightLayer, pos: [i32; 3]) -> i32 {
    let light = world.light(pos[0], pos[1], pos[2]);
    i32::from(match layer {
        LightLayer::Block => light & 15,
        LightLayer::Sky => light >> 4,
    })
}

/// The engine's direction of a mod API block face.
pub fn block_face_direction(
    face: mod_api::BlockFace,
) -> sunrise_core::geometry::Direction {
    use mod_api::BlockFace;
    use sunrise_core::geometry::Direction;
    match face {
        BlockFace::Down => Direction::Down,
        BlockFace::Up => Direction::Up,
        BlockFace::North => Direction::North,
        BlockFace::South => Direction::South,
        BlockFace::West => Direction::West,
        BlockFace::East => Direction::East,
        _ => unreachable!("BlockFace: unhandled variant"),
    }
}

use crate::game::tables::WorldTablesExt as _;
