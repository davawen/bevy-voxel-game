use std::time::{Instant, Duration};

use bevy::{
    math::DVec3,
    prelude::*,
    render::mesh::Indices
};
use itertools::Itertools;
use noise::NoiseFn;

use crate::{Noise, manager::{ChunkManager, CHUNK_SIZE, WORLD_HEIGHT}};

#[derive(Default, Clone, Copy)]
pub enum Block {
    #[default]
    Air,
    Dirt,
}

impl Block {
    pub fn transparent(&self) -> bool {
        use Block::*;
        match self {
            Air => true,
            Dirt => false,
        }
    }

    /// Returns wether the given block is a full block
    pub fn full(&self) -> bool {
        use Block::*;
        match self {
            Air => false,
            Dirt => true,
        }
    }
}


#[derive(Component)]
pub struct Chunk {
    pub key: IVec3,
    // mesh_generated: bool
}

#[derive(Component, bevy_inspector_egui::Inspectable)]
pub struct NeedsMesh(pub u32);

pub fn generate_terrain(
    query: Query<(&Chunk, &Transform)>,
    mut manager: ResMut<ChunkManager>,
    noise: Res<Noise>,
) {
    let start = Instant::now();
    for (chunk, transform) in query.iter() {
        let Some(data) = manager.chunks.get_mut(&chunk.key) else { 
            continue;
        };
        if data.generated {
            continue;
        }

        for (x, z) in (0..CHUNK_SIZE).cartesian_product(0..CHUNK_SIZE) {
            let mut pos = DVec3::new(x as f64, 0.0, z as f64) + transform.translation.as_dvec3();
            pos.y = 0.0;
            let height = noise.0.get((pos / 32.0).to_array()) / 2.0 + 0.5;
            let height = height as f32 * CHUNK_SIZE as f32 * WORLD_HEIGHT as f32;

            for y in 0..CHUNK_SIZE {
                let y_real = y as f32 + transform.translation.y;
                data.data[z][y][x] = if y_real < height {
                    Block::Dirt
                } else {
                    Block::Air
                }
            }
        }

        data.generated = true;
        // Limit chunk generation to 5ms
        if Instant::now() - start > Duration::from_millis(5) { break }
    }
}

pub fn generate_mesh(
    mut commands: Commands,
    query: Query<(Entity, &Chunk, &Handle<Mesh>, &NeedsMesh)>,
    manager: Res<ChunkManager>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let start = Instant::now();
    for (entity, chunk, mesh, &NeedsMesh(lod)) in query.iter() {
        let Some(data) = manager.chunks.get(&chunk.key) else { continue; };

        if !data.generated || ChunkManager::adjacent_keys(chunk.key).any(|c| !manager.is_generated(c)) {
            continue;
        }

        let mut vertices = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();

        let mut add_face = |local_pos: IVec3, dir: IVec3, points: [Vec3; 4]| {
            // Return early if the adjacent face is not visible
            let lod_num = 2u32.pow(lod);
            if manager
                .get_with_adjacent(chunk.key, local_pos + dir*lod_num as i32)
                .unwrap_or(Block::Air)
                .full()
            {
                return;
            }

            // if dir.abs() != IVec3::Y {
            //     let increment = if dir.abs() == IVec3::X { IVec3::Z } else { IVec3::X };
            //     for 
            // }

            let idx = vertices.len() as u32;

            let lod_multiplier = Vec3::new(lod_num as f32, 1.0, lod_num as f32);
            let pos = local_pos.as_vec3() + Vec3::splat(0.5) * lod_multiplier;
            for p in points {
                vertices.push((pos + p*lod_multiplier).to_array());
                normals.push(dir.as_vec3().to_array());
            }
            indices.extend_from_slice(&[idx + 2, idx + 1, idx, idx, idx + 3, idx + 2]);
        };

        for (block, x, y, z) in data.all_blocks_lod(lod) {
            if block.transparent() {
                continue;
            }

            let local_pos = IVec3::new(x as i32, y as i32, z as i32);

            add_face(
                local_pos,
                IVec3::Y,
                [
                    Vec3::new(0.5, 0.5, -0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                    Vec3::new(-0.5, 0.5, 0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                ],
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
            );
        }

        let mesh = meshes.get_mut(mesh).unwrap();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.set_indices(Some(Indices::U32(indices)));

        commands.entity(entity).remove::<NeedsMesh>();

        if Instant::now() - start > Duration::from_millis(5) { break }
    }
}
