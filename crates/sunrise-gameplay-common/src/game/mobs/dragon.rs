

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

pub fn player_box(feet: [f64; 3]) -> (Aabb, Vec3) {
        (
            Aabb::new(
                Vec3::new(feet[0] - 0.3, feet[1], feet[2] - 0.3),
                Vec3::new(feet[0] + 0.3, feet[1] + 1.8, feet[2] + 0.3),
            ),
            Vec3::new(feet[0], feet[1], feet[2]),
        )
    }
pub fn run_dragon_render_parity_fixture() {
        let path = std::env::var("SUNRISE_DRAGON_RENDER_OUTPUT").expect("harness output path");
        let mut cases = Vec::new();
        for t in [0.0, 0.05, 0.25, 0.5, 0.8, 0.9, 0.95, 1.0] {
            let mut vertices = Vec::new();
            dragon_rays(
                glam::Mat4::IDENTITY,
                t,
                sunrise_client::hud::Rect {
                    u0: 0.0,
                    v0: 0.0,
                    u1: 1.0,
                    v1: 1.0,
                    w: 1,
                    h: 1,
                },
                &mut vertices,
            );
            // The fourth quad vertex only pads a triangle for our index buffer.
            let triangles: Vec<_> = vertices
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|q| &q[..3])
                .map(|v| serde_json::json!([v.pos[0], v.pos[1], v.pos[2], v.color]))
                .collect();
            cases.push(triangles);
        }
        std::fs::write(path, serde_json::to_vec(&cases).unwrap()).unwrap();
    }
/// The damage type of the hits below, which count.
    pub const HIT: Option<HurtSource> = Some(HurtSource {
        damage_type: "minecraft:player_attack",
        causing: 0,
        direct: 0,
        position: None,
    });
#[cfg(test)]
fn dragon(phase: Phase) -> Dragon {
        let mut d = Dragon::new(Vec3::new(0.0, 70.0, 0.0), 0.0, 1);
        d.phase = phase;
        d
    }
pub fn flying() -> Phase {
        Phase::Holding {
            path: None,
            target: None,
            clockwise: false,
        }
    }
#[cfg(test)]
/// The dragon hits the hook of these tests saw: entity, damage type,
    /// causing and direct entities, amount.
    type Seen = (String, i32, i32, f32);
static SEEN: std::sync::Mutex<Vec<(i32, Seen)>> = std::sync::Mutex::new(Vec::new());
pub fn seen(entity: i32) -> Vec<Seen> {
        let seen = SEEN.lock().unwrap();
        seen.iter()
            .filter(|s| s.0 == entity)
            .map(|s| s.1.clone())
            .collect()
    }
/// Sets the native damage hook of these tests: it records the dragon
    /// hits of entities 9001 to 9005, cancels 9001's and doubles 9002's.
    pub fn hooks() -> sunrise_sim::health::DamageHooks {
        use sunrise_sim::health::{DamageContainer, DamageHooks, DamageLevel, Hurting, Victim};
        fn hook(
            _: DamageLevel<'_>,
            hurting: Hurting<'_>,
            _: Victim,
            container: &mut DamageContainer,
        ) -> bool {
            let Hurting::Dragon(entity @ 9001..=9005, s) = hurting else {
                return false;
            };
            let amount = container.new_damage();
            let hit = (s.damage_type.to_owned(), s.causing, s.direct, amount);
            SEEN.lock().unwrap().push((entity, hit));
            if entity == 9002 {
                container.record_new_damage(amount * 2.0);
            }
            entity == 9001
        }
        DamageHooks {
            incoming: Some(hook),
            ..DamageHooks::NONE
        }
    }
pub fn dragon_as(entity: u32) -> Dragon {
        let mut d = dragon(flying());
        d.id = entity;
        d
    }
pub fn player(who: FightWho, x: f64, creative: bool, spectator: bool) -> FightPlayer {
        let position = Vec3::new(x, 64.0, 0.0);
        FightPlayer {
            who,
            position,
            bounds: Aabb::new(position, Vec3::new(x + 0.6, 65.8, 0.6)),
            alive: true,
            creative,
            spectator,
        }
    }
}
