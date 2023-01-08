use std::f32::consts::PI;

use bevy::{
    input::mouse::MouseMotion,
    math::DVec3,
    prelude::*,
    render::{mesh::Indices, render_resource::PrimitiveTopology},
    utils::HashMap,
    window::CursorGrabMode,
};
use bevy_inspector_egui::WorldInspectorPlugin;
use itertools::Itertools;
use noise::{NoiseFn, OpenSimplex};

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

#[derive(Default, Resource)]
struct ChunkManager {
    chunks: HashMap<IVec3, ChunkData>,
}

impl ChunkManager {
    /// Gives the block in the given chunk if pos is in bounds, or a block in an adjacent chunk if pos is out of bounds
    /// @returns None if the adjacent chunk isn't generated
    fn get_with_adjacent(&self, key: IVec3, pos: IVec3) -> Option<Block> {
        // Get the chunk offset
        let chunk_key = key + IVec3::new(
            pos.x.div_euclid(CHUNK_SIZE as i32),
            pos.y.div_euclid(CHUNK_SIZE as i32),
            pos.z.div_euclid(CHUNK_SIZE as i32)
        );

        let chunk = self.chunks.get(&chunk_key)?;

        if chunk.generated {
            let pos = IVec3::new(
                pos.x.rem_euclid(CHUNK_SIZE as i32),
                pos.y.rem_euclid(CHUNK_SIZE as i32),
                pos.z.rem_euclid(CHUNK_SIZE as i32)
            );
            Some(chunk.get_unchecked(pos))
        }
        else { None }
    }
}

const CHUNK_SIZE: usize = 16;
struct ChunkData {
    entity: Entity,
    data: [[[Block; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
    generated: bool
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
}

#[derive(Component)]
struct Chunk {
    key: IVec3,
    // mesh_generated: bool
}

#[derive(Component)]
struct MeshGenerated;

#[derive(Resource)]
struct Noise(OpenSimplex);

fn generate_terrain(
    mut commands: Commands,
    query: Query<(Entity, &Chunk, &Transform)>,
    mut manager: ResMut<ChunkManager>,
    noise: Res<Noise>,
) {
    for (entity, chunk, transform) in query.iter() {
        let data = &mut manager.chunks.get_mut(&chunk.key).unwrap();
        if data.generated {
            continue;
        }

        for (x, z) in (0..CHUNK_SIZE).cartesian_product(0..CHUNK_SIZE) {
            let mut pos = DVec3::new(x as f64, 0.0, z as f64) + transform.translation.as_dvec3();
            pos.y = 0.0;
            let height = noise.0.get((pos / 10.0).to_array()) / 2.0 + 0.5;
            let height = height as f32 * CHUNK_SIZE as f32 * 4.0;

            for y in 0..CHUNK_SIZE {
                let y_real = y as f32 + transform.translation.y;
                data.data[z][y][x] = if y_real < height { Block::Dirt } else { Block::Air }
            }
        }

        data.generated = true;
    }
}

fn generate_mesh(
    mut commands: Commands,
    query: Query<
        (Entity, &Chunk, &Handle<Mesh>),
        Without<MeshGenerated>,
    >,
    manager: Res<ChunkManager>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, chunk, mesh) in query.iter() {
        let data = manager.chunks.get(&chunk.key).unwrap();

        if !data.generated {
            continue;
        }

        let mut vertices = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();

        let mut add_face = |local_pos: IVec3, dir: IVec3, points: [Vec3; 4]| {
            // Return early if the adjacent face is not visible
            if manager.get_with_adjacent(chunk.key, local_pos + dir).unwrap_or(Block::Air).full() {
                return;
            }
            let idx = vertices.len() as u32;
            let pos = local_pos.as_vec3() + Vec3::splat(0.5);
            for p in points {
                vertices.push((pos + p).to_array());
                normals.push(dir.as_vec3().to_array());
            }
            indices.extend_from_slice(&[idx + 2, idx + 1, idx, idx, idx + 3, idx + 2]);
        };

        for (block, x, y, z) in data.all_blocks() {
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

        commands.get_entity(entity).unwrap().insert(MeshGenerated);
    }
}

#[derive(Resource)]
struct CameraDisabled(bool);

fn move_camera(
    mut query: Query<&mut Transform, With<Camera>>,
    mut windows: ResMut<Windows>,
    mut camera_disabled: ResMut<CameraDisabled>,
    mut cursor_moved: EventReader<MouseMotion>,
    keyboard: Res<Input<KeyCode>>,
) {
    let mut camera = query.single_mut();

    if keyboard.just_pressed(KeyCode::F) {
        camera_disabled.0 = !camera_disabled.0;

        let window = windows.get_primary_mut().unwrap();
        if camera_disabled.0 {
            window.set_cursor_grab_mode(CursorGrabMode::None);
            window.set_cursor_visibility(true);
        } else {
            window.set_cursor_grab_mode(CursorGrabMode::Locked);
            window.set_cursor_visibility(false);
        }
    }

    if !camera_disabled.0 {
        for event in cursor_moved.iter() {
            let (yaw, pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
            let pitch = pitch.clamp(-PI / 4.0 + 0.01, PI / 4.0 - 0.01);
            camera.rotation = Quat::from_euler(
                EulerRot::YXZ,
                yaw - event.delta.x / 500.0,
                pitch - event.delta.y / 500.0,
                0.0,
            );
        }
    }

    let mut offset = Vec3::ZERO;

    if keyboard.pressed(KeyCode::W) {
        offset += Vec3::NEG_Z;
    }
    if keyboard.pressed(KeyCode::S) {
        offset += Vec3::Z;
    }
    if keyboard.pressed(KeyCode::A) {
        offset += Vec3::NEG_X;
    }
    if keyboard.pressed(KeyCode::D) {
        offset += Vec3::X;
    }
    if keyboard.pressed(KeyCode::C) {
        offset += Vec3::NEG_Y;
    }
    if keyboard.pressed(KeyCode::Space) {
        offset += Vec3::Y;
    }

    let offset = camera.rotation * offset * 0.1;
    camera.translation += offset;
}

fn startup(
    mut commands: Commands,
    mut windows: ResMut<Windows>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut manager: ResMut<ChunkManager>,
) {
    for k in 0..4 {
        for j in 0..16 {
            for i in 0..16 {
                let key = IVec3::new(i, k, j);

                let chunk = commands
                    .spawn((
                        Chunk {
                            key,
                        },
                        PbrBundle {
                            mesh: meshes.add(Mesh::new(PrimitiveTopology::TriangleList)),
                            material: materials.add(Color::WHITE.into()),
                            transform: Transform::from_xyz(
                                i as f32 * CHUNK_SIZE as f32,
                                k as f32 * CHUNK_SIZE as f32,
                                j as f32 * CHUNK_SIZE as f32,
                            ),
                            ..default()
                        },
                    ))
                    .id();

                manager.chunks.insert(key, ChunkData {
                    entity: chunk,
                    data: default(),
                    generated: false
                });
            }
        }
    }

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
        transform: Transform::from_xyz(-3.0, 0.5, 0.5)
            .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        ..default()
    });

    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_grab_mode(CursorGrabMode::Locked);
    window.set_cursor_visibility(false);
}

fn main() {
    App::new()
        .insert_resource(Noise(OpenSimplex::new(102)))
        .insert_resource(CameraDisabled(false))
        .insert_resource(ChunkManager::default())
        .add_plugins(DefaultPlugins)
        .add_plugin(WorldInspectorPlugin::default())
        .add_startup_system(startup)
        .add_system(generate_terrain)
        .add_system(generate_mesh)
        .add_system(move_camera)
        .run()
}
