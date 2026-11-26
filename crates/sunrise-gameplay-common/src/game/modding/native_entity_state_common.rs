use crate::game::commands_common::Target;
use mod_api::EntityRef;
use sunrise_sim::mobs::{Mob, Species};
use sunrise_sim::motion::PlayerMotion;

/// `Entity.getMaxAirSupply`: a dolphin's 4800, an axolotl's 6000, else 300.
pub fn max_air(species: Option<Species>) -> i32 {
    match species {
        Some(Species::Dolphin) => 4800,
        Some(Species::Axolotl) => sunrise_sim::mobs::axolotl::MAX_AIR,
        _ => 300,
    }
}

/// What a handle names, as the flags read it.
pub enum Body<'a> {
    Player(&'a PlayerMotion),
    Mob(&'a Mob),
    Other(Target),
}

pub trait NativeEntityStateCommonGameExt1<'__sg>: crate::game::SideGame<'__sg> {
     fn native_target_body<'__loan>(&'__loan self, target: Target) -> Option<Body<'_>> where '__sg: '__loan {
        Some(match target {
            Target::Player => Body::Player(self.player.as_ref()?),
            Target::Guest(conn) => Body::Player(
                crate::game::side_ops::player::SideOps::side_command_guest_motion(
                    self.side_state(), conn,
                )?,
            ),
            Target::Mob(id) => Body::Mob(self.mobs.iter().find(|m| m.id == id)?),
            t => Body::Other(t),
        })
    }

    /// Whether the entity touched water at its last fluid update
    /// (`Entity.isInWater`): players and mobs.
     fn entity_is_in_water(&self, entity: EntityRef) -> Option<bool> {
        match self.native_target_body(self.native_entity_read_target(entity)?)? {
            Body::Player(p) => Some(p.fluids.was_touching_water),
            Body::Mob(m) => Some(m.fluids.in_water),
            Body::Other(_) => None,
        }
    }

    /// The player's swimming flag, distinct from its pose (which also covers crawling).
     fn entity_is_swimming(&self, entity: EntityRef) -> Option<bool> {
        match self.native_target_body(self.native_entity_read_target(entity)?)? {
            Body::Player(p) => Some(p.swimming),
            _ => None,
        }
    }

    /// The fluids the entity touched at its last fluid update, each with how deep its box is in
    /// it (`IEntityExtension.getFluidTypeHeight`), highest first; empty in no fluid. Players and
    /// mobs: water and lava, the fluids Sunrise's entity update tracks. On a client these
    /// are the predicted local player's state or the displayed entity's last fluid update.
     fn entity_fluid_heights(
        &self,
        entity: EntityRef,
    ) -> Option<Vec<(String, f64)>> {
        use sunrise_sim::motion::{LAVA, WATER};
        Some(
            match self.native_target_body(self.native_entity_read_target(entity)?)? {
                Body::Player(p) => touched_fluids(
                    (p.fluid_height(WATER), p.fluids.was_touching_water),
                    (p.fluid_height(LAVA), p.is_in_lava()),
                ),
                Body::Mob(m) => touched_fluids(
                    (m.fluids.water, m.fluids.in_water),
                    (m.fluids.lava, m.fluids.in_lava),
                ),
                Body::Other(_) => return None,
            },
        )
    }

    /// The fluid at the entity's eyes from its last fluid update
    /// (`IEntityExtension.getFirstEyeInFluidType`): water or lava, none in air. Players and mobs.
    /// On a client this reads its own cached fluid update, without sampling the world again.
     fn entity_eye_fluid(&self, entity: EntityRef) -> Option<Option<String>> {
        use sunrise_sim::motion::{LAVA, WATER};
        Some(
            match self.native_target_body(self.native_entity_read_target(entity)?)? {
                Body::Player(p) => eye_fluid(p.is_eye_in_fluid(WATER), p.is_eye_in_fluid(LAVA)),
                Body::Mob(m) => eye_fluid(m.fluids.eye_in_water, m.fluids.eye_in_lava),
                Body::Other(_) => return None,
            },
        )
    }

    /// Entity.isUnderWater uses eye sample AND contact; LocalPlayer's override uses the sample alone.
     fn entity_is_under_water(&self, entity: EntityRef) -> Option<bool> {
        match self.native_target_body(self.native_entity_read_target(entity)?)? {
            Body::Player(p) if !self.is_logical_server() => Some(p.is_under_water()),
            Body::Player(p) => Some(p.fluids.was_eye_in_water && p.fluids.was_touching_water),
            Body::Mob(m) => Some(m.was_eye_in_water && m.fluids.in_water),
            Body::Other(_) => None,
        }
    }

    /// Entity.isOnFire: remaining ticks or the client's synced flag, unless fire immune.
     fn entity_is_on_fire(&self, entity: EntityRef) -> Option<bool> {
        let (ticks, immune) = match self
            .native_entity_read_target(entity)
            .and_then(|target| self.native_target_body(target))
        {
            Some(Body::Player(p)) => (p.fire_ticks, p.mod_fire_immune),
            Some(Body::Mob(m)) => (m.fire_ticks, m.fire_immune()),
            Some(Body::Other(Target::Stand(stand))) => {
                let stand = self.armor_stands.iter().find(|s| s.id == stand)?;
                (stand.fire_ticks, stand.mod_fire_immune)
            }
            Some(Body::Other(Target::Item(item))) => {
                let item = self.dropped.iter().find(|i| i.id == item)?;
                (
                    item.fire_ticks,
                    self.stack_can_be_hurt_by(&item.stack, "minecraft:in_fire") == Some(false),
                )
            }
            _ => (0, self.side_client_tracked_player_fire_immunity(entity)?),
        };
        let synced = !self.is_logical_server() && self.side_client_entity_on_fire(entity)?;
        Some(!immune && (ticks > 0 || synced))
    }

    /// `Entity.fireImmune`: effective immunity on the side of the host.
     fn entity_fire_immune(&self, entity: EntityRef) -> Option<bool> {
        match self
            .native_entity_read_target(entity)
            .and_then(|target| self.native_target_body(target))
        {
            Some(Body::Player(p)) => Some(p.mod_fire_immune),
            Some(Body::Mob(m)) => Some(m.fire_immune()),
            Some(Body::Other(Target::Stand(id))) => self
                .armor_stands
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.mod_fire_immune),
            Some(Body::Other(Target::Dragon(_))) => Some(true),
            _ => self.side_client_tracked_player_fire_immunity(entity),
        }
    }

    /// The entity's air (`Entity.getAirSupply`): players and mobs.
     fn entity_air_supply(&self, entity: EntityRef) -> Option<i32> {
        match self.native_target_body(self.native_entity_read_target(entity)?)? {
            Body::Player(p) => Some(p.air),
            Body::Mob(m) => Some(m.air),
            Body::Other(_) => None,
        }
    }

    /// The most air the entity holds (`Entity.getMaxAirSupply`): 300, a
    /// dolphin's 4800, an axolotl's 6000.
     fn entity_max_air_supply(&self, entity: EntityRef) -> Option<i32> {
        match self.native_target_body(self.native_entity_read_target(entity)?)? {
            Body::Mob(m) => Some(max_air(Some(m.species()))),
            _ => Some(max_air(None)),
        }
    }

    /// Ticks the entity has lived (`Entity.tickCount`): on a server, living entities, item
    /// entities, experience orbs and minecarts; on a client, every entity of the level it
    /// shows (`side_client_entity_tick_count`). Wraps as vanilla's `int` does.
     fn entity_tick_count(&self, entity: EntityRef) -> Option<i32> {
        if !self.is_logical_server() {
            return self.side_client_entity_tick_count(entity);
        }
        Some(match self.side_native_entity_read_target(entity)? {
            Target::Player => self.player.as_ref()?.health.tick_count,
            Target::Guest(conn) => {
                crate::game::side_ops::player::SideOps::side_command_guest_tick_count(
                    self.side_state(), conn,
                )?
            }
            Target::Mob(id) => self.mobs.iter().find(|m| m.id == id)?.age as i32,
            Target::Stand(id) => self.armor_stands.iter().find(|s| s.id == id)?.tick_count,
            Target::Dragon(id) => self.dragon.as_ref().filter(|d| d.id == id)?.ticks,
            Target::Item(id) => self.dropped.iter().find(|i| i.id == id)?.ticks as i32,
            Target::Orb(id) => self.orbs.iter().find(|o| o.id == id)?.ticks as i32,
            Target::Cart(id) => self.carts.iter().find(|c| c.id == id)?.tick_count,
            _ => return None,
        })
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeEntityStateCommonGameExt1<'__sg> for G {}


/// `(fluid, height)` for each fluid `(height, touched)` the entity touched, highest first;
/// water before lava at the same height (`getMaxHeightFluidType`'s order).
pub fn touched_fluids(water: (f64, bool), lava: (f64, bool)) -> Vec<(String, f64)> {
    let mut heights: Vec<(String, f64)> = [("minecraft:water", water), ("minecraft:lava", lava)]
        .into_iter()
        .filter(|(_, (_, touched))| *touched)
        .map(|(fluid, (height, _))| (fluid.to_owned(), height))
        .collect();
    // A stable sort keeps water first in a tie.
    heights.sort_by(|a, b| b.1.total_cmp(&a.1));
    heights
}

/// The first fluid at the eyes: water, then lava, as the trackers are kept.
pub fn eye_fluid(water: bool, lava: bool) -> Option<String> {
    let fluid = if water {
        Some("minecraft:water")
    } else {
        lava.then_some("minecraft:lava")
    };
    fluid.map(str::to_owned)
}

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;

use crate::game::side_ops::player::PlayerSideGameExt as _;

use crate::game::native_components::NativeComponentsGameExt1 as _;

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
