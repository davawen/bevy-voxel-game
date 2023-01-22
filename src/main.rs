use std::f32::consts::PI;

use bevy::{prelude::*, render::render_resource::PrimitiveTopology};
use bevy_inspector_egui::{RegisterInspectable, WorldInspectorPlugin};

mod chunk;
mod manager;
mod player;

use chunk::{generate_mesh, generate_terrain, NeedsMesh};
use manager::{load_chunks, unload_chunks, ChunkManager};
use noise::OpenSimplex;
use player::{BoundingBox, Velocity, VelocityMask};

#[derive(Resource)]
pub struct Noise(OpenSimplex);

#[derive(Default, Resource)]
pub struct AtlasImage {
    image: Handle<Image>,
    material: Handle<StandardMaterial>,
}

fn startup(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut atlas: ResMut<AtlasImage>,
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
        VelocityMask(Vec3::ONE),
        BoundingBox::from_size(Vec3::new(0.8, 1.9, 0.8)),
    ));

    atlas.image = server.load("atlas.png");
    atlas.material = materials.add(atlas.image.clone().into());
}

fn fix_atlas_filtering(
    mut events: EventReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    atlas: Res<AtlasImage>,
) {
    for event in events.iter() {
        if let AssetEvent::Created { handle } = event {
            if *handle == atlas.image {
                eprintln!("Handle created");
                let image = images.get_mut(handle).unwrap();
                image.sampler_descriptor = bevy::render::texture::ImageSampler::nearest();

                *materials.get_mut(&atlas.material).unwrap() = atlas.image.clone().into(); // regenerate material to pass sampler for some reason
            }
        }
    }
}

fn main() {
    App::new()
        .insert_resource(Noise(OpenSimplex::new(102)))
        .insert_resource(player::CameraDisabled(true))
        .insert_resource(ChunkManager::default())
        .insert_resource(AtlasImage { ..default() })
        .add_plugins(DefaultPlugins)
        .add_plugin(WorldInspectorPlugin::default())
        .register_inspectable::<Velocity>()
        .register_inspectable::<VelocityMask>()
        .add_startup_system(startup)
        .add_system(fix_atlas_filtering)
        // Player systems
        .add_system(player::rotate_camera)
        .add_system(player::move_camera)
        .add_system(player::collision.before(player::move_camera))
        //Chunk systems
        .add_system(generate_terrain)
        .add_system(generate_mesh)
        .add_system(load_chunks)
        .add_system(unload_chunks)
        .run()
}
