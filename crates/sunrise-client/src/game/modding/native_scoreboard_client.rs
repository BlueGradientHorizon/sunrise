//! Client-side scoreboard snapshot reads (`ClientHost::scoreboard`).
use super::Game;
use super::api_values::TryIntoApi;
use mod_api::{
    ClientObjective, ClientScore, ClientScoreboard, ClientTeam, CollisionRule, DisplaySlot,
    NumberFormat, ObjectiveRenderType, TeamColor, TeamVisibility, TextFormError,
};
use sunrise_client::{
    scoreboard::{self as sb, Comp, comp_text},
    text::Text as EngineText,
};

fn api_text(text: &EngineText) -> Result<mod_api::Text, TextFormError> {
    text.try_into_api()
}

fn api_comp(comp: &Comp) -> Result<mod_api::Text, TextFormError> {
    api_text(&comp_text(comp))
}

fn api_render_type(render: sb::RenderType) -> ObjectiveRenderType {
    match render {
        sb::RenderType::Integer => ObjectiveRenderType::Integer,
        sb::RenderType::Hearts => ObjectiveRenderType::Hearts,
    }
}

fn api_number_format(format: &sb::NumberFormat) -> Result<NumberFormat, TextFormError> {
    Ok(match format {
        sb::NumberFormat::Blank => NumberFormat::Blank,
        sb::NumberFormat::Fixed(comp) => NumberFormat::Fixed(api_text(&comp_text(comp))?),
        sb::NumberFormat::Styled(style) => {
            let mut json = style.clone();
            if let Comp::Object(fields) = &mut json {
                fields.insert("text".into(), Comp::String(String::new()));
            }
            NumberFormat::Styled(std::mem::take(&mut api_text(&comp_text(&json))?.style))
        }
    })
}

fn api_visibility(visibility: sb::Visibility) -> TeamVisibility {
    match visibility {
        sb::Visibility::Always => TeamVisibility::Always,
        sb::Visibility::Never => TeamVisibility::Never,
        sb::Visibility::HideForOtherTeams => TeamVisibility::HideForOtherTeams,
        sb::Visibility::HideForOwnTeam => TeamVisibility::HideForOwnTeam,
    }
}

fn api_collision_rule(rule: sb::CollisionRule) -> CollisionRule {
    match rule {
        sb::CollisionRule::Always => CollisionRule::Always,
        sb::CollisionRule::Never => CollisionRule::Never,
        sb::CollisionRule::PushOtherTeams => CollisionRule::PushOtherTeams,
        sb::CollisionRule::PushOwnTeam => CollisionRule::PushOwnTeam,
    }
}

fn api_color(team: &sb::Team) -> Option<TeamColor> {
    team.color.as_deref().and_then(TeamColor::from_name)
}

impl Game<'_> {
    /// What this client has been sent of the scoreboard; None outside a logged-in client world.
    pub(in crate::game) fn client_scoreboard(
        &self,
    ) -> Result<Option<ClientScoreboard>, TextFormError> {
        if self.native_session().is_none() {
            return Ok(None);
        }
        let board = &self.score.board;
        let mut objectives = Vec::new();
        for o in &board.objectives {
            let slots = DisplaySlot::all()
                .filter(|slot| board.display.get(&slot.name()) == Some(&o.name))
                .collect();
            let mut scores = Vec::new();
            for entry in board.list_scores(&o.name) {
                scores.push(ClientScore::new(
                    entry.owner,
                    entry.value,
                    entry
                        .display
                        .as_ref()
                        .map(|d| api_text(&comp_text(d)))
                        .transpose()?,
                    entry
                        .number_format
                        .as_ref()
                        .map(api_number_format)
                        .transpose()?,
                ));
            }
            objectives.push(ClientObjective::new(
                o.name.clone(),
                api_text(&o.display_text())?,
                api_render_type(o.render_type),
                o.number_format
                    .as_ref()
                    .map(api_number_format)
                    .transpose()?,
                slots,
                scores,
            ));
        }
        Ok(Some(ClientScoreboard::new(
            objectives,
            self.client_teams()?,
        )))
    }

    /// Every team this client was sent.
    pub(in crate::game) fn client_teams(&self) -> Result<Vec<ClientTeam>, TextFormError> {
        self.score
            .board
            .teams
            .iter()
            .map(|t| {
                let mut team = ClientTeam::new(t.name.clone());
                team.display_name = api_comp(&t.display_name)?;
                team.color = api_color(t);
                team.prefix = api_comp(&t.prefix)?;
                team.suffix = api_comp(&t.suffix)?;
                team.friendly_fire = t.allow_friendly_fire;
                team.see_friendly_invisibles = t.see_friendly_invisibles;
                team.name_tag_visibility = api_visibility(t.nametag_visibility);
                team.collision_rule = api_collision_rule(t.collision_rule);
                team.members = t.players.iter().cloned().collect();
                Ok(team)
            })
            .collect()
    }
}
