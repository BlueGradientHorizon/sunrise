//! The two names a command reads from an entity: `Entity.getDisplayName` for
//! the mobs command replies (the name as its team shows it, with the
//! `show_entity` hover and the UUID insertion) and `Nameable.getPlainTextName`
//! for the selector's `name=`.
use super::super::Game;
use super::Target;
use sunrise_client::text::Text;

impl Game<'_> {
    /// `Nameable.getPlainTextName`: `getName().getString()`, the name without
    /// the team's prefix and suffix that `getDisplayName` adds.
    pub(in crate::game) fn target_plain_name(&self, t: Target) -> String {
        match t {
            Target::Player => self.acting_player_name().to_owned(),
            Target::Guest(conn) => self
                .command_guest(conn)
                .map_or_else(String::new, |g| g.name.clone()),
            Target::Mob(id) => self
                .mobs
                .iter()
                .find(|m| m.id == id)
                .map_or_else(String::new, |m| m.command_name().plain()),
            Target::Bolt(id) => self.bolt_plain_name(id).plain(),
            // No other entity's name is team-formatted.
            other => self.target_name(other).plain(),
        }
    }
    pub(in crate::game) fn bolt_plain_name(&self, id: u32) -> Text {
        if let Some(name) = self
            .bolts
            .iter()
            .find(|b| b.id == id)
            .and_then(|b| b.custom_name.as_ref())
        {
            return sunrise_client::entity_nbt::name_text(name);
        }
        Text::translate("entity.minecraft.lightning_bolt", &[])
    }
    /// `Entity.getDisplayName` for a live lightning bolt: validated `CustomName`
    /// or type name, formatted with its team, plus `show_entity` hover and UUID insertion.
    pub(in crate::game) fn bolt_command_name(&self, id: u32) -> Text {
        let name = self.bolt_plain_name(id);
        let Some(uuid) = self.persist.uuid_of(id) else {
            return name;
        };
        let uuid = sunrise_client::save::uuid_text(uuid);
        let shown = match self.score.board.players_team(&uuid) {
            Some(team) => team.formatted_name(name.clone()),
            None => name.clone(),
        };
        shown.with_component_fields(vec![
            (
                "hover_event".into(),
                serde_json::json!({"action": "show_entity", "id": "minecraft:lightning_bolt", "uuid": uuid,
                    "name": name.component()}),
            ),
            ("insertion".into(), uuid.into()),
        ])
    }
    /// `Entity.getDisplayName` for the mob `id`: `PlayerTeam.formatNameForTeam`
    /// of `getName()`, then `withHoverEvent(createHoverEvent())` (a `ShowEntity`
    /// of the type, UUID and name) and `withInsertion(getStringUUID())`. A mob
    /// gets its UUID when it joins the level; one without keeps the bare name.
    pub(in crate::game) fn mob_command_name(&self, id: u32) -> Text {
        let Some(m) = self.mobs.iter().find(|m| m.id == id) else {
            return Text::translate("entity.", &[]);
        };
        let name = m.command_name();
        let shown = match self.mob_team_of(m) {
            Some(team) => team.formatted_name(name.clone()),
            None => name.clone(),
        };
        let Some(uuid) = self.persist.uuid_of(id) else {
            return shown;
        };
        let uuid = sunrise_client::save::uuid_text(uuid);
        let kind = sunrise_client::command::args::normalize(&m.kind.id);
        shown.with_component_fields(vec![
            (
                "hover_event".into(),
                serde_json::json!({"action": "show_entity", "id": kind, "uuid": uuid,
                    "name": name.component()}),
            ),
            ("insertion".into(), uuid.into()),
        ])
    }
}
