//! Borrowed parts of a living model during setup animation and after posing.

/// A part of a humanoid model, by the name vanilla's `PlayerModel` gives it. The outer skin
/// layer's parts (`Hat`, `Jacket`, the sleeves and the pants) are children of the part they cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sunrise_mod_api_macros::Wire)]
#[non_exhaustive]
pub enum HumanoidPart {
    /// The head.
    Head,
    /// The hat layer, over the head.
    Hat,
    /// The body.
    Body,
    /// The jacket layer, over the body.
    Jacket,
    /// The right arm.
    RightArm,
    /// The right sleeve, over the right arm.
    RightSleeve,
    /// The left arm.
    LeftArm,
    /// The left sleeve, over the left arm.
    LeftSleeve,
    /// The right leg.
    RightLeg,
    /// The right trouser leg, over the right leg.
    RightPants,
    /// The left leg.
    LeftLeg,
    /// The left trouser leg, over the left leg.
    LeftPants,
}

impl HumanoidPart {
    /// Every part, parents before their children.
    pub const ALL: [Self; 12] = [
        Self::Head,
        Self::Hat,
        Self::Body,
        Self::Jacket,
        Self::RightArm,
        Self::RightSleeve,
        Self::LeftArm,
        Self::LeftSleeve,
        Self::RightLeg,
        Self::RightPants,
        Self::LeftLeg,
        Self::LeftPants,
    ];

    /// Its name in the model (`right_sleeve`).
    pub fn model_name(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Hat => "hat",
            Self::Body => "body",
            Self::Jacket => "jacket",
            Self::RightArm => "right_arm",
            Self::RightSleeve => "right_sleeve",
            Self::LeftArm => "left_arm",
            Self::LeftSleeve => "left_sleeve",
            Self::RightLeg => "right_leg",
            Self::RightPants => "right_pants",
            Self::LeftLeg => "left_leg",
            Self::LeftPants => "left_pants",
        }
    }
}

/// How many parts [`HiddenParts`] holds.
pub const MAX_HIDDEN_PARTS: usize = 8;
/// The longest part name [`HiddenParts`] takes, in bytes.
pub const MAX_PART_NAME: usize = 24;

/// The parts a mod hid from a vanilla model this frame (`render_player_pre`, `render_living_pre`), by their
/// names in the model (`head`, `nose`, `right_front_leg`): at most [`MAX_HIDDEN_PARTS`] names of
/// at most [`MAX_PART_NAME`] bytes, nothing allocated. A hidden part takes its children with it
/// (the hat goes with the head). An unknown name refuses the draw with the mob type and name.
/// Typed humanoid parts have separate storage and always fit alongside named parts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
#[wire(via = HiddenPartsWire)]
#[non_exhaustive]
pub struct HiddenParts {
    names: [[u8; MAX_PART_NAME]; MAX_HIDDEN_PARTS],
    lens: [u8; MAX_HIDDEN_PARTS],
    count: u8,
    typed: u16,
}

/// Why [`HiddenParts::hide_named`] took no name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
#[non_exhaustive]
pub enum HideError {
    /// [`MAX_HIDDEN_PARTS`] names are hidden already.
    Full,
    /// The name is empty or longer than [`MAX_PART_NAME`] bytes.
    BadName,
}

impl HiddenParts {
    /// Leaves the humanoid `part` out of the vanilla model this frame.
    pub fn hide(&mut self, part: HumanoidPart) {
        self.typed |= 1 << (part as u16);
    }

    /// Leaves the part named `name` out of the vanilla model this frame; hiding a name twice
    /// keeps one.
    pub fn hide_named(&mut self, name: &str) -> Result<(), HideError> {
        if name.is_empty() || name.len() > MAX_PART_NAME {
            return Err(HideError::BadName);
        }
        if self.contains_named(name) {
            return Ok(());
        }
        let slot = usize::from(self.count);
        if slot == MAX_HIDDEN_PARTS {
            return Err(HideError::Full);
        }
        self.names[slot][..name.len()].copy_from_slice(name.as_bytes());
        self.lens[slot] = name.len() as u8;
        self.count += 1;
        Ok(())
    }

    /// Whether the humanoid `part` is hidden.
    pub fn contains(&self, part: HumanoidPart) -> bool {
        self.contains_named(part.model_name())
    }

    /// Whether the part named `name` is hidden.
    pub fn contains_named(&self, name: &str) -> bool {
        self.names().any(|n| n == name)
    }

    /// No part is hidden.
    pub fn is_empty(&self) -> bool {
        self.count == 0 && self.typed == 0
    }

    /// The hidden parts' names, in the order they were hidden.
    pub fn names<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        let (names, lens) = (&self.names, &self.lens);
        (0..usize::from(self.count))
            .map(move |i| {
                // Construction and Wire decoding validate the byte lengths and UTF-8 once.
                std::str::from_utf8(&names[i][..usize::from(lens[i])]).unwrap()
            })
            .chain(
                HumanoidPart::ALL
                    .into_iter()
                    .filter(move |part| self.typed & (1 << (*part as u16)) != 0)
                    .map(|part| -> &'a str { part.model_name() }),
            )
    }
}

#[derive(sunrise_mod_api_macros::Wire)]
struct HiddenPartsWire {
    names: [[u8; MAX_PART_NAME]; MAX_HIDDEN_PARTS],
    lens: [u8; MAX_HIDDEN_PARTS],
    count: u8,
    typed: u16,
}
impl From<&HiddenParts> for HiddenPartsWire {
    fn from(v: &HiddenParts) -> Self {
        Self {
            names: v.names,
            lens: v.lens,
            count: v.count,
            typed: v.typed,
        }
    }
}
impl TryFrom<HiddenPartsWire> for HiddenParts {
    type Error = crate::wire::WireError;
    fn try_from(v: HiddenPartsWire) -> Result<Self, Self::Error> {
        use crate::wire::WireError;
        if usize::from(v.count) > MAX_HIDDEN_PARTS || v.typed >> 12 != 0 {
            return Err(WireError::Invalid("HiddenParts count or typed bits"));
        }
        for i in 0..usize::from(v.count) {
            let len = usize::from(v.lens[i]);
            if len == 0 || len > MAX_PART_NAME {
                return Err(WireError::Length);
            }
            std::str::from_utf8(&v.names[i][..len]).map_err(|_| WireError::Utf8)?;
        }
        Ok(Self {
            names: v.names,
            lens: v.lens,
            count: v.count,
            typed: v.typed,
        })
    }
}

// no wire: borrows the baked hierarchy and final pose for this draw.
/// The read-only living model selected and posed for this draw. Registered layers and Post
/// hooks borrow it only for their callback; indices belong to this model, not to the entity or
/// to a later draw (which may select a different model).
pub trait FrameModel {
    /// The root of this model, even when it has no cubes of its own.
    fn root(&self) -> FrameModelPart<'_>;

    /// The first depth-first match for this exact name, including the root. Returns `None` if
    /// there is no match; use the hierarchy to distinguish parts with the same name.
    fn part(&self, name: &str) -> Option<FrameModelPart<'_>>;

    /// The part at a model-local index, or `None` if it is outside this model.
    fn part_at(&self, index: usize) -> Option<FrameModelPart<'_>>;
}

// no wire: name and local pose are borrowed from the current frame model.
/// A read-only view of one part within a [`FrameModel`]. Hierarchy links are indices for
/// [`FrameModel::part_at`]; a missing link is `None`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameModelPart<'a> {
    /// This part's index in the model.
    pub index: usize,
    /// Baked name of this part.
    pub name: &'a str,
    /// The parent, absent for the root.
    pub parent: Option<usize>,
    /// The first child, absent for a leaf.
    pub first_child: Option<usize>,
    /// The next child of the same parent, absent for the last sibling and for the root.
    pub next_sibling: Option<usize>,
    /// Position (x, y, z) in texels, then rotation (x, y, z) in radians, then scale (x, y, z)
    /// as unitless factors, all relative to the parent. The root is relative to the model origin.
    pub local_pose: &'a [f32; 9],
    /// Whether visibility allows traversal of this subtree: false if this part or an ancestor
    /// is hidden. A visible part can still submit no cubes of its own (an empty part, a part
    /// whose cubes were skipped, or a draw that submitted only layers).
    pub visible: bool,
}

impl FrameModelPart<'_> {
    /// The local pose as a [`crate::PartPose`] for drawing. Use the parent links to compose
    /// the full transform when attaching geometry to a nested part.
    pub fn pose(&self) -> crate::PartPose {
        let &[x, y, z, x_rot, y_rot, z_rot, x_scale, y_scale, z_scale] = self.local_pose;
        crate::PartPose {
            x,
            y,
            z,
            x_rot,
            y_rot,
            z_rot,
            x_scale,
            y_scale,
            z_scale,
        }
    }
}

// no wire: lends the live pose of this setup-animation invocation.
/// The live local pose at the mid-`HumanoidModel.setupAnim` seam, borrowed only
/// for the listener. The head, hat, body, arms and legs can be posed here if
/// present. Edits affect this draw only and are visible to later listeners.
/// Scale and visibility are not writable through this view.
pub trait HumanoidPose {
    /// The local [position, rotation] of a base part. Missing parts, sleeves, the
    /// jacket and pants return `None`.
    fn get(&self, part: HumanoidPart) -> Option<[[f32; 3]; 2]>;

    /// Writes local position components in texels. Non-finite components retain
    /// their previous values. Returns false for a missing or noneditable part,
    /// or if any component was refused.
    fn set_position(&mut self, part: HumanoidPart, position: [f32; 3]) -> bool;

    /// Writes local rotation components in radians, with the same refusals as
    /// [`Self::set_position`]. Finite angles are not clamped.
    fn set_rotation(&mut self, part: HumanoidPart, rotation: [f32; 3]) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_hidden_wire_is_refused() {
        use crate::wire::{Reader, Writer};
        for (count, len, byte) in [(9, 1, b'a'), (1, 25, b'a'), (1, 1, 255), (1, 0, 0)] {
            let raw = HiddenPartsWire {
                names: [[byte; MAX_PART_NAME]; MAX_HIDDEN_PARTS],
                lens: [len; MAX_HIDDEN_PARTS],
                count,
                typed: 0,
            };
            let mut w = Writer::new();
            w.put(&raw);
            assert!(Reader::new(w.bytes()).take::<HiddenParts>().is_err());
        }
    }

    #[test]
    fn hidden_parts_hold_names_of_any_model_up_to_their_capacity() {
        let mut hidden = HiddenParts::default();
        assert!(hidden.is_empty());
        hidden.hide(HumanoidPart::Head);
        assert_eq!(hidden.hide_named("nose"), Ok(()));
        assert_eq!(hidden.hide_named("nose"), Ok(()));
        assert!(hidden.contains(HumanoidPart::Head) && hidden.contains_named("head"));
        assert!(!hidden.contains(HumanoidPart::Hat) && !hidden.contains_named("nos"));
        assert_eq!(hidden.names().collect::<Vec<_>>(), ["nose", "head"]);
        assert_eq!(hidden.hide_named(""), Err(HideError::BadName));
        assert_eq!(
            hidden.hide_named(&"x".repeat(MAX_PART_NAME + 1)),
            Err(HideError::BadName)
        );
        for i in 0..MAX_HIDDEN_PARTS - 1 {
            assert_eq!(hidden.hide_named(&format!("part_{i}")), Ok(()));
        }
        assert_eq!(hidden.hide_named("one_more"), Err(HideError::Full));
        assert_eq!(hidden.names().count(), MAX_HIDDEN_PARTS + 1);
        for part in HumanoidPart::ALL {
            hidden.hide(part);
        }
        assert!(HumanoidPart::ALL.into_iter().all(|p| hidden.contains(p)));
    }
}
