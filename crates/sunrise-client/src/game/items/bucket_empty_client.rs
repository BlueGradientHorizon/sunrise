#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Hit};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::bucket_empty::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::first_person::UseResult;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{geometry::Direction, position::BlockPos};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::level::BucketContent;

impl Game<'_> {
    /// `bucket_contents` for the pour a server's client predicts: none for an
    /// item whose `empty_contents` Part answers on the server, whose result
    /// stands.
    pub(in crate::game::client_side) fn predicted_bucket_contents<'s>(
        &self,
        stack: &'s ItemStack,
    ) -> Option<(BucketContent, &'s str, Option<&'s str>)> {
        if self.empty_contents_listens(&stack.item) {
            return None;
        }
        sunrise_client::items::bucket_contents(&stack.item)
    }
}
