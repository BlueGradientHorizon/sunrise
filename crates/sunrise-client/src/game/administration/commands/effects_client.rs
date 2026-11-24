#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Arg, Context, Game, R, Target, err, texts};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::effects::Tracker;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::effects::effect_name;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::effects::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::EquipmentSlot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    brewing::{self, namespaced},
    effects::EffectInstance,
    text::{self, Style, Text},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_items::equipment::MAINHAND;

impl Game<'_> {
    /// `EffectCommands.giveEffect`.
    pub(in crate::game) fn cmd_effect_give(&mut self, ctx: &Context) -> R<i32> {
        let effect = Self::effect_arg(ctx)?;
        let seconds = if ctx.has("infinite") {
            Some(-1)
        } else {
            Self::int(ctx, "seconds")
        };
        let amplifier = Self::int(ctx, "amplifier").unwrap_or(0);
        let particles = !matches!(ctx.get("hideParticles"), Some(Arg::Bool(true)));
        // computeDurationInTicks
        let duration = match seconds {
            Some(s) if effect.instant => s,
            Some(-1) => -1,
            Some(s) => s * 20,
            None if effect.instant => 1,
            None => 600,
        };
        let mut tracker = Tracker::default();
        for t in self.living_targets(ctx)? {
            let changed = match t {
                Target::Player | Target::Guest(_) => {
                    // `new MobEffectInstance(effect, duration, amplifier, false, particles)`.
                    let instance = EffectInstance::new(effect.id, duration, amplifier)
                        .with_flags(false, particles);
                    self.for_player(t, |g| g.add_player_effect(instance))
                        .unwrap_or(false)
                }
                Target::Mob(id) => {
                    let Some(kind) = self
                        .mobs
                        .iter()
                        .find(|m| m.id == id && !m.removed)
                        .map(|m| m.kind.id.clone())
                    else {
                        tracker.track(t, false);
                        continue;
                    };
                    let name = namespaced(effect.id);
                    // `LivingEntity.addEffect` refuses what the mob cannot
                    // be affected by.
                    if !self.can_be_affected(Some(id as i32), &kind, name) {
                        false
                    } else {
                        match self.mobs.iter_mut().find(|m| m.id == id) {
                            Some(m) => {
                                let new = !m.has_effect(name);
                                // `new MobEffectInstance(effect, duration, amplifier, false, particles)`.
                                let changed =
                                    m.add_effect_with(name, duration, amplifier, false, particles);
                                if new
                                    && changed
                                    && let Some(sound) = super::super::potions::sound_on_added(name)
                                {
                                    let at = [m.position.x, m.position.y, m.position.z];
                                    self.audio.0.play(sound, Some(at), 1.0, 1.0);
                                }
                                self.mod_effect_added(name, Some(id as i32), amplifier, new);
                                changed
                            }
                            None => false,
                        }
                    }
                }
                _ => continue,
            };
            tracker.track(t, changed);
        }
        let message = match tracker.changed.as_slice() {
            [] => return Err(err("commands.effect.give.failed", &[])),
            [one] => Text::translate(
                "commands.effect.give.success.single",
                &[
                    effect_name(effect.id),
                    self.target_name(*one),
                    Text::literal((duration / 20).to_string()),
                ],
            ),
            many => Text::translate(
                "commands.effect.give.success.multiple",
                &texts(&[
                    effect_name(effect.id),
                    Text::literal(many.len().to_string()),
                    Text::literal((duration / 20).to_string()),
                ]),
            ),
        };
        self.feedback(message);
        Ok(tracker.total)
    }
}
