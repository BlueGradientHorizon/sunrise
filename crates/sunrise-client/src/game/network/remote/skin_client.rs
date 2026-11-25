#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::skin::*;

impl Game<'_> {
    /// Whether a downloaded skin or cape is in the atlas to draw, marking it
    /// drawn; if not (new, or its slot taken), its image is queued for upload
    /// again and it is drawn from the next frame.
    pub(in crate::game) fn again_skin(&self, location: &str) -> bool {
        let mut downloads = self.skin_downloads.0.borrow_mut();
        // A slot without its sprite (the atlas was rebuilt, or the upload
        // never happened) counts as missing, so the skin is stored again.
        if self.client.skin_slots.contains(location)
            && self.gui.atlas.sprites.contains_key(location)
        {
            downloads.drawn(location);
            true
        } else {
            downloads.again(location);
            false
        }
    }
    /// A mod's `GuiDraw::PlayerFace` skin (`skins::face_skin`): a player with a `PlayerInfo` on
    /// this connection draws its skin as the player list does; any other UUID its profile, looked
    /// up by id as a head's is. Until uploaded, the UUID's default. It borrows the name: a
    /// looked-up skin's is copied into `name`, reused from face to face.
    pub(in crate::game) fn player_face_skin<'a>(
        &'a self,
        uuid: u128,
        name: &'a mut String,
    ) -> &'a str {
        let listed = self
            .remote
            .as_ref()
            .filter(|r| r.tab_players.contains_key(&uuid))
            .map(|r| {
                let skin = r.skins.get(&uuid).map_or_else(
                    || sunrise_client::tab_list::default_skin(uuid),
                    |skin| skin.body(uuid).0,
                );
                self.shown_face(uuid, skin)
            });
        sunrise_client::skins::face_skin(
            uuid,
            listed,
            &mut self.skin_profiles.0.borrow_mut(),
            &mut self.skin_downloads.0.borrow_mut(),
            |location| self.gui.atlas.sprites.contains_key(location),
            name,
        )
    }
    /// Resolve inline `object:player` profiles through the same skin cache as
    /// player heads on items. The next frame uses the uploaded atlas texture.
    pub(in crate::game::client_side) fn resolve_inline_profiles(
        &mut self,
        profiles: &[std::sync::Arc<serde_json::Value>],
    ) {
        use sunrise_client::skins;
        for profile in profiles {
            let skin = skins::head_skin(
                profile,
                &mut self.skin_profiles.0.borrow_mut(),
                &mut self.skin_downloads.0.borrow_mut(),
                |location| self.gui.atlas.sprites.contains_key(location),
            );
            let id = skins::parse_profile(profile).id;
            let name = profile
                .as_str()
                .or_else(|| profile.get("name").and_then(serde_json::Value::as_str));
            // The player list's skin for the same profile is drawn too.
            let keys = [
                Some(format!("id:{id:032x}")),
                name.map(|n| format!("name:{}", n.to_ascii_lowercase())),
            ];
            for key in keys.iter().flatten() {
                if let Some(listed) = self.gui.atlas.object_skins.get(key)
                    && listed.starts_with("skins/")
                {
                    let listed = listed.clone();
                    self.again_skin(&listed);
                }
            }
            self.gui
                .atlas
                .set_object_skin(format!("id:{id:032x}"), skin.clone());
            if let Some(name) = name {
                self.gui
                    .atlas
                    .set_object_skin(format!("name:{}", name.to_ascii_lowercase()), skin);
            }
        }
    }
    /// Puts a downloaded skin in an atlas skin slot (`SkinSlots::place`),
    /// unless every slot holds a skin drawn in the last frame.
    pub(in crate::game::client_side) fn store_skin(&mut self, location: String, rgba: Vec<u8>) {
        let Some((slot, evicted)) = self.client.skin_slots.place(location.clone()) else {
            return;
        };
        if let Some(old) = evicted {
            self.gui.atlas.sprites.remove(&old);
        }
        let Some(mut rect) = self
            .gui
            .atlas
            .sprites
            .get(&format!("sunrise/skin{slot}"))
            .copied()
        else {
            return;
        };
        let (w, h) = (self.gui.atlas.width as f32, self.gui.atlas.height as f32);
        self.client.map_patches.push(sunrise_client::font::Patch {
            x: (rect.u0 * w).round() as u32,
            y: (rect.v0 * h).round() as u32,
            width: 64,
            height: 64,
            rgba,
        });
        // A cape is a 64x32 texture in the top half of the slot (skins::process_cape).
        if location.starts_with("capes/") {
            rect.v1 = (rect.v0 + rect.v1) / 2.0;
        }
        self.gui.atlas.sprites.insert(location, rect);
    }
}
