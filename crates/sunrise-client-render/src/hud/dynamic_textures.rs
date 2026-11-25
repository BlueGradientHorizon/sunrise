//! Client mods' dynamic textures (`ClientRenderClientHost::create_dynamic_texture`),
//! vanilla's `DynamicTexture` registered in the `TextureManager` under an id
//! (`TextureManager.register`, `release`). Each texture is a GPU texture of its own
//! (`RGBA8_UNORM`, one mip level, sampled nearest with repeat: `DynamicTexture`'s constructor),
//! drawn as a standalone texture by `HudBatch::dynamic_texture`.
//!
//! Two halves meet through [`Command`]s. [`DynamicTextures`] is the client thread's: which ids
//! exist, who owns them and how large they are, with every refusal decided there, and no pixels
//! kept. Each accepted call queues a command; a frame takes the queue
//! ([`DynamicTextures::take_commands`]) in the batch, and the render thread applies the commands
//! in order before the frame ([`Store::apply`]), with no merging. [`Store`] holds the GPU
//! textures. It is shared by the renderers that follow one another on the same device, so a
//! resource reload keeps every texture; a new device starts with an empty one, which
//! [`DynamicTextures::bind`] notices and answers by forgetting every id.
use mod_api::DynamicTextureError;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use sunrise_core::identifier::Identifier;

/// One change to the render thread's textures, applied in the order the calls were made.
/// `handle` names a texture for as long as its id lives, whatever size it is re-created at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// A new texture, or one replacing the handle's (a different size), holding `rgba`.
    Create {
        handle: u64,
        size: [u32; 2],
        rgba: Vec<u8>,
    },
    /// `CommandEncoder.writeToTexture`'s region overload: `rgba` into the region at `origin`.
    Write {
        handle: u64,
        origin: [u32; 2],
        size: [u32; 2],
        rgba: Vec<u8>,
    },
    /// `TextureManager.release`.
    Release { handle: u64 },
    /// A GPU copy of the whole texture into the HUD atlas at `dst`: how a skin a mod selects
    /// reaches the atlas slot the body, hand and preview sample.
    CopyToAtlas { handle: u64, dst: [u32; 2] },
}

fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The client thread's table of dynamic textures.
#[derive(Default)]
pub struct DynamicTextures {
    images: BTreeMap<Identifier, Image>,
    queue: Vec<Command>,
    /// The [`Store`] these ids are in, and the device's largest 2D texture side; none until a
    /// renderer is bound.
    bound: Option<(u64, u32)>,
    geometry_revision: u64,
    /// `mods_switched` as of the last [`Self::release_unloaded`].
    switched: u64,
}

struct Image {
    /// The native mod that created this texture.
    owner: Box<str>,
    handle: u64,
    size: [u32; 2],
}

/// `BadSize` for a side of 0 or pixels that are not `width * height * 4` bytes, `TooLarge` for a
/// side past `max`. The side is compared before the bytes are counted, so the count only sees
/// sides of at most `max` and, counted in `u64`, cannot overflow.
fn checked(size: [u32; 2], len: usize, max: u32) -> Result<(), DynamicTextureError> {
    if size.contains(&0) {
        return Err(DynamicTextureError::BadSize);
    }
    if size.iter().any(|side| *side > max) {
        return Err(DynamicTextureError::TooLarge);
    }
    if u64::from(size[0]) * u64::from(size[1]) * 4 != len as u64 {
        return Err(DynamicTextureError::BadSize);
    }
    Ok(())
}

impl DynamicTextures {
    /// Changes when a texture is added, removed or resized; pixel-only updates don't relayout text.
    pub fn geometry_revision(&self) -> u64 {
        self.geometry_revision
    }

    /// The renderer's `store` and its device's `max_side`, read once per renderer. A store that
    /// is not the one the ids were made in is a new device: every id is forgotten and every
    /// queued command dropped, so the owning mods read `Unknown` and create again.
    pub fn bind(&mut self, store: u64, max_side: u32) {
        if self.bound.map(|(id, _)| id) != Some(store) {
            self.images.clear();
            self.queue.clear();
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
        }
        self.bound = Some((store, max_side));
    }

    /// Mod `owner` creates texture `id` of `size` from `rgba`. Every refusal comes before the
    /// pixels are copied, and changes nothing.
    pub fn create(
        &mut self,
        owner: &str,
        id: &Identifier,
        size: [u32; 2],
        rgba: &[u8],
    ) -> Result<(), DynamicTextureError> {
        let (_, max) = self.bound.ok_or(DynamicTextureError::Unavailable)?;
        checked(size, rgba.len(), max)?;
        if self.images.contains_key(id) {
            return Err(DynamicTextureError::NameTaken);
        }
        let handle = next_id();
        self.queue.push(Command::Create {
            handle,
            size,
            rgba: rgba.to_vec(),
        });
        self.images.insert(
            id.clone(),
            Image {
                owner: owner.into(),
                handle,
                size,
            },
        );
        self.geometry_revision = self.geometry_revision.wrapping_add(1);
        Ok(())
    }

    /// Mod `owner` gives its texture `id` new pixels: the same size writes the whole texture, a
    /// different one makes a new texture under the id's handle.
    pub fn update(
        &mut self,
        owner: &str,
        id: &Identifier,
        size: [u32; 2],
        rgba: &[u8],
    ) -> Result<(), DynamicTextureError> {
        let (_, max) = self.bound.ok_or(DynamicTextureError::Unavailable)?;
        let image = self.owned_image(owner, id)?;
        checked(size, rgba.len(), max)?;
        let handle = image.handle;
        let rgba = rgba.to_vec();
        if image.size == size {
            self.queue.push(Command::Write {
                handle,
                origin: [0, 0],
                size,
                rgba,
            });
        } else {
            image.size = size;
            self.queue.push(Command::Create { handle, size, rgba });
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Mod `owner` replaces the region of its texture `id` at `origin` of `size` with `rgba`.
    pub fn update_region(
        &mut self,
        owner: &str,
        id: &Identifier,
        origin: [u32; 2],
        size: [u32; 2],
        rgba: &[u8],
    ) -> Result<(), DynamicTextureError> {
        let (_, max) = self.bound.ok_or(DynamicTextureError::Unavailable)?;
        let image = self.owned_image(owner, id)?;
        let (handle, extent) = (image.handle, image.size);
        checked(size, rgba.len(), max)?;
        let inside = |axis: usize| {
            origin[axis]
                .checked_add(size[axis])
                .is_some_and(|end| end <= extent[axis])
        };
        if !(inside(0) && inside(1)) {
            return Err(DynamicTextureError::OutOfBounds);
        }
        self.queue.push(Command::Write {
            handle,
            origin,
            size,
            rgba: rgba.to_vec(),
        });
        Ok(())
    }

    /// Mod `owner` releases its texture `id`.
    pub fn release(&mut self, owner: &str, id: &Identifier) -> Result<(), DynamicTextureError> {
        self.bound.ok_or(DynamicTextureError::Unavailable)?;
        let handle = self.owned_image(owner, id)?.handle;
        self.images.remove(id);
        self.queue.push(Command::Release { handle });
        self.geometry_revision = self.geometry_revision.wrapping_add(1);
        Ok(())
    }

    /// Mod `owner` unloaded: every texture it made is released.
    pub fn release_owned_by(&mut self, owner: &str) {
        let queue = &mut self.queue;
        let before = self.images.len();
        self.images.retain(|_, image| {
            let keep = image.owner.as_ref() != owner;
            if !keep {
                queue.push(Command::Release {
                    handle: image.handle,
                });
            }
            keep
        });
        if self.images.len() != before {
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
        }
    }

    /// When the mods were switched since the last call (`switched` changed), releases the
    /// textures of every mod `loaded` no longer holds on.
    pub fn release_unloaded(&mut self, switched: u64, loaded: impl Fn(&str) -> bool) {
        if std::mem::replace(&mut self.switched, switched) == switched {
            return;
        }
        let gone: Vec<Box<str>> = self
            .images
            .values()
            .map(|image| image.owner.clone())
            .filter(|owner| !loaded(owner))
            .collect();
        for owner in gone {
            self.release_owned_by(&owner);
        }
    }

    fn owned_image(
        &mut self,
        owner: &str,
        id: &Identifier,
    ) -> Result<&mut Image, DynamicTextureError> {
        let image = self
            .images
            .get_mut(id)
            .ok_or(DynamicTextureError::Unknown)?;
        if image.owner.as_ref() != owner {
            return Err(DynamicTextureError::NotOwned);
        }
        Ok(image)
    }

    /// The handle and size of the live texture `id`, if this id names one.
    pub fn get(&self, id: &Identifier) -> Option<(u64, [u32; 2])> {
        self.images.get(id).map(|image| (image.handle, image.size))
    }
    /// The dimensions of a live texture, if this id names one.
    pub fn size(&self, id: &Identifier) -> Option<[u32; 2]> {
        self.get(id).map(|(_, size)| size)
    }

    /// Queue the GPU copy of the live 64 x 64 texture `id` into the atlas at `dst`; false when
    /// `id` names no such texture.
    pub fn copy_to_atlas(&mut self, id: &Identifier, dst: [u32; 2]) -> bool {
        let Some((handle, [64, 64])) = self.get(id) else {
            return false;
        };
        self.queue.push(Command::CopyToAtlas { handle, dst });
        true
    }

    /// Move the queued commands, in order, into `out` for this frame.
    pub fn take_commands(&mut self, out: &mut Vec<Command>) {
        out.append(&mut self.queue);
    }
}

/// The render thread's textures, by handle.
#[derive(Clone)]
pub struct Store(Arc<StoreInner>);

struct StoreInner {
    id: u64,
    textures: Mutex<HashMap<u64, Gpu>>,
}

/// One texture and the view a bind group samples.
pub(super) struct Gpu {
    texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    /// Changes when the texture is made again, so a bind group built for the old one is stale.
    pub(super) revision: u64,
}

impl Default for Store {
    fn default() -> Self {
        Self(Arc::new(StoreInner {
            id: next_id(),
            textures: Mutex::default(),
        }))
    }
}

impl Store {
    /// What [`DynamicTextures::bind`] tells apart.
    pub fn id(&self) -> u64 {
        self.0.id
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Gpu>> {
        self.0
            .textures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The view and revision of texture `handle`.
    pub(super) fn view(&self, handle: u64) -> Option<(wgpu::TextureView, u64)> {
        self.lock()
            .get(&handle)
            .map(|gpu| (gpu.view.clone(), gpu.revision))
    }

    /// Apply `commands` in order into `encoder`, ahead of the frame's draws. `atlas` is the HUD
    /// atlas texture a `CopyToAtlas` writes. A command for a handle that no longer exists, or
    /// outside its texture, is skipped: the client thread only sends what it checked.
    pub fn apply(
        &self,
        device: &wgpu::Device,
        uploads: &mut crate::gfx::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        atlas: Option<&wgpu::Texture>,
        commands: &[Command],
    ) {
        let mut textures = self.lock();
        for command in commands {
            match command {
                Command::Create { handle, size, rgba } => {
                    // The sides were checked against the device's limit when the call was made. A
                    // failure the device still reports is logged and leaves no GPU texture, while the
                    // client table keeps the id: it draws vanilla's missing texture
                    // (`prepare_dynamic` draws it for a handle with no view), as an id released after
                    // its draw was submitted does (`TextureManager.release`). A later
                    // `update_dynamic_texture` of a different size creates it again.
                    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
                    let out_of_memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Dynamic texture"),
                        size: wgpu::Extent3d {
                            width: size[0],
                            height: size[1],
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    });
                    let failed = pollster::block_on(out_of_memory.pop())
                        .or(pollster::block_on(validation.pop()));
                    if let Some(error) = failed {
                        log::warn!("dynamic texture {handle} was not created: {error}");
                        textures.remove(handle);
                        continue;
                    }
                    uploads.texture(encoder, texture.as_image_copy(), size[0], size[1], rgba);
                    let view = texture.create_view(&Default::default());
                    textures.insert(
                        *handle,
                        Gpu {
                            texture,
                            view,
                            revision: next_id(),
                        },
                    );
                }
                Command::Write {
                    handle,
                    origin,
                    size,
                    rgba,
                } => {
                    let Some(gpu) = textures.get(handle) else {
                        continue;
                    };
                    let (width, height) = (gpu.texture.width(), gpu.texture.height());
                    if origin[0].saturating_add(size[0]) > width
                        || origin[1].saturating_add(size[1]) > height
                    {
                        continue;
                    }
                    uploads.texture(
                        encoder,
                        wgpu::TexelCopyTextureInfo {
                            texture: &gpu.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: origin[0],
                                y: origin[1],
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        size[0],
                        size[1],
                        rgba,
                    );
                }
                Command::Release { handle } => {
                    textures.remove(handle);
                }
                Command::CopyToAtlas { handle, dst } => {
                    let (Some(gpu), Some(atlas)) = (textures.get(handle), atlas) else {
                        continue;
                    };
                    let (width, height) = (gpu.texture.width(), gpu.texture.height());
                    if dst[0].saturating_add(width) > atlas.width()
                        || dst[1].saturating_add(height) > atlas.height()
                    {
                        continue;
                    }
                    encoder.copy_texture_to_texture(
                        gpu.texture.as_image_copy(),
                        wgpu::TexelCopyTextureInfo {
                            texture: atlas,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: dst[0],
                                y: dst[1],
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DynamicTextureError::*;

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).unwrap()
    }

    /// A table bound to a store whose device allows textures up to 128 pixels a side.
    fn textures() -> DynamicTextures {
        let mut textures = DynamicTextures::default();
        textures.bind(1, 128);
        textures
    }

    fn commands(textures: &mut DynamicTextures) -> Vec<Command> {
        let mut out = Vec::new();
        textures.take_commands(&mut out);
        out
    }

    #[test]
    fn nothing_is_made_before_a_renderer_is_bound() {
        let mut textures = DynamicTextures::default();
        let a = id("mod:a");
        assert_eq!(
            textures.create("mod", &a, [1, 1], &[0; 4]),
            Err(Unavailable)
        );
        assert_eq!(textures.release("mod", &a), Err(Unavailable));
    }

    #[test]
    fn a_pixel_buffer_of_the_wrong_length_or_size_is_refused_by_name() {
        let mut textures = textures();
        let a = id("mod:a");
        assert_eq!(textures.create("mod", &a, [2, 2], &[0; 15]), Err(BadSize));
        assert_eq!(textures.create("mod", &a, [2, 2], &[0; 17]), Err(BadSize));
        assert_eq!(textures.create("mod", &a, [0, 4], &[]), Err(BadSize));
        assert_eq!(textures.create("mod", &a, [4, 0], &[]), Err(BadSize));
        assert!(commands(&mut textures).is_empty());
        assert_eq!(textures.get(&a), None);
    }

    #[test]
    fn a_side_past_the_devices_maximum_is_too_large_before_its_length_is_counted() {
        let mut textures = textures();
        let a = id("mod:a");
        assert_eq!(
            textures.create("mod", &a, [129, 1], &[0; 516]),
            Err(TooLarge)
        );
        assert_eq!(textures.create("mod", &a, [1, 129], &[]), Err(TooLarge));
        assert_eq!(
            textures.create("mod", &a, [u32::MAX, u32::MAX], &[]),
            Err(TooLarge)
        );
        assert_eq!(textures.create("mod", &a, [128, 1], &[0; 512]), Ok(()));
    }

    #[test]
    fn a_name_is_taken_unknown_or_another_mods() {
        let mut textures = textures();
        let a = id("mod:a");
        textures.create("mod", &a, [1, 1], &[0; 4]).unwrap();
        assert_eq!(textures.create("mod", &a, [1, 1], &[0; 4]), Err(NameTaken));
        assert_eq!(
            textures.create("other", &a, [1, 1], &[0; 4]),
            Err(NameTaken)
        );
        let missing = id("mod:missing");
        assert_eq!(
            textures.update("mod", &missing, [1, 1], &[0; 4]),
            Err(Unknown)
        );
        assert_eq!(textures.release("mod", &missing), Err(Unknown));
        assert_eq!(textures.update("other", &a, [1, 1], &[0; 4]), Err(NotOwned));
        assert_eq!(
            textures.update_region("other", &a, [0, 0], [1, 1], &[0; 4]),
            Err(NotOwned)
        );
        assert_eq!(textures.release("other", &a), Err(NotOwned));
        assert_eq!(textures.size(&a), Some([1, 1]));
    }

    #[test]
    fn a_region_outside_the_texture_is_refused_by_name() {
        let mut textures = textures();
        let a = id("mod:a");
        textures.create("mod", &a, [4, 4], &[0; 64]).unwrap();
        commands(&mut textures);
        let region = |textures: &mut DynamicTextures, origin, size: [u32; 2]| {
            let rgba = vec![0; (size[0] * size[1] * 4) as usize];
            textures.update_region("mod", &a, origin, size, &rgba)
        };
        assert_eq!(region(&mut textures, [3, 0], [2, 1]), Err(OutOfBounds));
        assert_eq!(region(&mut textures, [0, 4], [1, 1]), Err(OutOfBounds));
        assert_eq!(
            region(&mut textures, [u32::MAX, 0], [1, 1]),
            Err(OutOfBounds)
        );
        assert_eq!(region(&mut textures, [0, 0], [5, 1]), Err(OutOfBounds));
        assert_eq!(
            textures.update_region("mod", &a, [0, 0], [1, 1], &[0; 3]),
            Err(BadSize)
        );
        assert!(commands(&mut textures).is_empty());
        assert_eq!(region(&mut textures, [2, 3], [2, 1]), Ok(()));
    }

    #[test]
    fn calls_queue_in_order_and_a_new_size_recreates_the_same_handle() {
        let mut textures = textures();
        let a = id("mod:a");
        textures.create("mod", &a, [2, 2], &[1; 16]).unwrap();
        let (handle, _) = textures.get(&a).unwrap();
        textures
            .update_region("mod", &a, [1, 1], [1, 1], &[2; 4])
            .unwrap();
        textures.update("mod", &a, [2, 2], &[3; 16]).unwrap();
        textures.update("mod", &a, [1, 3], &[4; 12]).unwrap();
        textures.release("mod", &a).unwrap();
        assert_eq!(
            commands(&mut textures),
            [
                Command::Create {
                    handle,
                    size: [2, 2],
                    rgba: vec![1; 16]
                },
                Command::Write {
                    handle,
                    origin: [1, 1],
                    size: [1, 1],
                    rgba: vec![2; 4]
                },
                Command::Write {
                    handle,
                    origin: [0, 0],
                    size: [2, 2],
                    rgba: vec![3; 16]
                },
                Command::Create {
                    handle,
                    size: [1, 3],
                    rgba: vec![4; 12]
                },
                Command::Release { handle },
            ]
        );
        assert_eq!(textures.get(&a), None);
        assert!(commands(&mut textures).is_empty());
    }

    #[test]
    fn unloading_a_mod_releases_only_its_textures() {
        let mut textures = textures();
        let (a, b) = (id("one:a"), id("two:b"));
        textures.create("one", &a, [1, 1], &[0; 4]).unwrap();
        textures.create("two", &b, [1, 1], &[0; 4]).unwrap();
        let (handle, _) = textures.get(&a).unwrap();
        commands(&mut textures);
        textures.release_owned_by("one");
        assert_eq!(commands(&mut textures), [Command::Release { handle }]);
        assert_eq!(textures.get(&a), None);
        assert!(textures.get(&b).is_some());
    }

    #[test]
    fn a_new_device_forgets_every_texture() {
        let mut textures = textures();
        let a = id("mod:a");
        textures.create("mod", &a, [1, 1], &[0; 4]).unwrap();
        textures.bind(1, 128);
        assert!(textures.get(&a).is_some());
        textures.bind(2, 64);
        assert_eq!(textures.get(&a), None);
        assert!(commands(&mut textures).is_empty());
        assert_eq!(textures.update("mod", &a, [1, 1], &[0; 4]), Err(Unknown));
    }

    #[test]
    fn only_a_64_by_64_texture_copies_into_a_skin_slot() {
        let mut textures = textures();
        let (skin, small) = (id("mod:skin"), id("mod:small"));
        textures
            .create("mod", &skin, [64, 64], &[0; 64 * 64 * 4])
            .unwrap();
        textures
            .create("mod", &small, [32, 64], &[0; 32 * 64 * 4])
            .unwrap();
        commands(&mut textures);
        assert!(!textures.copy_to_atlas(&small, [0, 0]));
        assert!(!textures.copy_to_atlas(&id("mod:none"), [0, 0]));
        assert!(textures.copy_to_atlas(&skin, [8, 16]));
        let (handle, _) = textures.get(&skin).unwrap();
        assert_eq!(
            commands(&mut textures),
            [Command::CopyToAtlas {
                handle,
                dst: [8, 16]
            }]
        );
    }
}
