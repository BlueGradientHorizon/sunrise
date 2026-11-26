

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

pub fn gui(root: &std::path::Path) -> UiResources {
        let atlas = if root.exists() {
            HudAtlas::fonts(&sunrise_assets::resource_source::ResourceSource::directory(
                root.to_path_buf(),
            ))
            .unwrap()
        } else {
            HudAtlas::boot()
        };
        UiResources {
            atlas,
            panorama: Arc::new(sunrise_client::gfx::panorama::Faces::colour([
                96, 110, 111, 255,
            ])),
            splashes: Arc::from(""),
            plain_texts: Default::default(),
        }
    }
pub fn saved_pack_options() -> sunrise_client::options_file::OptionsFile {
        sunrise_client::options_file::OptionsFile::parse(
            &std::fs::read_to_string(crate::game::options_path()).unwrap(),
        )
    }
}
