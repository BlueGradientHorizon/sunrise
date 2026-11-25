use super::*;

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn native_client_attack_target(
        &mut self,
        targets: &mut Vec<EntityRef>,
        canceled: &mut bool,
    ) {
        self.fire_client_attack_target(targets, canceled);
    }
}
