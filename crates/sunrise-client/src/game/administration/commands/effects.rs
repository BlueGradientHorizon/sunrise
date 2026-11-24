//! `/effect` (EffectCommands) and `/enchant` (EnchantCommand).
#[allow(unused_imports)]
use super::{Arg, Context, Game, R, Target, err, texts};
#[allow(unused_imports)]
use mod_api::EquipmentSlot;
#[allow(unused_imports)]
use sunrise_client::{
    brewing::{self, namespaced},
    effects::EffectInstance,
    text::{self, Style, Text},
};
#[allow(unused_imports)]
use sunrise_items::equipment::MAINHAND;

/// `CommandResponseTracker` over targets: elements with a non-zero result.
#[derive(Default)]
pub(in crate::game) struct Tracker {
    pub(in crate::game) total: i32,
    pub(in crate::game) changed: Vec<Target>,
}
impl Tracker {
    pub(in crate::game) fn track(&mut self, t: Target, changed: bool) {
        if changed {
            self.total += 1;
            self.changed.push(t);
        }
    }
}
pub(in crate::game) fn effect_name(id: &str) -> Text {
    Text::translate(&brewing::description_id(id), &[])
}
impl Game<'_> {
    pub(in crate::game) fn effect_arg(ctx: &Context) -> R<&'static brewing::EffectDef> {
        let Some(Arg::Resource(id)) = ctx.get("effect") else {
            return Err(err("argument.resource.invalid_type", &[]));
        };
        brewing::effect(id).ok_or_else(|| {
            err(
                "argument.resource.not_found",
                &[id, &"minecraft:mob_effect"],
            )
        })
    }
    /// The executing entity for argument-less forms (the player).
    pub(in crate::game) fn living_targets(&mut self, ctx: &Context) -> R<Vec<Target>> {
        if ctx.get("targets").is_some() {
            self.entities(ctx, "targets", false)
        } else {
            Ok(vec![self.implied_player()])
        }
    }
    /// `EffectCommands.clearEffects` / `clearEffect`.
    pub(in crate::game) fn cmd_effect_clear(&mut self, ctx: &Context) -> R<i32> {
        let effect = if ctx.get("effect").is_some() {
            Some(Self::effect_arg(ctx)?)
        } else {
            None
        };
        let mut tracker = Tracker::default();
        for t in self.living_targets(ctx)? {
            let changed = match (t, effect) {
                (Target::Player | Target::Guest(_), None) => self
                    .for_player(t, |g| g.remove_player_effects(None))
                    .unwrap_or(false),
                (Target::Player | Target::Guest(_), Some(e)) => self
                    .for_player(t, |g| g.remove_player_effects(Some(e.index)))
                    .unwrap_or(false),
                (Target::Mob(id), e) => match self.mobs.iter_mut().find(|m| m.id == id) {
                    Some(m) => m.retain_effects(|x| e.is_some_and(|e| x.id != namespaced(e.id))),
                    None => false,
                },
                _ => continue,
            };
            tracker.track(t, changed);
        }
        let message = match (tracker.changed.as_slice(), effect) {
            ([], None) => return Err(err("commands.effect.clear.everything.failed", &[])),
            ([], Some(_)) => return Err(err("commands.effect.clear.specific.failed", &[])),
            ([one], None) => Text::translate(
                "commands.effect.clear.everything.success.single",
                &[self.target_name(*one)],
            ),
            (many, None) => Text::translate(
                "commands.effect.clear.everything.success.multiple",
                &[Text::literal(many.len().to_string())],
            ),
            ([one], Some(e)) => Text::translate(
                "commands.effect.clear.specific.success.single",
                &[effect_name(e.id), self.target_name(*one)],
            ),
            (many, Some(e)) => Text::translate(
                "commands.effect.clear.specific.success.multiple",
                &texts(&[effect_name(e.id), Text::literal(many.len().to_string())]),
            ),
        };
        self.feedback(message);
        Ok(tracker.total)
    }
    /// `EnchantCommand.enchant`: enchant each living target's main-hand stack.
    pub(in crate::game) fn cmd_enchant(&mut self, ctx: &Context) -> R<i32> {
        let Some(Arg::Resource(id)) = ctx.get("enchantment") else {
            return Ok(0);
        };
        let level = Self::int(ctx, "level").unwrap_or(1);
        let Some(def) = self.stations().enchantments.get(id) else {
            return Err(err(
                "argument.resource.not_found",
                &[id, &"minecraft:enchantment"],
            ));
        };
        let max = def.max_level as i32;
        let targets = self.entities(ctx, "targets", false)?;
        if level > max {
            return Err(err("commands.enchant.failed.level", &[&level, &max]));
        }
        let level = level as u32; // The command's integer argument starts at zero.
        let single = targets.len() == 1;
        let mut tracker = Tracker::default();
        for t in targets {
            let changed = match t {
                Target::Player | Target::Guest(_) => self.for_player(t, |g| {
                    g.enchant_main_hand(Target::Player, id, level, single)
                }),
                Target::Mob(_) | Target::Stand(_) | Target::Dragon(_) => {
                    self.in_target_level(t, |g| g.enchant_main_hand(t, id, level, single))
                }
                _ if single => {
                    let name = self
                        .in_target_level(t, |g| g.target_name(t))
                        .unwrap_or_else(|| self.target_name(t));
                    return Err(err("commands.enchant.failed.entity", &[&name.plain()]));
                }
                _ => None,
            };
            tracker.track(t, changed.transpose()?.unwrap_or(false));
        }
        let (name, curse) = self.stations().enchantments.full_name(id, level);
        let style = Style {
            color: Some(if curse { text::RED } else { text::GRAY }),
            ..Style::default()
        };
        let full = Text::styled(name, style);
        let message = match tracker.changed.as_slice() {
            [] => return Err(err("commands.enchant.failed", &[])),
            [one] => {
                let target = if one.is_player() {
                    self.target_name(*one)
                } else {
                    self.in_target_level(*one, |g| g.target_name(*one))
                        .unwrap_or_else(|| self.target_name(*one))
                };
                Text::translate("commands.enchant.success.single", &[full, target])
            }
            many => Text::translate(
                "commands.enchant.success.multiple",
                &texts(&[full, Text::literal(many.len().to_string())]),
            ),
        };
        self.feedback(message);
        Ok(tracker.total)
    }

    /// Change components in the hand itself, without an equip event. The next
    /// inventory and equipment syncs publish the updated stack as usual.
    pub(in crate::game) fn enchant_main_hand(
        &mut self,
        t: Target,
        id: &str,
        level: u32,
        single: bool,
    ) -> R<bool> {
        let stack = match t {
            // Include enchantments kept in the mob's gear as well as in the stack.
            Target::Mob(id) => self
                .mobs
                .iter()
                .find(|m| m.id == id)
                .and_then(|m| sunrise_client::mobs::finalize::item_stack(m, MAINHAND)),
            _ => self
                .equipment_stack_for(t, EquipmentSlot::MainHand)
                .map(|stack| stack.project().into_owned()),
        };
        let Some(mut stack) = stack.filter(|s| s.count != 0 && &*s.item != "minecraft:air") else {
            return if single {
                Err(err(
                    "commands.enchant.failed.itemless",
                    &[&self.target_name(t).plain()],
                ))
            } else {
                Ok(false)
            };
        };
        let enchantments = &self.stations().enchantments;
        if !enchantments.can_enchant(self.items(), id, &stack.item)
            || !stack
                .crafting_enchantments()
                .iter()
                .all(|(other, _)| enchantments.compatible(other, id))
        {
            return if single {
                Err(err(
                    "commands.enchant.failed.incompatible",
                    &[&self.stack_hover_name(&stack)],
                ))
            } else {
                Ok(false)
            };
        }
        stack.enchant(id, level);
        match t {
            Target::Player => self.inventory[self.selected] = Some(stack),
            Target::Mob(id) => {
                let Some(mob) = self.mobs.iter_mut().find(|m| m.id == id) else {
                    return Ok(false);
                };
                mob.set_equipment_stack(MAINHAND, Some(stack));
            }
            Target::Stand(id) => {
                let Some(stand) = self.armor_stands.iter_mut().find(|a| a.id == id) else {
                    return Ok(false);
                };
                stand.equipment[MAINHAND] = Some(stack);
            }
            Target::Dragon(id) => {
                let Some(dragon) = self.dragon.as_mut().filter(|d| d.id == id) else {
                    return Ok(false);
                };
                dragon.equipment[MAINHAND] = Some(stack);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
