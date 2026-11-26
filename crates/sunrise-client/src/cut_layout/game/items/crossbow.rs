

#[cfg(test)]
pub(crate) mod tests {

use super::*;

pub(in crate::game) use crate::game::client_side::crossbow_client::tests::game_with_player;

/// `Projectile.applyOnProjectileSpawned` runs `EnchantmentHelper.onProjectileSpawned`
    /// for the weapon an arrow is fired from, bow or crossbow, so a Flame
    /// crossbow shoots burning arrows: Flame's `Ignite(constant(100))` is 100
    /// seconds, 2000 ticks, on every arrow of a Multishot volley (#10482).
    #[test]
    #[ignore = "daily"]
    fn a_flame_crossbow_shoots_burning_arrows() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let mut resources = None;
        let (mut game, _) = game_with_player(&mut resources, &root);
        // Shoots a crossbow with `enchantments` and one arrow loaded per shot,
        // and returns how long each new arrow burns for.
        let mut shoot = |enchantments: &[&str], shots: usize| -> Vec<i32> {
            let mut crossbow = ItemStack::new(CROSSBOW, 1);
            for id in enchantments {
                crossbow.enchant(id, 1);
            }
            crossbow.data_mut().charged = vec![ItemStack::new(ARROW, 1); shots];
            *game.held_mut() = Some(crossbow);
            let before = game.creatures.projectiles.len();
            game.shoot_crossbow();
            game.creatures.projectiles[before..]
                .iter()
                .map(|p| p.arrow.as_ref().expect("an arrow").fire_ticks)
                .collect()
        };
        assert_eq!(shoot(&[], 1), vec![0]);
        assert_eq!(shoot(&["minecraft:flame"], 1), vec![2000]);
        assert_eq!(
            shoot(&["minecraft:flame", "minecraft:multishot"], 3),
            vec![2000; 3]
        );
    }
}
