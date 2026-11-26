use super::*;

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
#[allow(dead_code)]
impl crate::game::CommonGame<'_> {
    /// A data file of the game's resources, content mods' included
    /// (`data/mymod/machines.json`), when there is one: what a mod's content
    /// says, read at runtime. Only relative paths of plain names. API 20.
    pub(in crate::game) fn native_data(&self, path: &str) -> Option<Vec<u8>> {
        let plain = |p: &str| !p.is_empty() && p != "." && p != ".." && !p.contains('\\');
        if !path.starts_with("data/") || !path.split('/').all(plain) {
            return None;
        }
        fs::read(self.options.resources.join(path)).ok()
    }
    pub(in crate::game) fn native_block_collision_in(
        &self,
        world: &sunrise_level::world::World,
        pos: [i32; 3],
    ) -> Vec<[f64; 6]> {
        let state = world.block(pos[0], pos[1], pos[2]);
        let binding = self.collision;
        let mut scene = sunrise_level::collisions::SceneCollisions {
            world,
            binding,
            subject: None,
        };
        binding
            .collision_shape_with_context(
                &mut scene,
                state,
                BlockPos::new(pos[0], pos[1], pos[2]),
                &sunrise_world::collision_context::CollisionContext::empty(false),
            )
            .unwrap_or_else(|_| sunrise_core::geometry::VoxelShape::empty())
            .boxes()
            .iter()
            .map(|bounds| {
                let min = bounds.min_position();
                let max = bounds.max_position();
                [min.x, min.y, min.z, max.x, max.y, max.z]
            })
            .collect()
    }
    /// The player, in a world.
    pub fn native_player(&self) -> Option<NativePlayer> {
        let p = self.player.as_ref()?;
        Some(NativePlayer {
            x: p.position.x,
            y: p.position.y,
            z: p.position.z,
            yaw: self.yaw,
            pitch: self.pitch,
            dimension: self.dim.id(),
        })
    }
    /// A native mod's own folder: `sunrise-mods/.sunrise-mod-data/<id>/`. Not created here.
    pub(in crate::game) fn native_data_dir(&self, mod_id: &str) -> PathBuf {
        crate::game::mod_data_root().join(mod_id)
    }
    /// No world is open: a client without a player (the title screen). A
    /// server's `Game` (the server thread, a dedicated server, a LAN host)
    /// has no player of its own, and is always in its world.
    pub fn outside_world(&self) -> bool {
        self.player.is_none() && self.options.execution_role.is_client()
    }
}

pub fn mod_data_root() -> PathBuf {
    crate::game::game_dir()
        .join(mod_list::DIR)
        .join(".sunrise-mod-data")
}

/// The game's `config/` folder beside `options.txt`, shared by every mod
/// as on Fabric and NeoForge. The same folder on the window, its server
/// thread and a LAN host; a dedicated server's is in its own folder,
/// beside `server.properties`. Made when missing. Mods reach their own
/// files in it through `Host::config_files`.
pub fn native_config_dir() -> std::io::Result<PathBuf> {
    config_dir(&crate::game::game_dir())
}
