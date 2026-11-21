mod callback_owner;
mod __private {
    mod game {
        pub use crate::callback_owner::{
            CallbackOwner, CallbackPanicNotice, take_callback_panic_notices,
        };
    }
}
