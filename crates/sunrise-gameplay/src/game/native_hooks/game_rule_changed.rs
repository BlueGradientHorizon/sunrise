use mod_api::GameRuleActor;
use sunrise_game_data::gamerules::Value as GameRuleValue;

/// The hook points. Each does nothing in Sunrise.
impl crate::game::ServerGame<'_> {
    /// A server game rule `rule` (`minecraft:keep_inventory`) was set to
    /// `value` (`GameRuleChangedEvent`, from `GameRules.set` with a server:
    /// `/gamerule`, the game rules screen, the management protocol, a test
    /// environment, `server.properties`). `actor` is present when a player
    /// caused the change. `level.game_rules(host)` of any of the server's
    /// levels reads every rule, the new value included. Server-side.
    pub(in crate::game) fn native_game_rule_changed(
        &mut self,
        rule: &str,
        value: GameRuleValue,
        actor: Option<&GameRuleActor<'_>>,
    ) {
        self.fire_game_rule_changed(rule, value, actor);
    }
}
