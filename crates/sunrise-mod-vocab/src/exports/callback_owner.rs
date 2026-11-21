mod callback_owner;
mod __private {
    mod game {
        pub use crate::callback_owner::{
            CallbackOwner, CallbackPanicNotice, CallbackValue, contain_unwind, panic_payload_text,
            record_contained_panic, retained_callback_active, take_callback_panic_notices,
        };
    }
}
