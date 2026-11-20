#!/usr/bin/env python3
"""Route item resolution through a layer visitor and isolate registered-special recipes."""

import subprocess
from pathlib import Path


def replace(text: str, old: str, new: str, count: int = 1) -> str:
    if text.count(old) != count:
        raise SystemExit(f"Expected {count} matches for {old!r}")
    return text.replace(old, new)


def item_layer_traversal(source: str) -> str:
    source = replace(
        source,
        "/// once (`item_render::PreparedSpecial` in the client), so a draw looks nothing up by name. Kept\n"
        "/// as long as the answer is; equal only to itself.",
        "/// once (`item_render::registered_special::PreparedSpecial` in the client), so a draw looks\n"
        "/// nothing up by name. Kept as long as the answer is; equal only to itself.",
    )
    source = replace(
        source,
        '''        env: &dyn ItemEnv,
    ) {
        let components = self.components_view(stack);
        let Some(Value::String(model_id)) = components.get("minecraft:item_model") else {
            return;
        };
        let id = identifier(model_id).unwrap_or_else(|_| model_id.clone());
        let definition = self.definitions.get(&id);
        out.oversized_in_gui = definition.is_some_and(|d| d.oversized_in_gui);
''',
        '''        env: &dyn ItemEnv,
    ) {
        if let Some(oversized_in_gui) =
            self.visit_layers(stack, ctx, env, LayerTransform::Model, &mut |layer, _| {
                out.layers.push(layer);
            })
        {
            out.oversized_in_gui = oversized_in_gui;
        }
    }
    /// Visit each layer in draw order with the stack it resolved from (which may be a bundle's
    /// selected stack). An inherited [`LayerTransform::None`] suppresses the model transform
    /// for every descendant. Returns the final `oversized_in_gui`, or `None` if the stack has
    /// no `minecraft:item_model` string. Like [`Self::append_layers`], this does not filter
    /// empty stacks.
    pub fn visit_layers(
        &self,
        stack: &StackView,
        ctx: &Context,
        env: &dyn ItemEnv,
        display_transform: LayerTransform,
        visit: &mut impl FnMut(Layer, &StackView),
    ) -> Option<bool> {
        let components = self.components_view(stack);
        let Some(Value::String(model_id)) = components.get("minecraft:item_model") else {
            return None;
        };
        let id = identifier(model_id).unwrap_or_else(|_| model_id.clone());
        let definition = self.definitions.get(&id);
''',
    )
    source = replace(
        source,
        '''            env,
        };
        match definition {
            Some(d) => state.node(out, &d.model, &Mat4::IDENTITY),
            None => state.missing(out, &Mat4::IDENTITY),
        }
''',
        '''            env,
            visit,
            oversized_in_gui: definition.is_some_and(|d| d.oversized_in_gui),
        };
        match definition {
            Some(d) => state.node(&d.model, &Mat4::IDENTITY, display_transform),
            None => state.missing(&Mat4::IDENTITY, display_transform),
        }
        Some(state.oversized_in_gui)
''',
    )
    source = replace(source, "struct Resolve<'a> {", "struct Resolve<'a, 'visit, F> {")
    source = replace(
        source,
        "    env: &'a dyn ItemEnv,\n}",
        "    env: &'a dyn ItemEnv,\n    visit: &'visit mut F,\n    oversized_in_gui: bool,\n}",
    )
    source = replace(
        source,
        "impl Resolve<'_> {",
        "impl<F: FnMut(Layer, &StackView)> Resolve<'_, '_, F> {",
    )

    start = source.index("    fn missing(&self, out: &mut ResolvedItem,")
    end = source.index("    fn item_in_compass_tag", start)
    traversal = source[start:end]
    traversal = replace(
        traversal,
        "    fn missing(&self, out: &mut ResolvedItem, transform: &Mat4) {",
        """    fn emit(&mut self, layer: Layer) {
        (self.visit)(layer, self.stack);
    }
    fn missing(&mut self, transform: &Mat4, display_transform: LayerTransform) {""",
    )
    traversal = replace(
        traversal,
        "    fn node(&mut self, out: &mut ResolvedItem, node: &ItemModelNode, parent: &Mat4) {",
        """    fn node(&mut self, node: &ItemModelNode, parent: &Mat4, display_transform: LayerTransform) {""",
    )
    traversal = replace(traversal, "out.layers.push(Layer {", "self.emit(Layer {", 5)
    traversal = replace(traversal, "display_transform: LayerTransform::Model,", "display_transform,", 5)
    for callee, args, count in (
        ("node", "m, &child", 3),
        ("node", "chosen, &child", 1),
        ("node", "f, &child", 2),
        ("missing", "&child", 2),
        ("fluid_container", "container, parent", 1),
        ("sprite_layer", "base, None, Vec::new(), parent", 1),
        ("sprite_layer", "&appearance.still, Some(mask), vec![tint], &parent", 1),
        ("sprite_layer", "drawn, Some(cover), Vec::new(), &parent", 1),
    ):
        traversal = replace(
            traversal,
            f"self.{callee}(out, {args})",
            f"self.{callee}({args}, display_transform)",
            count,
        )
    traversal = replace(
        traversal,
        '''                if let Some(selected) = self.bundle_selected() {
                    self.models
                        .append_layers(out, &selected, self.ctx, self.env);
                }''',
        '''                if let Some(selected) = self.bundle_selected()
                    && let Some(oversized_in_gui) = self.models.visit_layers(
                        &selected,
                        self.ctx,
                        self.env,
                        display_transform,
                        self.visit,
                    )
                {
                    self.oversized_in_gui = oversized_in_gui;
                }''',
    )
    traversal = replace(
        traversal,
        '''                        let first = out.layers.len();
                        self.node(out, model, &child);
                        // The layer's choice wins over what the model chose below it; the
                        // model's own transform is kept only where no layer above refused it.
                        if choice.transform == LayerTransform::None {
                            for layer in &mut out.layers[first..] {
                                layer.display_transform = LayerTransform::None;
                            }
                        }''',
        '''                        // The model's own transform is kept only where no layer above refused it.
                        let display_transform = if display_transform == LayerTransform::None {
                            display_transform
                        } else {
                            choice.transform
                        };
                        self.node(model, &child, display_transform);''',
    )
    traversal = replace(
        traversal,
        "    fn fluid_container(&self, out: &mut ResolvedItem, container: &FluidContainer, parent: &Mat4) {",
        """    fn fluid_container(
        &mut self,
        container: &FluidContainer,
        parent: &Mat4,
        display_transform: LayerTransform,
    ) {""",
    )
    traversal = replace(
        traversal,
        "        &self,\n        out: &mut ResolvedItem,\n        sprite: &str,",
        "        &mut self,\n        sprite: &str,",
    )
    traversal = replace(
        traversal,
        "        parent: &Mat4,\n    ) {\n        self.emit(Layer {",
        "        parent: &Mat4,\n        display_transform: LayerTransform,\n    ) {\n        self.emit(Layer {",
    )
    return source[:start] + traversal + source[end:]


def extract_registered_special(source: str) -> tuple[str, str]:
    start = source.index("    /// A special model renderer's answer `parts`")
    end = source.index("    /// `ItemModelResolver.updateForTopItem`.", start)
    methods = source[start:end]
    source = source[:start] + source[end:]
    start = source.index("/// A special model renderer's answer as [`ItemRenderer::prepare_parts`]")
    end = source.index("/// A `sunrise-assets` (row-major, JOML-exact) matrix as glam's.", start)
    state = source[start:end]
    source = source[:start] + source[end:]

    source = replace(source, "pub mod gui;\nmod special;", "pub mod gui;\nmod registered_special;\nmod special;")
    source = replace(source, "use glam::{Mat3, Mat4, Vec3};", "use glam::{Mat3, Mat4, Vec3};\nuse registered_special::{NESTING, ResolveDepth};")
    source = replace(
        source,
        "        LayerTransform, LevelState, OwnerState, PreparedParts, SpecialContent, SpecialModel,\n        StackView,",
        "        LevelState, OwnerState, SpecialModel, StackView,",
    )
    methods = replace(methods, "    fn push_parts(", "    pub(super) fn push_parts(")
    state = replace(state, "struct Nesting {", "pub(super) struct Nesting {")
    state = replace(state, "    depth: u32,", "    pub(super) depth: u32,")
    state = replace(state, "    refused: bool,", "    pub(super) refused: bool,")
    state = replace(state, "    static NESTING:", "    pub(super) static NESTING:")
    state = replace(state, "struct ResolveDepth;", "pub(super) struct ResolveDepth;")
    state = replace(state, "    fn enter() -> Self {", "    pub(super) fn enter() -> Self {")
    module = '''//! Preparation and expansion of registered special item renderers' recipes.
use super::{ItemRenderState, ItemRenderer, LayerGeometry, LayerRender, glam_matrix, sprite_name};
use glam::Vec3;
use std::sync::{Arc, Mutex};
use sunrise_assets::{
    item_bake::BakedItemModel,
    item_model::{
        DisplayContext, FoilType, ItemEnv, LayerTransform, LevelState, OwnerState, PreparedParts,
        SpecialContent, StackView,
    },
    resolve::MISSING_MODEL,
};

impl ItemRenderer {
''' + methods + "}\n\n" + state
    return source, module


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    assets = root / "crates/sunrise-assets/src/item_model.rs"
    renderer = root / "crates/sunrise-client/src/item_render.rs"
    target = renderer.with_suffix("") / "registered_special.rs"
    if target.exists():
        raise SystemExit(f"Already exists: {target}")

    traversal = item_layer_traversal(assets.read_text(encoding="utf-8"))
    parent, module = extract_registered_special(renderer.read_text(encoding="utf-8"))
    for path, source in ((assets, traversal), (renderer, parent), (target, module)):
        path.write_text(source, encoding="utf-8")
    subprocess.run(
        ["rustfmt", "--edition", "2024", "--config", "skip_children=true", assets, renderer, target],
        check=True,
    )


if __name__ == "__main__":
    main()
