#[allow(unused_imports)]
pub(in crate::game::client_side) use super::entity::Entity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, InteractTarget};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::ArmorStandAssets;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::ArmorSwap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::CartShove;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::TYPE_ID;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::broken_armor_stand_equipment;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::layer_color;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::preview_slot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::skull_kind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::stand_box;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::armor_stands::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_assets::resource_source::ResourceSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    cit::{Hand, Kind, Rule},
    entities::ArmorStand,
    items::ItemStack,
    join_seq::JoinSeq,
    mesher::Vertex,
    mobs::{Mob, Part, Species, Who, anim::Poses},
    trims::{TrimOverride, TrimRegistry, TrimTexture},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};

impl Game<'_> {
    /// The layers of the armour items `worn` (with their stacks when known,
    /// for trims and CIT armour rules), head to feet.
    pub(in crate::game) fn armor_layers(
        &self,
        worn: [Option<(&str, Option<&ItemStack>)>; 4],
    ) -> Vec<(&Part, [f32; 4], u32, u32, bool)> {
        let assets = &self.armor_stand_assets.0;
        let mut out = Vec::new();
        for (item, (layer, slot)) in worn.into_iter().zip([
            ("helmet", "head"),
            ("chestplate", "chest"),
            ("leggings", "legs"),
            ("boots", "feet"),
        ]) {
            let Some((item, stack)) = item else {
                continue;
            };
            let Some(piece) = self.equipment_piece(item, stack, slot, layer == "leggings", false)
            else {
                continue;
            };
            let Some(part) = assets.player.get(layer) else {
                continue;
            };
            for material in
                self.equipment_materials(&piece, stack, super::equipment_piece::EquipmentPass::Draw)
            {
                out.push((
                    material.model.map_or(part, |model| &**model),
                    material.binding.uv,
                    material.color,
                    material.overlay,
                    material.glows,
                ));
            }
        }
        out
    }
    /// ArmorStandRenderer: the stand with its default pose, then armor layers.
    pub(in crate::game::client_side) fn armor_stand_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
        run: std::ops::Range<usize>,
    ) {
        // The stands' models posed on the GPU path (`mobs::skin`), when it runs this frame.
        let skinning = self.skinning();
        for a in self.armor_stands[run]
            .iter()
            .filter(|a| !self.hides_as_camera(a.id))
        {
            // ArmorStand.shouldRenderAtSqrDistance: 256 times the box's mean
            // edge (4 for a marker's empty box), whatever the entity distance.
            // LivingEntityRenderer grows the culling box by 0.5 for a dragon head.
            let (width, height) = match (a.marker, a.small) {
                (true, _) => (0.0, 0.0),
                (false, true) => (0.25, 0.9875),
                (false, false) => (0.5, 1.975),
            };
            let edge = (width + height + width) / 3.0 * 4.0;
            // 26.3 compares the size with RenderSystem.DEFAULT_DEPTH_CLEAR_VALUE,
            // which is 0.0 (reverse Z; the bytecode is `dconst_0`): an empty
            // marker box falls back to 4, so a visible marker stand draws.
            // Holograms stay hidden through the invisible flag instead.
            let range = if edge == 0.0 { 4.0 } else { edge } * 64.0;
            let dragon = a.equipment[5]
                .as_ref()
                .is_some_and(|i| &*i.item == "minecraft:dragon_head");
            if !self.standing_entity_in_view(
                camera,
                super::entity_culling::at(a.previous),
                super::entity_culling::at(a.position),
                alpha,
                width,
                height,
                range,
                if dragon { 0.5 } else { 0.0 },
            ) {
                continue;
            }
            let culling = self.entity_cull_decision(a, alpha);
            if culling.hides_entity() {
                continue;
            }
            let entity_start = out.len();
            self.mark_motion(entity_start, Self::MOTION_OBJECT | u64::from(a.id));
            // EntityRenderer: drawn between last tick's position and this one's.
            let at = a.render_position(alpha);
            let p = super::entity_culling::at(at) - camera;
            // setupRotations: 180 - body yaw, plus the wobble for 5 ticks after a hit.
            let wiggle = (self.world.game_time - a.last_hit) as f32 + alpha as f32;
            let wobble = if wiggle < 5.0 {
                (wiggle / 1.5 * std::f32::consts::PI).sin() * 3.0
            } else {
                0.0
            };
            let base = glam::Mat4::from_translation(p.as_vec3())
                * glam::Mat4::from_rotation_y(
                    (180.0 - a.render_yaw(alpha as f32) + wobble).to_radians(),
                )
                * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0))
                * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.501, 0.0));
            // `EntityRenderer.getPackedLightCoords`: the block
            // `ArmorStand.getLightProbePosition` gives - the brightest cell of a
            // marker's own box, else the stand's eye - at 15 block light while
            // it burns.
            let probe = a.light_probe(alpha, |x, y, z| self.world.light(x, y, z));
            let light = self.world.light(probe[0], probe[1], probe[2]);
            let packed = a.render_light(light);
            let skinned = if skinning {
                let mut models = self.skin_models.0.borrow_mut();
                let mut frame = self.client.skin_frame.borrow_mut();
                let mark = frame.mark();
                let mut skin = sunrise_client::mobs::skin::Skin::new(&mut models, &mut frame);
                self.armor_stand_vertices(
                    a,
                    Some(alpha as f32),
                    base,
                    packed,
                    Some(&mut skin),
                    out,
                );
                Some((mark, skin.bounds))
            } else {
                self.armor_stand_vertices(a, Some(alpha as f32), base, packed, None, out);
                None
            };
            if !self.tracked_entity_glowing(a.id) {
                self.cull_skinned(out, entity_start, skinned);
            }
        }
    }
    /// `ItemInHandLayer` and `CustomHeadLayer` of an armour stand: the hand
    /// items in the third-person contexts, and a non-armour head item on the
    /// head (`translateToHead`: down a quarter, turned, 0.625 scale). With
    /// `skin`, items that can are posed on the GPU with the stand.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::game::client_side) fn armor_stand_items(
        &self,
        a: &ArmorStand,
        model: &Part,
        base: glam::Mat4,
        poses: &Poses,
        world_partial: Option<f32>,
        light: u32,
        mut skin: Option<&mut sunrise_client::mobs::skin::Skin>,
        out: &mut Vec<Vertex>,
    ) {
        let lights = sunrise_client::block_entity_render::diffuse_lights(self.nether_lighting());
        let mut submit = |state: &sunrise_client::item_render::ItemRenderState,
                          pose: glam::Mat4,
                          out: &mut Vec<Vertex>| {
            let on_gpu = skin
                .as_deref_mut()
                .is_some_and(|skin| self.skin_item(state, pose, light, lights, skin));
            if !on_gpu {
                self.submit_item_world(state, pose, light, out);
            }
        };
        use sunrise_assets::item_model::DisplayContext;
        use sunrise_client::mobs::render::part_matrix;
        let owner = super::item_draw::owner_at(
            [a.position.x, a.position.y, a.position.z],
            a.yaw,
            "minecraft:armor_stand",
        );
        // `useBabyOffset`: small stands use the baby hand offsets.
        let (ox, oy, oz) = if a.small {
            (0.0, 1.0, -4.5)
        } else {
            (1.0, 2.0, -10.0)
        };
        for (slot, left, context) in [
            (0usize, false, DisplayContext::ThirdPersonRightHand),
            (1, true, DisplayContext::ThirdPersonLeftHand),
        ] {
            let Some(stack) = &a.equipment[slot] else {
                continue;
            };
            let arm = if left { "left_arm" } else { "right_arm" };
            let Some(hand) = part_matrix(model, base, poses, arm, 0.0) else {
                continue;
            };
            let pose = hand
                * glam::Mat4::from_rotation_x((-90.0f32).to_radians())
                * glam::Mat4::from_rotation_y(std::f32::consts::PI)
                * glam::Mat4::from_translation(glam::Vec3::new(
                    if left { -ox } else { ox } / 16.0,
                    oy / 16.0,
                    oz / 16.0,
                ));
            let seed = a.id as i32 + if left { 1 } else { 2 };
            // OptiFine CIT `hand=off` rules: slot 1 is the off hand.
            let mut state = if left {
                self.resolve_item_in_hand(stack, context, Some(&owner), seed, Hand::Off)
            } else {
                self.resolve_item(stack, context, Some(&owner), seed)
            };
            if let Some(partial_tick) = world_partial {
                let hand = if left {
                    mod_api::Hand::Off
                } else {
                    mod_api::Hand::Main
                };
                self.tint_held_item(&mut state, stack, a.id, hand, partial_tick);
            }
            submit(&state, pose, out);
        }
        // `CustomHeadLayer`: a worn skull draws the skull model.
        if let Some(stack) = &a.equipment[5]
            && let Some(kind) = skull_kind(&stack.item)
            && let Some(head) = part_matrix(model, base, poses, "head", 0.0)
        {
            let pose = head * glam::Mat4::from_scale(glam::Vec3::splat(1.1875));
            let special = sunrise_assets::item_model::SpecialModel {
                kind: if kind == "player" {
                    sunrise_assets::item_model::SpecialKind::PlayerHead
                } else {
                    sunrise_assets::item_model::SpecialKind::Head {
                        kind: kind.to_owned(),
                        texture: None,
                        animation: 0.0,
                    }
                },
            };
            let mut sink = sunrise_client::item_render::WorldSink {
                out,
                light,
                lights: Some(sunrise_client::block_entity_render::diffuse_lights(
                    self.nether_lighting(),
                )),
                glint: self.glint(),
                tier: 0,
            };
            let profile = stack
                .extra
                .as_deref()
                .and_then(|d| {
                    d.preserved_components()
                        .iter()
                        .find(|(k, _)| k == "minecraft:profile")
                })
                .map(|(_, t)| self.skin_profiles.0.borrow_mut().head_profile_nbt(t))
                .unwrap_or_else(|| std::sync::Arc::new(serde_json::Value::Null));
            self.pack.item_renderer.submit_special(
                &special,
                &profile,
                false,
                pose,
                &self.gui.atlas.sprites,
                &self.item_host(),
                &mut sink,
            );
            return;
        }
        // A head item that is not worn armour.
        if let Some(stack) = &a.equipment[5]
            && !sunrise_client::item_sync::equippable(stack, self.res.items())
                .is_some_and(|e| e.asset_id.is_some() && e.slot == "head")
            && let Some(head) = part_matrix(model, base, poses, "head", 0.0)
        {
            let pose = head
                * glam::Mat4::from_translation(glam::Vec3::new(0.0, -0.25, 0.0))
                * glam::Mat4::from_rotation_y(std::f32::consts::PI)
                * glam::Mat4::from_scale(glam::Vec3::new(0.625, -0.625, -0.625));
            // `updateForLiving`: seeded by the entity id plus HEAD's ordinal.
            let state =
                self.resolve_item(stack, DisplayContext::Head, Some(&owner), a.id as i32 + 5);
            submit(&state, pose, out);
        }
    }
    /// `ArmorStandRenderer.submit` for stand `a` under `base` (its
    /// `setupRotations`): the body, the hand and head items, the armour and
    /// the wings, at packed light `packed`. `world_partial` is absent in the GUI preview.
    /// With `skin`, supported meshes are posed on the GPU.
    pub(in crate::game::client_side) fn armor_stand_vertices(
        &self,
        a: &ArmorStand,
        world_partial: Option<f32>,
        base: glam::Mat4,
        packed: u32,
        mut skin: Option<&mut sunrise_client::mobs::skin::Skin>,
        out: &mut Vec<Vertex>,
    ) {
        use sunrise_client::mobs::render::put;
        let assets = &self.armor_stand_assets.0;
        let sprites = &self.gui.atlas.sprites;
        let uv_of = |name: &str| sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
        // A texture's OptiFine emissive overlay. The lock is read per lookup and let go:
        // `wings_geometry` below reads it too.
        let glow_of = |texture: &str| {
            let emissive = self
                .res
                .mob_data()
                .emissive
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            emissive.overlay(texture).and_then(uv_of)
        };
        let Some(wood) = uv_of("entity/armorstand/armorstand") else {
            return;
        };
        let Some(model) = (if a.small { &assets.small } else { &assets.main }) else {
            return;
        };
        let mut poses = Poses::new(model);
        let partial = world_partial.unwrap_or(0.0);
        self.setup_armor_stand_pose(a, partial, &mut poses);
        // Item layers pose from the hidden arms too (`visible` only skips drawing).
        let posed = poses.clone();
        let mut hide = |name: &str| {
            if poses.has(name) {
                let pose = poses.get(name);
                pose[6] = 0.0;
                pose[7] = 0.0;
                pose[8] = 0.0;
            }
        };
        if !a.show_arms {
            hide("left_arm");
            hide("right_arm");
        }
        if a.no_base_plate {
            hide("base_plate");
        }
        // LivingEntityRenderer.submit: an invisible stand has no body render type, but its
        // layers (held items, custom head, armour) are submitted all the same.
        if !a.invisible {
            put(
                &mut skin,
                model,
                base,
                &poses,
                wood,
                0.0,
                0xFFFF_FFFF,
                packed,
                0,
                out,
            );
            if let Some(glow) = glow_of("entity/armorstand/armorstand") {
                let full = sunrise_client::mobs::render::FULL_BRIGHT;
                put(
                    &mut skin,
                    model,
                    base,
                    &poses,
                    glow,
                    0.0,
                    0xFFFF_FFFF,
                    full,
                    0,
                    out,
                );
            }
        }
        self.armor_stand_items(
            a,
            model,
            base,
            &posed,
            world_partial,
            packed,
            skin.as_deref_mut(),
            out,
        );
        // HumanoidArmorLayer: head, chest, legs, feet. One pose map for every layer
        // (`reset` keeps its capacity).
        let mut layer_poses = Poses::default();
        for (slot, layer, worn) in [
            (5usize, "helmet", "head"),
            (4, "chestplate", "chest"),
            (3, "leggings", "legs"),
            (2, "boots", "feet"),
        ] {
            let Some(stack) = &a.equipment[slot] else {
                continue;
            };
            let Some(piece) =
                self.equipment_piece(&stack.item, Some(stack), worn, layer == "leggings", false)
            else {
                continue;
            };
            let Some(part) = assets.layers.get(&(layer, a.small)) else {
                continue;
            };
            // ArmorStandArmorModel.setupAnim runs humanoid setup independently.
            layer_poses.reset(part);
            self.setup_armor_stand_pose(a, partial, &mut layer_poses);
            for material in self.equipment_materials(
                &piece,
                Some(stack),
                super::equipment_piece::EquipmentPass::Draw,
            ) {
                put(
                    &mut skin,
                    material.model.unwrap_or(part),
                    base,
                    &layer_poses,
                    material.binding.uv,
                    0.0,
                    material.color,
                    if material.glows {
                        sunrise_client::mobs::render::FULL_BRIGHT
                    } else {
                        packed
                    },
                    material.overlay,
                    out,
                );
            }
        }
        // WingsLayer: ArmorStandRenderer adds it after the armour layers. A stand
        // never crouches or glides, so its wings rest folded.
        self.wings_geometry(
            a.equipment[4].as_ref().map(|s| &*s.item),
            a.equipment[4].as_ref(),
            None,
            a.small,
            base,
            sunrise_client::mobs::ElytraAnimation::RESTING,
            false,
            packed,
            self.wings_overlay(a.equipment[4].as_ref()),
            out,
        );
    }
    /// `ArmorStand.brokenByAnything`, then `kill`, as burning out or a blast
    /// breaks a stand: its equipment pops above its block, the stand item does
    /// not drop.
    pub(in crate::game) fn break_armor_stand_by_anything(
        &mut self,
        i: usize,
        damage_type: &str,
        causing: u32,
    ) -> bool {
        let a = self.armor_stands[i].clone();
        let id = a.id;
        let at = a.position.components();
        let removed = self.remove_armor_stand_after(id, |game| {
            game.audio
                .0
                .play("minecraft:entity.armor_stand.break", Some(at), 1.0, 1.0);
            game.armor_stand_death_drops(&a, damage_type, causing);
            let (pos, drops) = broken_armor_stand_equipment(a);
            for stack in drops {
                game.pop_resource(pos, stack);
            }
            game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        });
        if removed {
            let source = self
                .level_entity(causing as i32)
                .or(Some(super::commands::Target::Stand(id)));
            self.entity_killed_event_by(at, source);
        }
        removed
    }
    /// The end of `ArmorStand.hurtServer` for `#can_break_armor_stand` or `#always_kills_armor_stands`.
    pub(in crate::game) fn break_armor_stand_by_hit(
        &mut self,
        i: usize,
        creative: bool,
        always_kills: bool,
        by: Option<super::commands::Target>,
        damage_type: &str,
        causing_id: u32,
    ) -> bool {
        let time = self.world.game_time;
        let at = self.armor_stands[i].position;
        let at = [at.x, at.y, at.z];
        if creative {
            let id = self.armor_stands[i].id;
            let (width, height) = stand_box(&self.armor_stands[i]);
            if !self.remove_armor_stand_after(id, |game| {
                game.audio
                    .0
                    .play("minecraft:entity.armor_stand.break", Some(at), 1.0, 1.0);
                game.show_breaking_particles(at, f64::from(width), f64::from(height));
            }) {
                return false;
            }
            // ArmorStand.kill(level, the causing entity).
            self.stand_game_event("minecraft:entity_die", at, by);
            return true;
        }
        if time - self.armor_stands[i].last_hit > 5 && !always_kills {
            // First hit: entity event 32, the wobble and the hit sound, then entity_damage.
            self.audio
                .0
                .play("minecraft:entity.armor_stand.hit", Some(at), 0.3, 1.0);
            self.stand_game_event("minecraft:entity_damage", at, by);
            self.armor_stands[i].last_hit = time;
            return true;
        }
        // brokenByPlayer: the stand item, then brokenByAnything: the equipment.
        let a = self.armor_stands[i].clone();
        let removed = self.remove_armor_stand_after(a.id, |game| {
            let (width, height) = stand_box(&a);
            let center = [at[0], at[1] + if a.small { 0.4875 } else { 0.9875 }, at[2]];
            let mut stand = ItemStack::new("minecraft:armor_stand", 1);
            if let Some(name) = &a.custom_name {
                stand
                    .data_mut()
                    .set_loot_name("minecraft:custom_name", &name.component());
            }
            game.spawn_item(stand, center, [0.0; 3], 10);
            game.audio
                .0
                .play("minecraft:entity.armor_stand.break", Some(at), 1.0, 1.0);
            game.armor_stand_death_drops(&a, damage_type, causing_id);
            for stack in a.equipment.into_iter().flatten() {
                let r = &mut game.world.random;
                let v = [
                    f64::from(r.next_float()) * 0.2 - 0.1,
                    0.2,
                    f64::from(r.next_float()) * 0.2 - 0.1,
                ];
                game.spawn_item(stack, center, v, 10);
            }
            game.show_breaking_particles(at, f64::from(width), f64::from(height));
        });
        if removed {
            self.stand_game_event("minecraft:entity_die", at, by);
        }
        removed
    }
    /// `SmithingScreen`'s armour stand: `updateArmorStandPreview` puts the
    /// result on a stand without its base plate and with its arms, turned to
    /// 210 degrees, and `extractEntityInInventory` draws it at scale 25 in
    /// the box (x0, y0) to (x1, y1), moved up 1 and tilted by `ARMOR_STAND_ANGLE`.
    pub(in crate::game::client_side) fn draw_smithing_armor_stand(
        &self,
        batch: &mut sunrise_client::hud::HudBatch,
        result: Option<&ItemStack>,
        [x0, y0, x1, y1]: [f32; 4],
    ) {
        let mut stand = ArmorStand::new(0, Vec3::ZERO, 210.0);
        stand.no_base_plate = true;
        stand.show_arms = true;
        if let Some(stack) = result {
            let equip = self
                .res
                .items()
                .get(&stack.item)
                .and_then(|i| i.equip_slot.clone());
            stand.equipment[preview_slot(equip.as_deref())] = Some(stack.clone());
        }
        // LivingEntityRenderer: setupRotations (180 - body yaw), the flip and
        // the 1.501 drop, at full light.
        let base = glam::Mat4::from_rotation_y((180.0 - stand.yaw).to_radians())
            * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.501, 0.0));
        let mut verts = Vec::new();
        self.armor_stand_vertices(&stand, None, base, 240 | 240 << 8, None, &mut verts);
        // ARMOR_STAND_ANGLE: rotationXYZ(0.43633232, 0, PI).
        let gui =
            glam::Mat4::from_translation(glam::Vec3::new((x0 + x1) / 2.0, (y0 + y1) / 2.0, 0.0))
                * glam::Mat4::from_scale(glam::Vec3::new(25.0, 25.0, -25.0))
                * glam::Mat4::from_translation(glam::Vec3::new(0.0, 1.0, 0.0))
                * glam::Mat4::from_rotation_x(0.436_332_32)
                * glam::Mat4::from_rotation_z(std::f32::consts::PI);
        let glints =
            super::perspective::preview_glint_quads(&verts, gui, 0..0, self.client.glint_frame);
        let quads =
            super::perspective::player_preview_quads(verts, gui, None, &self.gui.atlas.sprites);
        super::perspective::emit_preview(batch, quads, glints, [x0, y0, x1, y1]);
    }
    /// `PalettedTextureManager.getOrPrepare` for the trims drawn this frame
    /// (the player's armour and the armour stands'): each generated texture
    /// goes into an atlas trim slot, a free one, else the least recently
    /// drawn. Reuse the completed preparation while the requests and atlas stay the same.
    pub(in crate::game::client_side) fn prepare_trims(&mut self) {
        let assets = &self.armor_stand_assets.0;
        let items = self.items();
        let mut wanted: Vec<TrimTexture> = Vec::new();
        let guests = self.lan.iter().flat_map(|host| host.hub.guests.iter());
        let stacks =
            self.armor
                .iter()
                .chain(guests.flat_map(|g| g.armor.iter()))
                .chain(self.mobs.iter().flat_map(|m| {
                    sunrise_items::equipment::ARMOR.map(|slot| &m.extra.equipment[slot])
                }))
                .zip(
                    ["helmet", "chestplate", "leggings", "boots"]
                        .into_iter()
                        .cycle(),
                )
                .chain(self.armor_stands.iter().flat_map(|a| {
                    [5usize, 4, 3, 2]
                        .map(|slot| &a.equipment[slot])
                        .into_iter()
                        .zip(["helmet", "chestplate", "leggings", "boots"])
                }));
        // The smithing screen's preview stand wears the result slot's item.
        let preview = self
            .station
            .as_ref()
            .filter(|s| s.kind == super::stations::Kind::Smithing)
            .and_then(|s| s.result.as_ref())
            .map(|stack| {
                let equip = items.get(&stack.item).and_then(|i| i.equip_slot.as_deref());
                let layer = match preview_slot(equip) {
                    2 => "boots",
                    3 => "leggings",
                    4 => "chestplate",
                    5 => "helmet",
                    _ => "",
                };
                (Some(stack), layer)
            });
        let stacks = stacks.map(|(s, l)| (s.as_ref(), l)).chain(preview);
        for (stack, layer) in stacks {
            let Some(stack) = stack.filter(|s| s.extra.is_some()) else {
                continue;
            };
            let Some(asset) =
                sunrise_client::item_sync::equippable(stack, items).and_then(|e| e.asset_id)
            else {
                continue;
            };
            let kind = if layer == "leggings" {
                "humanoid_leggings"
            } else {
                "humanoid"
            };
            if let Some(t) = assets.trim_texture(&asset, kind, stack)
                && !wanted.contains(&t)
            {
                wanted.push(t);
            }
        }
        self.client.trim_slots.prepare(
            wanted,
            &assets.root,
            &mut self.gui.atlas,
            &mut self.client.map_patches,
        );
    }
    /// `ArmorStand.showBreakingParticles`: ten oak plank crumbs from the box,
    /// spread by a quarter of its width and height, sent like `sendParticles`.
    pub(in crate::game::client_side) fn show_breaking_particles(
        &mut self,
        at: [f64; 3],
        width: f64,
        height: f64,
    ) {
        let Some(state) = self
            .pack
            .assets
            .registry
            .default_state("minecraft:oak_planks")
        else {
            return;
        };
        let block = [
            at[0].floor() as i32,
            at[1].floor() as i32,
            at[2].floor() as i32,
        ];
        let Some((sprite, tint)) = self.particle_sprite(state, block) else {
            return;
        };
        let options = sunrise_client::particles::ParticleOptions::block(
            sunrise_client::particles::ParticleType::Block,
            sprite,
            tint,
        );
        // Entity.getY(2/3): the box's height is scaled by 2/3 from the feet.
        let centre = [at[0], at[1] + height * (2.0 / 3.0), at[2]];
        let spread = [width / 4.0, height / 4.0, width / 4.0];
        self.particles
            .0
            .send_particles(&options, centre, spread, 0.05, 10);
        self.lan_level_particles(&options, centre, spread, 0.05, 10, Some(state));
    }
}

#[cfg(test)]
pub(in crate::game) mod setup_anim_tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::armor_stands::setup_anim_tests::HEARD;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::armor_stands::setup_anim_tests::Listener;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::armor_stands::setup_anim_tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::atomic::{AtomicUsize, Ordering};

    /// The real stand adapter exposes humanoid riding/head preparation, then
    /// preserves position edits while saved rotations replace listener rotations.
    #[test]
    #[ignore = "daily"]
    fn armor_stand_prepares_humanoid_then_overrides_only_rotations() {
        let mods = vec![mod_api::ModEntry::new("pose", Box::new(Listener))].into();
        mod_platform::testing::with_mods(mods, || {
            assert!(mod_api::__private::listened(
                mod_api::HookId::HumanoidSetupAnim
            ));
            super::super::native_entity_refs::tests::with_client_game(|game| {
                let mut stand = ArmorStand::new(73, Vec3::ZERO, 0.0);
                stand.show_arms = true;
                stand.pitch = 30.0;
                stand.vehicle = Some(9);
                stand.pose = [[10.0, 20.0, 30.0]; 6];
                let model = game.armor_stand_assets.0.main.as_ref().unwrap();
                let mut poses = Poses::new(model);
                game.setup_armor_stand_pose(&stand, 0.5, &mut poses);
                for name in [
                    "head",
                    "body",
                    "left_arm",
                    "right_arm",
                    "left_leg",
                    "right_leg",
                ] {
                    let pose = poses.pose_of(name).unwrap();
                    assert_eq!(&pose[..3], &[7.0, 8.0, 9.0]);
                    assert_eq!(
                        &pose[3..6],
                        &[
                            10.0_f32.to_radians(),
                            20.0_f32.to_radians(),
                            30.0_f32.to_radians()
                        ]
                    );
                }
                let mut verts = Vec::new();
                game.armor_stand_vertices(
                    &stand,
                    Some(0.5),
                    glam::Mat4::IDENTITY,
                    0xf0f0,
                    None,
                    &mut verts,
                );
                assert!(!verts.is_empty());
            });
        });
        assert_eq!(HEARD.load(Ordering::Relaxed), 2);
    }
}
