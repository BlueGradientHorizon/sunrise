//! Server text displays: metadata, interpolation, billboard transforms and world text.
use super::Game;
use sunrise_assets::item_model::DisplayContext;
use sunrise_client::{
    gfx::{TEXT_POLYGON_OFFSET_OVERLAY, TEXT_POLYGON_OFFSET_SOLID_OVERLAY},
    items::ItemStack,
    mesher::Vertex,
    text::Text,
    text_display::{self, CachedQuads, TextDisplayData},
};
use sunrise_protocol::entity_data::Value;
use sunrise_world::registry::BlockStateId;

/// What a display shows besides its transformation: its text (in `data`),
/// a block state (`BlockDisplay`) or an item and how it is displayed
/// (`ItemDisplay`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Shows {
    Text,
    Block,
    Item,
}

impl Shows {
    /// The display's entity type, as the tick hooks name it.
    fn kind(self) -> &'static str {
        match self {
            Self::Text => "minecraft:text_display",
            Self::Block => "minecraft:block_display",
            Self::Item => "minecraft:item_display",
        }
    }
}

pub(super) struct RemoteTextDisplay {
    pub data: TextDisplayData,
    shows: Shows,
    /// Pending synced sub-state fields (`BlockDisplay` id 23, `ItemDisplay` ids 23-24).
    pending_block: Option<BlockStateId>,
    pending_item: Option<ItemStack>,
    pending_item_display: u8,
    /// Committed sub-state updated inside `tick()` only when `dirty` (`updateRenderSubState`).
    shown_block: Option<BlockStateId>,
    shown_item: Option<ItemStack>,
    shown_item_display: u8,
    from: TextDisplayData,
    shown: TextDisplayData,
    position: glam::DVec3,
    previous: glam::DVec3,
    target: glam::DVec3,
    rotation: [f32; 2],
    old_rotation: [f32; 2],
    target_rotation: [f32; 2],
    /// `LinearInterpolationHandler` length (raw client synced id 10, unclamped) and steps left.
    interp_length: i32,
    steps: i32,
    age: i64,
    start: i64,
    duration: i32,
    dirty: bool,
    start_dirty: bool,
    duration_dirty: bool,
    /// `Display.renderState != null`: false until the first tick after a render-state sync.
    render_state_ready: bool,
    /// `Display.lastProgress`: the progress the last drawn frame evaluated (untouched when culled
    /// or while `duration <= 0`).
    last_progress: f32,
    /// `Entity.DATA_SHARED_FLAGS_ID` (id 0): bit 0 = on fire, bit 5 = invisible.
    shared_flags: u8,
    /// `Entity.DATA_CUSTOM_NAME` (id 2) and `DATA_CUSTOM_NAME_VISIBLE` (id 3).
    custom_name: Option<Text>,
    custom_name_visible: bool,
    /// Local text geometry from the last visited frame; world poses do not
    /// invalidate `TextDisplay.clientDisplayCache` or the glyph vertices.
    lines: Option<CachedQuads>,
    /// `Entity.vehicle`: the mob it rides (`SetPassengers`), which places it
    /// every tick (`positionRider`), as NPC plugins hang a name over a head.
    pub(super) vehicle: Option<u32>,
}

/// `Display.RENDER_STATE_IDS` and each subclass's render-state synced data IDs (not view range,
/// width, height, or glow colour).
fn renders(shows: Shows, id: u8) -> bool {
    let shared = match id {
        11..=16 | 18 | 19 => true,
        23..=27 => shows == Shows::Text,
        _ => false,
    };
    shared
        || match shows {
            Shows::Block => id == 23,
            Shows::Item => matches!(id, 23 | 24),
            Shows::Text => false,
        }
}

/// `Mth.wrapDegrees(float)`.
fn wrap_degrees(value: f32) -> f32 {
    let mut f = value % 360.0;
    if f >= 180.0 {
        f -= 360.0;
    }
    if f < -180.0 {
        f += 360.0;
    }
    f
}

/// `Mth.rotLerp(float, float, float)`.
fn rot_lerp(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * wrap_degrees(end - start)
}

/// `Entity.setYRot(float)`.
fn set_y_rot(yaw: f32) -> f32 {
    if yaw.is_finite() { yaw % 360.0 } else { 0.0 }
}

/// `Entity.setXRot(float)`.
fn set_x_rot(pitch: f32) -> f32 {
    if pitch.is_finite() {
        pitch.clamp(-90.0, 90.0) % 360.0
    } else {
        0.0
    }
}

impl RemoteTextDisplay {
    pub fn new(shows: Shows, position: [f64; 3], yaw: f32) -> Self {
        let yaw = set_y_rot(yaw);
        Self {
            data: Default::default(),
            shows,
            pending_block: None,
            pending_item: None,
            pending_item_display: 0,
            shown_block: None,
            shown_item: None,
            shown_item_display: 0,
            from: Default::default(),
            shown: Default::default(),
            position: position.into(),
            previous: position.into(),
            target: position.into(),
            rotation: [yaw, 0.0],
            old_rotation: [yaw, 0.0],
            target_rotation: [yaw, 0.0],
            interp_length: 0,
            steps: 0,
            age: 0,
            // Display.interpolationStartClientTick starts at Integer.MIN_VALUE so duration > 0
            // without a start-tick update snaps to progress 1.0.
            start: i64::from(i32::MIN),
            duration: 0,
            dirty: false,
            start_dirty: false,
            duration_dirty: false,
            render_state_ready: false,
            last_progress: 0.0,
            shared_flags: 0,
            custom_name: None,
            custom_name_visible: false,
            lines: None,
            vehicle: None,
        }
    }
    /// Sets the initial spawn pitch (`Entity.absSnapTo` / `recreateFromPacket`).
    pub(super) fn with_pitch(mut self, pitch: f32) -> Self {
        let pitch = set_x_rot(pitch);
        self.rotation[1] = pitch;
        self.old_rotation[1] = pitch;
        self.target_rotation[1] = pitch;
        self
    }
    /// Where a passenger of this display sits, this tick and the last
    /// (`Display`'s box is 0 by 0, so its passenger attachment is its feet).
    pub(super) fn seat(&self) -> (glam::DVec3, glam::DVec3) {
        (self.position, self.previous)
    }
    /// `Entity.positionRider` through `setPos`: where its vehicle seats it this
    /// tick. The previous position stays the last tick's, so it moves smoothly
    /// with the vehicle; a teleport's interpolation no longer applies.
    pub(super) fn ride(&mut self, at: glam::DVec3) {
        self.position = at;
        self.target = at;
        self.steps = 0;
    }
    pub(super) fn position(&self) -> glam::DVec3 {
        self.position
    }
    pub(super) fn render_position(&self, partial: f64) -> glam::DVec3 {
        self.previous.lerp(self.position, partial)
    }
    pub(super) fn has_render_state(&self) -> bool {
        self.render_state_ready
    }
    pub(super) fn custom_name(&self) -> Option<&Text> {
        self.custom_name.as_ref()
    }
    pub(super) fn custom_name_visible(&self) -> bool {
        self.custom_name_visible
    }
    pub(super) fn kind(&self) -> &'static str {
        self.shows.kind()
    }
    fn progress(&self, partial: f32) -> f32 {
        if self.duration <= 0 {
            1.0
        } else {
            let elapsed = self.age.saturating_sub(self.start);
            ((elapsed as f32 + partial) / self.duration as f32).clamp(0.0, 1.0)
        }
    }
    /// `Display.calculateInterpolationProgress`: only updates `last_progress` when `duration > 0`.
    fn calculate_interpolation_progress(&mut self, partial: f32) -> f32 {
        if self.duration <= 0 {
            return 1.0;
        }
        let progress = self.progress(partial);
        self.last_progress = progress;
        progress
    }
    /// The casters this display adds to the shadow pass: its interpolated
    /// shadow radius and strength (`DisplayRenderer.getShadowRadius` and
    /// `DisplayRenderer.getShadowStrength`), suppressed before first tick or when invisible.
    pub(super) fn shadow(&self, partial: f64) -> Option<(glam::DVec3, f32, f32)> {
        if !self.render_state_ready || self.shared_flags & 0x20 != 0 {
            return None;
        }
        sunrise_client::shadows::display_caster(
            self.previous.lerp(self.position, partial),
            &self.from,
            &self.shown,
            self.progress(partial as f32),
        )
    }
    /// The entries after `Display`'s: a block state, or an item with its
    /// display context (the stack comes converted by the caller). Changes defer to `tick()`.
    pub fn show(&mut self, values: &[(u8, i32, Value)], item: Option<Option<ItemStack>>) {
        for (id, _, value) in values {
            match (self.shows, id, value) {
                (Shows::Block, 23, Value::Id(state)) => {
                    self.pending_block = Some(BlockStateId(u32::try_from(*state).unwrap_or(0)));
                    self.dirty = true;
                }
                (Shows::Item, 24, Value::Byte(context)) => {
                    self.pending_item_display = *context as u8;
                    self.dirty = true;
                }
                _ => {}
            }
        }
        if let Some(item) = item
            && self.shows == Shows::Item
        {
            self.pending_item = item;
            self.dirty = true;
        }
    }
    pub fn metadata(&mut self, values: &[(u8, i32, Value)]) {
        for (id, _, value) in values {
            match (id, value) {
                (0, Value::Byte(v)) => self.shared_flags = *v as u8,
                (2, Value::OptText(Some(tag))) | (2, Value::Text(tag)) => {
                    self.custom_name = Some(sunrise_client::entity_nbt::name_text(tag));
                }
                (2, Value::OptText(None)) => self.custom_name = None,
                (3, Value::Bool(v)) => self.custom_name_visible = *v,
                (8, _) => self.start_dirty = true,
                (9, _) => self.duration_dirty = true,
                (10, Value::Int(v)) => self.interp_length = *v,
                (id, _) if renders(self.shows, *id) => self.dirty = true,
                _ => {}
            }
        }
        self.data.apply(values);
    }
    fn tick(&mut self) {
        self.age += 1;
        self.previous = self.position;
        self.old_rotation = self.rotation;
        if self.steps > 0 {
            let alpha = 1.0 / f64::from(self.steps);
            self.position = self.position.lerp(self.target, alpha);
            let alpha_f32 = alpha as f32;
            self.rotation[0] = set_y_rot(rot_lerp(
                alpha_f32,
                self.rotation[0],
                self.target_rotation[0],
            ));
            self.rotation[1] = set_x_rot(
                self.rotation[1] + alpha_f32 * (self.target_rotation[1] - self.rotation[1]),
            );
            self.steps -= 1;
        }
        if self.start_dirty {
            self.start = self.age + i64::from(self.data.interpolation_delay);
        }
        if self.duration_dirty {
            self.duration = self.data.interpolation_duration;
        }
        if self.dirty {
            self.from = if self.render_state_ready && self.duration > 0 {
                self.shown.blend(&self.from, self.last_progress)
            } else {
                self.data.clone()
            };
            self.shown = self.data.clone();
            self.shown_block = self.pending_block;
            self.shown_item = self.pending_item.clone();
            self.shown_item_display = self.pending_item_display;
            self.render_state_ready = true;
        }
        self.dirty = false;
        self.start_dirty = false;
        self.duration_dirty = false;
    }
    /// `LinearInterpolationHandler.interpolateTo`: 0 snaps immediately, negative is a no-op,
    /// positive skips resetting remaining steps when the target is unchanged.
    fn move_to_opt(&mut self, position: Option<[f64; 3]>, rotation: Option<(f32, f32)>) {
        if self.interp_length == 0 {
            if let Some(pos) = position {
                let p = glam::DVec3::from(pos);
                self.position = p;
                self.previous = p;
                self.target = p;
            }
            if let Some((yaw, pitch)) = rotation {
                let rot = [set_y_rot(yaw), set_x_rot(pitch)];
                self.rotation = rot;
                self.old_rotation = rot;
                self.target_rotation = rot;
            }
            self.steps = 0;
            return;
        }
        if self.interp_length < 0 {
            self.steps = 0;
            return;
        }
        let next_pos = position.map(glam::DVec3::from).unwrap_or_else(|| {
            if self.steps > 0 {
                self.target
            } else {
                self.position
            }
        });
        let next_rot = rotation.map(|(y, p)| [y, p]).unwrap_or_else(|| {
            if self.steps > 0 {
                self.target_rotation
            } else {
                self.rotation
            }
        });
        if self.steps > 0 && next_pos == self.target && next_rot == self.target_rotation {
            return;
        }
        self.target = next_pos;
        self.target_rotation = next_rot;
        self.steps = self.interp_length;
    }
    fn move_to(&mut self, position: [f64; 3], rotation: Option<(f32, f32)>) {
        self.move_to_opt(Some(position), rotation);
    }
}

impl Game<'_> {
    pub(super) fn move_remote_text_display(
        &mut self,
        id: i32,
        position: [f64; 3],
        rotation: Option<(f32, f32)>,
    ) {
        if let Some(d) = self.text_displays.get_mut(&id) {
            d.move_to(position, rotation);
        }
    }
    pub(super) fn move_remote_text_display_opt(
        &mut self,
        id: i32,
        position: Option<[f64; 3]>,
        rotation: Option<(f32, f32)>,
    ) {
        if let Some(d) = self.text_displays.get_mut(&id) {
            d.move_to_opt(position, rotation);
        }
    }
    /// The displays' tick on a client. EntityTickEvent: once for the family,
    /// or each alone of a type a mod named, right before and right after its own
    /// tick; a canceled one keeps its state.
    pub(super) fn tick_text_displays(&mut self) {
        let mut round = self.native_tick_round();
        if !round.listening() {
            for display in self.text_displays.values_mut() {
                display.tick();
            }
            return;
        }
        let mut alone = Vec::new();
        for (&id, display) in &self.text_displays {
            if round.alone_kind(display.shows.kind()) {
                alone.push((id, display.shows.kind()));
            } else {
                round.add(
                    id as u32,
                    super::native_ticks::TickKind::Fixed(display.shows.kind()),
                );
            }
        }
        self.native_tick_pre_batch(&mut round);
        for (&id, display) in &mut self.text_displays {
            if !round.alone_kind(display.shows.kind()) && !round.canceled(id as u32) {
                display.tick();
            }
        }
        for (id, kind) in alone {
            if self.native_tick_pre_one(&mut round, id as u32, kind) {
                if let Some(display) = self.text_displays.get_mut(&id) {
                    display.tick();
                }
                self.native_tick_post_one(&round, id as u32, kind);
            }
        }
        self.native_tick_post_batch(round);
    }
    /// Active render `Camera` yaw and pitch (`DisplayRenderer.extractRenderState`), sharing the
    /// same camera source of truth as `particle_rotation` (`ThirdPersonFront` already baked in).
    fn display_camera_angles(&self) -> [f32; 2] {
        if self.studio.flying() {
            return [self.studio.freecam.yaw, self.studio.freecam.pitch];
        }
        let (yaw, pitch) = self
            .posed_rotation
            .unwrap_or((self.view.y_rot, self.view.x_rot));
        [yaw, pitch]
    }
    /// `DisplayRenderer.getBlockLightLevel` and `getSkyLightLevel`: the
    /// brightness override, else the light at the entity (block light 15 when on fire).
    fn display_light(
        world: &sunrise_client::world::World,
        d: &TextDisplayData,
        position: glam::DVec3,
        on_fire: bool,
    ) -> u32 {
        if d.brightness == -1 {
            let mut world_light = world.light(
                position.x.floor() as i32,
                position.y.floor() as i32,
                position.z.floor() as i32,
            );
            if on_fire {
                world_light |= 0x0F;
            }
            sunrise_client::block_entity_render::vertex_light(world_light)
        } else {
            ((d.brightness as u32 >> 20 & 15) * 16) | (((d.brightness as u32 >> 4 & 15) * 16) << 8)
        }
    }
    /// `Display.shouldRenderAtSqrDistance` (uninterpolated entity position, live `view_range`)
    /// and, when width and height are non-zero, `EntityRenderer.shouldRender` with
    /// `Display.getInterpolatedBoundingBox` inflated by 0.5.
    fn display_in_view(
        &self,
        camera: glam::DVec3,
        entity_pos: glam::DVec3,
        interpolated_pos: glam::DVec3,
        data: &TextDisplayData,
        view_scale: f64,
    ) -> bool {
        let range = f64::from(data.view_range) * 64.0 * view_scale;
        if (entity_pos - camera).length_squared() >= range * range {
            return false;
        }
        if text_display::no_culling(data.width, data.height) {
            return true;
        }
        let (mut lo, mut hi) = text_display::culling_box(interpolated_pos, data.width, data.height);
        lo -= glam::DVec3::splat(0.5);
        hi += glam::DVec3::splat(0.5);
        let size = ((hi.x - lo.x) + (hi.y - lo.y) + (hi.z - lo.z)) / 3.0;
        if !lo.is_finite() || !hi.is_finite() || size == 0.0 {
            lo = entity_pos - glam::DVec3::splat(2.0);
            hi = entity_pos + glam::DVec3::splat(2.0);
        }
        self.view_frustum
            .as_ref()
            .is_none_or(|view| view.intersects((lo - camera).as_vec3(), (hi - camera).as_vec3()))
    }

    /// `DisplayRenderer.BlockDisplayRenderer` and `ItemDisplayRenderer`: the
    /// block model or the item under the display's transformation, lit like
    /// any entity and never tinted by a hurt overlay.
    pub(super) fn block_item_display_geometry(
        &mut self,
        camera: glam::DVec3,
        partial: f64,
        out: &mut Vec<Vertex>,
    ) {
        let angles = self.display_camera_angles();
        let scale = self.view_scale();
        let now = std::time::Instant::now();
        let mut displays = std::mem::take(&mut self.text_displays);
        for shows in [Shows::Block, Shows::Item] {
            let kind = if shows == Shows::Block {
                "minecraft:block_display"
            } else {
                "minecraft:item_display"
            };
            self.mark_entity(out.len(), kind);
            for (id, display) in displays.iter_mut().filter(|(_, d)| d.shows == shows) {
                if !display.render_state_ready
                    || !self.entity_section_visible(display.position, now)
                {
                    continue;
                }
                let position = display.previous.lerp(display.position, partial);
                if !self.display_in_view(camera, display.position, position, &display.data, scale) {
                    continue;
                }
                let progress = display.calculate_interpolation_progress(partial as f32);
                let d = display.shown.blend(&display.from, progress);
                let partial_f32 = partial as f32;
                let rotation = [
                    rot_lerp(partial_f32, display.old_rotation[0], display.rotation[0]),
                    display.old_rotation[1]
                        + (display.rotation[1] - display.old_rotation[1]) * partial_f32,
                ];
                let matrix = glam::Mat4::from_translation((position - camera).as_vec3())
                    * d.matrix(rotation, angles);
                if !matrix.is_finite() {
                    continue;
                }
                let light = Self::display_light(
                    &self.world,
                    &d,
                    position,
                    display.shared_flags & 0x01 != 0,
                );
                if let Some(state) = display.shown_block {
                    // BlockModelResolver.update: model seed 42.
                    self.block_display_model_vertices(state, 42, matrix, light, out);
                    // CompositeBlockModel.update: the special model of a chest,
                    // head, banner and the like comes on top of the block model.
                    if self
                        .res
                        .facts()
                        .get(state)
                        .name
                        .strip_prefix("minecraft:")
                        .is_some_and(has_special_model)
                    {
                        let packed =
                            ((((light & 0xFF) / 16) << 4) | (((light >> 8) & 0xFF) / 16)) as u8;
                        self.special_block_model(state, matrix, packed, out);
                    }
                } else if let Some(stack) = &display.shown_item {
                    let context = match display.shown_item_display {
                        1 => DisplayContext::ThirdPersonLeftHand,
                        2 => DisplayContext::ThirdPersonRightHand,
                        3 => DisplayContext::FirstPersonLeftHand,
                        4 => DisplayContext::FirstPersonRightHand,
                        5 => DisplayContext::Head,
                        6 => DisplayContext::Gui,
                        7 => DisplayContext::Ground,
                        8 => DisplayContext::Fixed,
                        9 => DisplayContext::OnShelf,
                        _ => DisplayContext::None,
                    };
                    // updateForNonLiving: seeded by the entity id.
                    let state = self.resolve_item(stack, context, None, *id);
                    let pose = matrix * glam::Mat4::from_rotation_y(std::f32::consts::PI);
                    self.submit_item_world(&state, pose, light, out);
                }
            }
        }
        self.mark_entity(out.len(), "");
        self.text_displays = displays;
    }
    pub(super) fn text_display_geometry(
        &mut self,
        camera: glam::DVec3,
        partial: f64,
        normal: &mut Vec<Vertex>,
        see_through: &mut Vec<Vertex>,
    ) {
        self.text_display_vertices = 0;
        self.mark_translucent_entity(normal.len(), "minecraft:text_display");
        let angles = self.display_camera_angles();
        let scale = self.view_scale();
        let background = self.settings.text_background(0.25);
        let now = std::time::Instant::now();
        let mut inline_profiles = Vec::new();
        let mut st_verts = Vec::new();
        let mut st_submits: Vec<(f32, usize, std::ops::Range<usize>)> = Vec::new();
        let mut displays = std::mem::take(&mut self.text_displays);
        for (order, display) in displays
            .values_mut()
            .filter(|d| d.shows == Shows::Text)
            .enumerate()
        {
            if !display.render_state_ready || !self.entity_section_visible(display.position, now) {
                continue;
            }
            let position = display.previous.lerp(display.position, partial);
            if !self.display_in_view(camera, display.position, position, &display.data, scale) {
                // Out of view: its glyphs stay cached for when it comes back.
                continue;
            }
            let progress = display.calculate_interpolation_progress(partial as f32);
            let d = display.shown.blend(&display.from, progress);
            if d.text.is_empty() {
                continue;
            }
            let cached = display.lines.take();
            let partial_f32 = partial as f32;
            let rotation = [
                rot_lerp(partial_f32, display.old_rotation[0], display.rotation[0]),
                display.old_rotation[1]
                    + (display.rotation[1] - display.old_rotation[1]) * partial_f32,
            ];
            let mut cached = cached
                .filter(|c| {
                    c.matches(
                        &d,
                        background,
                        self.gui.atlas.font.epoch(),
                        self.gui.atlas.revision,
                    )
                })
                .unwrap_or_else(|| CachedQuads::new(&self.gui.atlas, &d, background));
            self.map_patches.append(&mut cached.batch.patches);
            inline_profiles.append(&mut cached.batch.inline_profiles);
            let cached = display.lines.insert(cached);
            let matrix = glam::Mat4::from_translation((position - camera).as_vec3())
                * d.matrix(rotation, angles)
                * cached.lines.pose();
            if !matrix.is_finite() || cached.batch.vertices.is_empty() {
                continue;
            }
            let batch = &cached.batch;
            let light =
                Self::display_light(&self.world, &d, position, display.shared_flags & 0x01 != 0);
            // Font.DisplayMode: SEE_THROUGH sorts per submit; POLYGON_OFFSET splits opaque text
            // (`canRenderAsSolid`) from backgrounds and translucent text.
            let through = d.flags & 2 != 0;
            let solid_text = d.opacity == -1;
            let st_start = st_verts.len();
            let out = if through { &mut st_verts } else { &mut *normal };
            for (quad_idx, quad) in batch.vertices.as_chunks::<6>().0.iter().enumerate() {
                let is_bg = quad_idx * 6 < cached.bg_vertices;
                let layer = if through {
                    2
                } else if !is_bg && solid_text {
                    TEXT_POLYGON_OFFSET_SOLID_OVERLAY
                } else {
                    TEXT_POLYGON_OFFSET_OVERLAY
                };
                for i in text_display::CORNERS {
                    let v = quad[i];
                    let point =
                        matrix.transform_point3(glam::Vec3::new(v.pos[0], v.pos[1], v.depth));
                    out.push(Vertex {
                        pos: point.to_array(),
                        uv: v.uv,
                        color: v.color,
                        light,
                        layer,
                        normal: [0; 4],

                        glint_uv: [0.0, 0.0],
                    });
                }
            }
            if through {
                let dist_sq = matrix.w_axis.truncate().length_squared();
                st_submits.push((dist_sq, order, st_start..st_verts.len()));
            }
            self.text_display_vertices += batch.vertices.len() / 6 * 4;
        }
        if !st_submits.is_empty() {
            st_submits.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for (_, _, range) in st_submits {
                see_through.extend_from_slice(&st_verts[range]);
            }
        }
        self.text_displays = displays;
        if !inline_profiles.is_empty() {
            self.resolve_inline_profiles(&inline_profiles);
        }
    }
}

/// Whether vanilla's built-in block models give this block a special model,
/// drawn by its block-entity renderer.
fn has_special_model(block: &str) -> bool {
    matches!(
        block,
        "bell"
            | "conduit"
            | "decorated_pot"
            | "enchanting_table"
            | "end_gateway"
            | "end_portal"
            | "chest"
            | "trapped_chest"
            | "ender_chest"
            | "shulker_box"
    ) || block.ends_with("_banner")
        || block.ends_with("_shulker_box")
        || block.ends_with("_skull")
        || (block.ends_with("_head") && block != "piston_head")
        || block.contains("copper_chest")
        || block.contains("copper_golem_statue")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn culled_display_keeps_quads_across_frame_rollover() {
        let atlas = sunrise_client::hud::HudAtlas::boot();
        let mut display = RemoteTextDisplay::new(Shows::Text, [0.0, 0.0, 5.0], 0.0);
        let d = TextDisplayData {
            text: std::sync::Arc::new(sunrise_client::text::Text::literal("Culled")),
            ..Default::default()
        };
        let cache = CachedQuads::new(&atlas, &d, 0);
        let allocation = cache.batch.vertices.as_ptr();
        display.lines = Some(cache);
        for _ in 0..2 {
            let cached = display
                .lines
                .take()
                .filter(|c| c.matches(&d, 0, atlas.font.epoch(), atlas.revision))
                .unwrap_or_else(|| panic!("culled display rebuilt its glyphs"));
            let cached = display.lines.insert(cached);
            let matrix = glam::Mat4::from_translation(glam::Vec3::Z * 5.0) * cached.lines.pose();
            let (lo, _) = cached.transformed_bounds(matrix);
            assert!(lo.z > 0.0, "the display is behind the camera");
            assert_eq!(cached.batch.vertices.as_ptr(), allocation);
        }
    }

    #[test]
    fn cached_text_quads_match_fresh_and_invalidate() {
        let atlas = sunrise_client::hud::HudAtlas::boot();
        let mut d = TextDisplayData {
            text: std::sync::Arc::new(sunrise_client::text::Text::literal("Hologram")),
            ..Default::default()
        };
        let cache = CachedQuads::new(&atlas, &d, 0);
        let mut fresh = sunrise_client::hud::HudBatch::scratch([1, 1]);
        text_display::draw(&mut fresh, &atlas, &d, &cache.lines, 0);
        let vertices = |b: &sunrise_client::hud::HudBatch| {
            b.vertices
                .iter()
                .map(|v| (v.pos, v.uv, v.color, v.depth))
                .collect::<Vec<_>>()
        };
        assert_eq!(vertices(&cache.batch), vertices(&fresh));
        let matches = |d: &TextDisplayData| cache.matches(d, 0, atlas.font.epoch(), atlas.revision);
        assert!(matches(&d));
        for change in 0..7 {
            let mut changed = d.clone();
            match change {
                0 => {
                    changed.text =
                        std::sync::Arc::new(sunrise_client::text::Text::literal("Changed"))
                }
                1 => changed.text = std::sync::Arc::new((*d.text).clone().bold()),
                2 => changed.opacity = 40,
                3 => changed.background = 0,
                4 => changed.flags = 1,
                5 => changed.flags = 2,
                _ => changed.line_width = 10,
            }
            assert!(!matches(&changed));
        }
        assert!(!cache.matches(&d, 1, atlas.font.epoch(), atlas.revision));
        assert!(!cache.matches(&d, 0, atlas.font.epoch() + 1, atlas.revision));
        assert!(!cache.matches(&d, 0, atlas.font.epoch(), atlas.revision + 1));
        d.text = std::sync::Arc::new(sunrise_client::text::Text::legacy("§kSecret"));
        let cache = CachedQuads::new(&atlas, &d, 0);
        assert!(!cache.matches(&d, 0, atlas.font.epoch(), atlas.revision));
        // A plugin hologram sends `§k` inside a literal JSON string: the font
        // resolves it, so its glyphs still change every frame.
        d.text = std::sync::Arc::new(sunrise_client::text::Text::literal("§kSecret"));
        let cache = CachedQuads::new(&atlas, &d, 0);
        assert!(!cache.matches(&d, 0, atlas.font.epoch(), atlas.revision));
    }

    #[test]
    fn transformed_text_bounds_reach_into_view_but_not_from_behind() {
        let atlas = sunrise_client::hud::HudAtlas::boot();
        let d = TextDisplayData {
            text: std::sync::Arc::new(sunrise_client::text::Text::literal("Wide hologram")),
            ..Default::default()
        };
        let cache = CachedQuads::new(&atlas, &d, 0);
        // A 90-degree camera looking down -Z has side planes x +/- z,
        // y +/- z, and the near plane z = -0.05.
        let visible = |matrix| {
            let (lo, hi) = cache.transformed_bounds(matrix);
            hi.x >= lo.z && lo.x <= -lo.z && hi.y >= lo.z && lo.y <= -lo.z && lo.z <= -0.05
        };
        let pose = |x, z, scale| {
            glam::Mat4::from_translation(glam::Vec3::new(x, 0.0, z))
                * glam::Mat4::from_scale(glam::Vec3::splat(scale))
                * cache.lines.pose()
        };
        assert!(visible(pose(6.0, -5.0, 20.0)));
        assert!(!visible(pose(100.0, -5.0, 1.0)));
        assert!(!visible(pose(0.0, 5.0, 1.0)));
    }

    #[test]
    fn metadata_and_position_interpolation_keep_their_separate_clocks() {
        let mut d = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 0.0);
        d.metadata(&[(11, 0, Value::Floats(vec![0.0, 0.0, 0.0]))]);
        d.tick();
        d.metadata(&[
            (8, 1, Value::Int(0)),
            (9, 1, Value::Int(4)),
            (10, 1, Value::Int(2)),
            (11, 0, Value::Floats(vec![4.0, 0.0, 0.0])),
        ]);
        d.tick();
        assert_eq!(d.shown.blend(&d.from, d.progress(0.5)).translation[0], 0.5);
        d.move_to([2.0, 0.0, 0.0], Some((90.0, 0.0)));
        d.tick();
        assert_eq!(d.position.x, 1.0);
        assert_eq!(d.rotation[0], 45.0);
        d.tick();
        assert_eq!(d.position.x, 2.0);
        assert_eq!(d.shown.blend(&d.from, d.progress(0.0)).translation[0], 2.0);
    }

    #[test]
    #[ignore = "daily"]
    fn handle_teleport_keeps_display_easing() {
        use crate::game::{commands::OtherEntity, native_entity_refs::tests::with_game};
        use sunrise_core::geometry::Vec3;
        use sunrise_core::nbt::{Compound, Tag};

        with_game(|game| {
            game.others.push(OtherEntity {
                attachment_lifetime: Default::default(),
                knot_check: None,
                eye: None,
                join: Default::default(),
                item_transformation: None,
                interaction: None,
                id: 12,
                kind: "minecraft:text_display".into(),
                position: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
            });
            let mut tag = Compound::new();
            tag.insert("teleport_duration".into(), Tag::Int(4));
            game.persist.raw.insert(12, Tag::Compound(tag));
            let entity = game.entity(12).unwrap();
            assert!(entity.teleport_to(game, [8.0, 0.0, 0.0], None));
            let metadata =
                sunrise_client::text_display::display_data(&game.persist.raw[&12], false);
            let mut client = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 0.0);
            client.metadata(&metadata);
            client.move_to(entity.position(game).unwrap(), None);
            client.tick();
            assert_eq!(client.position.x, 2.0);
            for _ in 0..3 {
                client.tick();
            }
            assert_eq!(client.position.x, 8.0);
        });
    }

    /// A display riding a vehicle sits where the vehicle seats it each tick,
    /// moving from the last tick's seat, and a teleport sent meanwhile does
    /// not drag it off the seat.
    #[test]
    fn a_ridden_display_moves_from_seat_to_seat() {
        let mut d = RemoteTextDisplay::new(Shows::Text, [0.0, 64.0, 0.0], 0.0);
        d.metadata(&[(10, 1, Value::Int(3))]);
        d.move_to([9.0, 64.0, 9.0], None);
        let seat = glam::DVec3::new(0.0, 65.8, 0.0);
        d.ride(seat);
        assert_eq!(d.position, seat);
        d.tick();
        assert_eq!((d.previous, d.position), (seat, seat));
        let next = glam::DVec3::new(1.0, 65.8, 0.0);
        d.ride(next);
        assert_eq!((d.previous, d.position), (seat, next));
    }
}

#[cfg(test)]
#[path = "text_displays/display_parity_tests.rs"]
mod display_parity_tests;
