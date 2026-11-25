//! The native parity harness runs the client boat ticker and authoritative
//! boat attacks without a renderer, on an empty world.
use super::{Game, boats::Boat};
use serde::Deserialize;
use serde_json::json;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use sunrise_client::Result;
use sunrise_core::geometry::Vec3;

#[derive(Deserialize)]
struct Case {
    name: String,
    velocity: [f64; 3],
    spin: f32,
    #[serde(default)]
    attack: Option<Attack>,
}

#[derive(Deserialize)]
struct Attack {
    yaw: f32,
    sprint: bool,
    ticks: u32,
    player_velocity: [f64; 3],
    #[serde(default)]
    knockback: u8,
    #[serde(default)]
    creative: bool,
}

impl Game<'_> {
    /// Runs authoritative boat-attack cases on a server-role `Game`.
    pub(in crate::game::server_side) fn run_boat_authority_attacks(
        &mut self,
        directory: &Path,
        out: &Arc<Mutex<Vec<(usize, serde_json::Value)>>>,
    ) -> Result<()> {
        let cases: Vec<Case> =
            serde_json::from_str(&std::fs::read_to_string(directory.join("inputs.json"))?)?;
        let mut results = Vec::new();
        for (index, case) in cases.into_iter().enumerate() {
            let Some(attack) = case.attack else {
                continue;
            };
            self.boats.clear();
            let mut boat = Boat::new(1, "oak".into(), Vec3::new(0.0, 100.0, 0.0), 0.0);
            let [x, y, z] = case.velocity;
            boat.velocity = Vec3::new(x, y, z);
            boat.delta_rotation = case.spin;
            self.boats.push(boat);
            let mut player = sunrise_client::motion::PlayerMotion::new(self.motion, Vec3::ZERO)?;
            player.velocity = Vec3::new(
                attack.player_velocity[0],
                attack.player_velocity[1],
                attack.player_velocity[2],
            );
            player.set_sprinting(attack.sprint)?;
            self.player = Some(player);
            self.inventory.fill(None);
            self.selected = 0;
            if attack.knockback > 0 {
                self.inventory[0] = sunrise_client::item_data::parse_stack(&format!(
                    r#"{{id:"minecraft:stick",count:1,components:{{"minecraft:enchantments":{{"minecraft:knockback":{}}}}}}}"#,
                    attack.knockback
                ))?;
            }
            self.yaw = attack.yaw;
            self.server.attack_ticks = attack.ticks;
            self.creative = attack.creative;
            self.strike_boat(0);
            let player = self.player.as_ref().unwrap();
            results.push((
                index,
                json!({
                    "name": case.name,
                    "velocity": self.boats.first().map(|boat| boat.velocity.components().map(|v| format!("{:016x}", v.to_bits()))),
                    "destroyed": self.boats.is_empty(),
                    "player_velocity": player.velocity.components().map(|v| format!("{:016x}", v.to_bits())),
                    "sprint": player.sprinting,
                }),
            ));
        }
        *out.lock().unwrap() = results;
        Ok(())
    }
}
