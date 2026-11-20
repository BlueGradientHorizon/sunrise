#!/usr/bin/env python3
"""Change render-layer storage to admit shared palettes and particle names (debt 63)."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src"


def rewrite(name, *edits):
    path = ROOT / name
    text = path.read_text()
    for old, new, count in edits:
        assert text.count(old) == count, (name, old)
        text = text.replace(old, new)
    path.write_text(text)


def main():
    rewrite(
        "item_render.rs",
        (
            "/// `ItemStackRenderState.LayerRenderState`.",
            """/// Tints computed for an ordinary layer, or a palette shared by special submissions.
#[derive(Clone, Debug)]
pub enum LayerTints {
    Owned(Vec<i32>),
    Shared(Arc<[i32]>),
}
impl LayerTints {
    fn as_slice(&self) -> &[i32] {
        match self {
            Self::Owned(tints) => tints,
            Self::Shared(tints) => tints,
        }
    }
}
/// `ItemStackRenderState.LayerRenderState`.""",
            1,
        ),
        ("pub tints: Vec<i32>,", "pub tints: LayerTints,", 1),
        ("pub particle: String,", "pub particle: Arc<str>,", 1),
        (".tints\n                        .get", ".tints.as_slice()\n                        .get", 1),
        (".tints\n                .get", ".tints.as_slice()\n                .get", 1),
        ("tints: layer.tints,", "tints: LayerTints::Owned(layer.tints),", 1),
        (
            "particle: sprite_name(&baked.particle.sprite).to_owned(),",
            "particle: Arc::from(sprite_name(&baked.particle.sprite)),",
            1,
        ),
        ("tints: Vec::new(),", "tints: LayerTints::Owned(Vec::new()),", 1),
        ("particle: String::new(),", 'particle: Arc::from(""),', 1),
    )
    rewrite(
        "item_render/registered_special.rs",
        ("LayerGeometry, LayerRender, corners", "LayerGeometry, LayerRender, LayerTints, corners", 1),
        ("tints: tints.clone(),", "tints: LayerTints::Shared(tints.clone()),", 1),
        ("tints: Vec<i32>,", "tints: Arc<[i32]>,", 1),
        (
            "MISSING_MODEL,\n                    Vec::new(),",
            "MISSING_MODEL,\n                    Arc::default(),",
            1,
        ),
        ("part(model, p.tints.clone(), p.pose)", "part(model, p.tints.as_slice().into(), p.pose)", 1),
        (
            "particle: sprite_name(&base.particle.sprite).to_owned(),",
            "particle: Arc::from(sprite_name(&base.particle.sprite)),",
            1,
        ),
    )
    rewrite(
        "item_render/gui.rs", ("layer.tints.clone(),", "layer.tints.as_slice().to_vec(),", 1)
    )
    rewrite(
        "item_render/glint_tests.rs",
        ("tints: vec![],", "tints: LayerTints::Owned(Vec::new()),", 1),
        (".tints = vec![0x7F_FF_60_11];", ".tints = LayerTints::Owned(vec![0x7F_FF_60_11]);", 1),
    )
    rewrite(
        "item_render/special.rs",
        ("LayerGeometry, LayerRender, NoHost", "LayerGeometry, LayerRender, LayerTints, NoHost", 1),
        ("tints: Vec::new(),", "tints: LayerTints::Owned(Vec::new()),", 1),
        ("particle: String::new(),", 'particle: "".into(),', 1),
    )
    rewrite(
        "item_render/special_parts_tests.rs", ("two.layers[0].particle,", "two.layers[0].particle.as_ref(),", 1)
    )


if __name__ == "__main__":
    main()
