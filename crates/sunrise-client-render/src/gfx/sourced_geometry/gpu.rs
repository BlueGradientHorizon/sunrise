//! Sourced geometry drawn on the GPU while GPU culling is on (`gpu_culling`).
//! Sourced geometry is the meshes a source hands the renderer, each over a cell of its
//! own (#2500's `Cell`), and nothing here knows what made one. The pass reads only what
//! the renderer's residency keeps for every sourced mesh: its cell, its arena range and
//! buckets by facing side, its bounds, and whether it fades in or out, stands in or goes.
//!
//! Every resident mesh with solid quads has a record, in a second table of GPU culling's
//! own pass (`GpuCulling::second`): the same `gpu_culling.wgsl` entry points, with the
//! view-distance test off (`NO_RANGE`), cave culling's graph off, and each record's box in
//! units of its cell's (`Record::units`). Its tables are GPU culling's too (`tables`): the
//! origins of the cells' corners along x and z, as the CPU's f64 difference with the
//! camera rounded to f32, the frustum planes' terms for them and the eye's keys against
//! them, so a mesh drawn here shows the quads the CPU's draw would, where it would.
//! A source's residency decides its range, so only the frustum is tested.
//!
//! The render thread then walks, sorts and lists only the meshes it still draws itself:
//! those fading in or out, standing in for their replacement or going (`FADING`), and
//! those with translucent quads, which stay the CPU's whatever the pass does: their sort
//! is unchanged.
//!
//! The table is made when the first mesh with solid quads is resident and dropped with
//! its last (`SourcedGpu::new`), and with GPU culling's pass, where that is offered:
//! with nothing resident, no pass is dispatched and no buffer is held. It draws only in
//! a frame GPU culling draws (`Renderer::prepare_sourced`). Everywhere else, sourced
//! geometry goes the CPU's way, unchanged: OpenGL, Metal, DX12 with flat page groups,
//! Intel Gen11/12's Vulkan, software adapters, a device without count draws, GPU culling
//! switched off, out of memory or after device loss. It asks the device for nothing GPU
//! culling's pass does not.
//!
//! Draw order: its solid draws go after the near sections' and the CPU's (`draw`), keeping
//! #627's near-first order: sourced geometry lies past the render distance, behind the near
//! terrain, so depth rejects its hidden pixels early. Within a page the draws come in id
//! order, the same every frame, and steady meshes do not overlap (a replaced mesh is going
//! and its replacement fades in, both the CPU's), so no pixel goes to whichever draws last.
use super::super::{
    Frustum, SectionKey,
    arena::Arena,
    gpu_culling::{Cells, FrameInputs, GpuCulling, Prepared, Record, Records, rounded_box},
    pack_timing::PassTiming,
    resident,
    section_fade::Fades,
    submit::Queue,
    upload::Uploads,
};
use crate::mesher::Sides;
use glam::{DVec3, Vec3};
use std::{collections::BTreeMap, time::Instant};

/// Cells are at least a section wide (a record's key is its corner in sections), and a box's
/// unit is a sixteenth of one's width: `Record::units` holds 15 bits of shift.
const MIN_SIZE_LOG2: u8 = 4;
const MAX_SIZE_LOG2: u8 = MIN_SIZE_LOG2 + 15;
/// A box's step along y, in blocks, as a shift (`cell_box`): codes reach -128 to 368, past the
/// overworld's -64 to 320.
const Y_SHIFT: u32 = 4;
/// Sections from the camera's column the tables reach, at most: past it a frame goes the CPU's
/// way (tables of millions of columns, from meshes strewn across far more than any range).
const MAX_RADIUS: i32 = 4096;

/// Where a sourced mesh lies (#2500's `Cell`): `1 << size_log2` blocks wide along x and z, at
/// `x` and `z` in cells of that size, its corner at y 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    pub x: i32,
    pub z: i32,
    pub size_log2: u8,
}

impl Cell {
    /// Its corner in sections, the key of its record; `None` where the pass does not take it
    /// (narrower than a section, or wider than a box's units hold, or its corner past an i32).
    fn key(self) -> Option<SectionKey> {
        let k = u32::from(self.size_log2.checked_sub(MIN_SIZE_LOG2)?);
        let corner = |c: i32| i32::try_from(i64::from(c) << k).ok();
        (self.size_log2 <= MAX_SIZE_LOG2).then_some(())?;
        Some((corner(self.x)?, 0, corner(self.z)?))
    }

    /// A box's step along x and z, as a shift: a sixteenth of its width.
    fn shift(self) -> u32 {
        u32::from(self.size_log2 - MIN_SIZE_LOG2)
    }
}

/// What the pass reads of a resident mesh.
pub(crate) struct Mesh<'a> {
    pub cell: Cell,
    /// Its solid quads' arena page, first vertex (as a draw's base vertex) and count.
    pub page: u32,
    pub start: u64,
    pub quads: u32,
    pub sides: &'a Sides,
    /// Its quads' bounds around the cell's corner.
    pub bounds: [Vec3; 2],
}

/// A record's box: the bounds rounded out to whole steps, `rounded_box`'s packing, in units of
/// a sixteenth of the cell's width along x and z and `1 << Y_SHIFT` blocks along y; `UNBOUNDED`
/// past what its codes hold.
fn cell_box(bounds: [Vec3; 2], cell: Cell) -> u32 {
    let flat = (1u32 << cell.shift()) as f32;
    let unit = Vec3::new(flat, (1u32 << Y_SHIFT) as f32, flat);
    rounded_box(Some(bounds.map(|b| b / unit)))
}

/// The second table, its pass's buffers (`GpuCulling`) and what follows its records.
pub(crate) struct SourcedGpu {
    gpu: GpuCulling,
    /// The flagged meshes that time alone makes whole, by when.
    due: Fades,
    /// The recorded cells' corners along x and z, in sections: the tables span them.
    corners: [BTreeMap<i32, u32>; 2],
    /// Per `Cell::shift`, the records with it: the largest decides the tables' margins.
    shifts: [u32; MAX_SIZE_LOG2 as usize - MIN_SIZE_LOG2 as usize + 1],
    /// The instances' UV step, f32 bits.
    step: u32,
}

impl SourcedGpu {
    /// The table beside `gpu`'s pass, for instances drawn with UV step `step`: `None` when the
    /// memory for its buffers is missing.
    pub fn new(gpu: &GpuCulling, device: &wgpu::Device, step: f32) -> Option<Self> {
        Some(Self {
            gpu: gpu.second(device)?,
            due: Fades::default(),
            corners: Default::default(),
            shifts: Default::default(),
            step: step.to_bits(),
        })
    }

    /// Whether no mesh has a record.
    pub fn is_empty(&self) -> bool {
        self.gpu.records.live() == 0
    }

    /// The table, for the arena's compaction to move its records with their meshes.
    pub fn records(&mut self) -> &mut Records {
        &mut self.gpu.records
    }

    /// `mesh`'s record, `FADING` while the CPU draws it; `None` for a cell the pass does not
    /// take (the CPU draws it).
    pub fn record(mesh: &Mesh, fading: bool) -> Option<Record> {
        let cell = mesh.cell;
        let boxed = cell_box(mesh.bounds, cell);
        let record = Record::new(cell.key()?, mesh.page, mesh.start, mesh.quads, boxed);
        Some(
            record
                .facing(mesh.sides)
                .units(cell.shift(), Y_SHIFT)
                .fading(fading),
        )
    }

    /// The mesh `mesh`, `fading` while the CPU draws it, has a record: its id, or `None` for a
    /// cell the pass does not take.
    pub fn add(&mut self, mesh: &Mesh, fading: bool) -> Option<u32> {
        let record = Self::record(mesh, fading)?;
        self.count(mesh.cell, 1);
        Some(self.gpu.records.insert(record))
    }

    /// The mesh in `cell` with record `id` went.
    pub fn remove(&mut self, cell: Cell, id: u32) {
        self.gpu.records.remove(id);
        self.count(cell, -1);
    }

    /// The mesh with record `id` began to be drawn by the CPU, or stopped.
    pub fn set_fading(&mut self, id: u32, on: bool) {
        self.gpu.records.set_fading(id, on);
    }

    /// The flagged mesh in `cell` is whole at `end` at the latest, if time alone makes it so:
    /// it is looked at again then (`take_due`).
    pub fn wait(&mut self, cell: Cell, end: Instant) {
        self.due
            .push(end, (cell.x, i32::from(cell.size_log2), cell.z));
    }

    /// Debug builds: the table, and the cells whose end is waited for.
    #[cfg(debug_assertions)]
    pub fn checked(&self) -> (&Records, impl Iterator<Item = Cell>) {
        let waits = self.due.pending().map(|(x, size, z)| Cell {
            x,
            z,
            size_log2: size as u8,
        });
        (&self.gpu.records, waits)
    }

    /// Calls `look` for each flagged mesh whose end is due at `now`, before a frame's
    /// `prepare`, which sends the flags, and no later than the frame's instances are decided.
    pub fn take_due(&mut self, now: Instant, mut look: impl FnMut(Cell)) {
        while let Some((x, size, z)) = self.due.pop_due(now) {
            look(Cell {
                x,
                z,
                size_log2: size as u8,
            });
        }
    }

    fn count(&mut self, cell: Cell, by: i32) {
        let Some((x, _, z)) = cell.key() else {
            return;
        };
        for (corners, at) in self.corners.iter_mut().zip([x, z]) {
            let n = corners.entry(at).or_insert(0);
            *n = n.wrapping_add_signed(by);
            if *n == 0 {
                corners.remove(&at);
            }
        }
        let shift = &mut self.shifts[cell.shift() as usize];
        *shift = shift.wrapping_add_signed(by);
    }

    /// Records the frame's uploads into `encoder` and clears its counts, before the walk, for a
    /// frame with the camera at `camera` over a level from `min_y`, culled with `frustum`,
    /// solid quads facing away from `eye` left out (`None`: all drawn). Then `dispatch`, and
    /// the world pass's `draw`. Unless the frame is `Prepared::Gpu`, nothing is drawn here:
    /// the CPU draws the meshes, and the records that changed wait for the next frame.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &Queue,
        uploads: &mut Uploads,
        encoder: &mut wgpu::CommandEncoder,
        camera: DVec3,
        min_y: i32,
        frustum: &Frustum,
        eye: Option<Vec3>,
        timing: Option<&mut PassTiming>,
    ) -> Prepared {
        if !camera.is_finite() {
            return Prepared::Cpu;
        }
        let (cx, cz) = resident::camera_column(camera);
        let reach = |corners: &BTreeMap<i32, u32>, at: i32| {
            let ends = corners.first_key_value().zip(corners.last_key_value());
            ends.map_or(0, |((&low, _), (&high, _))| (at - low).max(high - at))
        };
        let radius = reach(&self.corners[0], cx).max(reach(&self.corners[1], cz));
        if radius > MAX_RADIUS {
            return Prepared::Cpu;
        }
        let xz = self.shifts.iter().rposition(|&n| n > 0).unwrap_or(0) as u32;
        let cells = Cells {
            units: [xz, Y_SHIFT],
            step: self.step,
        };
        let input = FrameInputs::new(camera, min_y, 1, radius, frustum)
            .with_eye(eye)
            .with_cells(cells);
        self.gpu
            .prepare(device, queue, uploads, encoder, &input, timing)
    }

    /// The pass, after `prepare`: the changed records into the table, then the draws of the
    /// meshes it keeps.
    pub fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, timing: Option<&mut PassTiming>) {
        self.gpu.dispatch(encoder, timing);
    }

    /// The kept meshes' solid draws, a count draw per page, with the quad indices bound. Leaves
    /// the origins at vertex slot 1. `group`: the page's group 1 for pipelines that read the
    /// page themselves (`Flat::page_group`), else none.
    pub fn draw<'a>(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        arena: &Arena,
        group: impl Fn(u32) -> Option<&'a wgpu::BindGroup>,
    ) {
        self.gpu.draw(pass, arena, group, 0);
    }

    /// Draw calls `draw` makes.
    pub fn draw_calls(&self) -> usize {
        self.gpu.draw_calls()
    }
}

#[cfg(test)]
mod tests;
