pub(super) struct RegistrationContext(pub &'static str);

impl RegistrationContext {
    fn for_mod(&self, _: &'static str) -> Self {
        Self(self.0)
    }
}
