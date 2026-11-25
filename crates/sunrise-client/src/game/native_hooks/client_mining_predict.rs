use super::*;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn native_client_mining_predict(
        &self,
        prediction: ClientMiningPrediction,
        canceled: &mut bool,
    ) {
        self.fire_client_mining_predict(prediction, canceled);
    }
}
