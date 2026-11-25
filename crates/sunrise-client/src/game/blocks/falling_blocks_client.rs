#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
use crate::game::FallingBlock;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::falling_blocks::*;
use sunrise_client::{Result, collisions};
use sunrise_core::geometry::Vec3;
use sunrise_world::{
    collision_context::{CollisionContext, CollisionEntity},
    entity_move::MoverType,
    entity_obstruction::EntityIdentity,
};

impl Game<'_> {
    /// Client-side `FallingBlockEntity.tick` motion prediction (no landing, dropping,
    /// impact, portal transfer, lava-hurt, or server-broadcast particles).
    pub(in crate::game) fn client_fall_step(&mut self, f: &mut FallingBlock) -> Result<()> {
        if self.res.facts().is_air(f.state) {
            return Ok(());
        }
        f.previous = f.position;
        f.age = f.age.wrapping_add(1);
        f.velocity.y -= sunrise_client::entity_rules::EntityRules::get_gravity(f);
        let wanted = f.velocity;
        let mut gate = sunrise_client::move_gate::armed(
            &self.loop_context,
            f.id,
            "minecraft:falling_block",
            MoverType::SelfMovement,
            wanted,
        );
        let mut wanted = gate.delta();
        if f.stuck.length_squared() > 1.0e-7 {
            wanted = wanted.multiply(f.stuck);
            f.stuck = Vec3::ZERO;
            f.velocity = Vec3::ZERO;
        }
        let scene = collisions::SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: Some(collisions::PowderSnowSubject {
                fall_distance: f.fall_distance,
                falling_block: true,
                ..Default::default()
            }),
        };
        let context = CollisionContext::for_entity(
            CollisionEntity {
                identity: EntityIdentity(u64::from(f.id)),
                kind: sunrise_world::collision_context::EntityContextKind::Other,
                y: f.position.y,
                descending: false,
                main_hand: None,
            },
            false,
        );
        let mut collisions = collisions::EntityCollisions {
            blocks: scene,
            entities: &[],
            border: Some((self.border.geometry(), [f.position.x, f.position.z])),
        };
        let movement = sunrise_client::move_gate::entities_resolve_with_context(
            &mut gate,
            &mut collisions,
            f.bounds(),
            wanted,
            0.0,
            false,
            context,
        )?;
        let next = Vec3::new(
            f.position.x + movement.x,
            f.position.y + movement.y,
            f.position.z + movement.z,
        );
        if movement.y < 0.0 {
            f.fall_distance -= f64::from(movement.y as f32);
        }
        let from = f.position;
        f.position = next;
        let body = sunrise_client::motion::BodyMotion {
            world: &self.world,
            collision: self.collision,
            facts: self.res.facts(),
            blocks: Some(&self.res.motion().blocks),
            context: &self.loop_context,
            side: None,
        };
        if !gate.skipped() {
            body.record_move(
                sunrise_client::motion::BodyCollision {
                    footprint: (f.position, f.bounds()),
                    movement: (wanted, movement),
                    authoritative: false,
                    context,
                    subject: Some(collisions::PowderSnowSubject {
                        fall_distance: f.fall_distance,
                        falling_block: true,
                        ..Default::default()
                    }),
                },
                &mut f.ground,
            );
        }
        let effect = body.legacy_contact(f.position, &f.ground);
        let entity = (f.id, "minecraft:falling_block");
        let side_server = self.game_side_client();
        let response = self.body_motion(&game_side!(self, side_server)).restitute(
            entity,
            sunrise_client::motion::BodyMove {
                wanted,
                moved: movement,
                effect,
            },
            sunrise_client::entity_rules::EntityRules::get_gravity(f),
            0.98_f32,
            f.velocity,
            f.fall_distance,
        );
        f.velocity = response.velocity;
        let side = game_side!(self, side_server);
        let body = self.body_motion(&side);
        body.apply_block_speed(f.position, &mut f.velocity);
        let _ = self.body_step_on(entity, (f.position, f.bounds()), &f.ground, &mut f.velocity);
        let moved = movement.length_squared();
        let recorded = moved > 1.0e-7 || wanted.length_squared() - moved < 1.0e-7;
        let world = &self.world;
        let facts = self.res.facts();
        let (registry, collision) = (self.pack.assets.registry, self.collision);
        let mut tops = Vec::new();
        let stuck = sunrise_client::falling_block::apply_effects_from_blocks(
            from,
            f.position,
            recorded.then_some(wanted),
            &mut f.velocity,
            &mut f.fall_distance,
            &mut tops,
            |p| {
                use sunrise_client::falling_block::Inside;
                if &*facts.get(world.block(p.x, p.y, p.z)).name == "minecraft:cobweb" {
                    Inside::Cobweb
                } else if let Some(column) =
                    sunrise_client::motion::bubble_column_at(world, registry, facts, collision, p)
                {
                    Inside::BubbleColumn(column)
                } else {
                    Inside::Nothing
                }
            },
        );
        if let Some(multiplier) = stuck {
            f.stuck = multiplier;
        }
        let fluids = &self.res.motion().fluids;
        let touches = |family| {
            sunrise_client::entities::fluid_contacts(
                from,
                f.position,
                recorded.then_some(wanted),
                sunrise_client::falling_block::bounds,
                |p| sunrise_client::entities::fluid_height(world, fluids, p, family),
            ) > 0
        };
        let previous_fire = f.fire_ticks;
        let mut fire = previous_fire;
        if touches(sunrise_world::fluid::FluidKind::Lava) {
            fire = fire.max(300);
        }
        if fire > 0 && touches(sunrise_world::fluid::FluidKind::Water) {
            fire = 0;
        }
        if fire <= 0 && fire <= previous_fire {
            fire = 0;
        }
        f.fire_ticks = fire;
        f.velocity = sunrise_client::entity_rules::scale_air_drag(f.velocity);
        Ok(())
    }

    /// Cache only model-dependent work, never an entity's seed, light or pose.
    pub(in crate::game) fn falling_model_vertices(
        &self,
        state: BlockStateId,
        start_pos: BlockPos,
        origin: glam::Vec3,
        light: u32,
        out: &mut Vec<Vertex>,
    ) {
        // `FallingBlockRenderer`: the model is picked by the block's starting position.
        let seed = sunrise_core::random::position_seed(start_pos);
        let Ok(models) = self.pack.render.model_catalog.collect_seeded(state, seed) else {
            return;
        };
        // Baked models live at stable addresses until the pack is replaced.
        // Keep multipart order and duplicates: both affect the emitted vertices.
        let mut cache = self.client.falling_vertices.borrow_mut();
        let entries = cache.entry(state).or_default();
        let index = entries.iter().position(|(ptrs, _)| {
            ptrs.iter()
                .copied()
                .eq(models.iter().map(|(_, m)| *m as *const _ as usize))
        });
        let index = index.unwrap_or_else(|| {
            let mut vertices = Vec::new();
            self.block_model_vertices(state, seed, glam::Mat4::IDENTITY, 0, false, &mut vertices);
            // Preserve the original coordinates, including signed zero, rather
            // than composing an identity transform with the frame's translation.
            let positions = models.iter().flat_map(|(_, model)| {
                model
                    .faces
                    .iter()
                    .filter(|face| {
                        self.pack
                            .layer_names
                            .get(&face.layer)
                            .is_some_and(|name| self.gui.atlas.sprites.contains_key(name))
                    })
                    .flat_map(|face| face.quad.positions)
            });
            for (vertex, pos) in vertices.iter_mut().zip(positions) {
                vertex.pos = pos;
            }
            let ptrs = models
                .iter()
                .map(|(_, m)| *m as *const _ as usize)
                .collect();
            entries.push((ptrs, vertices));
            entries.len() - 1
        });
        let start = out.len();
        out.extend_from_slice(&entries[index].1);
        let matrix = glam::Mat4::from_translation(origin);
        for vertex in &mut out[start..] {
            vertex.pos = matrix
                .transform_point3(glam::Vec3::from_array(vertex.pos))
                .to_array();
            vertex.light = light;
        }
    }
}
