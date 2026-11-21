//! Atlas subrects whose tile period is smaller than a screen pixel. The shader paints a single
//! quad; its phase is prepared only once the HUD pass knows its device scale (including capture).

use crate::hud_layout::Pose;
use num_bigint::{BigInt, Sign};

pub(crate) struct SpriteRepeat {
    pub(crate) pose: Pose,
    pub(crate) origin: [f32; 2],
    pub(crate) period: [f32; 2],
}

impl SpriteRepeat {
    pub(super) fn device_phase(&self, scale: f32) -> [[u32; 3]; 2] {
        // Every finite f32 is an integer multiple of 2^-149. Compute with those integers
        // until after modular reduction: dividing first, even in f64, can round a huge
        // tile index to a whole number and lose the phase altogether.
        const SHIFT: usize = 149;
        let units = |value: f32| {
            let bits = value.to_bits();
            let exponent = (bits >> 23) & 0xff;
            let fraction = bits & 0x7f_ffff;
            let mantissa = fraction | (u32::from(exponent != 0) << 23);
            let magnitude = BigInt::from(mantissa) << exponent.saturating_sub(1) as usize;
            if bits >> 31 == 0 {
                magnitude
            } else {
                -magnitude
            }
        };
        let [a, b, c, d, tx, ty] = self.pose.map(units);
        let determinant = &a * &d - &b * &c;
        let scale = units(scale);
        let [x, y] = self.origin.map(units);
        let [w, h] = self.period.map(units);

        [(d, -c, x, w), (-b, a, y, h)].map(|(u, v, origin, period)| {
            // Inverse pose at device pixel centers (i + 1/2, j + 1/2), minus local tile
            // origin, all divided by tile period. Put the two steps and the offset over the
            // same denominator, in terms of the integer units above.
            let dx = &u << (3 * SHIFT);
            let dy = &v << (3 * SHIFT);
            let translation = ((u * &tx + v * &ty) * &scale) << (SHIFT + 1);
            let local_origin = (origin * &determinant * &scale) << 1usize;
            let at_center = &dx + &dy - translation - local_origin;
            let denominator = (&determinant * period * &scale) << 1usize;
            let negative = denominator.sign() == Sign::Minus;
            let denominator = denominator.magnitude();
            let fixed = |numerator: BigInt| {
                let numerator = if negative { -numerator } else { numerator };
                let reduced = numerator.magnitude() % denominator;
                let reduced = if numerator.sign() == Sign::Minus && reduced.bits() != 0 {
                    denominator - reduced
                } else {
                    reduced
                };
                // Quantize toward the start of the next tile, not the end of the previous one.
                // Device pixel indices are nonnegative, so the error in the accumulated phase
                // is nonnegative too: an exact seam stays on the opening side of the tile.
                // A coefficient of 2^32 is a whole repeat and encodes as zero.
                let rounded = ((reduced << 32usize) + denominator - 1u32) / denominator;
                rounded.iter_u32_digits().next().unwrap_or(0)
            };
            [fixed(dx << 1usize), fixed(dy << 1usize), fixed(at_center)]
        })
    }
}
