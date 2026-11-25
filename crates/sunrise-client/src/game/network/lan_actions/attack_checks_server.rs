#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::attack_checks::arrow_attackable;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::attack_checks::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::{
    packets::configuration::UpdateTags, text::translatable,
};

impl Game<'_> {
    /// Whether a guest's attack on entity `id` ends before `Player.attack` (handleAttack).
    /// The guest who attacks an `ItemEntity`, an `ExperienceOrb`, themselves or an
    /// `AbstractArrow` that is not `isAttackable` is disconnected with
    /// `multiplayer.disconnect.invalid_entity_attacked`.
    pub(in crate::game::server_side) fn guest_attack_stops(&mut self, gi: usize, id: i32) -> bool {
        use crate::game::commands::Target;
        let Some(host) = self.lan.as_ref() else {
            return true;
        };
        let guest = &host.hub.guests[gi];
        let present = host.hub.here(guest)
            || (guest.is_fake() && guest.placed && guest.dimension == host.hub.config.dimension);
        if !present || !guest.left_reason.is_empty() {
            return true;
        }
        let conn = guest.conn;
        let target = if guest.entity_id == id {
            Some(guest.bounds())
        } else {
            self.level_entity(id).and_then(|target| {
                let refused = match target {
                    Target::Item(_) | Target::Orb(_) => true,
                    Target::Projectile(_) | Target::Trident(_) => {
                        let kind = self.target_type(target);
                        if !matches!(
                            kind.as_str(),
                            "minecraft:arrow" | "minecraft:spectral_arrow" | "minecraft:trident"
                        ) {
                            return None;
                        }
                        let sync = host.hub.lan.shared.sync();
                        !arrow_attackable(&sync.tags, sync.id("minecraft:entity_type", &kind))
                    }
                    _ => false,
                };
                refused
                    .then(|| self.selector_target_bounds(target))
                    .flatten()
            })
        };
        let Some(target) = target else {
            return false;
        };
        if !guest.client_loaded()
            || !self.inside_border(&target)
            || self.as_guest_at(gi, conn, |g| {
                g.within_attack_range(&target) && !g.holds_piercing_weapon()
            }) != Ok(true)
        {
            return true;
        }
        let host = self.lan.as_mut().unwrap();
        let name = host.hub.guests[gi].name.clone();
        host.hub.kick(
            conn,
            translatable("multiplayer.disconnect.invalid_entity_attacked", &[]),
        );
        log::warn!("Player {name} tried to attack an invalid entity");
        true
    }
    /// The main-hand stack has `minecraft:piercing_weapon` (a spear's).
    pub(in crate::game::server_side) fn holds_piercing_weapon(&self) -> bool {
        self.held().is_some_and(|stack| {
            self.pack
                .item_renderer
                .models
                .components_view(&sunrise_client::item_render::stack_view(stack))
                .contains_key("minecraft:piercing_weapon")
        })
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::lan_actions::attack_checks::tests::disconnects;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::lan_actions::attack_checks::tests::tags;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::lan_actions::attack_checks::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::{
        Audio, Options, Progress, Settings, load_resources,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use std::sync::{Arc, atomic::AtomicI32, mpsc};
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::chunk::Chunk;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::lan::{
        Joined, NetConfig, Out, PlayerSender, guest::Guest, hub::HubConfig,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::position::ChunkPos;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_protocol::packets::configuration::{
        ClientInformation, RegistryTags, TagEntry, UpdateTags,
    };

    /// handleAttack disconnects a player who attacks a dropped item, an experience orb,
    /// themselves, an arrow or a thrown trident (`invalid_entity_attacked`); a pig is
    /// attacked as before, and an item out of reach, or attacked with a spear in hand,
    /// ends the attack without a kick.
    #[test]
    #[ignore = "daily"]
    fn attacking_an_item_an_orb_oneself_or_an_arrow_kicks_the_player() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir = Some(
            std::env::temp_dir().join(format!("sunrise-attack-checks-unit-{}", std::process::id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = Game::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap()
            .0;
        let mut chunk = Chunk::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 199, z, stone);
            }
        }
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        game.lan_host(
            crate::game::lan::LanSettings {
                port: 0,
                guest_commands: true,
                force_game_mode: false,
            },
            NetConfig {
                announce: false,
                ..NetConfig::loopback(0, "attack-checks-test".into(), 2)
            },
            HubConfig {
                default_game_mode: 0,
                guest_commands: true,
                force_game_mode: false,
                hardcore: false,
                view_distance: 2,
                simulation_distance: 2,
                dimension: "minecraft:overworld".into(),
                allow_flight: false,
                seed: 0,
                max_players: 2,
                integrated: false,
                levels: Default::default(),
                level_seeds: Default::default(),
            },
        )
        .unwrap();
        let summoned = |game: &mut Game<'_>, command: &str| {
            let before = game.server.next_entity;
            assert!(game.run_command(command), "{command}");
            game.lan_assign_ids();
            assert!(game.server.next_entity > before, "{command}");
            game.server.next_entity as i32
        };
        let pig = summoned(
            &mut game,
            "summon pig 6.5 200 4.5 {NoAI:1b,NoGravity:1b,PersistenceRequired:1b}",
        );
        let item = summoned(
            &mut game,
            "summon item 4.5 200 6 {NoGravity:1b,Item:{id:\"minecraft:stone\",count:1}}",
        );
        let far_item = summoned(
            &mut game,
            "summon item 4.5 200 12.5 {NoGravity:1b,Item:{id:\"minecraft:stone\",count:1}}",
        );
        let orb = summoned(
            &mut game,
            "summon experience_orb 3 200 4.5 {NoGravity:1b,Value:1s}",
        );
        let arrow = summoned(&mut game, "summon arrow 4.5 200.5 3 {NoGravity:1b}");
        let trident = summoned(&mut game, "summon trident 5.5 200.5 3 {NoGravity:1b}");
        let (tx, rx) = mpsc::channel();
        let fresh_guest = |conn, entity_id| {
            let mut guest = Guest::new(
                Joined {
                    conn,
                    uuid: 332,
                    name: "AttackGuest".into(),
                    properties: Vec::new(),
                    info: ClientInformation::new(2),
                    brand: "test".into(),
                    sender: PlayerSender::new(tx.clone(), Arc::default()),
                    latency: Arc::new(AtomicI32::new(0)),
                    address: "127.0.0.1:25565".parse().unwrap(),
                    expected: false,
                },
                entity_id,
                [4.5, 200.0, 4.5],
                2,
            );
            guest.dimension = "minecraft:overworld".into();
            guest.placed = true;
            guest.loaded = true;
            guest.load_timeout = 0;
            guest
        };
        game.server.next_entity += 1;
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .push(fresh_guest(2, game.server.next_entity as i32));
        let attack = |game: &mut Game<'_>, conn, id: i32| {
            game.lan_guest_packet(conn, S::Attack(play::Attack { id }));
        };
        let kicked = vec![translatable(
            "multiplayer.disconnect.invalid_entity_attacked",
            &[],
        )];

        let hp = |g: &Game<'_>| g.mobs.iter().find(|m| m.id as i32 == pig).unwrap().health;
        let full = hp(&game);
        attack(&mut game, 2, pig);
        assert_eq!(disconnects(&rx), vec![], "a pig is a target");
        assert!(hp(&game) < full, "the pig is not hurt: {full} health left");
        attack(&mut game, 2, far_item);
        assert_eq!(disconnects(&rx), vec![], "an item out of reach");
        game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
            guest.inventory[0] = Some(ItemStack::new("minecraft:iron_spear", 1));
            guest.selected = 0;
        });
        attack(&mut game, 2, item);
        assert_eq!(disconnects(&rx), vec![], "a spear in hand");
        game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
            guest.inventory[0] = None;
        });
        game.lan.as_mut().unwrap().hub.guests.remove(0);
        for (case, (what, target)) in [
            ("an item", Some(item)),
            ("an experience orb", Some(orb)),
            ("themselves", None),
            ("an arrow", Some(arrow)),
            ("a thrown trident", Some(trident)),
        ]
        .into_iter()
        .enumerate()
        {
            // Each kick ends a connection. A later invalid target needs a new
            // guest, not another packet from the already disconnected player.
            game.server.next_entity += 1;
            let me = game.server.next_entity as i32;
            let conn = 3 + case as u32;
            game.lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .push(fresh_guest(conn, me));
            attack(&mut game, conn, target.unwrap_or(me));
            assert_eq!(disconnects(&rx), kicked, "{what}");
            game.lan.as_mut().unwrap().hub.guests.remove(0);
        }
    }
}
