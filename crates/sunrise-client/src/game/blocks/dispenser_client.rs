#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::{
    TryIntoApi, TryIntoEnginePreservingCount,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Dim, Game, item_abilities::ability};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::DispenseScope;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::DispenseSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::SUCCESS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::optional_event;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::set_success;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::SoundSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::ops::ControlFlow;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    block_entities::BlockEntityIdentity,
    collisions::SceneCollisions,
    containers::BlockEntity,
    items::{ItemStack, Items},
    mobs::{Species, equip::equippable, sulfur_cube},
    particles::{ParticleOptions, ParticleType},
    projectiles::{Projectile, ProjectileKind},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Direction, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::dispense_rules::{
    CLICK, Dispensed, FAIL, Pos, Slot, body_equipment_species, equip_sound, mob_set_slot,
    mob_slot_item, offset, tipped_effect,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::dispense_rules::{
    LAUNCH, block_box, data_value, mob_can_dispenser_equip, mob_can_use_slot, overlaps,
    section_order, shrink, step, y_rot,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    fluid::FluidKind,
    level::{PlaceContext, PlacementSource},
    registry::BlockStateId,
};

impl Game<'_> {
    /// The dispenser level events: 1000 click, 1001 fail, 1002 launch, 1004
    /// firework shoot, 1018 blaze shoot and 2000 smoke shot out of the front.
    pub(in crate::game) fn dispenser_event(&mut self, id: i32, pos: Pos, data: i32) {
        super::dispense_fixture::record(|| format!("L {id} {},{},{} {data}", pos.0, pos.1, pos.2));
        let c = Self::center(pos);
        match id {
            1000 => self
                .audio
                .0
                .play("minecraft:block.dispenser.dispense", Some(c), 1.0, 1.0),
            1001 => self
                .audio
                .0
                .play("minecraft:block.dispenser.fail", Some(c), 1.0, 1.2),
            1002 => self
                .audio
                .0
                .play("minecraft:block.dispenser.launch", Some(c), 1.0, 1.2),
            1004 => self
                .audio
                .0
                .play("minecraft:entity.firework_rocket.shoot", Some(c), 1.0, 1.2),
            // LevelEventHandler runs on the client: its pitch draws come from
            // the client level's random, never the server stream.
            1018 => {
                let r = self.particles.0.random();
                let pitch = (r.next_float() - r.next_float()) * 0.2 + 1.0;
                self.audio
                    .0
                    .play("minecraft:entity.blaze.shoot", Some(c), 2.0, pitch);
            }
            1051 => {
                let pitch = 0.4 / (self.particles.0.random().next_float() * 0.4 + 0.8);
                self.audio
                    .0
                    .play("minecraft:entity.wind_charge.throw", Some(c), 0.5, pitch);
            }
            1049 => self
                .audio
                .0
                .play("minecraft:block.crafter.craft", Some(c), 1.0, 1.0),
            1050 => self
                .audio
                .0
                .play("minecraft:block.crafter.fail", Some(c), 1.0, 1.0),
            // LevelEventHandler.shootParticles: smoke for dispensers, white
            // smoke out of a crafter's front.
            2000 | 2010 => {
                let particle = if id == 2010 {
                    ParticleType::WhiteSmoke
                } else {
                    ParticleType::Smoke
                };
                let s = match data {
                    0 => [0, -1, 0],
                    1 => [0, 1, 0],
                    2 => [0, 0, -1],
                    3 => [0, 0, 1],
                    4 => [-1, 0, 0],
                    _ => [1, 0, 0],
                };
                for _ in 0..10 {
                    let r = self.particles.0.random();
                    let pow = r.next_double() * 0.2 + 0.01;
                    let (sx, sy, sz) = (f64::from(s[0]), f64::from(s[1]), f64::from(s[2]));
                    let p = [
                        f64::from(pos.0)
                            + sx * 0.6
                            + 0.5
                            + sx * 0.01
                            + (r.next_double() - 0.5) * sz * 0.5,
                        f64::from(pos.1)
                            + sy * 0.6
                            + 0.5
                            + sy * 0.01
                            + (r.next_double() - 0.5) * sy * 0.5,
                        f64::from(pos.2)
                            + sz * 0.6
                            + 0.5
                            + sz * 0.01
                            + (r.next_double() - 0.5) * sx * 0.5,
                    ];
                    let v = [
                        sx * pow + r.next_gaussian() * 0.01,
                        sy * pow + r.next_gaussian() * 0.01,
                        sz * pow + r.next_gaussian() * 0.01,
                    ];
                    self.particles
                        .0
                        .add(&ParticleOptions::simple(particle), p, v);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Fits;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Front;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Heard;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Item;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Probe;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Tailor;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::{Arc, atomic::AtomicI32, mpsc};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::lan::{
        Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::mobs::Mob;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_protocol::packets::configuration::ClientInformation;

    /// `DispenserBlock.getDispenseMethod`: the block the dispenser faces
    /// answers first and wins for any item; with no answer from it the item's
    /// part decides; with neither, vanilla drops one item as before.
    #[test]
    #[ignore = "daily"]
    fn the_faced_block_answers_before_the_item_and_vanilla_stands_when_no_part_listens() {
        use std::sync::Arc;
        use sunrise_client::chunk::Chunk;

        let heard = Heard::default();
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "dispense_probe",
            Box::new(Probe(heard.clone())),
        )]
        .into();
        super::super::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let stone = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                let air = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:air")
                    .unwrap();
                // The dispenser at (5, 64, 5) faces stone.
                game.world.set(6, 64, 5, stone).unwrap();
                game.audio.0.capture = Some(Vec::new());
                let dispense = |game: &mut Game<'_>, item: &str| {
                    game.dispense_item((5, 64, 5), Direction::East, ItemStack::new(item, 1))
                        .unwrap()
                        .continue_value()
                        .unwrap()
                };

                // 1. The faced block answers for an item with no part of its own: it wins, and
                //    the slot is emptied with the event it chose.
                assert_eq!(dispense(game, "minecraft:dirt"), None);
                assert_eq!(heard.take(), [("block", [5, 64, 5], stone.0)]);
                assert_eq!(
                    game.audio.0.capture.as_ref().unwrap().last().unwrap().event,
                    "minecraft:block.dispenser.fail"
                );
                assert!(
                    game.dropped.is_empty(),
                    "the block's answer owns the dispense"
                );
                game.audio.0.capture.as_mut().unwrap().clear();

                // 2. The faced block answers for an item that has its own part: its answer wins
                //    and the item's part is never asked.
                assert_eq!(dispense(game, "minecraft:cobblestone"), None);
                assert_eq!(heard.take(), [("block", [5, 64, 5], stone.0)]);
                assert_eq!(
                    game.audio.0.capture.as_ref().unwrap().last().unwrap().event,
                    "minecraft:block.dispenser.fail"
                );
                assert!(game.dropped.is_empty());
                game.audio.0.capture.as_mut().unwrap().clear();

                // 3. No block answer: the item's part decides.
                assert_eq!(dispense(game, "minecraft:stone"), None);
                assert_eq!(
                    heard.take(),
                    [("block", [5, 64, 5], stone.0), ("item", [5, 64, 5], 0)]
                );
                assert_eq!(
                    game.audio.0.capture.as_ref().unwrap().last().unwrap().event,
                    "minecraft:block.dispenser.dispense"
                );
                assert!(game.dropped.is_empty());
                game.audio.0.capture.as_mut().unwrap().clear();

                // 4. Neither the block in front nor the item has a part: vanilla drops one.
                game.world.set(6, 64, 5, air).unwrap();
                assert_eq!(dispense(game, "minecraft:stick"), None);
                assert!(heard.take().is_empty());
                assert_eq!(game.dropped.len(), 1);
                assert_eq!(game.dropped[0].stack, ItemStack::new("minecraft:stick", 1));
                assert_eq!(
                    game.audio.0.capture.as_ref().unwrap().last().unwrap().event,
                    "minecraft:block.dispenser.dispense"
                );
            });
        });
    }
}
