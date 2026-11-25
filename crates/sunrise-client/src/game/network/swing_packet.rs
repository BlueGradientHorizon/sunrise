use super::Game;
use sunrise_client::first_person::SwingAnimation;

impl Game<'_, crate::game::client_side::ClientState> {
    /// A locally requested swing sends Punch only when the item's Part permits it.
    pub(in crate::game::client_side) fn remote_swing_packet(&mut self) {
        if self.try_swing_hand(self.hand, SwingAnimation::DEFAULT) {
            self.send(sunrise_protocol::packets::play::Serverbound::Punch(
                sunrise_protocol::packets::Empty,
            ));
        }
    }
}
