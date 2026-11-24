//! Llamas on the game side: `AbstractChestedHorse.mobInteract` and
//! `Llama.handleEating` (wheat and hay), chests, carpets, shearing a carpet
//! off and riding.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use sunrise_client::actions::Action;
#[allow(unused_imports)]
use sunrise_client::mobs::{equine, llama};
#[allow(unused_imports)]
use sunrise_core::random::RandomSource;

impl Game<'_> {
    /// `Llama.handleEating`: wheat and hay bales heal, age up a baby, raise
    /// the temper and, from a hay bale, put a tame adult in love.
    pub(in crate::game) fn feed_llama(&mut self, index: usize, item: &str) -> bool {
        let (age_up, temper, heal) = match item {
            "minecraft:wheat" => (10, 3, 2.0),
            "minecraft:hay_block" => (90, 6, 10.0),
            _ => return false,
        };
        let m = &mut self.mobs[index];
        let mut used = false;
        let mut loved = false;
        if item == "minecraft:hay_block" && m.extra.tame && m.extra.age == 0 && m.extra.in_love <= 0
        {
            m.extra.in_love = 600;
            m.extra.love_player = true;
            used = true;
            loved = true;
        }
        if m.health < m.attrs.max_health {
            m.heal(&self.loop_context, heal);
            used = true;
        }
        let mut happy = false;
        if m.is_baby() && !m.extra.age_locked {
            m.extra.age = (m.extra.age + age_up * 20).min(0);
            used = true;
            happy = true;
        }
        if (used || !m.extra.tame) && equine::temper(m) < llama::MAX_TEMPER {
            equine::modify_temper(m, temper);
            used = true;
        }
        if happy {
            // AbstractHorse.handleEating: one at rest about the body.
            let id = m.id;
            self.particles_around_mob(
                id,
                "happy_villager",
                sunrise_client::particles::Burst::HORSE_EATING,
            );
        }
        if !used {
            return false;
        }
        // Animal.setInLove: entity event 18.
        if loved {
            let id = self.mobs[index].id;
            self.server.love_cause.insert(id, self.server.lan_acting);
            let kind = self.mobs[index].kind.id.clone();
            self.native_entity_enter_love_mode(id, &kind, self.server.lan_acting_uuid);
            self.mob_event_particles(id, 18);
        }
        self.llama_sound(index, "entity.llama.eat", 1.0);
        self.consume_held();
        true
    }
    pub(in crate::game) fn is_led_by_trader(&self, index: usize) -> bool {
        let m = &self.mobs[index];
        m.species() == sunrise_client::mobs::Species::TraderLlama
            && matches!(
                m.extra.leash,
                Some(sunrise_client::mobs::leash::Holder::Mob(id))
                    if self.mobs.iter().any(|o| o.id == id
                        && o.species() == sunrise_client::mobs::Species::WanderingTrader)
            )
    }
}
