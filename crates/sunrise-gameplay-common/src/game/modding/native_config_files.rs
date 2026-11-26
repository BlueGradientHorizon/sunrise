#[cfg(any(test, feature = "test-support"))]
pub mod tests {

    use mod_api::ConfigPath;
    pub fn path(path: &str) -> ConfigPath {
        ConfigPath::new(path).unwrap()
    }
    #[cfg(any())]
    pub fn as_mod<R>(id: &'static str, f: impl FnOnce() -> R) -> R {
        mod_platform::__private::initialize().unwrap();
        let _call = enter_native_mod(ModCall {
            id,
            hook: "world_load",
            part: None,
        });
        f()
    }
}
