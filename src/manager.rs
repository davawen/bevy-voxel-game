use bevy::{prelude::*, utils::HashMap};
use itertools::Itertools;

use crate::{chunk::{NeedsMesh, NeedsTerrain, Chunk}, AtlasImage, block::Block};

#[derive(Default, Resource)]
pub struct ChunkManager {
    pub chunks: HashMap<IVec3, ChunkData>,
    /// List of create meshes and their respective lod
    pub meshes: HashMap<IVec3, (Entity, u32)>
}

impl ChunkManager {
    /// Gives the block in the given chunk if pos is in bounds, or a block in an adjacent chunk if pos is out of bounds
    /// @returns None if the adjacent chunk isn't generated
    pub fn get_with_adjacent(&self, key: IVec3, pos: IVec3) -> Option<Block> {
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

    /// @returns A tuple of the key of the key of the chunk and the position inside the chunk
    pub fn get_keys(global_pos: IVec3) -> (IVec3, IVec3) {
        let key = IVec3::new(
            global_pos.x.div_euclid(CHUNK_SIZE as i32),
            global_pos.y.div_euclid(CHUNK_SIZE as i32),
            global_pos.z.div_euclid(CHUNK_SIZE as i32),
        );
        let pos = IVec3::new(
            global_pos.x.rem_euclid(CHUNK_SIZE as i32),
            global_pos.y.rem_euclid(CHUNK_SIZE as i32),
            global_pos.z.rem_euclid(CHUNK_SIZE as i32),
        );
        (key, pos)
    }

    pub fn is_loaded(&self, key: IVec3) -> bool {
        self.chunks.contains_key(&key)
    }

    pub fn is_generated(&self, key: IVec3) -> bool {
        if let Some(c) = self.chunks.get(&key) {
            c.generated
        }
        else {
            false
        }
    }

    pub fn entity_created(&self, key: IVec3) -> bool {
        self.meshes.contains_key(&key)
    }

    /// @returns An iterator to all valid adjacent chunks keys
    pub fn adjacent_keys(key: IVec3) -> impl Iterator<Item = IVec3> {
        (-1..=1)
            .cartesian_product(-1..=1)
            .cartesian_product(-1..=1)
            .map(move |((x, y), z)| IVec3::new(x, y, z) + key)
            .filter(move |v| v != &key)
            .filter(|&v| Self::in_world_range(v))
    }

    /// Checks if a chunk key is valid in the world
    pub fn in_world_range(key: IVec3) -> bool {
        (0..WORLD_HEIGHT).contains(&key.y)
    }
}

pub const CHUNK_SIZE: usize = 16;
/// Number of chunks constituting the world vertically
pub const WORLD_HEIGHT: i32 = 128 / CHUNK_SIZE as i32;

#[derive(Default)]
pub struct ChunkData {
    pub data: [[[Block; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
    pub generated: bool,
}

macro_rules! decompose_vec_into {
    ($vec:expr, $type:ty) => {
        ($vec.x as $type, $vec.y as $type, $vec.z as $type)
    };
}

impl ChunkData {
    pub fn get_unchecked(&self, p: IVec3) -> Block {
        let (x, y, z) = decompose_vec_into!(p, usize);
        self.data[z][y][x]
    }

    pub fn get(&self, p: IVec3) -> Option<Block> {
        if p.cmplt(IVec3::ZERO).any() || p.cmpge(IVec3::splat(CHUNK_SIZE as i32)).any() {
            None
        } else {
            Some(self.get_unchecked(p))
        }
    }

    /// @returns An iterator over only 2 dimensions of the chunk
    #[inline]
    pub fn slice() -> impl Iterator<Item = (usize, usize)> {
        (0..CHUNK_SIZE).cartesian_product(0..CHUNK_SIZE)
    }

    #[inline]
    pub fn all() -> impl Iterator<Item = (usize, usize, usize)> {
        Self::slice()
            .cartesian_product(0..CHUNK_SIZE)
            .map(|((x, y), z)| (x, y, z))
    }

    pub fn all_blocks(&self) -> impl Iterator<Item = (Block, usize, usize, usize)> + '_ {
        Self::all().map(|(x, y, z)| (self.data[z][y][x], x, y, z))
    }

    #[inline]
    pub fn slice_lod(lod: u32) -> impl Iterator<Item = (usize, usize)> {
        let base = (0..CHUNK_SIZE).step_by(2usize.pow(lod));

        base.clone().cartesian_product(base)
    }

    /// @returns An iterator over 3 dimensions of the chunk, with xz distorted by the given lod
    #[inline]
    pub fn all_lod(lod: u32) -> impl Iterator<Item = (usize, usize, usize)> {
        Self::slice_lod(lod)
            .cartesian_product(0..CHUNK_SIZE)
            .map(|((x, z), y)| (x, y, z))
    }

    pub fn all_blocks_lod(&self, lod: u32) -> impl Iterator<Item = (Block, usize, usize, usize)> + '_ {
        Self::all_lod(lod).map(|(x, y, z)| (self.data[z][y][x], x, y, z))
    }
}

// Specified at half size
const RENDER_DISTANCE: i32 = 8;
const LOD_RANGE: i32 = 3;
pub fn load_chunks(
    mut commands: Commands,
    mut manager: ResMut<ChunkManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    atlas: Res<AtlasImage>,
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
            let mut entity = commands
                .spawn((
                    Chunk { key },
                    NeedsMesh(lod),
                    PbrBundle {
                        mesh: meshes.add(Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList)),
                        material: atlas.material.clone(),
                        transform: Transform::from_translation(key.as_vec3() * CHUNK_SIZE as f32),
                        ..default()
                    },
                    Name::new(format!("{key}"))
                ));

            if !manager.is_generated(key) {
                entity.insert(NeedsTerrain);
            }
            manager.meshes.insert(key, (entity.id(), lod));
        }
    }
}

pub fn unload_chunks(
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
