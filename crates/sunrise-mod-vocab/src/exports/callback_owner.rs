mod callback_owner;
mod __private {
    mod game {
        pub use crate::callback_owner::{
            CallbackOwner, CallbackPanicNotice, retained_callback_active,
            take_callback_panic_notices,
        };
    }
}
