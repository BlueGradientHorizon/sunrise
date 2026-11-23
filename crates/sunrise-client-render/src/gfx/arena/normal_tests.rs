//! The normal stream beside an arena's vertices (`Arena::with_normals`): made, grown and moved
//! with its pages, and absent, the pages' bytes those of their vertices alone, without it.
use super::*;
use crate::mesher::TerrainVertex;

fn read(device: &wgpu::Device, queue: &wgpu::Queue, buffer: &wgpu::Buffer, offset: u64) -> Vec<u8> {
    let (tx, rx) = std::sync::mpsc::channel();
    wgpu::util::DownloadBuffer::read_buffer(
        device,
        queue,
        &buffer.slice(offset..offset + 4 * NORMAL_BYTES),
        move |result| tx.send(result.unwrap().to_vec()).unwrap(),
    );
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap()
}

/// A quad facing west (-x), U growing along +z: normal (-127, 0, 0), tangent (0, 0, 127), w 127.
fn west() -> [TerrainVertex; 4] {
    let uvs = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
    let corners = [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]];
    std::array::from_fn(|i| TerrainVertex::new(corners[i], uvs[i], 0, u32::MAX, 0))
}

const WEST: [i8; 8] = [-127, 0, 0, 0, 0, 0, 127, 127];

#[test]
#[ignore = "daily"]
fn normal_stream_grows_and_moves_with_its_pages() {
    let _gpu = crate::gfx::test_gpu::lock();
    let adapter = crate::gfx::test_gpu::adapter().expect("the arena's normal stream needs a GPU");
    eprintln!("ARENA_NORMALS_ADAPTER {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();

    // Without the stream: the pages hold their vertices alone.
    let mut plain = Arena::new(&device, 24, GRAIN, false);
    let mut encoder = device.create_command_encoder(&Default::default());
    let at = plain.alloc(&device, &mut encoder, 4).unwrap();
    assert!(!plain.has_normals());
    assert!(plain.normals(at.0).is_none());
    assert!(plain.normals_binding(at.0).is_none());
    assert_eq!(plain.bytes(), GRAIN * 24);
    let mut uploads = super::super::upload::Uploads::new(&device);
    plain.write_normals(&mut encoder, &mut uploads, at, &west());
    assert_eq!(plain.bytes(), GRAIN * 24, "no stream is made for a write");

    let mut arena = Arena::new(&device, 24, GRAIN, false).with_normals(true);
    arena.page_limit = GRAIN * 2;
    let first = arena.alloc(&device, &mut encoder, 4).unwrap();
    assert_eq!(arena.bytes(), GRAIN * (24 + 8));
    arena.write_normals(&mut encoder, &mut uploads, first, &west());
    // The page is full: the next range grows it, in the same encoder.
    let grown = arena.alloc(&device, &mut encoder, GRAIN).unwrap();
    assert_eq!(grown, (first.0, GRAIN), "grew the first page");
    assert_eq!(arena.bytes(), 2 * GRAIN * (24 + 8));
    // A third range starts a page of its own, and the quad moves there.
    let other = arena.alloc(&device, &mut encoder, GRAIN).unwrap();
    assert_ne!(other.0, first.0);
    assert_eq!(arena.bytes(), 4 * GRAIN * (24 + 8));
    arena.copy(&mut encoder, first, other, 4);
    uploads.finish(&encoder);
    queue.submit([encoder.finish()]);
    let expected: Vec<u8> = bytemuck::cast_slice(&[WEST; 4]).to_vec();
    assert_eq!(
        read(
            &device,
            &queue,
            arena.normals(first.0).unwrap(),
            first.1 * NORMAL_BYTES
        ),
        expected,
        "written before the grow, kept in the grown page"
    );
    assert_eq!(
        read(
            &device,
            &queue,
            arena.normals(other.0).unwrap(),
            other.1 * NORMAL_BYTES
        ),
        expected,
        "moved with its vertices"
    );
    // Draws bind the stream as far as the vertices: the grown page's two ranges.
    let bound = arena.normals_binding(first.0).unwrap();
    assert_eq!(bound.size(), 2 * GRAIN * NORMAL_BYTES);
    // A page opened for a shrink's moves has its stream too.
    let opened = arena.pages.len() as u32;
    assert!(arena.open_page(&device, GRAIN));
    assert!(
        arena.normals(opened).is_some(),
        "an opened page has a stream"
    );
    assert_eq!(arena.bytes(), 5 * GRAIN * (24 + 8));
    // The opened page holds no range: trimmed, its vertices and its stream go back together.
    assert_eq!(arena.trim().len(), 2, "the empty page's buffer and stream");
    // Two pages left, each a buffer and a stream.
    assert_eq!(arena.clear().len(), 4);
    assert_eq!(arena.bytes(), 0);

    // A mesh longer than a chunk of the stream (`terrain_normals::CHUNK`, 1024 vertices): 256
    // west quads, then an up one at vertices 1024 to 1027, whose entries land past the chunk.
    let mut long = Arena::new(&device, 24, GRAIN, false).with_normals(true);
    let mut encoder = device.create_command_encoder(&Default::default());
    let mut uploads = super::super::upload::Uploads::new(&device);
    let at = long.alloc(&device, &mut encoder, 1028).unwrap();
    let mut vertices: Vec<TerrainVertex> = (0..256).flat_map(|_| west()).collect();
    let uvs = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
    let up = [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]];
    vertices.extend((0..4).map(|i| TerrainVertex::new(up[i], uvs[i], 0, u32::MAX, 0)));
    long.write_normals(&mut encoder, &mut uploads, at, &vertices);
    uploads.finish(&encoder);
    queue.submit([encoder.finish()]);
    let stream = long.normals(at.0).unwrap();
    let up_entry: [i8; 8] = [0, 127, 0, 0, 127, 0, 0, 127];
    assert_eq!(
        read(&device, &queue, stream, (at.1 + 1020) * NORMAL_BYTES),
        bytemuck::cast_slice::<[i8; 8], u8>(&[WEST; 4]).to_vec(),
        "the last quad of the first chunk"
    );
    assert_eq!(
        read(&device, &queue, stream, (at.1 + 1024) * NORMAL_BYTES),
        bytemuck::cast_slice::<[i8; 8], u8>(&[up_entry; 4]).to_vec(),
        "the quad past the first chunk"
    );
}
