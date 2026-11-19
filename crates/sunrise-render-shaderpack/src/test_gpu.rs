//! GPU test helpers for sunrise-render-shaderpack.

use std::sync::{Mutex, MutexGuard};

static GPU_TEST: Mutex<()> = Mutex::new(());

pub(crate) fn lock() -> MutexGuard<'static, ()> {
    GPU_TEST.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
pub(crate) fn instance() -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::PRIMARY;
    }
    wgpu::Instance::new(descriptor.with_env())
}
