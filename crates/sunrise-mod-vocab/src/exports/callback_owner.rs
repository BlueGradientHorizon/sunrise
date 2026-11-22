mod callback_owner;
mod __private {
    mod game {
        #[cfg(feature = "testing")]
        pub use crate::callback_owner::capture_callback_panic_notices;
        pub use crate::callback_owner::{
            CallbackOwner, CallbackPanicNotice, contain_unwind, panic_payload_text,
            record_contained_panic, retained_callback_active, take_callback_panic_notices,
        };
    }
}
