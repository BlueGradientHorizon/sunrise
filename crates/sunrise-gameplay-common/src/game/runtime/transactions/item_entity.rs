/// First writes to each field, independently: moving an item need not clone its stack.
#[derive(Default)]
pub struct KeptItemEntity {
    pub(in crate::game) stack: Option<ItemStack>,
    pub pickup_delay: Option<i64>,
    pub target: Option<Option<[i32; 4]>>,
    pub velocity: Option<Vec3>,
}

impl KeptItemEntity {
    pub(in crate::game) fn merge(&mut self, child: Self) {
        self.stack = self.stack.take().or(child.stack);
        self.pickup_delay = self.pickup_delay.or(child.pickup_delay);
        self.target = self.target.or(child.target);
        self.velocity = self.velocity.or(child.velocity);
    }

    pub(in crate::game) fn restore(self, item: &mut sunrise_sim::entities::ItemEntity) {
        if let Some(stack) = self.stack {
            item.stack = stack;
        }
        if let Some(delay) = self.pickup_delay {
            item.pickup_delay = delay;
        }
        if let Some(target) = self.target {
            item.target = target;
        }
        if let Some(velocity) = self.velocity {
            item.velocity = velocity;
        }
    }
}
