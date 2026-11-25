use super::*;

impl GameLevel<'_> {
    pub(in crate::game) fn native_remove_block(&self, call: super::fire::RemoveBlockCall<'_>) {
        self.fire_remove_block(call);
    }
}
