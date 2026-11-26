

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::{Refused, WAKE_UP_FROM_SLEEP, vanilla_morning, wake_time};

use mod_api::{ClockAdjustment, Identifier};

use sunrise_gameplay::world_clock::TimeMarker;

pub const NOW: i64 = 13_000;
fn id(s: &str) -> Identifier {
        Identifier::parse(s).unwrap()
    }
pub fn marker(s: &str) -> ClockAdjustment {
        ClockAdjustment::Marker(id(s))
    }
#[cfg(test)]
/// The Overworld clock's markers as vanilla ships the two used here.
    fn overworld(id: &Identifier) -> Option<TimeMarker> {
        let at = |ticks| TimeMarker {
            ticks,
            period: Some(24_000),
            show: false,
        };
        match format!("{}:{}", id.namespace(), id.path()).as_str() {
            WAKE_UP_FROM_SLEEP => Some(at(0)),
            "minecraft:noon" => Some(at(6_000)),
            _ => None,
        }
    }
#[cfg(test)]
/// A clock that defines no markers, as every clock but the Overworld's today.
    fn bare(_: &Identifier) -> Option<TimeMarker> {
        None
    }
}
