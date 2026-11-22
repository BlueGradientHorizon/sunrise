use super::*;

impl Game<'_> {
    /// `ClientPacketListener.handleUpdateMobEffect` (`LivingEntity.forceAddEffect`): applies to
    /// the addressed living entity (local player or remote mob/player).
    pub(super) fn remote_effect(&mut self, p: &play::UpdateMobEffect) {
        let Some(def) = self.remote_effect_def(p.effect) else {
            return;
        };
        let id = def.id;
        let mut e = sunrise_client::effects::EffectInstance::from_def(def, p.duration, p.amplifier);
        e.ambient = p.flags & 1 != 0;
        e.visible = p.flags & 2 != 0;
        e.show_icon = p.flags & 4 != 0;
        let blend = p.flags & 8 != 0;
        if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
            let list = &mut self.potions.effects.list;
            match list.iter_mut().find(|x| x.id == id) {
                // forceAddEffect's copyBlendState: an updated effect keeps its fade.
                Some(existing) => {
                    e.client_blend = existing.client_blend;
                    *existing = e;
                }
                None => {
                    if id == "darkness" {
                        self.sculk.darkness = super::super::sculk::darkness_blend_added(&e, blend);
                    } else if id == "nausea" {
                        let (o_nausea, nausea) = screen_effects::nausea_blend_added(&e, blend);
                        self.screen_fx.o_nausea = o_nausea;
                        self.screen_fx.nausea = nausea;
                    } else if !blend {
                        let has_effect = !e.ends_within(0);
                        e.client_blend = (has_effect, has_effect);
                    }
                    list.push(e);
                }
            }
            return;
        }
        let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) else {
            return;
        };
        let list = &mut m.extra.client_effects.list;
        match list.iter_mut().find(|x| x.id == id) {
            Some(existing) => {
                e.client_blend = existing.client_blend;
                *existing = e;
            }
            None => {
                if id == "darkness" {
                    m.extra.darkness_blend = super::super::sculk::darkness_blend_added(&e, blend);
                } else if !blend {
                    let has_effect = !e.ends_within(0);
                    e.client_blend = (has_effect, has_effect);
                }
                list.push(e);
            }
        }
    }
}
