use crate::game::remote::movement::MovePacket;
pub(in crate::game::client_side) use crate::game::remote::movement::stepped_move;
use sunrise_world::stepped_interpolation::{PosRot, SteppedInterpolation};

impl MovePacket {
    /// `tooBigToInterpolate`: a move to `to` of more than 64 blocks snaps.
    /// A position sync measures it from where the entity is, a teleport from
    /// where it is headed (`PositionMoveRotation.of`'s
    /// `getClientPositionAndRotation`).
    pub(in crate::game) fn snaps(
        self,
        handler: &SteppedInterpolation,
        entity: PosRot,
        to: [f64; 3],
    ) -> bool {
        let from = match self {
            MovePacket::Move => return false,
            MovePacket::PositionSyncSnap => return true,
            MovePacket::PositionSync => entity.position,
            MovePacket::Teleport => handler.heading(entity).position,
        };
        let d = [0, 1, 2].map(|i| to[i] - from[i]);
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 4096.0
    }
}
