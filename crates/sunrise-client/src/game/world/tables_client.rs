use super::*;
use sunrise_client::{
    blocks::{MAP_COLORS, RenderShape},
    particles::{ParticleOptions, ParticleType},
};

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn block_provider_options(
        &self,
        ty: ParticleType,
        state: BlockStateId,
        pos: [i32; 3],
    ) -> Option<ParticleOptions> {
        self.pack.assets.registry.state(state)?;
        let facts = self.res.facts();
        let block = facts.get(state);
        let sprite =
            || Self::block_particle_sprite(&self.pack, state).unwrap_or_else(|| "missingno".into());
        let options = match ty {
            ParticleType::Block | ParticleType::DustPillar | ParticleType::BlockCrumble => {
                if facts.is_air(state)
                    || &*block.name == "minecraft:moving_piston"
                    || !block.spawn_terrain_particles
                {
                    return None;
                }
                ParticleOptions::block(
                    ty,
                    sprite(),
                    self.terrain_particle_tint(state, pos).unwrap_or(0xFFFFFF),
                )
            }
            ParticleType::FallingDust => {
                if !facts.is_air(state) && block.render_shape == RenderShape::Invisible {
                    return None;
                }
                let map_color = || MAP_COLORS[usize::from(block.map_color)];
                let tint = match &*block.name {
                    "minecraft:anvil" | "minecraft:chipped_anvil" | "minecraft:damaged_anvil" => {
                        map_color()
                    }
                    name if name.starts_with("minecraft:")
                        && name.ends_with("_concrete_powder") =>
                    {
                        map_color()
                    }
                    name => sunrise_client::animate_tick::fixed_falling_dust_color(name)
                        .or_else(|| self.terrain_particle_tint(state, pos))
                        .unwrap_or_else(map_color),
                };
                ParticleOptions::block(ty, String::new(), tint)
            }
            ParticleType::BlockMarker => ParticleOptions::block(ty, sprite(), 0xFFFFFF),
            _ => return None,
        };
        Some(options.with_source(&block.name))
    }
    pub(in crate::game) fn craft_recipe(&self) -> Option<(String, ItemStack)> {
        let n = self.craft_size;
        let grid: Vec<Option<ItemStack>> = (0..n * n).map(|i| self.craft_grid[i].clone()).collect();
        let scale = |id: i32| self.maps.data.get(&id).map(|m| m.scale);
        self.recipes()
            .find_with(&self.loop_context, &grid, n, &scale)
            .map(|(name, result)| (name.to_owned(), result))
    }
    pub(in crate::game) fn display_index(&self) -> &sunrise_client::recipe_book::RecipeIndex {
        if self.is_logical_server() {
            &self.stations().recipe_index
        } else {
            self.recipe_tables
                .synced_index
                .as_ref()
                .unwrap_or(&self.stations().recipe_index)
        }
    }
    pub(in crate::game) fn falling_block_face_sprite(
        &self,
        state: BlockStateId,
    ) -> Option<(String, u32)> {
        let pack = &self.pack;
        let models = pack
            .render
            .model_catalog
            .collect(state, sunrise_core::position::BlockPos::new(0, 0, 0))
            .ok()?;
        let face = models
            .iter()
            .flat_map(|(_, m)| m.faces.iter())
            .find(|f| f.quad.facing == sunrise_assets::model::Direction::Up)
            .or_else(|| models.iter().flat_map(|(_, m)| m.faces.iter()).next())?;
        let name = pack.layer_names.get(&face.layer)?.clone();
        let tint = if face.quad.tint_index >= 0 {
            let n = &self.res.facts().get(state).name;
            if n.contains("leaves") || n.contains("vine") {
                0x48B518
            } else {
                0x91BD59
            }
        } else {
            0xFFFFFF
        };
        Some((name, tint))
    }
    /// The biome colours used by the mesher and client tint queries. On a
    /// world without its own table, a colour refresh updates the pack's table.
    pub(in crate::game::client_side) fn mesh_biomes(&self) -> &Arc<Biomes> {
        self.world_biomes
            .as_ref()
            .unwrap_or(&self.pack.context.biomes)
    }
    /// The server's recipes a client was sent, over its own; None on a
    /// server and until a server sends any.
    pub(in crate::game) fn synced_recipes(&self) -> Option<&sunrise_client::crafting::Recipes> {
        self.recipe_tables.synced.as_ref()
    }
    pub(in crate::game) fn client_recipes(&self) -> &Recipes {
        self.synced_recipes().unwrap_or_else(|| self.res.recipes())
    }
    pub(in crate::game) fn worker_context(&self) -> Arc<WorkerContext> {
        self.pack.context_with(self.mesh_biomes().clone())
    }
}
