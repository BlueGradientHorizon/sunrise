

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

use crate::game::client_side::advancement_toast::{Toast, toast_label};

use sunrise_client::toasts::Slide;

pub fn recipes(n: usize) -> Toast {
        Toast::recipe(
            (0..n)
                .map(|i| {
                    let item = ItemStack::new(&format!("minecraft:item_{i}"), 1);
                    ("minecraft:crafting_table".to_owned(), item)
                })
                .collect(),
        )
    }
/// Drives a toast frame by frame (every `step` ms) and returns the slide
    /// portions at the given times and when it finished.
    pub fn run(
        toast: &mut Toast,
        step: i64,
        until: i64,
        at: &[i64],
    ) -> (Vec<f32>, Option<i64>) {
        let mut slide = Slide::default();
        let mut seen = Vec::new();
        for now in (0..=until).step_by(step as usize) {
            let (_, finished) = slide.update(now, |f| toast.wants_show(f, 1.0));
            if finished {
                return (seen, Some(now));
            }
            if at.contains(&now) {
                seen.push(slide.portion);
            }
        }
        (seen, None)
    }
pub fn announced(
        title: Value,
        description: Value,
    ) -> adv::Advancement {
        adv::Advancement {
            id: "quests:board".into(),
            parent: None,
            display: Some(adv::Display {
                icon: sunrise_items::items::ItemStack::new("minecraft:book", 1),
                title,
                description,
                frame: Frame::Task,
                background: None,
                show_toast: true,
                announce: true,
                hidden: false,
            }),
            criteria: Vec::new(),
            requirements: Vec::new(),
            rewards: adv::Rewards::default(),
        }
    }
}
