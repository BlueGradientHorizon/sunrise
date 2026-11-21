pub use world::random::{LegacyRandomSource, RandomSource};

pub mod __private {
    pub use crate::world::random::BorrowedRandom;
}
