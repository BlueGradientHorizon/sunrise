pub trait DrawContexts {
    #[draw_context(
        surface = "crate::world_geometry::WorldGeometry",
        name = "WorldDrawContext"
    )]
    fn world_draw_context();
}
pub trait Hooks {
    /// Submit unlit geometry after particle submission, before opaque rendering
    /// (NeoForge 26.3 `SubmitCustomGeometryEvent`, vanilla `LevelRenderer.submitFeatures`).
    /// Client-only, noncancellable. `view` holds only camera, partial tick and game time,
    /// not full extracted level state or visible sections. Submit from extracted state.
    /// A listener panic disables that mod's listener and reports it by name. Geometry
    /// already submitted to this view is not rolled back.
    #[notes = r#"## Custom world geometry

`Hooks::submit_custom_geometry` runs on the client after particles, before rendering. Its
`WorldRenderView` holds camera, partial tick and game time; `WorldDrawContext::lines` and `quads`
accept unlit world-space ARGB geometry with depth testing or through walls. `lines` and `quads`
fail atomically for invalid inputs, and `lines` enforces a fixed per-view limit of 4,096
(`usage().line_capacity`). `quads`, `pipeline_quads`, and `name_tag` count toward
`usage().quads` and grow dynamically into the GPU buffers with no fixed per-view cap;
`pipeline_quads` checks `RenderPipelineRef` validity and staleness
(`PipelineDrawError::Pipeline`) and skips any quad with non-finite coordinates or UVs
individually without rejecting valid quads in the same slice. Output clears every view. Submit
extracted state, without world writes or resource loading. A listener panic is logged and shown in
an error toast naming the mod, and disables that listener for the session. Geometry it already
submitted stays in the current view; the next listener can still submit geometry.

`WorldDrawContext::draw_model_instances` draws one batch of a resolved `ModelRef`. Each
`ModelInstance` carries an f64 world origin, a row-major 3×4 local transform, an `0xAARRGGBB`
tint (the same convention as `WorldQuad::color`) and `LightCoords` block and sky light, each
0 to 15, in vanilla's own convention. The engine converts tint and light to the frame format;
mods never assume that format or supply a packed light word. The view holds 16,384 instances,
counted by `usage().instances_used` against `instances_capacity`. A refused batch draws nothing.
`ModelInstance::color` (`0xAARRGGBB`, white by default) multiplies every face's colour; with alpha below `0xFF`, or for a model with a translucent face, the copy is drawn in the sorted translucent material (vanilla's `translucent_moving_block`), its quads far to near. `ModelInstance::hidden_faces` leaves out faces whose `cull` side is set.
Copies outside the frustum are omitted and do not count. Shader packs and any route that cannot
take GPU instances bake the same prepared geometry on the CPU instead of dropping the model; a
`shaders/` pack shades those copies as block entities, as it shades `BlockEntityDrawContext::model`.

`WorldDrawContext::name_tag(at, &text, NameTagOptions::new(see_through, background, light))` draws
a label as vanilla draws a name tag (`submitNameTag`): 0.5 above `at`, facing the camera, 0.025
blocks a font pixel, so it shrinks with distance (scale by distance yourself for a constant size),
centred, in the text's styles. `background` puts the black backdrop behind it at the player's
text background opacity; `light` is the light at the label, sky light in the high four bits and
block light in the low four. Without `see_through` it is drawn once, depth tested and translucent:
the faded text on the backdrop, in `light` as given, hidden behind what is in front. With
`see_through` it is drawn twice: opaque in the world with no backdrop, its sky and block light each
raised to at least 2, and faded on the backdrop over everything, walls included, unlit. The
backdrop's alpha is 255 times the text background opacity (0.25 when it is set for chat only), the
faded text's 255 times the larger of (opacity + 0.75) / 2 and 0.5. Labels draw with the world's
name tags whatever the stage, sorted far to near. Each glyph and backdrop quad, both copies
counted, is counted in `usage().quads` with no fixed per-view cap. A position that is not
finite (`DrawError::NonFinite`), text deeper than 512 component nodes (`DrawError::TextTooDeep`)
or otherwise refused (`DrawError::TextRefused`) is refused whole: nothing is drawn. `see_through` can show a
label through any wall, which vanilla does only for the names its own renderers choose; the call
sends nothing and reads nothing the client does not already hold."#]
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "SubmitCustomGeometryEvent",
        cancellable = false,
        version = 2
    )]
    fn submit_custom_geometry(view: &mod_api::WorldRenderView, draw: &mut dyn WorldDrawContext);
}
