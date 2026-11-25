//! Common Ender Dragon helpers shared by client rendering/picking and server simulation.
#[allow(unused_imports)]
use super::{Game, view_vector};
#[allow(unused_imports)]
use crate::game::server_side::dragon::{
    Crystal, Dragon, Phase, SPECIAL, dragon_part_wire_id, wrap,
};
#[allow(unused_imports)]
use sunrise_core::geometry::{Aabb, Vec3, trig};

/// The part index (`0..8`) of `candidate_id` under `dragon_id`, if it is one of
/// the eight `EnderDragonPart` sub-entities (`dragon_id + 1..=dragon_id + 8`).
pub(in crate::game) fn dragon_part_index(dragon_id: u32, candidate_id: u32) -> Option<usize> {
    let offset = candidate_id.wrapping_sub(dragon_id);
    (1..=8).contains(&offset).then(|| (offset - 1) as usize)
}

impl Game<'_> {
    /// The boxes of the living dragon's parts a projectile can hit, in `getSubEntities` order.
    pub(in crate::game) fn dragon_part_boxes(&self) -> Option<[Aabb; 8]> {
        self.dragon
            .as_ref()
            .filter(|d| d.alive())
            .map(|d| std::array::from_fn(|i| d.part_box(i)))
    }

    /// Whether `id` is a live End crystal entity id.
    pub(in crate::game) fn is_crystal_id(&self, id: u32) -> bool {
        self.crystals.iter().any(|c| c.id == id)
    }

    /// Targets for projectiles: crystals (by real entity id) and dragon parts.
    pub(in crate::game) fn special_targets(&self) -> Vec<(u32, Aabb)> {
        let mut out = Vec::new();
        for c in self.crystals.iter() {
            out.push((c.id, c.bounds()));
        }
        if let Some(boxes) = self.dragon_part_boxes() {
            for (i, part) in boxes.into_iter().enumerate() {
                out.push((SPECIAL + i as u32, part));
            }
        }
        out
    }

    /// Resolves a crosshair target id (including synthetic dragon-part ids) to its wire entity id.
    pub(in crate::game) fn wire_entity_id(&self, id: u32) -> Option<i32> {
        if (SPECIAL..SPECIAL + 8).contains(&id) {
            let dragon = self
                .dragon
                .as_ref()
                .filter(|d| d.id != 0 || self.remote.is_some())?;
            return Some(dragon_part_wire_id(dragon.id, (id - SPECIAL) as usize));
        }
        Some(id as i32)
    }

    /// The dragon the level holds and the index (`getSubEntities` order) of
    /// its part with that entity id, while the dragon is there, dying or
    /// not: a part is an entity for as long as its parent is.
    pub(in crate::game) fn resident_dragon_part(&self, id: u32) -> Option<(&Dragon, usize)> {
        let part = self.side_resident_dragon_part_index(id)?;
        Some((self.dragon.as_ref()?, part))
    }

    /// The box of the dragon part or end crystal guests know as entity `id`
    /// (`EnderDragonPart`s follow the dragon's id for its tracking lifetime).
    pub(in crate::game) fn tracked_special_box(&self, id: i32) -> Option<Aabb> {
        let uid = id as u32;
        if let Some(d) = self
            .dragon
            .as_ref()
            .filter(|d| d.id != 0 || self.remote.is_some())
            && let Some(part) = dragon_part_index(d.id, uid)
        {
            return Some(d.part_box(part));
        }
        let id = u32::try_from(id).ok().filter(|&id| id != 0)?;
        self.crystals
            .iter()
            .find(|c| c.id == id)
            .map(Crystal::bounds)
    }

    /// `getHeightmapPos(MOTION_BLOCKING_NO_LEAVES)`.
    pub(in crate::game) fn motion_blocking_no_leaves(&self, x: i32, z: i32) -> i32 {
        (0..256)
            .rev()
            .find(|&y| {
                let f = self.res.facts().get(self.world.block(x, y, z));
                (f.full_collision
                    || f.legacy_solid
                    || f.fluid != sunrise_client::blocks::Fluid::None)
                    && !f.name.ends_with("_leaves")
            })
            .map_or(0, |y| y + 1)
    }

    pub(in crate::game) fn podium_top(&self) -> Vec3 {
        Vec3::new(0.5, f64::from(self.motion_blocking_no_leaves(0, 0)), 0.5)
    }

    /// `getHeadLookVector`.
    pub(in crate::game) fn dragon_look(&self) -> [f64; 3] {
        let Some(d) = self.dragon.as_ref() else {
            return [0.0; 3];
        };
        let pitch = match d.phase {
            Phase::Landing { .. } | Phase::Takeoff { .. } => {
                let p = self.podium_top();
                let dist = ((p.x - d.position.x).powi(2)
                    + (p.y - d.position.y).powi(2)
                    + (p.z - d.position.z).powi(2))
                .sqrt() as f32
                    / 4.0;
                -6.0 / dist.max(1.0) * 1.5 * 5.0
            }
            _ if d.phase.sitting() => -45.0,
            _ => 0.0,
        };
        view_vector(d.yaw, pitch)
    }

    /// `getHeadLookVector(1).normalize()`, which the phases' `doClientTick`s
    /// take (a look the dragon has never turned from stays the zero vector).
    pub(in crate::game) fn dragon_look_normalised(&self) -> [f64; 3] {
        let look = self.dragon_look();
        let norm = (look[0] * look[0] + look[1] * look[1] + look[2] * look[2]).sqrt();
        if norm < 1.0e-5 {
            [0.0; 3]
        } else {
            [look[0] / norm, look[1] / norm, look[2] / norm]
        }
    }

    /// The part positions from the yaw and the flight history.
    pub(in crate::game) fn place_dragon_parts(&mut self) {
        let Some(d) = self.dragon.as_mut() else {
            return;
        };
        // Mth.sin / Mth.cos on floats, as EnderDragon.aiStep places them.
        let msin = |a: f32| trig::sin(f64::from(a));
        let mcos = |a: f32| trig::cos(f64::from(a));
        let tilt = (d.sample(5).0 - d.sample(10).0) as f32 * 10.0 * trig::DEG_TO_RAD;
        let (ct, st) = (mcos(tilt), msin(tilt));
        let r1 = d.yaw * trig::DEG_TO_RAD;
        let (s1, c1) = (msin(r1), mcos(r1));
        let p = d.position;
        let set = |d: &mut Dragon, i: usize, x: f32, y: f64, z: f32| {
            d.parts[i].0 = Vec3::new(p.x + f64::from(x), p.y + y, p.z + f64::from(z));
        };
        set(d, 2, s1 * 0.5, 0.0, -c1 * 0.5);
        set(d, 6, c1 * 4.5, 2.0, s1 * 4.5);
        set(d, 7, c1 * -4.5, 2.0, s1 * -4.5);
        let r2 = d.yaw * trig::DEG_TO_RAD - d.yaw_accel * 0.01;
        let (s2, c2) = (msin(r2), mcos(r2));
        let y_off = if d.phase.sitting() {
            -1.0
        } else {
            d.sample(5).0 - d.sample(0).0
        } as f32;
        set(
            d,
            0,
            s2 * 6.5 * ct,
            f64::from(y_off + st * 6.5),
            -c2 * 6.5 * ct,
        );
        set(
            d,
            1,
            s2 * 5.5 * ct,
            f64::from(y_off + st * 5.5),
            -c2 * 5.5 * ct,
        );
        let p1 = d.sample(5);
        for i in 0..3 {
            let p0 = d.sample(12 + i * 2);
            let rot = d.yaw * trig::DEG_TO_RAD + wrap(p0.1 - p1.1) * trig::DEG_TO_RAD;
            let (ss, cc) = (msin(rot), mcos(rot));
            let dd = (i as f32 + 1.0) * 2.0;
            let x = -(s1 * 1.5 + ss * dd) * ct;
            let y = p0.0 - p1.0 - f64::from((dd + 1.5) * st) + 1.5;
            let z = (c1 * 1.5 + cc * dd) * ct;
            set(d, 3 + i, x, y, z);
        }
    }

    /// `EndCrystalItem.useOn`'s checks: obsidian or bedrock at `pos`, air
    /// above it and no entity in the two blocks above (else FAIL).
    pub(in crate::game) fn end_crystal_fits(&self, pos: [i32; 3]) -> bool {
        let name = &self
            .res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .name;
        if !matches!(&**name, "minecraft:obsidian" | "minecraft:bedrock") {
            return false;
        }
        let above = [pos[0], pos[1] + 1, pos[2]];
        if !self
            .res
            .facts()
            .is_air(self.world.block(above[0], above[1], above[2]))
        {
            return false;
        }
        let (x, y, z) = (
            f64::from(above[0]),
            f64::from(above[1]),
            f64::from(above[2]),
        );
        !self.side_entity_in(Aabb::new(
            Vec3::new(x, y, z),
            Vec3::new(x + 1.0, y + 2.0, z + 1.0),
        ))
    }

    /// `EnderDragon.checkCrystals`' search: the nearest crystal whose box
    /// meets the dragon's box grown by 32.
    pub(in crate::game) fn nearest_crystal_to(&self, p: Vec3) -> Option<usize> {
        // The dragon's 16 by 8 box.
        let reach = Aabb::new(
            Vec3::new(p.x - 8.0, p.y, p.z - 8.0),
            Vec3::new(p.x + 8.0, p.y + 8.0, p.z + 8.0),
        )
        .inflate(Vec3::new(32.0, 32.0, 32.0));
        self.crystals
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                // An end crystal's 2 by 2 box.
                reach.intersects(Aabb::new(
                    Vec3::new(c.position.x - 1.0, c.position.y, c.position.z - 1.0),
                    Vec3::new(c.position.x + 1.0, c.position.y + 2.0, c.position.z + 1.0),
                ))
            })
            .min_by(|a, b| {
                let da = (a.1.position.x - p.x).powi(2)
                    + (a.1.position.y - p.y).powi(2)
                    + (a.1.position.z - p.z).powi(2);
                let db = (b.1.position.x - p.x).powi(2)
                    + (b.1.position.y - p.y).powi(2)
                    + (b.1.position.z - p.z).powi(2);
                da.total_cmp(&db)
            })
            .map(|(i, _)| i)
    }
}
