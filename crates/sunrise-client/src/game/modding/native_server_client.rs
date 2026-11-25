use super::{Dim, Game};
use crate::game::native_server::PlayerRead;
use mod_api::__private::game::*;
use mod_api::LevelRef;
use mod_api::PlayerRef;
use std::{collections::HashMap, sync::Arc};
use sunrise_client::{items::ItemStack, lan::abilities::PlayerAbilities};
use sunrise_core::position::BlockPos;

/// Helpers for server mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
#[allow(dead_code)]
impl Game<'_, crate::game::client_side::ClientState> {
    /// Reads live `Remote::abilities` with predicted `flying` and `flying_speed` from the body.
    pub(in crate::game) fn client_abilities(&self) -> Option<PlayerAbilities> {
        let mut a = *self.remote_abilities()?;
        if let Some(body) = self.player.as_ref() {
            a.flying = body.flying;
            a.flying_speed = body.flying_speed;
        }
        Some(a)
    }

    pub(in crate::game) fn client_player(&self, uuid: u128) -> Option<PlayerRef> {
        let epoch = self.native_lifecycle.epoch()?;
        let remote = self
            .remote
            .as_ref()
            .filter(|r| r.logged_in && r.profile == uuid)?;
        Some(mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(epoch, u32::try_from(remote.entity_id).ok()?),
            uuid,
        ))
    }

    pub(in crate::game) fn client_player_read_both(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerRead<'_>> {
        (self.local_player()? == player).then(|| PlayerRead::Acting {
            mode: self
                .player_info_game_mode()
                .map(sunrise_client::command::args::GameMode::id),
            body: self.player.as_ref(),
            abilities: self.client_abilities(),
            last_death: self.level.last_death.as_ref(),
        })
    }

    pub(in crate::game) fn client_player_dimension_key(
        &self,
        player: PlayerRef,
    ) -> Option<mod_api::DimensionKey> {
        let epoch = self.native_lifecycle.epoch()?;
        if epoch != player.fields().entity.epoch() {
            return None;
        }
        (self.local_player()? == player).then(|| self.native_level_key())
    }

    pub(in crate::game) fn client_player_collision_flags(
        &self,
        player: PlayerRef,
    ) -> Result<Option<mod_api::CollisionFlags>, mod_api::LevelReadError> {
        Ok(self.client_player_read_both(player).and_then(|read| {
            let PlayerRead::Acting {
                body: Some(body), ..
            } = read
            else {
                return None;
            };
            let flags = body.collision;
            Some(mod_api::CollisionFlags::new(
                flags.horizontal,
                flags.minor,
                flags.vertical,
                flags.below,
            ))
        }))
    }

    pub(in crate::game) fn client_player_client_brand(&self, _player: PlayerRef) -> Option<&str> {
        None
    }

    pub(in crate::game) fn client_tick_uses_server_sections(&self) -> bool {
        false
    }

    pub(in crate::game) fn client_crafting_remainder_hook(
        &self,
        _stack: &ItemStack,
        _remainder: &mut Option<ItemStack>,
    ) {
    }

    pub(in crate::game) fn client_world_table_items(
        &self,
    ) -> Option<&Arc<sunrise_client::items::Items>> {
        None
    }

    pub(in crate::game) fn client_lid_openers(&self) -> Option<&HashMap<(i32, i32, i32), i32>> {
        None
    }

    pub(in crate::game) fn client_signal_capability_site(
        &self,
        _at: mod_api::BlockSide,
    ) -> Option<mod_api::BlockCapabilitySite> {
        None
    }

    pub(in crate::game) fn client_signal_container_size(
        &self,
        _at: mod_api::BlockSide,
    ) -> Option<usize> {
        None
    }

    pub(in crate::game) fn client_signal_container_stack(
        &self,
        _at: mod_api::BlockSide,
        _slot: usize,
    ) -> Option<mod_api::ItemStack> {
        None
    }

    pub(in crate::game) fn client_level_block_face_sturdy(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        face: mod_api::BlockFace,
        support: mod_api::SupportType,
    ) -> Option<bool> {
        crate::game::server_side::native_world_read::block_face_sturdy(
            self.client_level_view_world(level)?,
            self.res.behaviours(),
            self.res.collision(),
            pos,
            face,
            support,
        )
    }

    pub(in crate::game) fn client_signal_block_entity(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<mod_api::BlockEntityRef> {
        let world = self.client_level_view_world(level)?;
        crate::game::server_side::native_block_entity_refs::world_block_entity(world, level, pos)
    }

    pub(in crate::game) fn client_signal_block_entity_data(
        &self,
        entity: mod_api::BlockEntityRef,
    ) -> Option<mod_api::Compound> {
        let world = self.client_level_view_world(entity.fields().level)?;
        crate::game::server_side::native_block_entity_refs::world_block_entity_data(
            world,
            self.pack.assets.registry,
            self.items(),
            entity,
        )
    }

    pub(in crate::game) fn client_level_is_raining_at(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        let world = self.client_level_view_world(level)?;
        if !world.is_loaded_at(pos[0], pos[1], pos[2]) {
            return None;
        }
        let (rain, _) = crate::game::mod_api_host_common::weather_for_level(
            world.shape,
            level.dimension() == Dim::End.key(),
            world.weather.levels(1.0),
        );
        if rain <= 0.2 {
            return Some(false);
        }
        Some(crate::game::server_side::native_level_world::rain_at(
            world,
            self.res.facts(),
            self.world_biomes
                .as_deref()
                .unwrap_or_else(|| self.res.biomes()),
            self.res.behaviours(),
            Dim::from_id(level.dimension_id()),
            BlockPos::new(pos[0], pos[1], pos[2]),
        ))
    }
}
