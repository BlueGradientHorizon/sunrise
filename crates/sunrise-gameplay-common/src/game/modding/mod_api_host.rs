use super::api_values::{TryIntoApi, TryIntoEngine};
pub use mod_api::{AttachmentAccess, BlockPos, Direction};
use {
    sunrise_mod_runtime::attachments::{ATTACHMENTS_KEY, attachment_compound},
    sunrise_world::level::{LevelEvent, Ticks},
};

pub fn world_attachments(
    world: &mut sunrise_level::world::World,
    holder: mod_api::Holder,
) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
    use mod_api::{AttachmentError, Holder};
    if holder == Holder::Level {
        return Ok(&mut world.attachments);
    }
    if let Holder::BlockEntity([x, y, z]) = holder {
        let pos = (x, y, z);
        let chunk = world
            .chunks
            .get(&(x >> 4, z >> 4))
            .ok_or(AttachmentError::MissingHolder)?;
        attachment_compound(
            chunk
                .saved
                .as_ref()
                .and_then(|e| e.block_entity_sources.iter().find(|(p, _)| *p == pos))
                .and_then(|(_, tag)| tag.get(ATTACHMENTS_KEY)),
        )?;
        let map = world
            .block_entities
            .attachments_mut(pos)
            .ok_or(AttachmentError::MissingHolder)?;
        world.modified.insert((x >> 4, z >> 4));
        return Ok(map);
    }
    let Holder::Chunk([x, z]) = holder else {
        return Err(AttachmentError::MissingHolder);
    };
    let map = world.chunks.attachments_mut(&(x, z))?;
    world.modified.insert((x, z));
    Ok(map)
}

/// A level a both-side level hook reads, which knows the side it is on.
pub(in crate::game) trait SidedLevel: mod_api::LevelHost {
    fn client_side(&self) -> bool;
}

impl SidedLevel for super::GameLevel<'_> {
    fn client_side(&self) -> bool {
        sunrise_world::level::LevelAccess::is_client_side(self)
    }
}

impl<
    'a: 'b,
    'b,
    L: std::ops::Deref<Target = super::GameLevel<'a>>,
    T: std::ops::Deref<Target = Ticks>,
> SidedLevel for LevelTicks<'b, L, T>
{
    fn client_side(&self) -> bool {
        self.level.client_side()
    }
}

impl SidedLevel for super::native_hooks::HitLevel<'_> {
    fn client_side(&self) -> bool {
        self.0.client_side()
    }
}

pub(in crate::game) fn level_side(level: &impl SidedLevel) -> mod_api::LevelSide<'_> {
    if level.client_side() {
        mod_api::LevelSide::Client(level)
    } else {
        mod_api::LevelSide::Server(level)
    }
}

/// The prediction's block states and the live block entities it invalidated.
pub(in crate::game) trait PredictedLevel:
    sunrise_world::level::LevelAccess
{
    fn block_entity_removed(&self, pos: BlockPos) -> bool;
}

/// The level read by fire-odds listeners, including edits already made in a dry prediction.
pub(in crate::game) struct FireOddsLevel<'a> {
    pub(in crate::game) base: &'a super::GameLevel<'a>,
    pub(in crate::game) prediction: Option<&'a dyn PredictedLevel>,
}

impl FireOddsLevel<'_> {
    fn block_entity_removed(&self, [x, y, z]: [i32; 3]) -> bool {
        self.prediction
            .is_some_and(|level| level.block_entity_removed(BlockPos::new(x, y, z)))
    }

    fn block(&self, [x, y, z]: [i32; 3]) -> sunrise_world::registry::BlockStateId {
        let pos = BlockPos::new(x, y, z);
        match self.prediction {
            Some(level) => level.block_state(pos),
            None => sunrise_world::level::LevelAccess::block_state(self.base, pos),
        }
    }
}

impl SidedLevel for FireOddsLevel<'_> {
    fn client_side(&self) -> bool {
        self.base.client_side()
    }
}

// Prediction uses the same implementation for reads not affected by staged blocks.
macro_rules! base_level_reads {
    ($(fn $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(fn $method(&self, $($arg: $ty),*) -> $ret {
            mod_api::LevelHost::$method(self.base, $($arg),*)
        })*
    };
}

impl mod_api::LevelHost for FireOddsLevel<'_> {
    base_level_reads! {
        fn block_states() -> mod_api::BlockStates<'_>;
        fn container_size(at: mod_api::BlockSide) -> Option<usize>;
        fn container_stack(at: mod_api::BlockSide, slot: usize) -> Option<mod_api::ItemStack>;
        fn block_state_id(name: &str) -> Option<mod_api::BlockState>;
        fn block_state_default(block: mod_api::BlockKey) -> Option<mod_api::BlockState>;
        fn state_block(state: mod_api::BlockState) -> Option<&str>;
        fn state_string(state: mod_api::BlockState) -> Option<String>;
        fn min_y() -> i32;
        fn height() -> i32;
        fn block_in_tag(block: mod_api::BlockKey, tag: &str) -> Option<bool>;
        fn tags() -> &dyn mod_api::Tags;
        fn state_fluid_state(state: mod_api::BlockState) -> Option<mod_api::FluidState>;
        fn fluid_in_tag(fluid: &str, tag: &str) -> Option<bool>;
        fn state_with_property(state: mod_api::BlockState, key: &str, value: &str) -> Option<mod_api::BlockState>;
        fn brightness(layer: mod_api::LightLayer, pos: [i32; 3]) -> i32;
        fn raw_brightness(pos: [i32; 3], sky_darken: i32) -> i32;
        fn is_loaded(pos: [i32; 3]) -> bool;
        fn enabled_features() -> mod_api::FeatureFlags;
    }

    fn has_item_capability(&self, pos: [i32; 3], side: Option<mod_api::Direction>) -> Option<bool> {
        let level = self.base.api_level?;
        let at = mod_api::BlockSide { level, pos, side };
        let site = self.base.containers().capability_site(
            at,
            self.block(pos),
            self.block_entity_removed(pos),
        )?;
        Some(mod_api::__private::item_capability_present(self, site))
    }
    fn block_entity(&self, pos: [i32; 3]) -> Option<mod_api::BlockEntityRef> {
        if self.block_entity_removed(pos) {
            return None;
        }
        mod_api::LevelHost::block_entity(self.base, pos)
    }
    fn attachment_read_scope(&self) -> mod_api::AttachmentReadScope<'_> {
        mod_api::LevelHost::attachment_read_scope(self.base)
    }
    fn block_entity_part_snapshot(
        &self,
        entity: mod_api::BlockEntityRef,
        namespace: &str,
        name: &str,
    ) -> Result<mod_api::AttachmentBatch, mod_api::PartStateError> {
        use mod_api::__private::game::Fields;
        let fields = entity.fields();
        // The live reader still diagnoses handles from another level.
        if self.base.api_level == Some(fields.level) && self.block_entity_removed(fields.position) {
            return Err(mod_api::PartStateError::Stale);
        }
        mod_api::LevelHost::block_entity_part_snapshot(self.base, entity, namespace, name)
    }
    fn block_entity_data(&self, entity: mod_api::BlockEntityRef) -> Option<mod_api::Compound> {
        use mod_api::__private::game::Fields;
        if self.block_entity_removed(entity.fields().position) {
            return None;
        }
        mod_api::LevelHost::block_entity_data(self.base, entity)
    }
    fn block_state(&self, pos: [i32; 3]) -> Option<mod_api::BlockState> {
        self.block(pos).try_into_api().ok()
    }
    fn fluid_state(&self, pos: [i32; 3]) -> Option<mod_api::FluidState> {
        if !self.is_loaded(pos) {
            return None;
        }
        crate::game::native_world_read_common::state_fluid(
            self.base.behaviours,
            self.base.registry,
            self.block(pos),
        )
    }
    fn collision_shape(&self, pos: [i32; 3]) -> Option<Vec<[f64; 6]>> {
        if !self.is_loaded(pos) {
            return None;
        }
        let [x, y, z] = pos;
        let state = self.block(pos);
        let binding = self.base.collision;
        let world = &*self.base.world;
        if !self.block_entity_removed(pos)
            && (world.block(x, y, z) == state || world.block_entities.contains_key(&(x, y, z)))
        {
            // Use the staged state but preserve the live entity's geometry when it survives.
            return Some(super::native_state_shapes::collision_boxes(
                binding, world, state, pos,
            ));
        }
        let mut scene = sunrise_level::collisions::LookupCollisions {
            block: &|x, y, z| self.block([x, y, z]),
            binding,
        };
        let shape = binding.collision_shape_with_context(
            &mut scene,
            state,
            BlockPos::new(x, y, z),
            &sunrise_world::collision_context::CollisionContext::empty(false),
        );
        Some(shape.map_or_else(
            |_| Vec::new(),
            |shape| {
                shape
                    .boxes()
                    .iter()
                    .map(|bounds| {
                        let min = bounds.min_position();
                        let max = bounds.max_position();
                        [min.x, min.y, min.z, max.x, max.y, max.z]
                    })
                    .collect()
            },
        ))
    }
    fn is_raining_at(&self, [x, y, z]: [i32; 3]) -> bool {
        if let Some(level) = self.prediction {
            self.is_loaded([x, y, z])
                && self
                    .base
                    .behaviours
                    .is_raining_at(level, BlockPos::new(x, y, z))
        } else {
            mod_api::LevelHost::is_raining_at(self.base, [x, y, z])
        }
    }
}

impl mod_api::LevelHost for super::GameLevel<'_> {
    fn block_states(&self) -> mod_api::BlockStates<'_> {
        mod_api::BlockStates::of(&self.tables)
    }
    fn block_face_sturdy(
        &self,
        pos: [i32; 3],
        face: mod_api::BlockFace,
        support: mod_api::SupportType,
    ) -> Option<bool> {
        if !self.native_authoritative {
            self.api_level?;
        }
        crate::game::native_world_read_common::block_face_sturdy(
            self.world,
            self.behaviours,
            self.collision,
            pos,
            face,
            support,
        )
    }
    fn can_see_sky(&self, pos: [i32; 3]) -> Option<bool> {
        if !self.native_authoritative {
            self.api_level?;
        }
        Some(crate::game::native_world_read_common::can_see_sky(
            self.world, pos,
        ))
    }
    fn has_item_capability(&self, pos: [i32; 3], side: Option<mod_api::Direction>) -> Option<bool> {
        let level = self.api_level?;
        let [x, y, z] = pos;
        let at = mod_api::BlockSide { level, pos, side };
        let site = self
            .containers()
            .capability_site(at, self.world.block(x, y, z), false)?;
        Some(mod_api::__private::item_capability_present(self, site))
    }
    fn block_entity(&self, pos: [i32; 3]) -> Option<mod_api::BlockEntityRef> {
        crate::game::native_block_entity_read::world_block_entity(self.world, self.api_level?, pos)
    }
    fn container_size(&self, at: mod_api::BlockSide) -> Option<usize> {
        if !self.native_authoritative
            || self.api_level != Some(at.level)
            || !self.world.is_loaded_at(at.pos[0], at.pos[1], at.pos[2])
        {
            return None;
        }
        self.containers().capability_size(at)
    }
    fn container_stack(&self, at: mod_api::BlockSide, slot: usize) -> Option<mod_api::ItemStack> {
        if !self.native_authoritative
            || self.api_level != Some(at.level)
            || !self.world.is_loaded_at(at.pos[0], at.pos[1], at.pos[2])
        {
            return None;
        }
        self.containers()
            .capability_stack(at, slot)?
            .try_into_api()
            .ok()
    }
    fn attachment_read_scope(&self) -> mod_api::AttachmentReadScope<'_> {
        mod_api::__private::game::attachment_read_scope(self.attachment_query_scopes)
    }
    fn block_entity_part_snapshot(
        &self,
        entity: mod_api::BlockEntityRef,
        namespace: &str,
        name: &str,
    ) -> Result<mod_api::AttachmentBatch, mod_api::PartStateError> {
        crate::game::native_block_entity_read::part_snapshot(
            self.world,
            self.api_level,
            self.native_authoritative,
            self.part_state_index,
            self.attachment_query_scopes,
            entity,
            (namespace, name),
        )
    }
    fn block_entity_data(&self, entity: mod_api::BlockEntityRef) -> Option<mod_api::Compound> {
        use mod_api::__private::game::Fields;
        let fields = entity.fields();
        if self.api_level != Some(fields.level) {
            return None;
        }
        let [x, y, z] = fields.position;
        let key = (x, y, z);
        if self.world.block_entities.generation(&key) != Some(fields.generation) {
            return None;
        }
        let entity = self.world.block_entities.get(&key)?;
        let name = &self.facts.get(self.world.block(x, y, z)).name;
        crate::game::native_block_entity_read::save_without_metadata(
            self.world, key, entity, name, self.items,
        )
    }

    fn block_state_id(&self, name: &str) -> Option<mod_api::BlockState> {
        super::native_block_states::parse(self.block_registries, self.registry, name)?
            .try_into_api()
            .ok()
    }
    fn block_state_default(&self, block: mod_api::BlockKey) -> Option<mod_api::BlockState> {
        super::api_values::default_state(block)?.try_into_api().ok()
    }
    fn block_state(&self, pos: [i32; 3]) -> Option<mod_api::BlockState> {
        sunrise_world::level::LevelAccess::block_state(self, BlockPos::new(pos[0], pos[1], pos[2]))
            .try_into_api()
            .ok()
    }
    fn state_block(&self, state: mod_api::BlockState) -> Option<&str> {
        self.native_state_block(state.try_into_engine(&()).ok()?)
    }
    fn state_string(&self, state: mod_api::BlockState) -> Option<String> {
        self.registry
            .state(state.try_into_engine(&()).ok()?)
            .map(super::native_block_states::serialize)
    }
    fn min_y(&self) -> i32 {
        self.world.shape.min_y
    }
    fn height(&self) -> i32 {
        self.world.shape.height
    }
    fn block_in_tag(&self, block: mod_api::BlockKey, tag: &str) -> Option<bool> {
        if !self.native_authoritative {
            return None;
        }
        crate::game::native_world_read_common::block_in_tag(block, tag)
    }
    fn tags(&self) -> &dyn mod_api::Tags {
        self
    }
    fn fluid_state(&self, pos: [i32; 3]) -> Option<mod_api::FluidState> {
        let [x, y, z] = pos;
        if !(self.world.shape.contains_y(y) && self.world.loaded(x, z)) {
            return None;
        }
        crate::game::native_world_read_common::state_fluid(
            self.behaviours,
            self.registry,
            self.world.block(x, y, z),
        )
    }
    fn state_fluid_state(&self, state: mod_api::BlockState) -> Option<mod_api::FluidState> {
        crate::game::native_world_read_common::state_fluid(
            self.behaviours,
            self.registry,
            state.try_into_engine(&()).ok()?,
        )
    }
    fn collision_shape(&self, pos: [i32; 3]) -> Option<Vec<[f64; 6]>> {
        let [x, y, z] = pos;
        if !(self.world.shape.contains_y(y) && self.world.loaded(x, z)) {
            return None;
        }
        Some(super::native_state_shapes::collision_boxes(
            self.collision,
            self.world,
            self.world.block(x, y, z),
            pos,
        ))
    }
    fn fluid_in_tag(&self, fluid: &str, tag: &str) -> Option<bool> {
        self.native_authoritative
            .then(|| crate::game::native_world_read_common::fluid_in_tag(self.items, fluid, tag))
    }
    fn state_with_property(
        &self,
        state: mod_api::BlockState,
        key: &str,
        value: &str,
    ) -> Option<mod_api::BlockState> {
        self.native_state_with_property(state.try_into_engine(&()).ok()?, key, value)?
            .try_into_api()
            .ok()
    }
    fn brightness(&self, layer: mod_api::LightLayer, pos: [i32; 3]) -> i32 {
        crate::game::native_world_read_common::world_brightness(self.world, layer, pos)
    }
    fn raw_brightness(&self, pos: [i32; 3], sky_darken: i32) -> i32 {
        sunrise_world::level::raw_brightness(
            self,
            BlockPos::new(pos[0], pos[1], pos[2]),
            sky_darken,
        )
    }
    fn is_loaded(&self, pos: [i32; 3]) -> bool {
        self.world.is_loaded_at(pos[0], pos[1], pos[2])
    }
    fn is_raining_at(&self, pos: [i32; 3]) -> bool {
        self.world.is_loaded_at(pos[0], pos[1], pos[2])
            && self
                .behaviours
                .is_raining_at(self, BlockPos::new(pos[0], pos[1], pos[2]))
    }
    fn enabled_features(&self) -> mod_api::FeatureFlags {
        mod_api::FeatureFlags::new(self.enabled_features.clone())
    }
}

impl mod_api::AttachmentHost for super::GameLevel<'_> {
    fn validate_attachment(
        &self,
        namespace: &str,
        name: &str,
        sync: bool,
    ) -> Result<(), mod_api::AttachmentError> {
        mod_api::__private::validate_attachment_use(namespace, name, sync)
    }
    fn attachments(
        &mut self,
        holder: mod_api::Holder,
        _: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        if !self.native_authoritative {
            return Err(mod_api::AttachmentError::MissingHolder);
        }
        self.world.attachment_syncs.insert(holder);
        world_attachments(self.world, holder)
    }
}

impl mod_api::LevelRandomHost for super::GameLevel<'_> {
    fn container_signal(&mut self, pos: [i32; 3]) -> Option<i32> {
        if !self.native_authoritative || !self.world.is_loaded_at(pos[0], pos[1], pos[2]) {
            return None;
        }
        Some(crate::game::machine_containers::container_signal(
            self,
            BlockPos::new(pos[0], pos[1], pos[2]),
        ))
    }
    fn attachments(&mut self) -> &mut dyn mod_api::AttachmentStore {
        self
    }
    fn lend_random(&mut self, draw: mod_api::RandomDraw<'_>) {
        if self.native_authoritative {
            super::api_values::with_live_level_random(&mut self.world.random, draw);
        }
    }
}

impl mod_api::LevelHost for super::native_hooks::HitLevel<'_> {
    fn block_entity_data(&self, _: mod_api::BlockEntityRef) -> Option<mod_api::Compound> {
        None
    }

    fn block_state_id(&self, name: &str) -> Option<mod_api::BlockState> {
        super::native_block_states::parse(self.0.block_registries()?, self.0.registry(), name)?
            .try_into_api()
            .ok()
    }
    fn block_state_default(&self, block: mod_api::BlockKey) -> Option<mod_api::BlockState> {
        self.0.block_registries()?;
        super::api_values::default_state(block)?.try_into_api().ok()
    }
    fn block_in_tag(&self, block: mod_api::BlockKey, tag: &str) -> Option<bool> {
        self.0.tags()?;
        crate::game::native_world_read_common::block_in_tag(block, tag)
    }
    fn tags(&self) -> &dyn mod_api::Tags {
        self
    }
    fn fluid_in_tag(&self, fluid: &str, tag: &str) -> Option<bool> {
        Some(crate::game::native_world_read_common::fluid_in_tag(
            self.0.tags()?,
            fluid,
            tag,
        ))
    }
    fn block_state(&self, pos: [i32; 3]) -> Option<mod_api::BlockState> {
        self.0.block(pos).try_into_api().ok()
    }
    fn state_block(&self, state: mod_api::BlockState) -> Option<&str> {
        self.0
            .registry()
            .state(state.try_into_engine(&()).ok()?)
            .map(|s| &*s.name)
    }
    fn state_string(&self, state: mod_api::BlockState) -> Option<String> {
        self.0
            .registry()
            .state(state.try_into_engine(&()).ok()?)
            .map(super::native_block_states::serialize)
    }
    fn min_y(&self) -> i32 {
        self.0.shape().min_y
    }
    fn height(&self) -> i32 {
        self.0.shape().height
    }
    fn state_with_property(
        &self,
        state: mod_api::BlockState,
        key: &str,
        value: &str,
    ) -> Option<mod_api::BlockState> {
        super::native_hooks::with_property(
            self.0.registry(),
            state.try_into_engine(&()).ok()?,
            key,
            value,
        )?
        .try_into_api()
        .ok()
    }
}

impl mod_api::AttachmentHost for super::native_hooks::HitLevel<'_> {
    fn validate_attachment(
        &self,
        namespace: &str,
        name: &str,
        sync: bool,
    ) -> Result<(), mod_api::AttachmentError> {
        mod_api::__private::validate_attachment_use(namespace, name, sync)
    }
    fn attachments(
        &mut self,
        holder: mod_api::Holder,
        access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        let _ = access;
        self.0.attachments(holder)
    }
    /// The hit's victim and causing entity lend their maps for reading only.
    fn read_only_attachments(
        &self,
        holder: mod_api::Holder,
    ) -> Option<Result<Option<&mod_api::Attachments>, mod_api::AttachmentError>> {
        use mod_api::__private::LivingParty;
        let body = [LivingParty::Victim, LivingParty::Causing]
            .into_iter()
            .find_map(|party| {
                self.party_body(party)
                    .filter(|&body| super::native_living_body::body_holder(body) == Some(holder))
            })?;
        let reads = self.0.reads()?;
        Some(Ok(match (holder, body) {
            (mod_api::Holder::Entity(id), _) => {
                reads.entity_attachments.and_then(|maps| maps.get(&id))
            }
            // A parked guest's map is its hub row's; the player in place's is the game's.
            (mod_api::Holder::Player(uuid), sunrise_sim::health::LivingBody::Player(player)) => {
                match player.attachments {
                    Some(map) => Some(map),
                    None if reads.player_uuid == Some(uuid) => reads.player_attachments,
                    None => None,
                }
            }
            _ => None,
        }))
    }
}

impl mod_api::LevelRandomHost for super::native_hooks::HitLevel<'_> {
    fn attachments(&mut self) -> &mut dyn mod_api::AttachmentStore {
        self
    }
    fn lend_random(&mut self, draw: mod_api::RandomDraw<'_>) {
        super::api_values::with_live_level_random(self.0.random(), draw);
    }
}

impl mod_api::HitHost for super::native_hooks::HitLevel<'_> {
    fn play_sound_at(
        &mut self,
        sound: &str,
        at: [f64; 3],
        source: mod_api::SoundSource,
        volume: f32,
        pitch: f32,
    ) -> bool {
        super::native_hooks::mod_sound_ok(sound, at, source.index(), volume, pitch)
            && self
                .0
                .hit_effects()
                .is_some_and(|hit| hit.play(sound, at, source, volume, pitch, None))
    }
    fn play_sound_from(
        &mut self,
        entity: mod_api::EntityRef,
        sound: &str,
        source: mod_api::SoundSource,
        volume: f32,
        pitch: f32,
    ) -> bool {
        let Some(hit) = self.0.hit_effects() else {
            return false;
        };
        let Some((id, at, _)) = hit.entity(entity) else {
            return false;
        };
        super::native_hooks::mod_sound_ok(sound, at, source.index(), volume, pitch)
            && hit.play(sound, at, source, volume, pitch, Some(id))
    }
    fn entity_next_float(&mut self, entity: mod_api::EntityRef) -> Option<f32> {
        use sunrise_core::random::RandomSource;
        Some(self.0.hit_effects()?.entity(entity)?.2.next_float())
    }
    fn entity_next_double(&mut self, entity: mod_api::EntityRef) -> Option<f64> {
        use sunrise_core::random::RandomSource;
        Some(self.0.hit_effects()?.entity(entity)?.2.next_double())
    }
    fn entity_next_bool(&mut self, entity: mod_api::EntityRef) -> Option<bool> {
        use sunrise_core::random::RandomSource;
        Some(self.0.hit_effects()?.entity(entity)?.2.next_boolean())
    }
    fn entity_next_int(&mut self, entity: mod_api::EntityRef, bound: i32) -> Option<i32> {
        use sunrise_core::random::RandomSource;
        self.0
            .hit_effects()?
            .entity(entity)?
            .2
            .next_int_bound(bound)
            .ok()
    }
    fn victim(&self) -> Option<mod_api::LivingView<'_>> {
        mod_api::LivingView::new(self, mod_api::__private::LivingParty::Victim)
    }
    fn causing(&self) -> Option<mod_api::LivingView<'_>> {
        mod_api::LivingView::new(self, mod_api::__private::LivingParty::Causing)
    }
}

impl super::native_hooks::HitLevel<'_> {
    /// The body `party` names in this hit: the victim the hurt code lends, or the causer its
    /// site lends when it is the hook's causing entity
    /// ([`DamageLevel::causing_body`](sunrise_client::health::DamageLevel::causing_body)).
    fn party_body(
        &self,
        party: mod_api::__private::LivingParty,
    ) -> Option<sunrise_sim::health::LivingBody<'_>> {
        let body = match party {
            mod_api::__private::LivingParty::Victim => self.0.victim(),
            mod_api::__private::LivingParty::Causing => self.0.causing_body(),
            _ => None,
        }?;
        Some(match body {
            // A lent attacking player is the player in place, whose UUID the reads know.
            sunrise_sim::health::LivingBody::Player(mut p) if p.uuid.is_none() => {
                p.uuid = self.0.reads().and_then(|reads| reads.player_uuid);
                sunrise_sim::health::LivingBody::Player(p)
            }
            body => body,
        })
    }
}

/// A hit's victim and causer, read through the same functions as the `LivingRef` reads.
impl mod_api::__private::LivingReads for super::native_hooks::HitLevel<'_> {
    fn living_entity(&self, party: mod_api::__private::LivingParty) -> Option<mod_api::EntityRef> {
        let reads = self.0.reads()?;
        let id = match self.party_body(party)? {
            sunrise_sim::health::LivingBody::Mob(m) => m.id,
            sunrise_sim::health::LivingBody::Player(p) => u32::try_from(p.id).ok()?,
            sunrise_sim::health::LivingBody::Stand(s)
            | sunrise_sim::health::LivingBody::Dragon(s) => s.id,
        };
        Some(mod_api::EntityRef::new(reads.epoch, id))
    }
    fn living_kind(
        &self,
        party: mod_api::__private::LivingParty,
    ) -> Option<mod_api::EntityTypeKey> {
        let keys = self.0.reads()?.living_types;
        super::native_living_body::body_kind(self.party_body(party)?, keys)
    }
    fn living_holder(&self, party: mod_api::__private::LivingParty) -> Option<mod_api::Holder> {
        super::native_living_body::body_holder(self.party_body(party)?)
    }
    fn living_health(&self, party: mod_api::__private::LivingParty) -> Option<f32> {
        super::native_living_body::body_health(self.party_body(party)?)
    }
    fn living_absorption(&self, party: mod_api::__private::LivingParty) -> Option<f32> {
        super::native_living_body::body_absorption(self.party_body(party)?)
    }
    fn living_max_health(&self, party: mod_api::__private::LivingParty) -> Option<f32> {
        super::native_living_body::body_max_health(self.party_body(party)?)
    }
    fn living_effect(
        &self,
        party: mod_api::__private::LivingParty,
        effect: mod_api::EffectKey,
    ) -> Option<mod_api::ActiveEffect> {
        let def = mod_api::__private::effect_index(effect)
            .and_then(sunrise_game_data::potions::effect_by_index)?;
        super::native_living_body::body_effect(self.party_body(party)?, def.index)
    }
    fn living_effects(
        &self,
        party: mod_api::__private::LivingParty,
    ) -> Option<Vec<mod_api::ActiveEffect>> {
        super::native_living_body::body_effects(self.party_body(party)?)
    }
    fn living_potion_affected(
        &self,
        party: mod_api::__private::LivingParty,
        effect: mod_api::EffectKey,
    ) -> bool {
        mod_api::__private::effect_index(effect)
            .and_then(sunrise_game_data::potions::effect_by_index)
            .zip(self.party_body(party))
            .is_some_and(|(def, body)| {
                super::native_living_body::body_potion_affected(body, def.index)
            })
    }
    fn living_attribute_value(
        &self,
        party: mod_api::__private::LivingParty,
        id: &str,
    ) -> Option<f64> {
        super::native_living_body::body_attribute(
            self.party_body(party)?,
            self.0.reads()?.attributes,
            id,
            sunrise_world::attribute_map::ValueKind::Computed,
        )
    }
    fn living_hand_item(
        &self,
        party: mod_api::__private::LivingParty,
        hand: mod_api::Hand,
        read: &mut dyn for<'a> FnMut(mod_api::ItemStackView<'a>),
    ) -> Option<()> {
        let stack = super::native_living_body::body_equipment(
            self.party_body(party)?,
            crate::game::native_equipment_read::hand_slot(hand),
        )?;
        crate::game::native_equipment_read::lend_equipment(stack, read)
    }
    fn living_no_ai(&self, party: mod_api::__private::LivingParty) -> Option<bool> {
        super::native_living_body::body_no_ai(self.party_body(party)?)
    }
}

/// A level with the simulation's ticks beside it: `&mut Ticks` in a write or tick hook, `&Ticks`
/// where the listener only reads (placement and survival: [`LevelView`](super::native_hooks::LevelView)).
/// Reads need only a shared view of the ticks; scheduling and writes need them mutable.
pub struct LevelTicks<'b, L, T = &'b mut Ticks> {
    level: L,
    ticks: T,
    unpack_container_loot: bool,
    /// The blocks a preview standing in for the level changed, which the reads see over it.
    preview: Option<&'b sunrise_world::level::PreviewBlocks>,
    /// The level's and the ticks' borrow, which the delegating reads below are bounded by.
    borrow: std::marker::PhantomData<&'b ()>,
}

impl<'b, L, T> LevelTicks<'b, L, T> {
    pub fn new(level: L, ticks: T) -> Self {
        Self {
            level,
            ticks,
            unpack_container_loot: true,
            preview: None,
            borrow: std::marker::PhantomData,
        }
    }

    /// `place_liquid` reads stored slots whether it uses a direct or journalled host.
    pub(in crate::game) fn without_container_loot(mut self) -> Self {
        self.unpack_container_loot = false;
        self
    }

    /// This level as the preview that changed `preview` sees it (`None`: as it is).
    pub fn previewing(
        mut self,
        preview: Option<&'b sunrise_world::level::PreviewBlocks>,
    ) -> Self {
        self.preview = preview;
        self
    }
    /// The block the preview put at `pos`, if it changed it.
    fn previewed(&self, pos: BlockPos) -> Option<sunrise_world::registry::BlockStateId> {
        self.preview?.get(&pos).copied()
    }
}

impl<
    'a: 'b,
    'b,
    L: std::ops::Deref<Target = super::GameLevel<'a>>,
    T: std::ops::Deref<Target = Ticks>,
> mod_api::LevelHost for LevelTicks<'b, L, T>
{
    fn block_states(&self) -> mod_api::BlockStates<'_> {
        mod_api::LevelHost::block_states(&*self.level)
    }
    fn block_face_sturdy(
        &self,
        pos: [i32; 3],
        face: mod_api::BlockFace,
        support: mod_api::SupportType,
    ) -> Option<bool> {
        mod_api::LevelHost::block_face_sturdy(&*self.level, pos, face, support)
    }
    fn can_see_sky(&self, pos: [i32; 3]) -> Option<bool> {
        mod_api::LevelHost::can_see_sky(&*self.level, pos)
    }
    fn has_item_capability(&self, pos: [i32; 3], side: Option<Direction>) -> Option<bool> {
        let level = self.level.api_level?;
        let [x, y, z] = pos;
        let preview = self.previewed(BlockPos::new(x, y, z));
        let state = preview.unwrap_or_else(|| self.level.world.block(x, y, z));
        let site = self.level.containers().capability_site(
            mod_api::BlockSide { level, pos, side },
            state,
            preview.is_some(),
        )?;
        Some(mod_api::__private::item_capability_present(self, site))
    }
    fn block_entity(&self, pos: [i32; 3]) -> Option<mod_api::BlockEntityRef> {
        // A block the preview changed has no block entity left.
        if self
            .previewed(BlockPos::new(pos[0], pos[1], pos[2]))
            .is_some()
        {
            return None;
        }
        mod_api::LevelHost::block_entity(&*self.level, pos)
    }
    fn container_size(&self, at: mod_api::BlockSide) -> Option<usize> {
        if self
            .previewed(BlockPos::new(at.pos[0], at.pos[1], at.pos[2]))
            .is_some()
        {
            return None;
        }
        mod_api::LevelHost::container_size(&*self.level, at)
    }
    fn container_stack(&self, at: mod_api::BlockSide, slot: usize) -> Option<mod_api::ItemStack> {
        if self
            .previewed(BlockPos::new(at.pos[0], at.pos[1], at.pos[2]))
            .is_some()
        {
            return None;
        }
        mod_api::LevelHost::container_stack(&*self.level, at, slot)
    }
    fn attachment_read_scope(&self) -> mod_api::AttachmentReadScope<'_> {
        mod_api::LevelHost::attachment_read_scope(&*self.level)
    }
    fn block_entity_part_snapshot(
        &self,
        entity: mod_api::BlockEntityRef,
        namespace: &str,
        name: &str,
    ) -> Result<mod_api::AttachmentBatch, mod_api::PartStateError> {
        mod_api::LevelHost::block_entity_part_snapshot(&*self.level, entity, namespace, name)
    }
    fn block_entity_data(&self, entity: mod_api::BlockEntityRef) -> Option<mod_api::Compound> {
        use mod_api::__private::game::Fields;
        let [x, y, z] = entity.fields().position;
        if self.previewed(BlockPos::new(x, y, z)).is_some() {
            return None;
        }
        mod_api::LevelHost::block_entity_data(&*self.level, entity)
    }
    fn block_state_id(&self, name: &str) -> Option<mod_api::BlockState> {
        mod_api::LevelHost::block_state_id(&*self.level, name)
    }
    fn block_state_default(&self, block: mod_api::BlockKey) -> Option<mod_api::BlockState> {
        mod_api::LevelHost::block_state_default(&*self.level, block)
    }
    fn block_state(&self, pos: [i32; 3]) -> Option<mod_api::BlockState> {
        match self.previewed(BlockPos::new(pos[0], pos[1], pos[2])) {
            Some(state) => state.try_into_api().ok(),
            None => mod_api::LevelHost::block_state(&*self.level, pos),
        }
    }
    fn state_block(&self, state: mod_api::BlockState) -> Option<&str> {
        mod_api::LevelHost::state_block(&*self.level, state)
    }
    fn state_string(&self, state: mod_api::BlockState) -> Option<String> {
        mod_api::LevelHost::state_string(&*self.level, state)
    }
    fn min_y(&self) -> i32 {
        mod_api::LevelHost::min_y(&*self.level)
    }
    fn height(&self) -> i32 {
        mod_api::LevelHost::height(&*self.level)
    }
    fn block_in_tag(&self, block: mod_api::BlockKey, tag: &str) -> Option<bool> {
        mod_api::LevelHost::block_in_tag(&*self.level, block, tag)
    }
    fn tags(&self) -> &dyn mod_api::Tags {
        mod_api::LevelHost::tags(&*self.level)
    }
    fn fluid_state(&self, pos: [i32; 3]) -> Option<mod_api::FluidState> {
        match self.previewed(BlockPos::new(pos[0], pos[1], pos[2])) {
            Some(state) => self.state_fluid_state(state.try_into_api().ok()?),
            None => mod_api::LevelHost::fluid_state(&*self.level, pos),
        }
    }
    fn state_fluid_state(&self, state: mod_api::BlockState) -> Option<mod_api::FluidState> {
        mod_api::LevelHost::state_fluid_state(&*self.level, state)
    }
    fn fluid_in_tag(&self, fluid: &str, tag: &str) -> Option<bool> {
        mod_api::LevelHost::fluid_in_tag(&*self.level, fluid, tag)
    }
    fn state_with_property(
        &self,
        state: mod_api::BlockState,
        key: &str,
        value: &str,
    ) -> Option<mod_api::BlockState> {
        mod_api::LevelHost::state_with_property(&*self.level, state, key, value)
    }
    fn brightness(&self, layer: mod_api::LightLayer, pos: [i32; 3]) -> i32 {
        mod_api::LevelHost::brightness(&*self.level, layer, pos)
    }
    fn raw_brightness(&self, pos: [i32; 3], sky_darken: i32) -> i32 {
        mod_api::LevelHost::raw_brightness(&*self.level, pos, sky_darken)
    }
    fn is_loaded(&self, pos: [i32; 3]) -> bool {
        mod_api::LevelHost::is_loaded(&*self.level, pos)
    }
    fn is_raining_at(&self, pos: [i32; 3]) -> bool {
        mod_api::LevelHost::is_raining_at(&*self.level, pos)
    }
    fn enabled_features(&self) -> mod_api::FeatureFlags {
        mod_api::LevelHost::enabled_features(&*self.level)
    }
    fn signal(&self, pos: [i32; 3], direction: Direction) -> Option<i32> {
        self.read_signals(|s| s.signal(BlockPos::new(pos[0], pos[1], pos[2]), direction))
    }
    fn direct_signal(&self, pos: [i32; 3], direction: Direction) -> Option<i32> {
        self.read_signals(|s| s.direct_signal(BlockPos::new(pos[0], pos[1], pos[2]), direction))
    }
    fn direct_signal_to(&self, pos: [i32; 3]) -> Option<i32> {
        self.read_signals(|s| s.direct_signal_to(BlockPos::new(pos[0], pos[1], pos[2])))
    }
    fn best_neighbor_signal(&self, pos: [i32; 3]) -> Option<i32> {
        self.read_signals(|s| s.best_neighbor_signal(BlockPos::new(pos[0], pos[1], pos[2])))
    }
    fn has_neighbor_signal(&self, pos: [i32; 3]) -> Option<bool> {
        self.read_signals(|s| s.has_neighbor_signal(BlockPos::new(pos[0], pos[1], pos[2])))
    }
    fn collision_shape(&self, pos: [i32; 3]) -> Option<Vec<[f64; 6]>> {
        let Some(state) = self.previewed(BlockPos::new(pos[0], pos[1], pos[2])) else {
            return mod_api::LevelHost::collision_shape(&*self.level, pos);
        };
        Some(super::native_state_shapes::collision_boxes(
            self.level.collision,
            self.level.world,
            state,
            pos,
        ))
    }
}

impl<'a: 'b, 'b, T: std::ops::Deref<Target = Ticks>> mod_api::LevelRandomHost
    for LevelTicks<'b, &mut super::GameLevel<'a>, T>
{
    fn container_signal(&mut self, pos: [i32; 3]) -> Option<i32> {
        if self.unpack_container_loot {
            return mod_api::LevelRandomHost::container_signal(&mut *self.level, pos);
        }
        let [x, y, z] = pos;
        (self.level.native_authoritative && self.level.world.is_loaded_at(x, y, z))
            .then(|| self.level.containers().stored_container_signal((x, y, z)))
    }
    fn attachments(&mut self) -> &mut dyn mod_api::AttachmentStore {
        mod_api::LevelRandomHost::attachments(&mut *self.level)
    }
    fn lend_random(&mut self, draw: mod_api::RandomDraw<'_>) {
        mod_api::LevelRandomHost::lend_random(&mut *self.level, draw);
    }
}

impl<
    'a: 'b,
    'b,
    L: std::ops::Deref<Target = super::GameLevel<'a>>,
    T: std::ops::DerefMut<Target = Ticks>,
> mod_api::LevelTicksHost for LevelTicks<'b, L, T>
{
    fn schedule_block_tick(&mut self, pos: [i32; 3], block: mod_api::BlockKey, delay: i32) -> bool {
        self.level
            .native_schedule_block_tick(&mut self.ticks, pos, block, delay)
    }
    fn schedule_fluid_tick(&mut self, pos: [i32; 3], fluid: &str, delay: i32) -> bool {
        self.level
            .native_schedule_fluid_tick(&mut self.ticks, pos, fluid, delay)
    }
}

impl<
    'a: 'b,
    'b,
    L: std::ops::Deref<Target = super::GameLevel<'a>>,
    T: std::ops::Deref<Target = Ticks>,
> LevelTicks<'b, L, T>
{
    /// A read of the level in place (over the preview's blocks) through the game's own
    /// `SignalGetter`, with these ticks' comparator outputs. A client's ticks don't hold the
    /// server's outputs: a read that needed one it lacks is unknown there, `None`.
    fn read_signals<R>(
        &self,
        read: impl FnOnce(&sunrise_world::level::Signals<'_, Self>) -> R,
    ) -> Option<R> {
        let behaviours = self.level.behaviours;
        let signals = if self.level.native_authoritative && self.preview.is_some() {
            behaviours.uncached_signals(self, &self.ticks)
        } else {
            behaviours.signals(self, &self.ticks)
        };
        let value = read(&signals);
        (!(self.level.client_side && signals.read_unknown_output())).then_some(value)
    }
}

/// The signal reads of the level in place, over the preview's blocks: a block the preview
/// changed has no block entity left, as `DryLevel` reads it.
impl<
    'a: 'b,
    'b,
    L: std::ops::Deref<Target = super::GameLevel<'a>>,
    T: std::ops::Deref<Target = Ticks>,
> sunrise_world::level::signal_access::SignalAccess for LevelTicks<'b, L, T>
{
    fn block_state(&self, pos: BlockPos) -> sunrise_world::registry::BlockStateId {
        self.previewed(pos)
            .unwrap_or_else(|| sunrise_world::level::LevelAccess::block_state(&*self.level, pos))
    }
    fn shulker_is_closed(&self, pos: BlockPos) -> Option<bool> {
        if self.previewed(pos).is_some() {
            return None;
        }
        sunrise_world::level::LevelAccess::shulker_is_closed(&*self.level, pos)
    }
    fn published_collision_shape(
        &self,
        pos: BlockPos,
        state: sunrise_world::registry::BlockStateId,
    ) -> Option<sunrise_core::geometry::VoxelShape> {
        if self.previewed(pos).is_some() {
            return None;
        }
        sunrise_world::level::LevelAccess::published_collision_shape(&*self.level, pos, state)
    }
    fn adjust_signal_source(
        &self,
        state: sunrise_world::registry::BlockStateId,
        vanilla: bool,
    ) -> bool {
        sunrise_world::level::LevelAccess::adjust_signal_source(&*self.level, state, vanilla)
    }
    fn has_weak_power_hook(&self) -> bool {
        sunrise_world::level::LevelAccess::has_weak_power_hook(&*self.level)
    }
    fn adjust_should_check_weak_power(
        &self,
        pos: BlockPos,
        state: sunrise_world::registry::BlockStateId,
        direction: Direction,
        vanilla: bool,
    ) -> bool {
        sunrise_world::level::LevelAccess::adjust_should_check_weak_power(
            &*self.level,
            pos,
            state,
            direction,
            vanilla,
        )
    }
    fn has_redstone_wire_connection_hook(&self) -> bool {
        sunrise_world::level::LevelAccess::has_redstone_wire_connection_hook(&*self.level)
    }
    fn adjust_should_redstone_wire_connect_to(
        &self,
        pos: BlockPos,
        state: sunrise_world::registry::BlockStateId,
        direction: Option<Direction>,
        vanilla: bool,
    ) -> bool {
        sunrise_world::level::LevelAccess::adjust_should_redstone_wire_connect_to(
            &*self.level,
            pos,
            state,
            direction,
            vanilla,
        )
    }
    fn adjust_signal(
        &self,
        pos: BlockPos,
        state: sunrise_world::registry::BlockStateId,
        direction: Direction,
        direct: bool,
        vanilla: i32,
    ) -> i32 {
        sunrise_world::level::LevelAccess::adjust_signal(
            &*self.level,
            pos,
            state,
            direction,
            direct,
            vanilla,
        )
    }
    fn block_entity_signal(
        &self,
        pos: BlockPos,
        state: sunrise_world::registry::BlockStateId,
        direction: Direction,
    ) -> i32 {
        if self.previewed(pos).is_some() {
            return 0;
        }
        sunrise_world::level::LevelAccess::block_entity_signal(&*self.level, pos, state, direction)
    }
}

impl<'a: 'b, 'b, T: std::ops::DerefMut<Target = Ticks>> mod_api::LevelWriteHost
    for LevelTicks<'b, &mut super::GameLevel<'a>, T>
{
    fn set_block(
        &mut self,
        pos: [i32; 3],
        state: mod_api::BlockState,
        flags: mod_api::BlockUpdateFlags,
    ) -> bool {
        let Ok(state) = state.try_into_engine(&()) else {
            return false;
        };
        self.level
            .native_set_block(&mut self.ticks, pos, state, flags.bits())
    }

    fn remove_block(&mut self, pos: [i32; 3], moved: bool) -> bool {
        self.level.remove_block_write(&mut self.ticks, pos, moved)
    }

    fn set_block_property(&mut self, pos: [i32; 3], key: &str, value: &str, flags: i32) -> bool {
        self.level
            .native_set_block_property(&mut self.ticks, pos, key, value, flags)
    }

    fn play_sound(
        &mut self,
        sound: &str,
        at: [f64; 3],
        source: usize,
        volume: f32,
        pitch: f32,
    ) -> bool {
        self.level
            .native_play_sound(sound, at, source, volume, pitch)
    }

    fn send_particles(
        &mut self,
        options: &mod_api::ParticleOptions,
        state: Option<mod_api::BlockState>,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: i32,
    ) -> bool {
        let Ok(state) = state.try_into_engine(&()) else {
            return false;
        };
        let Ok(options) = options.try_into_engine(self.level.items) else {
            return false;
        };
        self.level
            .native_send_particles(&options, state, at, spread, speed, count)
    }

    fn level_event(&mut self, id: i32, pos: [i32; 3], data: i32) -> bool {
        self.level.native_level_event(id, pos, data)
    }
}

super::native_tags::impl_tags!(
    super::GameLevel<'_>,
    level,
    level.native_authoritative.then_some(level.items)
);
super::native_tags::impl_tags!(super::native_hooks::HitLevel<'_>, level, level.0.tags());

/// Whether level event `id` can play with `data`: the events that carry a
/// block state (2001, 2014, 3008) need `data` to be one of the `states`.
pub fn level_event_data_ok(id: i32, data: i32, states: usize) -> bool {
    !matches!(id, 2001 | 2014 | 3008) || usize::try_from(data).is_ok_and(|d| d < states)
}

impl super::GameLevel<'_> {
    /// `Level.levelEvent(null, id, pos, data)` for a hook that holds the
    /// level's write host: the event joins the ones the block behaviour
    /// emits, in order, and plays when the write that runs the hook is
    /// done (`process_level_events`), with the guests told as for any event
    /// the behaviour raises. False off an authoritative server, or for an
    /// event that carries a block state (see `level_level_event`)
    /// when `data` isn't one.
    pub(in crate::game) fn native_level_event(
        &mut self,
        id: i32,
        pos: [i32; 3],
        data: i32,
    ) -> bool {
        if !self.native_authoritative || !level_event_data_ok(id, data, self.facts.states.len()) {
            return false;
        }
        self.events.push(LevelEvent::LevelEvent {
            id,
            pos: sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]),
            data,
            source: None,
        });
        true
    }
}

use crate::game::native_block_entity_read::NativeBlockEntityReadGameExt1 as _;

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use crate::game::native_world_read_common::NativeWorldReadGameExt1 as _;
