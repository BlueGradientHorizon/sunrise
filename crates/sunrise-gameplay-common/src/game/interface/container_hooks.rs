

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

use sunrise_hooks::containers::{ContainerHooks, engine::with_registrants};

#[cfg(test)]
/// Takes every press, drag and key.
    struct Take;
#[cfg(test)]
impl ContainerHooks for Take {
        fn press(&self, _: &mut dyn Container, _: Option<SlotId>, _: Button) -> bool {
            true
        }
        fn drag(&self, _: &mut dyn Container, _: Option<SlotId>, _: Button) -> bool {
            true
        }
        fn key(&self, _: &mut dyn Container, _: Option<SlotId>, _: &str) -> bool {
            true
        }
    }
pub static TAKE: Take = Take;
#[cfg(test)]
/// Picks up slot 9 on every press.
    struct PickNine;
#[cfg(test)]
impl ContainerHooks for PickNine {
        fn press(&self, c: &mut dyn Container, _: Option<SlotId>, _: Button) -> bool {
            c.click(SlotId(9), Click::Pick { right: false })
        }
    }
pub static PICK_NINE: PickNine = PickNine;
#[cfg(test)]
/// Takes every release.
    struct TakeRelease;
#[cfg(test)]
impl ContainerHooks for TakeRelease {
        fn release(&self, _: &mut dyn Container, _: Option<SlotId>, _: Button) -> bool {
            true
        }
    }
pub static TAKE_RELEASE: TakeRelease = TakeRelease;
#[cfg(test)]
/// Reads slots no menu has, counting each call whose reads all come back empty.
    struct ReadFar(std::sync::atomic::AtomicUsize);
#[cfg(test)]
impl ReadFar {
        fn read(&self, c: &dyn Container) -> bool {
            // None of these is a slot of the player's inventory screen (500 to 504, its armour, are).
            let far = [SlotId(36), SlotId(600), SlotId(9999)];
            let empty = far
                .iter()
                .all(|&s| c.stack(s).is_none() && c.name(s).is_none());
            if empty {
                self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            empty
        }
    }
#[cfg(test)]
impl ContainerHooks for ReadFar {
        fn press(&self, c: &mut dyn Container, _: Option<SlotId>, _: Button) -> bool {
            self.read(c)
        }
        fn draw(&self, c: &dyn Container, _: &mut dyn hud::Canvas) {
            self.read(c);
        }
    }
pub static READ_FAR: ReadFar = ReadFar(std::sync::atomic::AtomicUsize::new(0));
#[cfg(test)]
/// Takes nothing.
    pub struct Pass;
#[cfg(test)]
impl ContainerHooks for Pass {}
pub static PASS: Pass = Pass;
}
