//! The mesh visibility design's acceptance cases that need the real mesher and prepared
//! resources (`SUNRISE_MESH_TEST_RESOURCES`), like the other mesher geometry tests.
use super::super::{BiomeZoom, LeafCullingMode, MeshScratch, MeshTables, Mesher};
use super::*;
use crate::{
    assets::{Assets, RenderAssets},
    biome::Biomes,
    blocks::BlockFacts,
    chunk::{Chunk, MIN_Y},
    models,
};

struct Fixture {
    assets: Assets,
    biomes: Biomes,
    render: RenderAssets,
    visibility: sunrise_world::block_render::FaceVisibility,
    tables: MeshTables,
}

fn fixture() -> Fixture {
    let root = std::path::PathBuf::from(
        std::env::var_os("SUNRISE_MESH_TEST_RESOURCES").expect("SUNRISE_MESH_TEST_RESOURCES"),
    );
    let source = sunrise_assets::resource_source::ResourceRoot::directory(root.clone());
    let assets = Assets::load(&root).unwrap();
    let facts = BlockFacts::load(&root, assets.registry).unwrap();
    let biomes = Biomes::load(&source).unwrap();
    let render = assets.reload(&source).unwrap();
    let visibility = models::load_visibility(&root, assets.registry).unwrap();
    let tables = MeshTables::new(&assets, &render, &facts, visibility.clone(), None).unwrap();
    Fixture {
        assets,
        biomes,
        render,
        visibility,
        tables,
    }
}

fn state(f: &Fixture, name: &str) -> u32 {
    f.assets.registry.default_state(name).unwrap().0
}

const BASE: i32 = MIN_Y + 4;

fn mesh(f: &Fixture, chunk: &Chunk, plan: Option<&SectionVisibilityPlan>) -> SectionMesh {
    let region = [[Some(chunk); 3]; 3];
    let mut mesher = Mesher {
        nether_shading: false,
        shader_pack_lighting: false,
        separate_ao: false,
        shader_pack: false,
        assets: &f.assets,
        render: &f.render,
        tables: &f.tables,
        biomes: &f.biomes,
        visibility: f.visibility.clone(),
        zoom: BiomeZoom::new(0),
        blend_radius: 0,
        smooth_lighting: true,
        cutout_leaves: true,
        opaque_ice: false,
        translucent_layers: &[],
        leaf_culling: LeafCullingMode::Off,
        hidden_states: &[],
    };
    let mut scratch = MeshScratch::default();
    mesher.mesh_planned(&region, 0, SectionMesh::default(), &mut scratch, plan)
}

fn quads(mesh: &SectionMesh) -> usize {
    (mesh.solid.len() + mesh.translucent.len()) / 4
}

fn plan(edit: impl FnOnce(&mut SectionPlanBuilder)) -> SectionVisibilityPlan {
    let mut builder = SectionPlanBuilder::new();
    edit(&mut builder);
    plan_section(builder.finish().as_ref(), |_| None).unwrap()
}

fn stone(f: &Fixture, cells: &[[i32; 3]]) -> Chunk {
    let mut chunk = Chunk::empty(0, 0);
    for &[x, y, z] in cells {
        chunk.set(x, BASE + y, z, state(f, "minecraft:stone"));
    }
    chunk
}

#[test]
#[ignore = "daily"]
fn a_face_mask_removes_faces_and_hidden_cells_open_their_neighbours() {
    let f = fixture();
    let one = stone(&f, &[[8, 0, 8]]);
    assert_eq!(quads(&mesh(&f, &one, None)), 6);
    let all = plan(|b| b.restrict([8, 4, 8], true, true, 63));
    assert_eq!(quads(&mesh(&f, &one, Some(&all))), 6);
    let no_up = plan(|b| b.restrict([8, 4, 8], true, true, 61));
    assert_eq!(quads(&mesh(&f, &one, Some(&no_up))), 5);
    let two = stone(&f, &[[8, 0, 8], [9, 0, 8]]);
    assert_eq!(quads(&mesh(&f, &two, None)), 10);
    let hidden = plan(|b| b.restrict([9, 4, 8], false, false, 0));
    assert_eq!(
        quads(&mesh(&f, &two, Some(&hidden))),
        6,
        "the hidden cube draws nothing; its neighbour's face toward it shows"
    );
    // A cube whose west face was removed: it loses that face (already culled), and its
    // neighbour's east face toward it shows, as it no longer occludes there.
    let open_west = plan(|b| b.restrict([9, 4, 8], true, true, ALL_FACES_BUT_WEST));
    assert_eq!(quads(&mesh(&f, &two, Some(&open_west))), 11);
}

/// Every face kept but the west one (bit 4).
const ALL_FACES_BUT_WEST: u8 = 0b10_1111;

#[test]
#[ignore = "daily"]
fn hide_terrain_keeps_only_block_entities() {
    let f = fixture();
    let mut chunk = stone(&f, &[[1, 0, 1]]);
    chunk.set(3, BASE, 3, state(&f, "minecraft:water"));
    chunk.set(5, BASE, 5, state(&f, "minecraft:chest"));
    let full = mesh(&f, &chunk, None);
    assert!(quads(&full) > 6);
    let hidden = mesh(&f, &chunk, Some(&plan(SectionPlanBuilder::hide_terrain)));
    assert_eq!(quads(&hidden), 0);
    assert_eq!(hidden.block_entities, full.block_entities);
    assert_eq!(hidden.block_entities.len(), 1);
    assert_eq!(hidden.visibility, ALL_VISIBLE);
}

#[test]
#[ignore = "daily"]
fn masked_translucent_faces_stay_in_the_translucent_pass() {
    let f = fixture();
    let mut chunk = Chunk::empty(0, 0);
    chunk.set(2, BASE, 2, state(&f, "minecraft:glass"));
    chunk.set(5, BASE, 2, state(&f, "minecraft:water"));
    let full = mesh(&f, &chunk, None);
    let masked = mesh(
        &f,
        &chunk,
        Some(&plan(|b| {
            b.restrict([2, 4, 2], true, true, 61);
            b.restrict([5, 4, 2], true, true, 61);
        })),
    );
    // Glass and water each lose their up face; what is left stays translucent.
    assert!(masked.translucent.len() < full.translucent.len());
    assert_eq!(masked.solid.len(), full.solid.len());
}

/// The no-mod path is the mesher of before: no plan meshes byte for byte as `mesh_reusing`, and
/// a plan that keeps every cell (which the world thread never sends; it sends none) meshes the
/// same bytes through the masked path.
#[test]
#[ignore = "daily"]
fn without_a_plan_meshes_are_byte_identical() {
    let f = fixture();
    let mut chunk = Chunk::empty(0, 0);
    let names = [
        "minecraft:stone",
        "minecraft:glass",
        "minecraft:water",
        "minecraft:oak_leaves",
        "minecraft:chest",
        "minecraft:torch",
        "minecraft:oak_slab",
        "minecraft:grass_block",
    ];
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..6 {
                let pick = (x * 7 + z * 3 + y * 5) as usize % (names.len() + 2);
                if let Some(name) = names.get(pick) {
                    chunk.set(x, BASE + y, z, state(&f, name));
                }
            }
        }
    }
    let region = [[Some(&chunk); 3]; 3];
    let mut mesher = Mesher {
        nether_shading: false,
        shader_pack_lighting: false,
        separate_ao: false,
        shader_pack: false,
        assets: &f.assets,
        render: &f.render,
        tables: &f.tables,
        biomes: &f.biomes,
        visibility: f.visibility.clone(),
        zoom: BiomeZoom::new(0),
        blend_radius: 0,
        smooth_lighting: true,
        cutout_leaves: true,
        opaque_ice: false,
        translucent_layers: &[],
        leaf_culling: LeafCullingMode::Off,
        hidden_states: &[],
    };
    let mut scratch = MeshScratch::default();
    let before = mesher.mesh_reusing(&region, 0, SectionMesh::default(), &mut scratch);
    let none = mesher.mesh_planned(&region, 0, SectionMesh::default(), &mut scratch, None);
    let keep_all = SectionVisibilityPlan {
        hide_terrain: false,
        cells: vec![ALL; P * P * P].into_boxed_slice(),
    };
    let kept = mesher.mesh_planned(
        &region,
        0,
        SectionMesh::default(),
        &mut scratch,
        Some(&keep_all),
    );
    for other in [&none, &kept] {
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&before.solid),
            bytemuck::cast_slice::<_, u8>(&other.solid)
        );
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&before.translucent),
            bytemuck::cast_slice::<_, u8>(&other.translucent)
        );
        assert_eq!(before.solid_sources, other.solid_sources);
        assert_eq!(before.visibility, other.visibility);
        assert_eq!(before.block_entities, other.block_entities);
        assert_eq!(before.lights, other.lights);
        assert_eq!(before.sprite_layers, other.sprite_layers);
    }
    assert!(quads(&before) > 100);
}
