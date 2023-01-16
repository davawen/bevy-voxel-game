use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_inspector_egui::{WorldInspectorPlugin, RegisterInspectable};

mod player;
mod chunk;
mod manager;

use chunk::{NeedsMesh, generate_mesh, generate_terrain};
use manager::{ChunkManager, load_chunks, unload_chunks};
use noise::OpenSimplex;

#[derive(Resource)]
pub struct Noise(OpenSimplex);

fn startup(
    mut commands: Commands,
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
        transform: Transform::from_xyz(0.0, 80.0, 0.0),
        projection: Projection::Perspective(PerspectiveProjection {
            fov: PI / 2.0,
            ..default()
        }),
        ..default()
    });
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
        .add_system(player::move_camera)
        .add_system(generate_terrain)
        .add_system(generate_mesh)
        .add_system(load_chunks)
        .add_system(unload_chunks)
        .run()
}
