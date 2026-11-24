#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Arg, Context, Game, R, Target, err};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat::PLAYER_NAME;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::misc::millis;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::misc::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    clock::SprintReport,
    text::{self, Text},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::java_number::format_fixed;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::random::RandomSource;

impl Game<'_> {
    pub(in crate::game) fn cmd_random(&mut self, id: &str, ctx: &Context) -> R<i32> {
        let seed = self.level.seed;
        if id == "random.reset" {
            let sequence = match ctx.get("sequence") {
                Some(Arg::Resource(s)) => Some(s.clone()),
                _ => None,
            };
            let int = |name: &str| match ctx.get(name) {
                Some(Arg::Int(v)) => Some(*v),
                _ => None,
            };
            let flag = |name: &str| !matches!(ctx.get(name), Some(Arg::Bool(false)));
            let name_text = |s: &str| Text::literal(s.to_owned());
            return match (sequence, int("seed")) {
                (None, None) => {
                    let n = self.server.rand_sequences.clear();
                    self.feedback(Text::translate(
                        "commands.random.reset.all.success",
                        &[Text::literal(n.to_string())],
                    ));
                    Ok(n as i32)
                }
                (None, Some(salt)) => {
                    self.server.rand_sequences.set_defaults(
                        salt,
                        flag("includeWorldSeed"),
                        flag("includeSequenceId"),
                    );
                    let n = self.server.rand_sequences.clear();
                    self.feedback(Text::translate(
                        "commands.random.reset.all.success",
                        &[Text::literal(n.to_string())],
                    ));
                    Ok(n as i32)
                }
                (Some(s), None) => {
                    self.server.rand_sequences.reset(&s, seed);
                    self.feedback(Text::translate(
                        "commands.random.reset.success",
                        &[name_text(&s)],
                    ));
                    Ok(1)
                }
                (Some(s), Some(salt)) => {
                    self.server.rand_sequences.reset_with(
                        &s,
                        seed,
                        salt,
                        flag("includeWorldSeed"),
                        flag("includeSequenceId"),
                    );
                    self.feedback(Text::translate(
                        "commands.random.reset.success",
                        &[name_text(&s)],
                    ));
                    Ok(1)
                }
            };
        }
        let Some(Arg::IntRange((lo, hi))) = ctx.get("range").cloned() else {
            return Ok(0);
        };
        let (min, max) = (lo.unwrap_or(i32::MIN), hi.unwrap_or(i32::MAX));
        let span = i64::from(max) - i64::from(min);
        if span == 0 {
            return Err(err("commands.random.error.range_too_small", &[]));
        }
        if span >= i64::from(i32::MAX) {
            return Err(err("commands.random.error.range_too_large", &[]));
        }
        let value = match ctx.get("sequence") {
            Some(Arg::Resource(s)) => {
                let s = s.clone();
                self.server
                    .rand_sequences
                    .get(&s, seed)
                    .next_int_between_inclusive(min, max)
            }
            _ => self.world.random.next_int_between_inclusive(min, max),
        }
        .unwrap_or(min);
        if id == "random.roll" {
            let line = Text::translate(
                "commands.random.roll",
                &[
                    self.source_display_name(),
                    Text::literal(value.to_string()),
                    Text::literal(min.to_string()),
                    Text::literal(max.to_string()),
                ],
            );
            // PlayerList.broadcastSystemMessage: every player hears the roll,
            // the source's own copy with the captured output it has (`/say`'s).
            let captured = self
                .server
                .chat_capture
                .as_ref()
                .and(self.server.lan_acting);
            if let Some(host) = &self.lan {
                let content = sunrise_client::lan::data::component_nbt(&line.component());
                match captured {
                    Some(conn) => host.hub.system_message_except(content, conn),
                    None => host.hub.system_message(content),
                }
            }
            // MinecraftServer.sendSystemMessage: the server's log, unless the
            // console's captured reply below is what logs it (`console_reply`).
            if self.options.dedicated.is_some()
                && (self.server.lan_acting.is_some() || self.server.chat_capture.is_none())
            {
                super::super::dedicated::log("INFO", &format!("System chat: {}", line.plain()));
            }
            self.system_message(line);
        } else {
            self.feedback(Text::translate(
                "commands.random.sample.success",
                &[Text::literal(value.to_string())],
            ));
        }
        Ok(value)
    }
    /// `/teammsg`: the executor's team hears the message.
    pub(in crate::game) fn cmd_teammsg(&mut self, ctx: &Context) -> R<i32> {
        let entity = match self.server.command_source {
            Some(s) => s.entity,
            None => Some(Target::Player),
        };
        let Some(entity) = entity else {
            return Err(err("permissions.requires.entity", &[]));
        };
        let Some(holder) = self.holder_name(entity) else {
            return Err(err("permissions.requires.entity", &[]));
        };
        let Some(team) = self.score.board.players_team(&holder).cloned() else {
            return Err(err("commands.teammsg.failed.noteam", &[]));
        };
        // Only the local player can be on the receiving end.
        let player_name = PLAYER_NAME.to_owned();
        let player_in_team = self
            .score
            .board
            .players_team(&player_name)
            .is_some_and(|t| t.name == team.name);
        if !player_in_team {
            return Ok(0);
        }
        let raw = match ctx.get("message") {
            Some(Arg::Str(m)) => m.clone(),
            _ => String::new(),
        };
        let message = self.expand_selectors(&raw);
        let hover = text::Style {
            hover: Some(std::sync::Arc::new(Text::translate(
                "chat.type.team.hover",
                &[],
            ))),
            click: Some(text::ClickEvent::SuggestCommand("/teammsg ".into())),
            ..text::Style::default()
        };
        let team_name = team.formatted_display_name().with_style(&hover);
        let sender = self.target_name(entity);
        let key = if entity == Target::Player {
            "chat.type.team.sent"
        } else {
            "chat.type.team.text"
        };
        self.system_message(Text::translate(key, &[team_name, sender, message]));
        Ok(1)
    }
}

#[cfg(test)]
#[path = "misc/random_command_tests_server.rs"]
pub(in crate::game) mod random_command_tests;
