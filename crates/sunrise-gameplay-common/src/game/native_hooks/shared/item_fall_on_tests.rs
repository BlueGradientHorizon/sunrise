use super::*;
use mod_api::{BlockPart, FallOnEvent, Hooks, LevelRandomHost, Registrar};
use std::sync::{Arc, Mutex};

/// What the part on stone heard of a landing: entity, kind, block, fall and damage. It
/// asks for the block to be destroyed.
#[derive(Clone, Default)]
pub(in crate::game) struct Lands(
    pub(in crate::game) Arc<Mutex<Vec<(i32, String, BlockPos, f64, f32)>>>,
);
#[mod_api::sunrise_part]
impl BlockPart for Lands {
    fn fall_on(&self, _: &mut dyn LevelRandomHost, event: &mut FallOnEvent<'_>) {
        self.0.lock().unwrap().push((
            event.entity,
            event.kind.to_owned(),
            event.pos,
            event.distance,
            event.damage_multiplier,
        ));
        event.request(mod_api::LandingWrite::DestroyBlock { drop: false });
    }
}
pub(in crate::game) struct Mod(pub(in crate::game) Lands);
#[mod_api::__subscriptions]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        let block_key = r.block_key("minecraft:stone");
        r.block_part(block_key, self.0.clone());
    }
}

pub static LOAN_VALUE: mod_api::AttachmentType<i32> = mod_api::AttachmentType {
    namespace: "loan_test",
    name: "progress",
    default: || 0,
    codec: None,
    copy_on_death: false,
    sync: false,
};

pub(in crate::game) struct LoanPart(pub(in crate::game) std::sync::Arc<()>);
#[mod_api::sunrise_part]
impl BlockPart for LoanPart {
    fn fall_on(&self, host: &mut dyn LevelRandomHost, event: &mut FallOnEvent<'_>) {
        let draw = host
            .with_random(|random| random.next_int())
            .expect("authoritative landing random");
        assert_eq!(draw, 1553932502);
        LOAN_VALUE
            .set(host.attachments(), mod_api::Holder::Level, 73)
            .unwrap();
        event.request(mod_api::LandingWrite::DestroyBlock { drop: false });
        std::panic::panic_any(self.0.clone());
    }
}
pub(in crate::game) struct LoanMod(pub(in crate::game) std::sync::Arc<()>);
#[mod_api::__subscriptions]
impl Hooks for LoanMod {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        r.register_attachment_type("loan_test:progress", false)
            .unwrap();
        let key = r.block_key("minecraft:stone");
        r.block_part(key, LoanPart(self.0.clone()));
    }
}
