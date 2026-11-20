use super::{HudAtlas, MISSING_SPRITE, Rect};

/// The packed key of a GUI sprite id: `minecraft:boss_bar/pink_background` is
/// `gui/sprites/boss_bar/pink_background`, another namespace's `ns:gui/sprites/...`.
pub fn atlas_key_of_gui_sprite(id: &str) -> String {
    match id.split_once(':') {
        Some(("minecraft", path)) => format!("gui/sprites/{path}"),
        Some((namespace, path)) => format!("{namespace}:gui/sprites/{path}"),
        None => format!("gui/sprites/{id}"),
    }
}

impl HudAtlas {
    /// `AtlasGlyphProvider`: the sprite `sprite` of the atlas named `atlas`,
    /// or the missing sprite when that atlas has none by that name.
    pub fn inline_sprite(&self, atlas: &str, sprite: &str) -> Option<Rect> {
        self.find_inline_sprite(atlas, sprite)
            .or_else(|| self.sprites.get(MISSING_SPRITE).copied())
    }

    /// Resolve an inline sprite without substituting the missing sprite.
    pub fn find_inline_sprite(&self, atlas: &str, sprite: &str) -> Option<Rect> {
        let plain = sprite.strip_prefix("minecraft:").unwrap_or(sprite);
        let gui_atlas = atlas.strip_prefix("minecraft:").unwrap_or(atlas) == "gui";
        // The GUI atlas's sprites are found by GUI id; another atlas's by its packed name, then
        // as a GUI sprite.
        let found = if gui_atlas {
            self.gui_rect(plain)
        } else {
            self.sprites.get(plain).or_else(|| self.gui_rect(plain))
        };
        found.copied()
    }
    /// `inline_sprite` for an inline atlas object; `None` for any other object.
    pub(super) fn inline_object_sprite(&self, object: &crate::text::InlineObject) -> Option<Rect> {
        match object {
            crate::text::InlineObject::Atlas { atlas, sprite, .. } => {
                self.inline_sprite(atlas, sprite)
            }
            _ => None,
        }
    }
}

pub(crate) fn find_inline_sprite(
    sprites: &std::collections::HashMap<String, Rect>,
    atlas: &str,
    sprite: &str,
) -> Option<Rect> {
    let plain = sprite.strip_prefix("minecraft:").unwrap_or(sprite);
    let gui = atlas_key_of_gui_sprite(sprite);
    let gui_atlas = atlas.strip_prefix("minecraft:").unwrap_or(atlas) == "gui";
    let (first, second) = if gui_atlas {
        (gui.as_str(), plain)
    } else {
        (plain, gui.as_str())
    };
    sprites.get(first).or_else(|| sprites.get(second)).copied()
}

#[cfg(test)]
mod tests {
    use super::atlas_key_of_gui_sprite;

    #[test]
    fn vanilla_ids_drop_their_namespace_and_a_mods_keep_theirs() {
        assert_eq!(
            atlas_key_of_gui_sprite("minecraft:recipe_book/tab"),
            "gui/sprites/recipe_book/tab"
        );
        assert_eq!(
            atlas_key_of_gui_sprite("recipe_book/tab"),
            "gui/sprites/recipe_book/tab"
        );
        assert_eq!(
            atlas_key_of_gui_sprite("demo:book/on"),
            "demo:gui/sprites/book/on"
        );
    }
}
