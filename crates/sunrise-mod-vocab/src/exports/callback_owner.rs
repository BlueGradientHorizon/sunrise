mod callback_owner;
mod __private {
    mod game {
        pub use crate::callback_owner::{
            CallbackOwner, contain_unwind, panic_payload_text, record_contained_panic,
        };
    }
}
