use std::{time::{Instant, Duration}, sync::{Arc, Mutex}};

use bevy::{
    math::DVec3,
    prelude::*,
    render::mesh::Indices, ecs::component
};
use itertools::Itertools;
use noise::NoiseFn;

use crate::{Noise, manager::{ChunkManager, CHUNK_SIZE, WORLD_HEIGHT, ChunkData}};

#[derive(Default, Clone, Copy)]
pub enum Block {
    #[default]
    Air,
    Grass,
    Dirt,
    Stone,
}

impl Block {
    pub fn transparent(&self) -> bool {
        use Block::*;
        match self {
            Air => true,
            _ => false
        }
    }

    /// Returns wether the given block is a full block
    pub fn full(&self) -> bool {
        use Block::*;
        match self {
            Air => false,
            _ => true
        }
    }

    pub fn uvs(&self) -> Option<(Vec2, Vec2)> {
        const TEXTURE_BLOCK_SIZE: f32 = 16.0;
        const ATLAS_SIZE: f32 = 256.0;

        use Block::*;
        let atlas_coordinate = match self {
            Grass => Some(IVec2::new(0, 0)),
            Dirt => Some(IVec2::new(1, 0)),
            Stone => Some(IVec2::new(2, 0)),
            _ => None
        };

        atlas_coordinate
            .and_then(|c| Some((
                c.as_vec2()*TEXTURE_BLOCK_SIZE / ATLAS_SIZE,
                ((c + 1).as_vec2()*TEXTURE_BLOCK_SIZE - 1.0) / ATLAS_SIZE
            )))
            //.and_then(|(p1, p2)| Some((p1.as_vec2() / 256.0, p2.as_vec2() / 256.0))) // divide by atlas size
    }
}


#[derive(Component)]
pub struct Chunk {
    pub key: IVec3,
    // mesh_generated: bool
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NeedsTerrain;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NeedsMesh(pub u32);

pub fn generate_terrain(
    commands: Commands,
    query: Query<(Entity, &Chunk, &Transform), With<NeedsTerrain>>,
    manager: ResMut<ChunkManager>,
    noise: Res<Noise>,
) {
    // let start = Instant::now();
    let commands = Arc::new(Mutex::new(commands));
    let manager = Arc::new(Mutex::new(manager));

    query.par_for_each(10, |(entity, chunk, transform)| {
        // If the chunk isn't yet loaded or it's already generated, skip it
        if !manager.lock().unwrap().is_generated(chunk.key) {
            let mut data = ChunkData::default();
            for (x, z) in (0..CHUNK_SIZE).cartesian_product(0..CHUNK_SIZE) {
                let mut pos = DVec3::new(x as f64, 0.0, z as f64) + transform.translation.as_dvec3();
                pos.y = 0.0;
                let height = noise.0.get((pos / 32.0).to_array()) / 2.0 + 0.5;
                let height = height as f32 * CHUNK_SIZE as f32 * WORLD_HEIGHT as f32;
                let height = height as usize;

                for y in 0..CHUNK_SIZE {
                    let y_real = y as usize + chunk.key.y as usize * CHUNK_SIZE;
                    data.data[z][y][x] = if y_real > height {
                            Block::Air
                        } else if y_real == height {
                            Block::Grass
                        } else if y_real > height-3 {
                            Block::Dirt
                        } else {
                            Block::Stone
                        }
                }
            }

            data.generated = true;

            *manager.lock().unwrap().chunks.get_mut(&chunk.key).unwrap() = data;
        }

        commands.lock().unwrap().entity(entity).remove::<NeedsTerrain>();
        // Limit chunk generation to 5ms
        // if Instant::now() - start > Duration::from_millis(5) { break }
    });
}

pub fn generate_mesh(
    commands: Commands,
    query: Query<(Entity, &Chunk, &Handle<Mesh>, &NeedsMesh)>,
    manager: Res<ChunkManager>,
    meshes: ResMut<Assets<Mesh>>,
) {
    // let start = Instant::now();
    let commands = Arc::new(Mutex::new(commands));
    let meshes = Arc::new(Mutex::new(meshes));
    query.par_for_each(10, |(entity, chunk, mesh, &NeedsMesh(lod))| {
        let Some(data) = manager.chunks.get(&chunk.key) else { return; };

        if !data.generated || ChunkManager::adjacent_keys(chunk.key).any(|c| !manager.is_generated(c)) {
            return;
        }

        let mut vertices = Vec::new();
        let mut normals = Vec::new();
        let mut texture_coordinates = Vec::new();
        let mut indices = Vec::new();

        let mut add_face = |local_pos: IVec3, dir: IVec3, points: [Vec3; 4], uvs: &[Vec2; 4]| {
            // Return early if the adjacent face is not visible
            let lod_num = 2u32.pow(lod);
            if manager
                .get_with_adjacent(chunk.key, local_pos + dir*lod_num as i32)
                .unwrap_or(Block::Air)
                .full()
            {
                return;
            }

            let idx = vertices.len() as u32;

            let lod_multiplier = Vec3::new(lod_num as f32, 1.0, lod_num as f32);
            let pos = local_pos.as_vec3() + Vec3::splat(0.5) * lod_multiplier;
            for p in points {
                vertices.push((pos + p*lod_multiplier).to_array());
                normals.push(dir.as_vec3().to_array());
            }
            texture_coordinates.extend_from_slice(uvs);
            indices.extend_from_slice(&[idx + 2, idx + 1, idx, idx, idx + 3, idx + 2]);
        };

        for (block, x, y, z) in data.all_blocks_lod(lod) {
            if block.transparent() {
                continue;
            }

            let local_pos = IVec3::new(x as i32, y as i32, z as i32);
            let (uv0, uv1) = block.uvs().unwrap_or((Vec2::splat(240.0 / 256.0), Vec2::splat(1.0)));
            let uvs = &[ uv0, Vec2::new(uv1.x, uv0.y), uv1, Vec2::new(uv0.x, uv1.y) ];

            add_face(
                local_pos,
                IVec3::Y,
                [
                    Vec3::new(0.5, 0.5, -0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                    Vec3::new(-0.5, 0.5, 0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                ],
                uvs
            );

            add_face(
                local_pos,
                IVec3::NEG_Y,
                [
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(-0.5, -0.5, 0.5),
                ],
                uvs
            );

            add_face(
                local_pos,
                IVec3::X,
                [
                    Vec3::new(0.5, 0.5, 0.5),
                    Vec3::new(0.5, 0.5, -0.5),
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(0.5, -0.5, 0.5),
                ],
                uvs
            );

            add_face(
                local_pos,
                IVec3::NEG_X,
                [
                    Vec3::new(-0.5, -0.5, 0.5),
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                    Vec3::new(-0.5, 0.5, 0.5),
                ],
                uvs
            );

            add_face(
                local_pos,
                IVec3::Z,
                [
                    Vec3::new(-0.5, 0.5, 0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(-0.5, -0.5, 0.5),
                ],
                uvs
            );

            add_face(
                local_pos,
                IVec3::NEG_Z,
                [
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(0.5, 0.5, -0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                ],
                uvs
            );
        }

        let mut meshes = meshes.lock().unwrap();
        let mesh = meshes.get_mut(mesh).unwrap();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, texture_coordinates);
        mesh.set_indices(Some(Indices::U32(indices)));

        // drop(mesh_lock);

        commands.lock().unwrap().entity(entity).remove::<NeedsMesh>();

        // if Instant::now() - start > Duration::from_millis(5) { break }
    });
}
