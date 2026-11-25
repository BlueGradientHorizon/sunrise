#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    pub(in crate::game) fn guest_drop_carried(&mut self, gi: usize) {
        let carried = self.with_own_record(gi, |host, _| {
            let conn = host.hub.guests[gi].conn;
            let mut stacks = host.hub.take_closed_menu_cursors(conn).collect::<Vec<_>>();
            let (to_return, to_drop) = host.hub.take_closed_processing_inputs(conn);
            stacks.extend(to_return);
            stacks.extend(to_drop);
            host.hub
                .guests
                .update(gi, |g| stacks.extend(g.take_menu_cursors()));
            Some(stacks)
        });
        if let Some(carried) = carried.filter(|stacks| !stacks.is_empty()) {
            self.drop_guest_items(gi, carried);
        }
    }

    pub(in crate::game) fn drop_guest_items(&mut self, gi: usize, stacks: Vec<ItemStack>) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let g = &host.hub.guests[gi];
        let eye = [
            g.position[0],
            g.position[1] + g.eye_height() - f64::from(0.3f32),
            g.position[2],
        ];
        let (yaw, pitch) = (g.yaw, g.pitch);
        let mut r = sunrise_client::random_source::entity(); // the guest's Entity.random
        for stack in stacks {
            let v = sunrise_client::entities::drop_motion(&mut r, yaw, pitch, false);
            self.spawn_item(stack, eye, v, 40);
        }
    }
}
