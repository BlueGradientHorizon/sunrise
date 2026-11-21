mod callback_owner;
mod __private {
    mod game {
        pub use crate::callback_owner::{CallbackOwner, retained_callback_active};
    }
}
