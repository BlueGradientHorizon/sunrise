//! Scoped editors for pending children in `baby_entity_spawn` and `mob_split`.
//! No resident entity ID, attachment holder, or join is published here: children live on the
//! dispatch stack or inside owned [`mod_api::UnspawnedMob`] values and drop cleanly via RAII if
//! left unattached.

use super::native_unspawned_mob;
use mod_api::{
    AttributeModifier, BabyEntitySpawnSlot, EquipmentSlot, Identifier, ItemStack,
    MobSplitChildList, PendingChildEditor, PendingChildWrite, PendingMobSpec, UnspawnedMob,
    UnspawnedMobError, UnspawnedMobRead, UnspawnedMobView, UnspawnedMobWrite,
};
use std::sync::Arc;
use sunrise_client::mobs::{Mob, Species, finalize};
use sunrise_core::geometry::Vec3;
use sunrise_mod_runtime::attachments::Attachments;
use sunrise_world::attributes::entity::Catalog;

/// All unpublished per-object data travels with its actual Mob. A route must move its
/// existing UUID and attachments here, then install them only at residency commit.
pub(super) struct PendingMob {
    pub(super) mob: Mob,
    /// Set for a child a mod constructed; an engine-made child already owns its published UUID.
    pub(super) uuid: Option<[i32; 4]>,
    pub(super) attachments: Attachments,
    pub(super) dimension: super::Dim,
    pub(super) resources: Arc<super::Resources>,
    pub(super) attributes: Arc<Catalog>,
    pub(super) breeding: bool,
}

impl UnspawnedMobRead for PendingMob {
    fn kind(&self) -> Result<Identifier, UnspawnedMobError> {
        native_unspawned_mob::read_mob_kind(&self.mob)
    }

    fn position(&self) -> Result<[f64; 3], UnspawnedMobError> {
        native_unspawned_mob::read_mob_position(&self.mob)
    }

    fn equipment(&self, slot: EquipmentSlot) -> Result<Option<ItemStack>, UnspawnedMobError> {
        native_unspawned_mob::read_mob_equipment(&self.mob, slot)
    }

    fn attribute_base(&self, attribute: &Identifier) -> Result<f64, UnspawnedMobError> {
        native_unspawned_mob::read_mob_attribute_base(&self.mob, &self.attributes, attribute)
    }

    fn attribute_value(&self, attribute: &Identifier) -> Result<f64, UnspawnedMobError> {
        native_unspawned_mob::read_mob_attribute_value(&self.mob, &self.attributes, attribute)
    }

    fn tags(&self) -> Result<Vec<String>, UnspawnedMobError> {
        native_unspawned_mob::read_mob_tags(&self.mob)
    }

    fn custom_name(&self) -> Result<Option<mod_api::Text>, UnspawnedMobError> {
        native_unspawned_mob::read_mob_custom_name(&self.mob)
    }
}

impl UnspawnedMobWrite for PendingMob {
    fn set_equipment(
        &mut self,
        slot: EquipmentSlot,
        item: Option<ItemStack>,
    ) -> Result<(), UnspawnedMobError> {
        native_unspawned_mob::write_mob_equipment(
            &mut self.mob,
            self.resources.items(),
            &self.attributes,
            slot,
            item,
        )
    }

    fn set_attribute_base(
        &mut self,
        attribute: &Identifier,
        base: f64,
    ) -> Result<(), UnspawnedMobError> {
        native_unspawned_mob::write_mob_attribute_base(
            &mut self.mob,
            self.resources.items(),
            &self.attributes,
            attribute,
            base,
        )
    }

    fn add_attribute_modifier(
        &mut self,
        attribute: &Identifier,
        modifier: AttributeModifier,
    ) -> Result<(), UnspawnedMobError> {
        native_unspawned_mob::write_mob_add_attribute_modifier(
            &mut self.mob,
            self.resources.items(),
            &self.attributes,
            attribute,
            modifier,
        )
    }

    fn remove_attribute_modifier(
        &mut self,
        attribute: &Identifier,
        id: &Identifier,
    ) -> Result<bool, UnspawnedMobError> {
        native_unspawned_mob::write_mob_remove_attribute_modifier(
            &mut self.mob,
            self.resources.items(),
            &self.attributes,
            attribute,
            id,
        )
    }

    fn add_tag(&mut self, tag: &str) -> Result<bool, UnspawnedMobError> {
        native_unspawned_mob::write_mob_add_tag(&mut self.mob, tag)
    }

    fn remove_tag(&mut self, tag: &str) -> Result<bool, UnspawnedMobError> {
        native_unspawned_mob::write_mob_remove_tag(&mut self.mob, tag)
    }

    fn set_custom_name(&mut self, name: Option<mod_api::Text>) -> Result<(), UnspawnedMobError> {
        native_unspawned_mob::write_mob_set_custom_name(&mut self.mob, name)
    }
}

impl PendingChildWrite for PendingMob {
    fn set_position(&mut self, position: [f64; 3]) -> Result<(), UnspawnedMobError> {
        if !super::native_mobs::spawnable(position) {
            return Err(UnspawnedMobError::Invalid);
        }
        self.mob.position = Vec3::new(position[0], position[1], position[2]);
        self.mob.previous = self.mob.position;
        Ok(())
    }

    fn set_yaw(&mut self, yaw: f32) -> Result<(), UnspawnedMobError> {
        if !yaw.is_finite() {
            return Err(UnspawnedMobError::Invalid);
        }
        self.mob.yaw = yaw;
        self.mob.previous_yaw = yaw;
        self.mob.body_yaw = yaw;
        self.mob.previous_body_yaw = yaw;
        self.mob.head_yaw = yaw;
        self.mob.previous_head_yaw = yaw;
        Ok(())
    }

    fn set_baby(&mut self, baby: bool) -> Result<(), UnspawnedMobError> {
        if (self.breeding && !baby) || (baby && !finalize::supports_baby(self.mob.species())) {
            return Err(UnspawnedMobError::Invalid);
        }
        finalize::set_baby(&mut self.mob, baby);
        Ok(())
    }

    fn set_size(&mut self, size: u32) -> Result<(), UnspawnedMobError> {
        if !size_ok(self.mob.species(), Some(size)) {
            return Err(UnspawnedMobError::Invalid);
        }
        apply_size(&mut self.mob, size);
        Ok(())
    }

    fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

/// Constructs an unspawned pending child from `spec`, validating all domain invariants before
/// allocating or drawing RNG.
pub(super) fn construct_pending_mob(
    resources: &Arc<super::Resources>,
    attributes: &Arc<Catalog>,
    dimension: super::Dim,
    breeding: bool,
    spec: PendingMobSpec,
) -> Result<PendingMob, UnspawnedMobError> {
    if spec.dimension.to_string() != dimension.id()
        || !super::native_mobs::spawnable(spec.position)
        || !spec.yaw.is_finite()
        || (breeding && !spec.baby)
    {
        return Err(UnspawnedMobError::Invalid);
    }
    let kind = resources
        .mob_types()
        .get(&spec.kind.to_string())
        .ok_or(UnspawnedMobError::Invalid)?
        .clone();
    if (spec.baby && !finalize::supports_baby(kind.species))
        || !size_ok(kind.species, spec.size)
        || (kind.species == Species::SulfurCube && !spec.baby && spec.size == Some(1))
    {
        return Err(UnspawnedMobError::Invalid);
    }
    // The UUID and complete constructor RNG are retained without publishing persist data.
    let (random, uuid) = sunrise_client::mobs::living_random();
    let mut mob = Mob::with_random(
        0,
        kind,
        Vec3::new(spec.position[0], spec.position[1], spec.position[2]),
        spec.yaw,
        random,
    );
    finalize::set_baby(&mut mob, spec.baby);
    if let Some(size) = spec.size {
        apply_size(&mut mob, size);
    }
    Ok(PendingMob {
        mob,
        uuid: Some(uuid),
        attachments: Attachments::default(),
        dimension,
        resources: resources.clone(),
        attributes: attributes.clone(),
        breeding,
    })
}

/// Scoped single-slot editor backing [`mod_api::BabyEntitySpawnChild`] for `baby_entity_spawn`.
pub(super) struct BabyChildSlot<'a> {
    pub(super) slot: &'a mut Option<PendingMob>,
    pub(super) resources: Arc<super::Resources>,
    pub(super) attributes: Arc<Catalog>,
    pub(super) dimension: super::Dim,
}

impl BabyEntitySpawnSlot for BabyChildSlot<'_> {
    fn is_present(&self) -> bool {
        self.slot.is_some()
    }

    fn get(&self) -> Option<UnspawnedMobView<'_>> {
        self.slot.as_ref().map(|p| UnspawnedMobView::new(p))
    }

    fn get_mut(&mut self) -> Option<PendingChildEditor<'_>> {
        self.slot.as_mut().map(|p| PendingChildEditor::new(p))
    }

    fn construct(&self, spec: PendingMobSpec) -> Result<UnspawnedMob, UnspawnedMobError> {
        let pending = construct_pending_mob(
            &self.resources,
            &self.attributes,
            self.dimension,
            true,
            spec,
        )?;
        Ok(UnspawnedMob::new(Box::new(pending)))
    }

    fn replace(&mut self, child: UnspawnedMob) -> Result<Option<UnspawnedMob>, UnspawnedMobError> {
        let mut pending = child
            .into_any()
            .downcast::<PendingMob>()
            .map_err(|_| UnspawnedMobError::Invalid)?;
        if !valid_child(&self.resources, self.dimension, true, &pending) {
            return Err(UnspawnedMobError::Invalid);
        }
        pending.breeding = true;
        Ok(self
            .slot
            .replace(*pending)
            .map(|prev| UnspawnedMob::new(Box::new(prev))))
    }

    fn take(&mut self) -> Option<UnspawnedMob> {
        self.slot.take().map(|p| UnspawnedMob::new(Box::new(p)))
    }
}

/// Scoped unbounded list editor backing [`mod_api::MobSplitChildren`] for `mob_split`.
pub(super) struct SplitChildList<'a> {
    pub(super) children: &'a mut Vec<PendingMob>,
    pub(super) resources: Arc<super::Resources>,
    pub(super) attributes: Arc<Catalog>,
    pub(super) dimension: super::Dim,
}

impl MobSplitChildList for SplitChildList<'_> {
    fn len(&self) -> usize {
        self.children.len()
    }

    fn get(&self, index: usize) -> Option<UnspawnedMobView<'_>> {
        self.children.get(index).map(|p| UnspawnedMobView::new(p))
    }

    fn get_mut(&mut self, index: usize) -> Option<PendingChildEditor<'_>> {
        self.children
            .get_mut(index)
            .map(|p| PendingChildEditor::new(p))
    }

    fn construct(&self, spec: PendingMobSpec) -> Result<UnspawnedMob, UnspawnedMobError> {
        let pending = construct_pending_mob(
            &self.resources,
            &self.attributes,
            self.dimension,
            false,
            spec,
        )?;
        Ok(UnspawnedMob::new(Box::new(pending)))
    }

    fn push(&mut self, child: UnspawnedMob) -> Result<(), UnspawnedMobError> {
        let mut pending = child
            .into_any()
            .downcast::<PendingMob>()
            .map_err(|_| UnspawnedMobError::Invalid)?;
        if !valid_child(&self.resources, self.dimension, false, &pending) {
            return Err(UnspawnedMobError::Invalid);
        }
        pending.breeding = false;
        self.children.push(*pending);
        Ok(())
    }

    fn insert(&mut self, index: usize, child: UnspawnedMob) -> Result<(), UnspawnedMobError> {
        if index > self.children.len() {
            return Err(UnspawnedMobError::Invalid);
        }
        let mut pending = child
            .into_any()
            .downcast::<PendingMob>()
            .map_err(|_| UnspawnedMobError::Invalid)?;
        if !valid_child(&self.resources, self.dimension, false, &pending) {
            return Err(UnspawnedMobError::Invalid);
        }
        pending.breeding = false;
        self.children.insert(index, *pending);
        Ok(())
    }

    fn replace(
        &mut self,
        index: usize,
        child: UnspawnedMob,
    ) -> Result<UnspawnedMob, UnspawnedMobError> {
        if index >= self.children.len() {
            return Err(UnspawnedMobError::Invalid);
        }
        let mut pending = child
            .into_any()
            .downcast::<PendingMob>()
            .map_err(|_| UnspawnedMobError::Invalid)?;
        if !valid_child(&self.resources, self.dimension, false, &pending) {
            return Err(UnspawnedMobError::Invalid);
        }
        pending.breeding = false;
        let prev = std::mem::replace(&mut self.children[index], *pending);
        Ok(UnspawnedMob::new(Box::new(prev)))
    }

    fn remove(&mut self, index: usize) -> Option<UnspawnedMob> {
        if index < self.children.len() {
            Some(UnspawnedMob::new(Box::new(self.children.remove(index))))
        } else {
            None
        }
    }

    fn clear(&mut self) {
        self.children.clear();
    }
}

/// Whether a final child satisfies every object invariant: a registered kind, the action's
/// dimension, a finite spawnable position and yaw, baby state the species supports (a
/// breeding child must be one) and a size the species allows.
fn valid_child(
    resources: &super::Resources,
    dimension: super::Dim,
    breeding: bool,
    child: &PendingMob,
) -> bool {
    let mob = &child.mob;
    let species = mob.species();
    child.dimension == dimension
        && resources.mob_types().contains_key(&mob.kind.id)
        && super::native_mobs::spawnable([mob.position.x, mob.position.y, mob.position.z])
        && [
            mob.yaw,
            mob.previous_yaw,
            mob.body_yaw,
            mob.previous_body_yaw,
            mob.head_yaw,
            mob.previous_head_yaw,
        ]
        .into_iter()
        .all(f32::is_finite)
        && (!breeding || mob.is_baby())
        && (!mob.is_baby() || finalize::supports_baby(species))
        && match species {
            Species::Slime | Species::MagmaCube | Species::SulfurCube | Species::Phantom => {
                u32::try_from(mob.extra.size).is_ok_and(|size| size_ok(species, Some(size)))
            }
            _ => true,
        }
}

fn size_ok(species: Species, size: Option<u32>) -> bool {
    match (species, size) {
        (Species::Slime | Species::MagmaCube | Species::SulfurCube, Some(size)) => {
            i32::try_from(size).is_ok_and(|size| sunrise_client::mobs::cube::SIZES.contains(&size))
        }
        (Species::Phantom, Some(size)) => i32::try_from(size)
            .is_ok_and(|size| sunrise_client::mobs::phantom::SIZES.contains(&size)),
        (Species::Slime | Species::MagmaCube | Species::SulfurCube | Species::Phantom, None) => {
            false
        }
        (_, None) => true,
        (_, Some(_)) => false,
    }
}

fn apply_size(mob: &mut Mob, size: u32) {
    match mob.species() {
        Species::Slime | Species::MagmaCube => {
            sunrise_client::mobs::cube::set_size(mob, size as i32)
        }
        Species::SulfurCube => sunrise_client::mobs::sulfur_cube::set_size(mob, size as i32, true),
        Species::Phantom => sunrise_client::mobs::phantom::set_size(mob, size as i32),
        _ => unreachable!("species size checked before mutation"),
    }
}

#[cfg(test)]
#[path = "native_pending_children/tests.rs"]
mod tests;
