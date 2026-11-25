//! Client-side composter level event feedback (`ComposterBlock.handleFill`).
use super::Game;
use sunrise_client::particles::{ParticleOptions, ParticleType};
use sunrise_core::{position::BlockPos, random::RandomSource};

impl Game<'_> {
    /// `ComposterBlock.handleFill` (level event 1500) on the client: the
    /// fill sound and ten composter particles above the compost.
    pub(in crate::game) fn composter_fill_event(&mut self, pos: BlockPos, success: bool) {
        let center = [
            f64::from(pos.x) + 0.5,
            f64::from(pos.y) + 0.5,
            f64::from(pos.z) + 0.5,
        ];
        let sound = if success {
            "minecraft:block.composter.fill_success"
        } else {
            "minecraft:block.composter.fill"
        };
        // `handleFill` is `playLocalSound`: the client that receives event 1500
        // plays it. Capturing it here would also send a sound packet.
        self.audio.0.play_unshared(sound, Some(center), 1.0, 1.0);
        // getShape(..).max(Y, 0.5, 0.5): the compost surface (SHAPES[8] = SHAPES[7]).
        let level = self.composter_level([pos.x, pos.y, pos.z]).unwrap_or(0);
        let height = f64::from((1 + level.min(7) * 2).clamp(2, 16)) / 16.0 + 0.03125;
        let options = ParticleOptions::simple(ParticleType::Composter);
        for _ in 0..10 {
            let r = self.particles.0.random();
            let v = [
                r.next_gaussian() * 0.02,
                r.next_gaussian() * 0.02,
                r.next_gaussian() * 0.02,
            ];
            let p = [
                f64::from(pos.x) + 0.1875 + 0.625 * f64::from(r.next_float()),
                f64::from(pos.y) + height + f64::from(r.next_float()) * (1.0 - height),
                f64::from(pos.z) + 0.1875 + 0.625 * f64::from(r.next_float()),
            ];
            self.particles.0.add(&options, p, v);
        }
    }
}
