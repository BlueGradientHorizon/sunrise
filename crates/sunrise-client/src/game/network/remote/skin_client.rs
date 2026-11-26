use super::*;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// Whether a downloaded skin or cape is in the atlas to draw, marking it
    /// drawn; if not (new, or its slot taken), its image is queued for upload
    /// again and it is drawn from the next frame.
    pub(in crate::game) fn again_skin(&self, location: &str) -> bool {
        let mut downloads = self.side.skin_downloads.0.borrow_mut();
        // A slot without its sprite (the atlas was rebuilt, or the upload
        // never happened) counts as missing, so the skin is stored again.
        if self.side.skin_slots.contains(location)
            && self.side.gui.atlas.sprites.contains_key(location)
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
    pub(in crate::game::client_side) fn player_face_skin<'a>(
        &'a self,
        uuid: u128,
        name: &'a mut String,
    ) -> &'a str {
        let listed = self
            .side
            .remote
            .as_ref()
            .filter(|r| r.tab_players.contains_key(&uuid))
            .map(|r| {
                let skin = r.skins.get(&uuid).map_or_else(
                    || sunrise_level::profile_lookup::default_skin(uuid),
                    |skin| skin.body(uuid).0,
                );
                self.shown_face(uuid, skin)
            });
        sunrise_client::skins::face_skin(
            uuid,
            listed,
            &mut self.side.skin_profiles.borrow_mut(),
            &mut self.side.skin_downloads.0.borrow_mut(),
            |location| self.side.gui.atlas.sprites.contains_key(location),
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
                &mut self.side.skin_profiles.borrow_mut(),
                &mut self.side.skin_downloads.0.borrow_mut(),
                |location| self.side.gui.atlas.sprites.contains_key(location),
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
                if let Some(listed) = self.side.gui.atlas.object_skins.get(key)
                    && listed.starts_with("skins/")
                {
                    let listed = listed.clone();
                    self.again_skin(&listed);
                }
            }
            self.side
                .gui
                .atlas
                .set_object_skin(format!("id:{id:032x}"), skin.clone());
            if let Some(name) = name {
                self.side
                    .gui
                    .atlas
                    .set_object_skin(format!("name:{}", name.to_ascii_lowercase()), skin);
            }
        }
    }
    /// Puts a downloaded skin in an atlas skin slot (`SkinSlots::place`),
    /// unless every slot holds a skin drawn in the last frame.
    pub(in crate::game::client_side) fn store_skin(&mut self, location: String, rgba: Vec<u8>) {
        let Some((slot, evicted)) = self.side.skin_slots.place(location.clone()) else {
            return;
        };
        if let Some(old) = evicted {
            self.side.gui.atlas.sprites.remove(&old);
        }
        let Some(mut rect) = self
            .side
            .gui
            .atlas
            .sprites
            .get(&format!("sunrise/skin{slot}"))
            .copied()
        else {
            return;
        };
        let (w, h) = (
            self.side.gui.atlas.width as f32,
            self.side.gui.atlas.height as f32,
        );
        self.side.map_patches.push(sunrise_client::font::Patch {
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
        self.side.gui.atlas.sprites.insert(location, rect);
    }

    /// The player list's view of the connection: the listed players (with
    /// their downloaded skins once ready), online mode, and which players
    /// are in the level with an upside-down name.
    pub(in crate::game) fn publish_tab_object_skins(&mut self) {
        let Some(r) = self.side.remote.as_mut() else {
            return;
        };
        let fingerprint = (
            r.listed.len(),
            r.listed.iter().fold(0, |xor, uuid| xor ^ uuid),
            r.skins_revision,
            self.side.gui.atlas.object_skins_revision,
        );
        if r.published_skins == Some(fingerprint) {
            return;
        }
        for uuid in &r.listed {
            let Some(p) = r.tab_players.get(uuid) else {
                continue;
            };
            let skin = r
                .skins
                .get(uuid)
                .map(|skin| skin.body(*uuid).0)
                .unwrap_or_else(|| sunrise_level::profile_lookup::default_skin(*uuid));
            for key in [
                format!("id:{uuid:032x}"),
                format!("name:{}", p.name.to_ascii_lowercase()),
            ] {
                if object_skin_changed(&self.side.gui.atlas.object_skins, &key, skin) {
                    self.side.gui.atlas.set_object_skin(key, skin.to_owned());
                }
            }
        }
        // Publication itself mutates the atlas; cache its final revision.
        r.published_skins = Some((
            fingerprint.0,
            fingerprint.1,
            fingerprint.2,
            self.side.gui.atlas.object_skins_revision,
        ));
    }

    pub(in crate::game) fn poll_own_profile(&mut self) {
        let auth = self.auth();
        let Some(r) = self.side.remote.as_mut() else {
            return;
        };
        match &mut r.own_profile {
            OwnProfile::Idle => {
                let Some(auth) = auth else {
                    r.own_profile = OwnProfile::Done(None);
                    return;
                };
                let (send, recv) = std::sync::mpsc::channel();
                std::thread::Builder::new()
                    .name("Own profile".into())
                    .spawn(move || {
                        let _ = send.send(sunrise_client::auth::own_textures(auth.as_ref()));
                    })
                    .ok();
                r.own_profile = OwnProfile::Fetching(recv);
            }
            OwnProfile::Fetching(recv) => {
                if let Ok(own) = recv.try_recv() {
                    r.own_profile = OwnProfile::Done(own);
                }
            }
            OwnProfile::Done(_) => {}
        }
    }

    /// Whether `skin` is a downloaded skin not yet in the atlas ([`Self::again_skin`]).
    pub(in crate::game) fn skin_pending(&self, skin: &str) -> bool {
        (skin.starts_with("skins/") || skin == "sunrise/offline_skin") && !self.again_skin(skin)
    }

    /// A face in the player list or spectator menu as drawn, borrowed: `skin`, or the
    /// UUID's default (`DefaultPlayerSkin.get`) while the downloaded skin is not yet in the
    /// atlas, as `PlayerInfo.getSkin` gives before the texture has loaded.
    pub(in crate::game) fn shown_face<'s>(&self, uuid: u128, skin: &'s str) -> &'s str {
        match self.skin_pending(skin) {
            true => sunrise_level::profile_lookup::default_skin(uuid),
            false => skin,
        }
    }

    /// [`Self::shown_face`], owned.
    pub(in crate::game) fn face_skin(&self, uuid: u128, skin: &str) -> String {
        self.shown_face(uuid, skin).to_owned()
    }
}
