//! Parts whose exports carry more than one import: `can_equip` is asked through a prepared
//! entry of its own, and `explosion_resistance` is asked with the part's purity declaration.
use mod_api::{BlockPart, CanEquipEvent, ExplosionResistanceEvent, Host, ItemPart, LevelHost};

pub struct Equip;
#[mod_api::sunrise_part]
impl ItemPart for Equip {
    fn can_equip(&self, _: &dyn Host, event: &mut CanEquipEvent<'_>) {
        let _ = event;
    }
}

pub struct Resistance;
#[mod_api::sunrise_part]
impl BlockPart for Resistance {
    // Declared once per target, the other target's first: the purity mark follows the declaration
    // that is compiled, not the first one written.
    #[cfg(not(target_arch = "wasm32"))]
    fn explosion_resistance(
        &self,
        _: &dyn LevelHost,
        _: &mut ExplosionResistanceEvent<'_>,
    ) -> Option<f32> {
        None
    }
    #[cfg(target_arch = "wasm32")]
    fn explosion_resistance(
        &self,
        _: &dyn LevelHost,
        _: &mut ExplosionResistanceEvent<'_>,
    ) -> Option<f32> {
        None
    }
}

pub fn register(r: &mut dyn mod_api::Registrar, cx: &mod_api::RegistrationContext) {
    if cx.mods().get("export_marks_probe").is_none() {
        return;
    }
    let helmet = r.item_key("minecraft:iron_helmet");
    r.item_part(helmet, Equip);
    let stone = r.block_key("minecraft:stone");
    r.block_part(stone, Resistance);
}
