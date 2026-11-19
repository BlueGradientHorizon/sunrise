//! `centerDepthSmooth`'s input: `depthtex0` at the centre of the screen (re-exported from sunrise_render_shaderpack).
#[allow(unused_imports)]
pub use sunrise_render_shaderpack::pack_center_depth::*;

#[cfg(test)]
mod tests {
    const SLOTS: usize = 3;
    use super::*;

    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let adapter = crate::gfx::test_gpu::adapter()?;
        Some(crate::gfx::test_gpu::device(&adapter))
    }

    fn clear(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: f32) {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
    }

    /// Inline and on the submit thread: each finished frame's depth, then none left to
    /// take; several outstanding samples are refused.
    #[test]
    #[ignore = "daily"]
    fn a_finished_frame_s_depth_is_taken_alone_never_an_older_one() {
        let _gpu = crate::gfx::test_gpu::lock();
        let Some((device, raw)) = test_device() else {
            return;
        };
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 3,
                height: 3,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = depth.create_view(&Default::default());
        for threaded in [false, true] {
            let queue = super::super::submit::Queue::new(raw.clone(), threaded);
            if threaded {
                assert!(queue.threaded(), "deferred submit unavailable");
            }
            let mut center = CenterDepth::new(&device);
            assert!(center.take_completed(&device).is_err());
            for value in [0.125, 0.75, 0.25] {
                let mut encoder = device.create_command_encoder(&Default::default());
                clear(&mut encoder, &view, value);
                center.copy(&device, &mut encoder, &depth);
                let after = center.submitted().expect("one copied sample");
                if threaded {
                    let job = super::super::submit::Job {
                        encoder,
                        surface: None,
                        after: Some(Box::new(after)),
                    };
                    assert!(queue.submit_later(job).is_ok());
                    queue.wait();
                } else {
                    queue.submit([encoder.finish()]);
                    after();
                }
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: Some(std::time::Duration::from_secs(30)),
                    })
                    .unwrap();
                assert_eq!(center.take_completed(&device).unwrap(), value);
                assert_eq!(center.collect(&device), Some(value));
                assert!(
                    center.take_completed(&device).is_err(),
                    "no stale latest fallback"
                );
            }
            // Several outstanding copies are ambiguous, even after they all map.
            let mut encoder = device.create_command_encoder(&Default::default());
            for _ in 0..SLOTS {
                center.copy(&device, &mut encoder, &depth);
            }
            queue.submit([encoder.finish()]);
            center.submitted().unwrap()();
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert!(center.take_completed(&device).is_err());
        }
    }

    /// Frames that go to the submit thread start their readback there: the centre depth
    /// follows the frames instead of keeping the first ones' depth.
    #[test]
    fn the_centre_depth_follows_frames_submitted_on_the_submit_thread() {
        let _gpu = crate::gfx::test_gpu::lock();
        let Some((device, queue)) = test_device() else {
            return;
        };
        let queue = super::super::submit::Queue::new(queue, true);
        if !queue.threaded() {
            return;
        }
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = depth.create_view(&Default::default());
        let mut center = CenterDepth::new(&device);
        let mut latest = None;
        for frame in 1..=4 * SLOTS {
            let value = frame as f32 / 16.0;
            let mut encoder = device.create_command_encoder(&Default::default());
            clear(&mut encoder, &view, value);
            center.copy(&device, &mut encoder, &depth);
            let after = center
                .submitted()
                .map(|f| Box::new(f) as super::super::submit::AfterSubmit);
            let job = super::super::submit::Job {
                encoder,
                surface: None,
                after,
            };
            assert!(queue.submit_later(job).is_ok());
            queue.wait();
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            latest = center.collect(&device);
            assert_eq!(latest, Some(value), "frame {frame}");
        }
        assert_eq!(latest, Some(0.75));
    }
}
