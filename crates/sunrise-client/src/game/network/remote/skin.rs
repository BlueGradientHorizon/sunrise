#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// The player list's view of the connection: the listed players (with
    /// their downloaded skins once ready), online mode, and which players
    /// are in the level with an upside-down name.
    pub(in crate::game) fn publish_tab_object_skins(&mut self) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let fingerprint = (
            r.listed.len(),
            r.listed.iter().fold(0, |xor, uuid| xor ^ uuid),
            r.skins_revision,
            self.gui.atlas.object_skins_revision,
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
                .unwrap_or_else(|| sunrise_client::tab_list::default_skin(*uuid));
            for key in [
                format!("id:{uuid:032x}"),
                format!("name:{}", p.name.to_ascii_lowercase()),
            ] {
                if object_skin_changed(&self.gui.atlas.object_skins, &key, skin) {
                    self.gui.atlas.set_object_skin(key, skin.to_owned());
                }
            }
        }
        // Publication itself mutates the atlas; cache its final revision.
        r.published_skins = Some((
            fingerprint.0,
            fingerprint.1,
            fingerprint.2,
            self.gui.atlas.object_skins_revision,
        ));
    }
    pub(in crate::game) fn poll_own_profile(&mut self) {
        let auth = self.auth();
        let Some(r) = self.remote.as_mut() else {
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
            true => sunrise_client::tab_list::default_skin(uuid),
            false => skin,
        }
    }
    /// [`Self::shown_face`], owned.
    pub(in crate::game) fn face_skin(&self, uuid: u128, skin: &str) -> String {
        self.shown_face(uuid, skin).to_owned()
    }
}
