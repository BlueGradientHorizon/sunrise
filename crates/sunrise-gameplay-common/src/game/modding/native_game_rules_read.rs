use crate::game::CommonGame;
use mod_api::GameRuleReadError;
use sunrise_game_data::gamerules::Value as GameRuleValue;

impl crate::game::CommonGame<'_> {
    pub fn game_rule(&self, id: &str) -> Result<GameRuleValue, GameRuleReadError> {
        self.ui.rules.get(id).ok_or(GameRuleReadError::UnknownRule)
    }

    pub fn game_rules(&self) -> Vec<(String, GameRuleValue)> {
        self.ui
            .rules
            .available_rules()
            .filter_map(|rule| Some((rule.name.clone(), self.ui.rules.get(&rule.name)?)))
            .collect()
    }
}
