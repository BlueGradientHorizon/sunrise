#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;

pub(in crate::game) fn player_teleport(
    game: &mut Game<'_>,
    had_player: bool,
    from: Vec3,
    to: Vec3,
) {
    if had_player && let Ok(id) = u32::try_from(game.own_entity_id()) {
        game.remote_entity_section_step(id, from, to);
    }
}

pub(in crate::game) fn tick_items(
    game: &mut Game<'_>,
    entered: &mut Vec<sunrise_client::entities::ItemSplash>,
) {
    let mut items = game.dropped.lift();
    let mut i = 0;
    while i < items.len() {
        let (_, discarded) =
            game.tick_item_with_block_contacts(&mut items[i], None, &[], true, &mut |splash| {
                entered.push(splash)
            });
        let from = items[i].previous;
        let to = items[i].position;
        if !discarded {
            let live = game.entity_section_step_visible(
                items[i].id,
                [from.x, from.y, from.z],
                [to.x, to.y, to.z],
            );
            if !live {
                items.remove(i);
                continue;
            }
        }
        if discarded {
            items.remove(i);
        } else {
            i += 1;
        }
    }
    game.put_back_dropped(items);
}
