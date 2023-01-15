use std::{f32::consts::PI, time::{Duration, Instant}};

use bevy::{
    math::DVec3,
    prelude::*,
    render::{mesh::Indices, render_resource::PrimitiveTopology},
    utils::HashMap,
    window::CursorGrabMode,
};
use bevy_inspector_egui::{WorldInspectorPlugin, RegisterInspectable};
use itertools::Itertools;
use noise::{NoiseFn, OpenSimplex};

mod camera;

#[derive(Default, Clone, Copy)]
enum Block {
    #[default]
    Air,
    Dirt,
}

impl Block {
    fn transparent(&self) -> bool {
        use Block::*;
        match self {
            Air => true,
            Dirt => false,
        }
    }

    /// Returns wether the given block is a full block
    fn full(&self) -> bool {
        use Block::*;
        match self {
            Air => false,
            Dirt => true,
        }
    }
}

const CHUNK_SIZE: usize = 16;
/// Number of chunks constituting the world vertically
const WORLD_HEIGHT: i32 = 128 / CHUNK_SIZE as i32;

#[derive(Default, Resource)]
struct ChunkManager {
    chunks: HashMap<IVec3, ChunkData>,
    /// List of create meshes and their respective lod
    meshes: HashMap<IVec3, (Entity, u32)>
}

impl ChunkManager {
    /// Gives the block in the given chunk if pos is in bounds, or a block in an adjacent chunk if pos is out of bounds
    /// @returns None if the adjacent chunk isn't generated
    fn get_with_adjacent(&self, key: IVec3, pos: IVec3) -> Option<Block> {
        // Get the chunk offset
        let chunk_key = key
            + IVec3::new(
                pos.x.div_euclid(CHUNK_SIZE as i32),
                pos.y.div_euclid(CHUNK_SIZE as i32),
                pos.z.div_euclid(CHUNK_SIZE as i32),
            );

        let chunk = self.chunks.get(&chunk_key)?;

        if chunk.generated {
            let pos = IVec3::new(
                pos.x.rem_euclid(CHUNK_SIZE as i32),
                pos.y.rem_euclid(CHUNK_SIZE as i32),
                pos.z.rem_euclid(CHUNK_SIZE as i32),
            );
            Some(chunk.get_unchecked(pos))
        } else {
            None
        }
    }

    fn is_loaded(&self, key: IVec3) -> bool {
        self.chunks.contains_key(&key)
    }

    fn is_generated(&self, key: IVec3) -> bool {
        if let Some(c) = self.chunks.get(&key) {
            c.generated
        }
        else {
            false
        }
    }

    fn entity_created(&self, key: IVec3) -> bool {
        self.meshes.contains_key(&key)
    }

    /// @returns An iterator to all valid adjacent chunks keys
    fn adjacent_keys(key: IVec3) -> impl Iterator<Item = IVec3> {
        (-1..=1)
            .cartesian_product(-1..=1)
            .cartesian_product(-1..=1)
            .map(move |((x, y), z)| IVec3::new(x, y, z) + key)
            .filter(move |v| v != &key)
            .filter(|&v| Self::in_world_range(v))
    }

    /// Checks if a chunk key is valid in the world
    fn in_world_range(key: IVec3) -> bool {
        (0..WORLD_HEIGHT).contains(&key.y)
    }
}

struct ChunkData {
    data: [[[Block; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
    generated: bool,
}

macro_rules! decompose_vec_into {
    ($vec:expr, $type:ty) => {
        ($vec.x as $type, $vec.y as $type, $vec.z as $type)
    };
}

impl ChunkData {
    fn get_unchecked(&self, p: IVec3) -> Block {
        let (x, y, z) = decompose_vec_into!(p, usize);
        self.data[z][y][x]
    }

    fn get(&self, p: IVec3) -> Option<Block> {
        if p.cmplt(IVec3::ZERO).any() || p.cmpge(IVec3::splat(CHUNK_SIZE as i32)).any() {
            None
        } else {
            Some(self.get_unchecked(p))
        }
    }

    /// @returns An iterator over only 2 dimensions of the chunk
    #[inline]
    fn slice() -> impl Iterator<Item = (usize, usize)> {
        (0..CHUNK_SIZE).cartesian_product(0..CHUNK_SIZE)
    }

    #[inline]
    fn all() -> impl Iterator<Item = (usize, usize, usize)> {
        Self::slice()
            .cartesian_product(0..CHUNK_SIZE)
            .map(|((x, y), z)| (x, y, z))
    }

    fn all_blocks(&self) -> impl Iterator<Item = (Block, usize, usize, usize)> + '_ {
        Self::all().map(|(x, y, z)| (self.data[z][y][x], x, y, z))
    }

    #[inline]
    fn slice_lod(lod: u32) -> impl Iterator<Item = (usize, usize)> {
        let base = (0..CHUNK_SIZE).step_by(2usize.pow(lod));

        base.clone().cartesian_product(base)
    }

    /// @returns An iterator over 3 dimensions of the chunk, with xz distorted by the given lod
    #[inline]
    fn all_lod(lod: u32) -> impl Iterator<Item = (usize, usize, usize)> {
        Self::slice_lod(lod)
            .cartesian_product(0..CHUNK_SIZE)
            .map(|((x, z), y)| (x, y, z))
    }

    fn all_blocks_lod(&self, lod: u32) -> impl Iterator<Item = (Block, usize, usize, usize)> + '_ {
        Self::all_lod(lod).map(|(x, y, z)| (self.data[z][y][x], x, y, z))
    }
}

#[derive(Component)]
struct Chunk {
    key: IVec3,
    // mesh_generated: bool
}

#[derive(Component, bevy_inspector_egui::Inspectable)]
struct NeedsMesh(u32);

#[derive(Resource)]
struct Noise(OpenSimplex);

fn generate_terrain(
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

fn generate_mesh(
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

// Specified at half size
const RENDER_DISTANCE: i32 = 16;
const LOD_RANGE: i32 = 4;
fn load_chunks(
    mut commands: Commands,
    mut manager: ResMut<ChunkManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<Camera>>,
) {
    let player = player.single();
    let mut player_key = (player.translation / CHUNK_SIZE as f32).as_ivec3();
    player_key.y = 0;

    for ((i, j), k) in (-RENDER_DISTANCE..=RENDER_DISTANCE).cartesian_product(0..WORLD_HEIGHT).cartesian_product(-RENDER_DISTANCE..=RENDER_DISTANCE) {
        let key = IVec3::new(i, j, k) + player_key;
        if !ChunkManager::in_world_range(key) {
            continue;
        }

        if !manager.is_loaded(key) {
            manager.chunks.insert(
                key,
                ChunkData {
                    generated: false,
                    data: default(),
                },
            );
        }

        let lod = (i / LOD_RANGE).abs().max((k / LOD_RANGE).abs()) as u32;
        let lod = lod.min((CHUNK_SIZE as f32).log2() as u32); // limit lod level to chunk size

        if let Some((entity, loaded_lod)) = manager.meshes.get_mut(&key) {
            if *loaded_lod == lod { continue; }

            // eprintln!("Recreating mesh of {key}");
            commands.entity(*entity).insert(NeedsMesh(lod));
            *loaded_lod = lod;
        }
        else {
            let entity = commands
                .spawn((
                    Chunk { key },
                    NeedsMesh(lod),
                    PbrBundle {
                        mesh: meshes.add(Mesh::new(PrimitiveTopology::TriangleList)),
                        material: materials.add(Color::WHITE.into()),
                        transform: Transform::from_translation(key.as_vec3() * CHUNK_SIZE as f32),
                        ..default()
                    },
                ))
                .id();
            manager.meshes.insert(key, (entity, lod));
        }
    }
}

fn unload_chunks(
    mut commands: Commands,
    mut manager: ResMut<ChunkManager>,
    chunks: Query<(Entity, &Chunk)>,
    player: Query<&Transform, With<Camera>>,
) {
    let player = player.single();
    let mut player_key = (player.translation / CHUNK_SIZE as f32).as_ivec3();
    player_key.y = 0;

    for (entity, chunk) in chunks.iter() {
        let relative_key = chunk.key - player_key;

        if relative_key
            .cmplt(IVec3::new(
                -RENDER_DISTANCE,
                0,
                -RENDER_DISTANCE,
            ))
            .any()
            || relative_key
                .cmpgt(IVec3::new(
                    RENDER_DISTANCE,
                    WORLD_HEIGHT,
                    RENDER_DISTANCE,
                ))
                .any()
        {
            // WARNING: REMEMBER TO ADD THIS BACK
            // MEMORY LEAK
            // manager.chunks.remove(&chunk.key);
            commands.entity(entity).despawn();
            manager.meshes.remove(&chunk.key);
        }
    }
}

fn startup(
    mut commands: Commands,
    mut windows: ResMut<Windows>,
) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_rotation(
            Quat::from_rotation_x(-1.5) * Quat::from_rotation_y(1.0),
        ),
        ..default()
    });

    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 90.0, 0.0),
        projection: Projection::Perspective(PerspectiveProjection {
            fov: PI / 2.0,
            ..default()
        }),
        ..default()
    });

    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_grab_mode(CursorGrabMode::Locked);
    window.set_cursor_visibility(false);
}

fn main() {
    App::new()
        .insert_resource(Noise(OpenSimplex::new(102)))
        .insert_resource(camera::CameraDisabled(false))
        .insert_resource(ChunkManager::default())
        .add_plugins(DefaultPlugins)
        .add_plugin(WorldInspectorPlugin::default())
        .register_inspectable::<NeedsMesh>()
        .add_startup_system(startup)
        .add_system(camera::move_camera)
        .add_system(generate_terrain)
        .add_system(generate_mesh)
        .add_system(load_chunks)
        .add_system(unload_chunks)
        .run()
}
