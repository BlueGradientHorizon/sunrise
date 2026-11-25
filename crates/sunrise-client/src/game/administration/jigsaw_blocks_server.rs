use super::Game;
use crate::game::jigsaw_blocks::Pos;
use sunrise_client::{
    containers::BlockEntity,
    jigsaw_block::{JigsawBlock, JointType},
};
use sunrise_protocol::packets::play;
use {sunrise_core::nbt::Tag, sunrise_world::worldgen::generator::JigsawQuery};

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn jigsaw_block_mut(&mut self, pos: Pos) -> Option<&mut JigsawBlock> {
        match self.world.block_entities.get_mut(&pos)? {
            BlockEntity::JigsawBlock(b) => Some(b),
            _ => None,
        }
    }

    /// `save::decode_block_entity` for data loaded into the block at `pos`
    /// (`/setblock`, `/data`): a jigsaw without `joint` takes the default its
    /// facing gives (`JigsawBlockEntity.loadAdditional`), which the decode
    /// alone cannot know.
    pub(in crate::game) fn decode_block_entity_at(
        &self,
        pos: Pos,
        tag: &Tag,
    ) -> Option<(Pos, BlockEntity)> {
        let (key, mut entity) = sunrise_client::save::decode_block_entity_replacing(
            tag,
            &self.data_block_name([pos.0, pos.1, pos.2]),
            self.world.block_entities.get(&pos),
        )?;
        if let BlockEntity::JigsawBlock(b) = &mut entity
            && tag.get("joint").is_none()
            && let Some(front) = self.jigsaw_front(pos)
        {
            b.joint = JointType::default_for_front(&front);
        }
        Some((key, entity))
    }

    pub(in crate::game) fn apply_set_jigsaw_block(&mut self, p: &play::SetJigsawBlock) {
        let pos = (p.pos[0], p.pos[1], p.pos[2]);
        let Some(b) = self.jigsaw_block_mut(pos) else {
            return;
        };
        b.name = p.name.clone();
        b.target = p.target.clone();
        b.pool = p.pool.clone();
        b.final_state = p.final_state.clone();
        // JointType.byName(name, ALIGNED).
        b.joint = JointType::of_serialized_name(&p.joint).unwrap_or(JointType::Aligned);
        b.placement_priority = p.placement_priority;
        b.selection_priority = p.selection_priority;
        self.mark_block_entity_updated(pos);
    }

    /// `handleSetJigsawBlock` for a guest: only a creative player with game
    /// master commands edits the block.
    pub(in crate::game::server_side) fn lan_set_jigsaw_block(
        &mut self,
        conn: u32,
        p: play::SetJigsawBlock,
    ) {
        let Some(g) = self.lan.as_ref().and_then(|host| host.hub.guest(conn)) else {
            return;
        };
        let allowed = g.creative() && g.permission >= 2;
        self.as_guest(conn, |game| {
            if allowed {
                game.apply_set_jigsaw_block(&p);
            }
        });
    }

    /// `handleJigsawGenerate` for a guest.
    pub(in crate::game) fn lan_jigsaw_generate(&mut self, conn: u32, p: play::JigsawGenerate) {
        let Some(g) = self.lan.as_ref().and_then(|host| host.hub.guest(conn)) else {
            return;
        };
        let allowed = g.creative() && g.permission >= 2;
        self.as_guest(conn, |game| {
            if allowed {
                game.generate_jigsaw_block(&p);
            }
        });
    }

    /// `JigsawBlockEntity.generate`: the pool is built from the block's
    /// front neighbour, `levels` deep.
    pub(in crate::game) fn generate_jigsaw_block(&mut self, p: &play::JigsawGenerate) {
        let pos = (p.pos[0], p.pos[1], p.pos[2]);
        let Some(BlockEntity::JigsawBlock(b)) = self.world.block_entities.get(&pos) else {
            return;
        };
        let (pool, target) = (b.pool.clone(), b.target.clone());
        let Some(front) = self.jigsaw_front(pos) else {
            return;
        };
        let step = crate::game::facing_offset(&front);
        let position = [pos.0 + step[0], pos.1 + step[1], pos.2 + step[2]];
        // registry.getOrThrow(pool): an unknown pool throws, ending the packet.
        if !self
            .ui
            .registries
            .ids("minecraft:worldgen/template_pool")
            .contains(&pool)
        {
            return;
        }
        let Some(terrain) = self.end.locator() else {
            return;
        };
        let query = JigsawQuery {
            pool: &pool,
            target: &target,
            max_depth: p.levels,
            position,
        };
        let keep = p.keep_jigsaws;
        let (_, entities) = self.with_level(|b, level, ticks| {
            b.place_jigsaw(level, ticks, position, keep, || {
                terrain.jigsaw_pieces(&query)
            })
        });
        for entity in entities {
            self.spawn_entity_from_nbt(Tag::Compound(entity));
        }
        let _ = self.server_process_level_events();
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use sunrise_client::containers::BlockEntity;

    /// A single-player jigsaw edit is applied by the game that is its own
    /// server, as `handleSetJigsawBlock` does there, and marks the chunk.
    #[test]
    #[ignore = "daily"]
    fn a_local_jigsaw_edit_applies_and_marks_the_chunk() {
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        let pos = (2, 4, 3);
        let state = game
            .pack
            .assets
            .registry
            .default_state("minecraft:jigsaw")
            .unwrap();
        game.world.set(pos.0, pos.1, pos.2, state).unwrap();
        game.world
            .block_entities
            .insert(pos, BlockEntity::JigsawBlock(Box::default()));
        game.world.modified.clear();
        let edit = sunrise_protocol::packets::play::SetJigsawBlock {
            pos: [2, 4, 3],
            name: "minecraft:test".into(),
            target: "minecraft:test".into(),
            pool: "minecraft:empty".into(),
            final_state: "minecraft:air".into(),
            joint: "aligned".into(),
            selection_priority: 1,
            placement_priority: 2,
        };

        game.apply_set_jigsaw_block(&edit);

        let Some(BlockEntity::JigsawBlock(b)) = game.world.block_entities.get(&pos) else {
            panic!("no jigsaw");
        };
        assert_eq!(&*b.name, "minecraft:test", "the edit landed");
        assert_eq!((b.selection_priority, b.placement_priority), (1, 2));
        assert!(
            game.world.modified.contains(&(0, 0)),
            "the edit left the chunk unmarked"
        );
    }
}
