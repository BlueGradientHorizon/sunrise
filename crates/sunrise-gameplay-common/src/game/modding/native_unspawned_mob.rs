

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use mod_api::Identifier;
#[cfg(test)]
use mod_api::ItemStack;

pub const AT: [f64; 3] = [8.5, 64.0, 8.5];
pub fn id(s: &str) -> Identifier {
        Identifier::parse(s).unwrap()
    }
#[cfg(test)]
fn pumpkin() -> ItemStack {
        crate::game::api_values::test_snapshot(&sunrise_items::items::ItemStack::new(
            "minecraft:carved_pumpkin",
            1,
        ))
    }
#[cfg(test)]
/// A mod goal that only names itself: a mob carries it under this name
    /// while it has its goal, and as `ModGoal` once a copy left it out.
    struct Kept;
#[cfg(test)]
impl mod_api::Goal for Kept {
        fn flags(&self) -> mod_api::GoalFlags {
            mod_api::GoalFlags::MOVE
        }
        fn name(&self) -> &'static str {
            "Kept"
        }
        fn can_use(&mut self, _: &mut mod_api::GoalContext<'_>) -> bool {
            false
        }
    }
#[cfg(test)]
struct KeptGoal;
#[cfg(test)]
#[mod_api::sunrise_part]
    impl mod_api::EntityPart for KeptGoal {
        fn register_goals(&self, goals: &mut mod_api::GoalRegistrar<'_>) {
            goals.add_goal(7, Kept);
        }
    }
#[cfg(test)]
struct KeptGoalMod;
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for KeptGoalMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.entity_part("minecraft:zombie", KeptGoal);
        }
    }
}
