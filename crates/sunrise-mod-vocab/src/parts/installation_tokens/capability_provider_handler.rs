//! Erased provider metadata retained behind the native binding.
/// Its validated handler name for registration collision checks.
pub fn capability_provider_handler(provider: &crate::NativeCapabilityProvider) -> &'static str {
    provider.handler
}
