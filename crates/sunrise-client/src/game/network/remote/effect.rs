use super::*;
use sunrise_client::effects::{EffectInstance, PlayerEffects};

/// Another living entity's effects as the server sent them (`LivingEntity.activeEffects` on a
/// client), with its darkness fade, which the local player keeps in `sculk.darkness`. One
/// instance per effect kind, so it holds at most the effect table's size.
#[derive(Default)]
pub(in crate::game) struct EntityEffects {
    pub(in crate::game) effects: PlayerEffects,
    pub(in crate::game) darkness: (f32, f32),
}

/// `LivingEntity.forceAddEffect` with the packet's `shouldBlend` (flag 8): the effect replaces
/// any of its kind, an update keeping its fade (`copyBlendState`). Returns the instance when it
/// is new, for the fades kept beside the list (darkness, nausea).
fn force_add_effect(
    list: &mut Vec<EffectInstance>,
    mut e: EffectInstance,
    blend: bool,
) -> Option<&EffectInstance> {
    match list.iter().position(|x| x.id == e.id) {
        Some(i) => {
            e.client_blend = list[i].client_blend;
            list[i] = e;
            None
        }
        None => {
            if e.id != "darkness" && e.id != "nausea" && !blend {
                let has_effect = !e.ends_within(0);
                e.client_blend = (has_effect, has_effect);
            }
            list.push(e);
            list.last()
        }
    }
}

impl Game<'_> {
    /// ClientPacketListener.handleUpdateMobEffect (`LivingEntity.forceAddEffect`): for the
    /// player itself, or for another living entity the level holds (a mob, another player or
    /// an armor stand), whose effects the view reads while it is the camera entity. Any other
    /// entity, or an effect the registry doesn't name, changes nothing.
    pub(super) fn remote_effect(&mut self, p: &play::UpdateMobEffect) {
        let Some(own) = self.remote.as_ref().map(|r| r.entity_id == p.id) else {
            return;
        };
        if !own && !self.remote_living(p.id) {
            return;
        }
        let Some(def) = self.remote_effect_def(p.effect) else {
            return;
        };
        let mut e = EffectInstance::from_def(def, p.duration, p.amplifier);
        e.ambient = p.flags & 1 != 0;
        e.visible = p.flags & 2 != 0;
        e.show_icon = p.flags & 4 != 0;
        let blend = p.flags & 8 != 0;
        if !own {
            if let Some(r) = self.remote.as_mut() {
                let fx = r.entity_effects.entry(p.id).or_default();
                if let Some(e) = force_add_effect(&mut fx.effects.list, e, blend)
                    && e.id == "darkness"
                {
                    fx.darkness = super::super::sculk::darkness_blend_added(e, blend);
                }
            }
            return;
        }
        match force_add_effect(&mut self.potions.effects.list, e, blend) {
            Some(e) if e.id == "darkness" => {
                self.sculk.darkness = super::super::sculk::darkness_blend_added(e, blend);
            }
            Some(e) if e.id == "nausea" => {
                let (o_nausea, nausea) = screen_effects::nausea_blend_added(e, blend);
                self.screen_fx.o_nausea = o_nausea;
                self.screen_fx.nausea = nausea;
            }
            _ => {}
        }
    }

    /// `ClientLevel.getEntity(id) instanceof LivingEntity` among the entities this client
    /// holds: a mob or another player, or an armor stand.
    fn remote_living(&mut self, id: i32) -> bool {
        let id = id as u32;
        self.mob_index.get(&self.mobs, id).is_some() || self.armor_stands.iter().any(|a| a.id == id)
    }

    /// `LivingEntity.tickEffects` on a client (`MobEffectInstance.tickClient`) for the other
    /// living entities' effects: they count down and blend on the ticks their entity ticks
    /// (`ClientLevel.tickEntities`: a frozen level ticks only the players, the mount this
    /// client drives ticks too, a tick a mod canceled doesn't run), and the server removes
    /// them. An entry whose entity this client no longer holds is dropped.
    pub(super) fn tick_remote_entity_effects(
        &mut self,
        running: bool,
        driven: Option<u32>,
        canceled: impl Fn(u32) -> bool,
    ) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if r.entity_effects.is_empty() {
            return;
        }
        let (index, mobs, stands) = (&mut self.mob_index, &self.mobs, &self.armor_stands);
        let players = &r.level_players;
        r.entity_effects.retain(|&id, fx| {
            let entity = id as u32;
            let ticks = if index.get(mobs, entity).is_some() {
                running || players.contains_key(&id) || driven == Some(entity)
            } else if stands.iter().any(|a| a.id == entity) {
                running
            } else {
                return false;
            };
            if ticks && !canceled(entity) {
                super::super::potions::client_tick_effects(&mut fx.effects, &mut fx.darkness);
            }
            true
        });
    }
}
