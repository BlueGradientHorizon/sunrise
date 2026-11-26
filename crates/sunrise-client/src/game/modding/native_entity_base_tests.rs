//! #5082: the base `Entity` state (`Fire`, `Invulnerable`, `NoGravity`, `Silent`, `Glowing`)
//! reads and writes through a handle for the kinds that had no support before, and lands in
//! the tags vanilla's `Entity.saveWithoutId` uses. The general entity's own test is in
//! `entities/general/save_load_tests.rs`.
#![cfg(test)]
use crate::game::Game;
use crate::game::boats::Boat;
use crate::game::minecarts::{CartKind, Minecart};
use crate::game::native_entity_refs::tests::with_game;
use mod_api::EntityRef;
use std::sync::Arc;
use sunrise_client::projectiles::{Projectile, ProjectileKind};
use sunrise_core::{geometry::Vec3, nbt::Tag};

fn open_chunk(game: &mut Game<'_>) {
    game.world
        .chunks
        .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
}

/// Writes every base value through `entity`, and checks the reads answer them.
fn write_and_read_back(game: &mut Game<'_>, entity: EntityRef) {
    assert_eq!(entity.fire_ticks(game), Some(0));
    assert!(entity.set_remaining_fire_ticks(game, 60));
    assert!(entity.set_no_gravity(game, true));
    assert!(entity.set_silent(game, true));
    assert!(entity.set_glowing(game, true));
    assert!(entity.set_invulnerable(game, true));
    assert_eq!(entity.fire_ticks(game), Some(60));
    assert_eq!(entity.is_on_fire(game), Some(true));
    assert_eq!(entity.no_gravity(game), Some(true));
    assert_eq!(entity.is_silent(game), Some(true));
    assert_eq!(entity.glowing(game), Some(true));
    assert_eq!(entity.permanently_invulnerable(game), Some(true));
    assert!(entity.set_no_gravity(game, false));
    assert_eq!(entity.no_gravity(game), Some(false));
    assert!(entity.clear_fire(game));
    assert_eq!(entity.is_on_fire(game), Some(false));
}

#[test]
#[ignore = "daily"]
fn a_boat_keeps_its_base_state() {
    with_game(|game| {
        open_chunk(game);
        game.boats
            .push(Boat::new(31, "oak".into(), Vec3::new(8.5, 65.0, 8.5), 0.0));
        let entity = game.entity(31).unwrap();
        write_and_read_back(game, entity);
        let raw = game.persist.raw.get(&31).unwrap();
        assert_eq!(raw.get("Silent"), Some(&Tag::Byte(1)));
        assert_eq!(raw.get("Glowing"), Some(&Tag::Byte(1)));
    });
}

#[test]
#[ignore = "daily"]
fn a_minecart_keeps_its_base_state() {
    with_game(|game| {
        open_chunk(game);
        game.carts.push(Minecart::new(
            32,
            CartKind::Rideable,
            Vec3::new(8.5, 65.0, 8.5),
        ));
        let entity = game.entity(32).unwrap();
        write_and_read_back(game, entity);
        let raw = game.persist.raw.get(&32).unwrap();
        assert_eq!(raw.get("Glowing"), Some(&Tag::Byte(1)));
        assert_eq!(raw.get("Invulnerable"), Some(&Tag::Byte(1)));
    });
}

#[test]
#[ignore = "daily"]
fn a_projectile_keeps_its_base_state() {
    with_game(|game| {
        open_chunk(game);
        let mut shot = Projectile::new(ProjectileKind::Trident, Vec3::new(8.5, 65.0, 8.5), None);
        shot.id = game.take_entity_id();
        let id = shot.id;
        game.creatures.projectiles.push(shot);
        let entity = game.entity(id).unwrap();
        write_and_read_back(game, entity);
        assert_eq!(entity.fire_immune(game), Some(false));
    });
}

#[test]
#[ignore = "daily"]
fn a_marker_keeps_its_base_state_in_its_saved_tags() {
    with_game(|game| {
        open_chunk(game);
        game.others.push(crate::game::commands::OtherEntity {
            size_answer: None,
            attachment_lifetime: Default::default(),
            knot_check: None,
            eye: None,
            join: Default::default(),
            id: 33,
            kind: "minecraft:marker".into(),
            position: Vec3::new(8.5, 65.0, 8.5),
            yaw: 0.0,
            pitch: 0.0,
            display: None,
            interaction: None,
        });
        let entity = game.entity(33).unwrap();
        write_and_read_back(game, entity);
        let raw = game.persist.raw.get(&33).unwrap();
        assert_eq!(raw.get("Silent"), Some(&Tag::Byte(1)));
        assert_eq!(raw.get("NoGravity"), None);
    });
}
