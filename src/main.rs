use std::f32::consts::PI;

use bevy::{prelude::*, render::render_resource::PrimitiveTopology};
use bevy_inspector_egui::{WorldInspectorPlugin, RegisterInspectable};

mod player;
mod chunk;
mod manager;

use chunk::{NeedsMesh, generate_mesh, generate_terrain};
use manager::{ChunkManager, load_chunks, unload_chunks};
use noise::OpenSimplex;
use player::{Velocity, BoundingBox};

#[derive(Resource)]
pub struct Noise(OpenSimplex);

#[derive(Component)]
pub struct AAAAAA;

fn startup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>
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

    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 80.0, 0.0),
            projection: Projection::Perspective(PerspectiveProjection {
                fov: PI / 2.0,
                ..default()
            }),
            ..default()
        },
        Velocity(Vec3::ZERO),
        BoundingBox::from_size(Vec3::new(0.8, 1.9, 0.8))
    ));

    let b = BoundingBox::from_size(Vec3::new(0.8, 1.9, 0.8));
    let mut m = Mesh::new(PrimitiveTopology::LineList);
    let vertices = b.points().to_vec();
    let normals = vec![Vec3::Y; 8];
    let indices = vec![
        0, 1, 2, 3, 0, 2, 1, 3,
        0, 4, 1, 5, 2, 6, 3, 7,
        4, 5, 6, 7, 4, 6, 5, 7
    ];

    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    m.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(m),
            material: materials.add(Color::BLUE.into()),
            ..default()
        },
        AAAAAA
    ));
}

fn main() {
    App::new()
        .insert_resource(Noise(OpenSimplex::new(102)))
        .insert_resource(player::CameraDisabled(true))
        .insert_resource(ChunkManager::default())
        .add_plugins(DefaultPlugins)
        .add_plugin(WorldInspectorPlugin::default())
        .register_inspectable::<NeedsMesh>()
        .add_startup_system(startup)

        // Player systems
        .add_system(player::rotate_camera)
        .add_system(player::move_camera)
        .add_system(player::collision)
        .add_system(player::aaaaa)

        //Chunk systems
        .add_system(generate_terrain)
        .add_system(generate_mesh)
        .add_system(load_chunks)
        .add_system(unload_chunks)
        .run()
}
