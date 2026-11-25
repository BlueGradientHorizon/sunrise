//! Shared `entity_size` helpers used by both server and client sizing passes.
use super::Game;
use mod_api::{EntityRef, Pose};
use sunrise_client::mobs::Mob;
use sunrise_client::pose::{Dimensions, SizeAnswer};

/// The pose a client sizes its copy of a server mob by (`Entity.getPose`).
pub(in crate::game) fn synced_pose(m: &Mob) -> Pose {
    match &*m.kind.id {
        "minecraft:player" => Pose::from_id(i32::from(m.extra.synced_pose)),
        "minecraft:mannequin" => m.extra.avatar_pose.into(),
        _ => Pose::from_id(i32::from(m.extra.pose)),
    }
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Ask `entity_size` for an entity whose vanilla size in `pose` is `vanilla`, unless
    /// `previous` was already asked for both. `None`: nothing to ask, keep `previous`.
    pub(in crate::game) fn ask_entity_size(
        &self,
        id: u32,
        pose: Pose,
        vanilla: [f32; 3],
        previous: Option<SizeAnswer>,
    ) -> Option<SizeAnswer> {
        if previous.is_some_and(|answer| answer.asked(pose, vanilla)) {
            return None;
        }
        let entity = EntityRef::new(self.native_lifecycle.epoch()?, id);
        let old = previous.map_or(vanilla, |answer| answer.effective());
        let mut size = vanilla;
        self.native_entity_size(entity, pose, old, &mut size);
        let usable = size
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0 && *v <= 32.0);
        Some(SizeAnswer {
            pose,
            vanilla,
            size: (usable && size != vanilla).then_some(size),
        })
    }

    /// A body's pose or size changed since it was last asked: ask, and size it by the answer.
    pub(in crate::game) fn ask_body_size(
        &self,
        id: u32,
        body: &sunrise_client::motion::PlayerMotion,
    ) -> Option<SizeAnswer> {
        let vanilla = body.pose.dimensions(body.scale).to_array();
        body.pose_fit_sizes
            .iter()
            .flatten()
            .copied()
            .find(|answer| answer.asked(body.pose.into(), vanilla))
            .or_else(|| self.ask_entity_size(id, body.pose.into(), vanilla, body.size_answer))
    }

    /// Snapshot only the three candidate boxes the shared pose fit test asks about.
    pub(in crate::game) fn body_fit_sizes(
        &self,
        id: u32,
        body: &sunrise_client::motion::PlayerMotion,
        previous: Option<SizeAnswer>,
    ) -> [Option<SizeAnswer>; 3] {
        if !mod_api::__private::listened(mod_api::HookId::EntitySize) {
            return [None; 3];
        }
        let Ok(scale) = body.attributes.computed_value_of("minecraft:scale") else {
            return [None; 3];
        };
        use sunrise_client::pose::Pose as MotionPose;
        [
            MotionPose::Standing,
            MotionPose::Crouching,
            MotionPose::Swimming,
        ]
        .map(|pose| {
            let vanilla = pose.dimensions(scale as f32).to_array();
            self.ask_entity_size(id, pose.into(), vanilla, previous)
                .or_else(|| previous.filter(|answer| answer.asked(pose.into(), vanilla)))
        })
    }

    pub(in crate::game) fn apply_body_size(
        body: &mut sunrise_client::motion::PlayerMotion,
        answer: SizeAnswer,
    ) {
        body.size_answer = Some(answer);
        body.dimensions = Dimensions::from_array(answer.effective());
    }
}
