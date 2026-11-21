pub use item::item_model_providers::{
    ItemModelDecoder, ItemModelFn, ItemModelPropertyInput, ItemModelUpdateInput, ItemTintInput,
    ItemTintSourceDecoder, ItemTintSourceFn, RangeSelectPropertyDecoder, RangeSelectPropertyFn,
    SelectPropertyDecoder, SelectPropertyFn,
};
use item::item_model_providers::{
    ItemModelRegistration, ItemTintSourceRegistration, RangeSelectPropertyRegistration,
    SelectPropertyRegistration,
};
mod __private {
    mod game {
        pub use crate::item::item_model_providers::{
            ItemModelRegistration, ItemTintSourceRegistration, RangeSelectPropertyRegistration,
            SelectPropertyRegistration,
        };
    }
}
