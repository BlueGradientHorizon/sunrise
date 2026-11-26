#[cfg(any(test, feature = "test-support"))]
pub mod tests {

    use super::*;

    #[cfg(test)]
    struct Observer(std::sync::Arc<AttackBatch>);
    #[cfg(test)]
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Observer {
        fn client_attack_target(
            &self,
            _: &mut dyn mod_api::ClientHost,
            event: &mut mod_api::ClientAttackTargetEvent<'_>,
        ) -> mod_api::Flow {
            *event.targets = self.0.targets.lock().unwrap().clone();
            if self.0.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                mod_api::Flow::Cancel
            } else {
                mod_api::Flow::Continue
            }
        }
    }
}
