//! A client mod's retained GPU storage buffers (`ClientRenderClientHost::create_storage_buffer`,
//! `StorageBufferRef::write_range`, `size` and `release`): vanilla's `GpuBuffer` written through
//! `CommandEncoder.writeToBuffer(GpuBufferSlice, ByteBuffer)` and bound by range
//! (`GpuBuffer.slice`, `GpuBufferSlice`).

use super::render_pipeline::BufferError;

/// A live client context that can operate on retained GPU [`StorageBufferRef`] handles.
pub trait StorageBufferContext {
    /// The scoped engine access used by [`StorageBufferRef`] methods.
    // hidden: internal trait method opening the retained-buffer backend for StorageBufferRef.
    #[doc(hidden)]
    fn storage_buffer_backend(&self) -> Option<StorageBufferBackendRef<'_>> {
        None
    }
}

/// Opaque access to retained storage-buffer operations, not to the world or other client services.
// hidden: only the handle can open the storage-buffer backend.
// exhaustive: private field; only constructed inside this crate.
// no wire: scoped backend borrow.
#[doc(hidden)]
pub struct StorageBufferBackendRef<'a>(&'a dyn crate::prelude::ClientRenderClientBackend);

impl<H: crate::ClientHost + ?Sized> StorageBufferContext for H {
    fn storage_buffer_backend(&self) -> Option<StorageBufferBackendRef<'_>> {
        self.backend()
            .map(|backend| StorageBufferBackendRef(backend.open()))
    }
}

/// Generational handle to a retained GPU storage buffer, created via
/// [`crate::prelude::ClientRenderClientHost::create_storage_buffer`].
///
/// Handles are owner-checked. The buffer outlives a resource reload and a world close, and is
/// freed by [`Self::release`] or when its mod unloads.
// no wire: process-local generational handle to a retained GPU storage buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: only the engine creates handles.
pub struct StorageBufferRef(pub(crate) u64, pub(crate) u32);

impl StorageBufferRef {
    pub(crate) fn from_parts(generation: u64, slot: u32) -> Result<Self, &'static str> {
        if generation == 0 {
            return Err("unpublished storage buffer generation");
        }
        Ok(Self(generation, slot))
    }

    /// Queue `data` for `offset` of this buffer (`CommandEncoder.writeToBuffer` on a
    /// `GpuBufferSlice`). `offset` and `data.len()` are multiples of 4 and the range lies inside
    /// the buffer. Writes apply in call order before the next frame.
    pub fn write_range(
        self,
        host: &(impl StorageBufferContext + ?Sized),
        offset: u64,
        data: &[u8],
    ) -> Result<(), BufferError> {
        host.storage_buffer_backend()
            .ok_or(BufferError::Unavailable)?
            .0
            .storage_buffer_write_range(self, offset, data)
    }

    /// The size in bytes this buffer was created with.
    pub fn size(self, host: &(impl StorageBufferContext + ?Sized)) -> Result<u64, BufferError> {
        host.storage_buffer_backend()
            .ok_or(BufferError::Unavailable)?
            .0
            .storage_buffer_size(self)
    }

    /// Immediately free this buffer and retire the handle ([`BufferError::Stale`] thereafter).
    /// Writes queued for it and not yet applied are dropped.
    pub fn release(self, host: &(impl StorageBufferContext + ?Sized)) -> Result<(), BufferError> {
        host.storage_buffer_backend()
            .ok_or(BufferError::Unavailable)?
            .0
            .storage_buffer_release(self)
    }

    /// The range `offset..offset + length` of this buffer, to bind with
    /// [`crate::DrawBindings::storage_buffer`] (`GpuBuffer.slice`). The range is checked when
    /// it is bound, not here.
    pub const fn slice(self, offset: u64, length: u64) -> StorageBufferSlice {
        StorageBufferSlice {
            buffer: self,
            offset,
            length,
        }
    }
}

impl crate::__private::game::Fields for StorageBufferRef {
    type Fields<'a> = crate::__private::game::ModelRefFields;
    fn fields(&self) -> Self::Fields<'_> {
        crate::__private::game::ModelRefFields {
            generation: self.0,
            slot: self.1,
        }
    }
}

/// A byte range of a [`StorageBufferRef`] (vanilla's `GpuBufferSlice`).
///
/// A binding takes a range of at most the device's `max_storage_buffer_binding_size` bytes, its
/// offset a multiple of `min_storage_buffer_offset_alignment`, its length a non-zero multiple of
/// 4. The buffer itself may be as large as `max_buffer_size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
// no wire: local client storage buffer range.
pub struct StorageBufferSlice {
    /// The buffer the range lies in.
    pub buffer: StorageBufferRef,
    /// First byte of the range.
    pub offset: u64,
    /// Length of the range in bytes.
    pub length: u64,
}

/// What [`crate::prelude::ClientRenderClientHost::create_storage_buffer`] makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
// no wire: local client storage buffer descriptor.
pub struct StorageBufferDescriptor {
    /// Size in bytes: at least 4, a multiple of 4 and at most the device's `max_buffer_size`.
    pub size: u64,
    /// Whether the buffer can also hold indirect draw or dispatch arguments (`wgpu`'s
    /// `BufferUsages::INDIRECT`; vanilla's `USAGE_INDIRECT_PARAMETERS`). Off by default.
    pub indirect: bool,
}

impl StorageBufferDescriptor {
    /// A storage buffer of `size` bytes.
    pub const fn new(size: u64) -> Self {
        Self {
            size,
            indirect: false,
        }
    }

    /// Let the buffer also hold indirect draw or dispatch arguments.
    pub const fn indirect(mut self, indirect: bool) -> Self {
        self.indirect = indirect;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_descriptor_starts_direct_and_a_slice_names_its_range() {
        let desc = StorageBufferDescriptor::new(64);
        assert_eq!((desc.size, desc.indirect), (64, false));
        assert!(desc.indirect(true).indirect);
        let buffer = StorageBufferRef::from_parts(3, 1).unwrap();
        let slice = buffer.slice(16, 32);
        assert_eq!((slice.buffer, slice.offset, slice.length), (buffer, 16, 32));
        assert!(StorageBufferRef::from_parts(0, 0).is_err());
    }

    #[test]
    fn storage_buffer_ref_routes_through_backend_and_refuses_without_one() {
        struct Bare;
        impl crate::Host for Bare {}
        crate::impl_host_areas!(Bare);
        impl crate::ClientHost for Bare {}
        crate::impl_client_host_areas!(Bare);

        let buf = StorageBufferRef::from_parts(1, 0).unwrap();
        assert_eq!(
            buf.write_range(&Bare, 0, &[0; 16]),
            Err(BufferError::Unavailable)
        );
        assert_eq!(buf.size(&Bare), Err(BufferError::Unavailable));
        assert_eq!(buf.release(&Bare), Err(BufferError::Unavailable));

        struct Recording {
            written: std::cell::RefCell<Option<(u64, Vec<u8>)>>,
        }
        impl crate::Host for Recording {
            fn backend(&self) -> Option<crate::BackendRef<'_>> {
                Some(crate::BackendRef::of(self))
            }
        }
        crate::impl_host_areas!(Recording);
        impl crate::ClientHost for Recording {}
        crate::impl_client_host_areas!(Recording);
        impl crate::Backend for Recording {}
        crate::impl_backend_areas!(Recording, except(client_render));
        impl crate::prelude::ClientRenderClientBackend for Recording {
            fn storage_buffer_write_range(
                &self,
                _: StorageBufferRef,
                offset: u64,
                data: &[u8],
            ) -> Result<(), BufferError> {
                *self.written.borrow_mut() = Some((offset, data.to_vec()));
                Ok(())
            }
            fn storage_buffer_size(&self, _: StorageBufferRef) -> Result<u64, BufferError> {
                Ok(64)
            }
            fn storage_buffer_release(&self, _: StorageBufferRef) -> Result<(), BufferError> {
                Err(BufferError::Stale)
            }
        }

        let host = Recording {
            written: std::cell::RefCell::new(None),
        };
        buf.write_range(&host, 8, &[1, 2, 3, 4]).unwrap();
        assert_eq!(*host.written.borrow(), Some((8, vec![1, 2, 3, 4])));
        assert_eq!(buf.size(&host), Ok(64));
        assert_eq!(buf.release(&host), Err(BufferError::Stale));
    }
}
