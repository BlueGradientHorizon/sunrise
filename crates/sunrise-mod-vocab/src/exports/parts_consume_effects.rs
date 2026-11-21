//! Exports for custom consume-effect registrations.
pub use crate::parts::ConsumeEffectType;
mod __private {
    pub use crate::parts::{
        RegisteredConsumeEffect, freeze_consume_effect_types, is_builtin_consume_effect,
    };
}
