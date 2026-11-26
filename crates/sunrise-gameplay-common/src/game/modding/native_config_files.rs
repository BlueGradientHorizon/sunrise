#[cfg(any(test, feature = "test-support"))]
pub mod tests {

    #[cfg(test)]
    use crate::game::native_config_file_services::FileRoot;
    use mod_api::ConfigPath;
    #[cfg(test)]
    use std::fs;

    use sunrise_level::crash::{ModCall, enter_native_mod};

    #[cfg(test)]
    struct Root(FileRoot);
    #[cfg(test)]
    impl Root {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "sunrise-config-files-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self(FileRoot {
                path: root,
                bulk: false,
            })
        }
    }
    #[cfg(test)]
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0.path);
        }
    }
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
