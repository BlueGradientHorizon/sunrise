#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::entity::{Entity as _, with_entity, with_entity_mut};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::synced_pose;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{EntityRef, Pose};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::Mob;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::pose::{Dimensions, SizeAnswer};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{Aabb, Vec3};

impl Game<'_> {
    /// The vanilla size of a client's copy `id` of a non-mob entity of type `kind`
    /// (`Entity.getDimensions(pose)`): its type's, or what its synched values make it.
    pub(in crate::game) fn client_object_dimensions(
        &self,
        id: u32,
        kind: &str,
    ) -> Option<[f32; 3]> {
        if let Some(a) = self.armor_stands.iter().find(|a| a.id == id) {
            return Some(a.default_dimensions());
        }
        if let Some(i) = self.client.interactions.get(&(id as i32)) {
            // Interaction.getDimensions: EntityDimensions.scalable(width, height).
            return Some([i.width, i.height, i.height * 0.85]);
        }
        if let Some(c) = self.potions.clouds.iter().find(|c| c.id == id) {
            // AreaEffectCloud.getDimensions: scalable(radius * 2, 0.5).
            return Some([c.radius * 2.0, 0.5, 0.5 * 0.85]);
        }
        super::entity_type_rows::entity_type_rows(&self.options.resources).dimensions(kind)
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::Calls;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::Grow;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::Log;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::Seen;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::SetSize;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::ShortCrouch;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::add_entity;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::corners;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::deliver;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::doubled;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::entity_data;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::in_fresh_process;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::m_pose;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::made_box;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::scale;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::with_loopback_client;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::with_server;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::entity_sizes::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::{EntitySizeEvent, Hooks, Host, Pose};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::{Arc, Mutex};

    /// A client's copies of entities that aren't mobs ask too: a minecart's box, the
    /// crosshair's pick, its F3+B outline and the box mods read take the answer; an armor stand
    /// asks again when it turns small; a display asks from its constructor.
    #[test]
    #[ignore = "daily"]
    fn client_copies_of_other_kinds_ask_and_take_the_answer() {
        use sunrise_protocol::entity_data::{Value, serializer};
        if in_fresh_process("client_copies_of_other_kinds_ask_and_take_the_answer") {
            return;
        }
        with_loopback_client(|game, seen| {
            let at = [8.5, 70.0, 8.5];
            add_entity(game, 32, "minecraft:minecart", at);
            add_entity(game, 41, "minecraft:armor_stand", [2.5, 70.0, 2.5]);
            add_entity(game, 61, "minecraft:block_display", [4.5, 70.0, 4.5]);
            // EntityType.MINECART: 0.98 by 0.7, eyes 0.595; ARMOR_STAND: 0.5 by 1.975, eyes
            // 1.7775; BLOCK_DISPLAY: none.
            let (cart, stand) = ([0.98, 0.7, 0.595], [0.5, 1.975, 1.7775]);
            // ArmorStand.DATA_CLIENT_FLAGS small: BABY_DIMENSIONS, the type's at half scale
            // with its eyes at 0.9875.
            entity_data(game, 41, vec![(15, serializer::BYTE, Value::Byte(1))]);
            assert_eq!(
                seen.lock().unwrap().sizes,
                [
                    (32, Pose::Standing, cart, cart),
                    (41, Pose::Standing, stand, stand),
                    (61, Pose::Standing, [0.0; 3], [0.0; 3]),
                    (41, Pose::Standing, doubled(stand), [0.25, 0.9875, 0.9875]),
                ]
            );
            let [width, height, _] = doubled(cart);
            let half = f64::from(width / 2.0);
            let min = [at[0] - half, at[1], at[2] - half];
            let max = [at[0] + half, at[1] + f64::from(height), at[2] + half];
            let entity = game.entity(32).unwrap();
            assert_eq!(game.entity_bounding_box(entity), Some([min, max]));
            // Only the cart and the armor stand show an outline: keep the cart's.
            game.armor_stands.clear();
            assert_eq!(white_outline(game), (min, max));
            // Aimed over a vanilla cart (y 70.7) but into the answered box: the crosshair
            // picks the cart.
            game.title.0 = None;
            game.player = Some(
                sunrise_client::motion::PlayerMotion::new(
                    game.motion,
                    sunrise_core::geometry::Vec3::new(5.5, 69.5, 8.5),
                )
                .unwrap(),
            );
            (game.yaw, game.pitch, game.entity_reach) = (-90.0, 0.0, 3.0);
            game.refresh_client_pick(1.0);
            assert!(matches!(
                game.crosshair_target(),
                Some(mod_api::CrosshairTarget::Entity { entity: hit, .. }) if hit == entity
            ));
        });
    }

    /// The F3+B outline's white edges: their lowest and highest corner.
    pub(in crate::game) fn white_outline(game: &mut crate::game::Game<'_>) -> ([f64; 3], [f64; 3]) {
        // F3+B, on entries with no file behind them: nothing is saved.
        game.debug_entries.0 = sunrise_client::debug_entries::DebugEntries::default();
        game.debug_entries.0.toggle_status("entity_hitboxes", false);
        let gizmos = game.debug_gizmos(1.0, [0.0; 3]);
        let ends: Vec<[f64; 3]> = gizmos
            .lines
            .iter()
            .filter(|line| line.color == sunrise_client::gizmos::WHITE)
            .flat_map(|line| [line.start, line.end])
            .collect();
        assert_eq!(ends.len(), 24, "one white cuboid");
        let extent = |pick: fn(f64, f64) -> f64, from: f64| {
            std::array::from_fn(|axis| ends.iter().map(|end| end[axis]).fold(from, pick))
        };
        (
            extent(f64::min, f64::INFINITY),
            extent(f64::max, f64::NEG_INFINITY),
        )
    }
}
