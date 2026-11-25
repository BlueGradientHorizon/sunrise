use super::{Game, view_vector};
use crate::game::creatures::world_view::MobWorldSideParams;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use sunrise_client::actions::Action;
use sunrise_client::mobs::{MobWorld, PlayerView, Seen, Species, Vehicle};
use sunrise_core::geometry::{Aabb, Vec3};

static EMPTY_LID_OPENERS: OnceLock<HashMap<(i32, i32, i32), i32>> = OnceLock::new();

impl Game<'_> {
    pub(in crate::game) fn client_player_view(&self) -> Option<PlayerView> {
        let p = self.player.as_ref()?;
        let w = f64::from(p.dimensions.width) / 2.0;
        let bounds = Aabb::new(
            Vec3::new(p.position.x - w, p.position.y, p.position.z - w),
            Vec3::new(
                p.position.x + w,
                p.position.y + f64::from(p.dimensions.height),
                p.position.z + w,
            ),
        );
        let gold_tag = self
            .res
            .items()
            .item_tags
            .get("minecraft:piglin_safe_armor");
        let gold_armor = self
            .armor
            .iter()
            .rev()
            .flatten()
            .any(|s| gold_tag.is_some_and(|t| t.contains(&*s.item)));
        let vehicle = self.riding.and_then(|id| {
            if let Some(b) = self.boats.iter().find(|b| b.id == id) {
                let w = 0.6875;
                return Some(Vehicle {
                    nautilus: false,
                    bounds: Aabb::new(
                        Vec3::new(b.position.x - w, b.position.y, b.position.z - w),
                        Vec3::new(b.position.x + w, b.position.y + 0.5625, b.position.z + w),
                    ),
                    velocity: b.velocity,
                });
            }
            let m = self.mobs.iter().find(|m| m.id == id)?;
            matches!(m.species(), Species::Nautilus | Species::ZombieNautilus).then(|| Vehicle {
                nautilus: true,
                bounds: m.bounds(),
                velocity: m.velocity,
            })
        });
        Some(PlayerView {
            position: p.position,
            bounds,
            eye: [
                p.position.x,
                p.position.y + f64::from(p.dimensions.eye_height),
                p.position.z,
            ],
            alive: !p.health.dead_or_dying(),
            invulnerable: p.invulnerable
                || p.health.permanently_invulnerable
                || p.health.invulnerable_time > 0,
            creative: self.creative || self.spectator(),
            sneaking: self.held_mapping(Action::Sneak),
            swimming: p.swimming,
            look: view_vector(self.yaw, self.pitch),
            main_hand: self.held().map(|s| s.item.clone()),
            off_hand: self.offhand.as_ref().map(|s| s.item.clone()),
            head: self.armor[0].as_ref().map(|s| s.item.clone()),
            chest: self.armor[1].as_ref().map(|s| s.item.clone()),
            legs: self.armor[2].as_ref().map(|s| s.item.clone()),
            feet: self.armor[3].as_ref().map(|s| s.item.clone()),
            armor_cover: self.armor.iter().filter(|a| a.is_some()).count() as f32 / 4.0,
            invisible: self.potions.effects.get("invisibility").is_some(),
            spectator: self.spectator(),
            gold_armor,
            hurt_by: None,
            last_hurt: p.health.last_hurt_mob,
            velocity: p.velocity,
            yaw: self.yaw,
            sprinting: p.sprinting,
            riding: self.riding.is_some(),
            hero: self.potions.effects.get("hero_of_the_village").is_some(),
            effects: self.potions.effects.list.iter().map(|e| e.id).collect(),
            uuid: self.level.player_uuid.unwrap_or([0; 4]),
            allies: Arc::new(HashSet::new()),
            remote: true,
            sleeping: self.sleeping.is_some(),
            sleep_timer: self.sleep_timer,
            passenger: self.riding.is_some(),
            riding_vehicle: self.riding,
            vehicle,
        })
    }

    pub(in crate::game) fn client_mob_side(&self) -> crate::game::native_surface::GameSide<'_> {
        use sunrise_items::equipment::{BODY, SADDLE};
        let level = self.native_lifecycle.epoch().and_then(|epoch| {
            let key = self.dim.key();
            mod_api::__private::dimension_id(key)?;
            Some(mod_api::LevelRef::with_generation(epoch, key, 0))
        });
        crate::game::native_surface::GameSide {
            dim: self.dim,
            level,
            facts: self.res.facts(),
            biomes: match &self.world_biomes {
                Some(b) => b,
                None => self.res.biomes(),
            },
            behaviours: self.res.behaviours(),
            features: match &self.remote {
                Some(remote) => &remote.features,
                None => crate::game::tables::default_client_features(),
            },
            items: self.res.items(),
            damage_types: &self.health_resources.damage,
            player_equipment: [
                self.inventory[self.selected].as_ref(),
                self.offhand.as_ref(),
                self.armor[3].as_ref(),
                self.armor[2].as_ref(),
                self.armor[1].as_ref(),
                self.armor[0].as_ref(),
                self.equipment.get(BODY),
                self.equipment.get(SADDLE),
            ],
            hit_borrows: crate::game::native_surface::HitBorrows {
                living_types: self.res.living_type_keys(),
                attributes: &self.motion.attributes,
                player_effects: &self.potions.effects,
                player_uuid: None,
                player_attachments: &self.player_state.attachments,
                entity_attachments: &self.entity_attachments,
            },
            tag_items: self.remote.as_ref().and_then(|remote| remote.tags.as_ref()),
        }
    }

    pub(in crate::game) fn client_mob_world_base<'s>(
        &'s self,
        others: &'s [Seen],
        side: &'s dyn sunrise_client::motion::SideLevel,
    ) -> MobWorld<'s> {
        let at = self
            .player
            .as_ref()
            .map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]);
        self.mob_world_base_with(
            others,
            MobWorldSideParams {
                side,
                trading_players: None,
                player: self.client_player_view(),
                player_identity: None,
                leash_boats: &[],
                teams: Arc::new(self.client_mob_teams()),
                lid_openers: EMPTY_LID_OPENERS.get_or_init(HashMap::new),
                mod_effects: None,
                mob_hooks: None,
                goal_host: None,
                asks_animal_tame: false,
                asks_living_destroy_block: false,
                asks_baby_spawn: false,
                who_lookup: None,
                difficulty: self.local_difficulty(at),
            },
        )
    }
}
