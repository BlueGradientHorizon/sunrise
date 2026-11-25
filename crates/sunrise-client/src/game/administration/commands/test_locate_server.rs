#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Arg, Context, Game, R, err};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::test_locate::Pos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::test_locate::horizontal_distance;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::test_locate::link;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::test_locate::locate_command;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::test_locate::north_turned;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::test_locate::path_of;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::test_locate::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::test_instances::TestDef;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    containers::BlockEntity,
    test_instance_block::{Status, TestInstanceBlock},
    text::{self, ClickEvent, Style, Text},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{self, Clientbound as C};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::worldgen::structure::Rotation;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `TestCommand.locate`.
    pub(in crate::game) fn test_locate(&mut self, ctx: &Context) -> R<i32> {
        let selected: Vec<String> = match ctx.get("tests") {
            Some(Arg::Str(pattern)) => sunrise_client::command::args::select_resources(
                self.ui.registries.ids("minecraft:test_instance"),
                pattern,
            )
            .into_iter()
            .cloned()
            .collect(),
            _ => Vec::new(),
        };
        // CommandSourceStack.sendSystemMessage: no feedback rule.
        if !self.side.command_source.is_some_and(|s| s.silent) {
            self.server_system_message(Text::translate("commands.test.locate.started", &[]));
        }
        let center = self.test_source_block();
        let source_dim = self.dim;
        // findSelectedTestPositionsAcrossDimensions: every level's test
        // blocks within 250 blocks whose test was selected.
        let mut found: Vec<(Pos, String, Rotation)> = Vec::new();
        if !selected.is_empty() {
            for dim in self.loaded_dims() {
                let back = self.enter_command_level(dim);
                for pos in self.find_test_instance_blocks(center, 250) {
                    let key = self.test_instance_at(pos).and_then(|b| b.data.test.clone());
                    let key = key.map(|k| {
                        if k.contains(':') {
                            k
                        } else {
                            format!("minecraft:{k}")
                        }
                    });
                    if key.is_some_and(|k| selected.contains(&k)) {
                        found.push((pos, dim.id().to_owned(), self.test_rotation(pos)));
                    }
                }
                if let Some(back) = back {
                    self.enter_command_level(back);
                }
            }
        }
        let mut dimensions: Vec<String> = Vec::new();
        for (pos, dimension, rotation) in &found {
            if !dimensions.contains(dimension) {
                dimensions.push(dimension.clone());
            }
            let ((dx, dz), y_rot) = north_turned(*rotation);
            let tp = (pos.0 + dx * 3, pos.1, pos.2 + dz * 3);
            let command = locate_command(dimension, tp, y_rot);
            let coordinates = link(
                Text::tr("chat.coordinates", &[&pos.0, &pos.1, &pos.2])
                    .append(format!(" ->[{}]", path_of(dimension))),
                text::GREEN,
                command,
            );
            let distance = if *dimension == source_dim.id() {
                horizontal_distance(center.0 - pos.0, center.2 - pos.2).to_string()
            } else {
                "N/A".to_owned()
            };
            self.feedback_unbroadcast(Text::translate(
                "commands.test.locate.found",
                &[coordinates, Text::literal(distance)],
            ));
        }
        let count = found.len() as i32;
        if count == 0 {
            return Err(err("commands.test.error.no_test_instances", &[]));
        }
        self.output_player_coordinates(&dimensions);
        self.feedback(Text::translate(
            "commands.test.locate.done",
            &[Text::literal(count.to_string())],
        ));
        Ok(count)
    }
    /// `TestCommand.showPos`, but for the highlight the client draws
    /// (`ClientboundGameTestHighlightPosPacket`).
    pub(in crate::game) fn test_show_pos(&mut self, ctx: &Context) -> R<i32> {
        // `source.getPlayerOrException().pick(10, 1, false)`: the source
        // player's own eyes and view. A connected player's are in place only
        // for the pick; the test is looked up, and the issuer told, here.
        let target = match self.side.command_source.map(|s| s.entity) {
            Some(Some(super::Target::Guest(conn))) => self
                .for_player(super::Target::Guest(conn), |g| g.test_pick_block())
                .ok_or_else(|| err("permissions.requires.player", &[]))?,
            _ => {
                self.source_player()?;
                self.test_pick_block()
            }
        };
        let var = match ctx.get("var") {
            Some(Arg::Str(v)) => v.clone(),
            _ => "pos".to_owned(),
        };
        let target = (target[0], target[1], target[2]);
        let containing = |game: &Self, radius: i32| {
            game.find_test_instance_blocks(target, radius)
                .into_iter()
                .find(|&p| {
                    game.test_structure_box(p).is_some_and(|b| {
                        (0..3).all(|i| {
                            let t = [target.0, target.1, target.2][i];
                            t >= b.min[i] && t <= b.max[i]
                        })
                    })
                })
        };
        let found = containing(self, 15).or_else(|| containing(self, 250));
        let Some(block) = found else {
            return Err(err(
                "commands.test.error.no_test_containing_pos",
                &[&target.0, &target.1, &target.2],
            ));
        };
        let Some(origin) = self.test_structure_box(block).map(|b| b.min) else {
            return Err(err("commands.test.error.test_instance_not_found", &[]));
        };
        let relative = [
            target.0 - origin[0],
            target.1 - origin[1],
            target.2 - origin[2],
        ];
        let description = format!("{}, {}, {}", relative[0], relative[1], relative[2]);
        let coords = Text::translate(
            "commands.test.coordinates",
            &relative.map(|v| Text::literal(v.to_string())),
        )
        .with_style(&Style {
            bold: Some(true),
            click: Some(ClickEvent::CopyToClipboard(format!(
                "final BlockPos {var} = new BlockPos({description});"
            ))),
            hover: Some(std::sync::Arc::new(Text::translate(
                "commands.test.coordinates.copy",
                &[],
            ))),
            ..Style::color(text::GREEN)
        });
        let name = self.test_name(block).plain();
        self.feedback_unbroadcast(Text::translate(
            "commands.test.relative_position",
            &[Text::literal(name), coords],
        ));
        // `player.connection.send(new ClientboundGameTestHighlightPosPacket(..))`.
        if self.side.lan_acting.is_some() {
            self.side
                .guest_packets
                .push(C::GameTestHighlightPos(play::GameTestHighlightPos {
                    absolute: [target.0, target.1, target.2],
                    relative,
                }));
        }
        Ok(1)
    }
}
