use std::f32::consts::PI;

use bevy::{prelude::*, input::mouse::MouseMotion, window::CursorGrabMode};

use crate::{manager::ChunkManager, AAAAAA};

#[derive(Resource)]
pub struct CameraDisabled(pub bool);

#[derive(Component)]
pub struct Velocity(pub Vec3);

#[derive(Component, Clone)]
pub struct BoundingBox {
    pub center: Vec3,
    pub half_extents: Vec3
}

#[allow(unused)]
impl BoundingBox {
    pub fn from_min_max(minimum: Vec3, maximum: Vec3) -> Self {
        Self {
            center: (minimum + maximum) / 2.0,
            half_extents: (maximum - minimum) / 2.0
        }
    }
    pub fn from_size(size: Vec3) -> Self {
        Self {
            center: Vec3::ZERO,
            half_extents: size / 2.0
        }
    }

    fn min(&self) -> Vec3 {
        self.center - self.half_extents
    }
    fn max(&self) -> Vec3 {
        self.center + self.half_extents
    }

    /// @returns An array of the bounding box's 8 corners
    pub fn points(&self) -> [Vec3; 8] {
        let c = self.center;
        let e = self.half_extents;
        [
            c - e,
            c + Vec3::new( e.x, -e.y, -e.z),
            c + Vec3::new(-e.x, -e.y,  e.z),
            c + Vec3::new( e.x, -e.y,  e.z),
            c + Vec3::new( e.x,  e.y, -e.z),
            c + Vec3::new(-e.x,  e.y,  e.z),
            c + Vec3::new( e.x,  e.y,  e.z),
            c + e,
        ]
    }
}

pub fn rotate_camera(
    mut query: Query<&mut Transform, With<Camera>>,
    mut windows: ResMut<Windows>,
    mut camera_disabled: ResMut<CameraDisabled>,
    mut cursor_moved: EventReader<MouseMotion>,
    keyboard: Res<Input<KeyCode>>
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
            let pitch = pitch.clamp(-PI / 2.0 + 0.1, PI / 2.0 - 0.1);
            camera.rotation = Quat::from_euler(
                EulerRot::YXZ,
                yaw - event.delta.x / 500.0,
                pitch - event.delta.y / 500.0,
                0.0,
            );
        }
    }

}

pub fn move_camera(
    mut query: Query<(&mut Transform, &mut BoundingBox, &mut Velocity), With<Camera>>,
    keyboard: Res<Input<KeyCode>>,
    time: Res<Time>
) {
    let (mut camera, mut bounding, mut velocity) = query.single_mut();

    let mut relative_offset = Vec3::ZERO;

    if keyboard.pressed(KeyCode::W) {
        relative_offset += Vec3::NEG_Z;
    }
    if keyboard.pressed(KeyCode::S) {
        relative_offset += Vec3::Z;
    }
    if keyboard.pressed(KeyCode::A) {
        relative_offset += Vec3::NEG_X;
    }
    if keyboard.pressed(KeyCode::D) {
        relative_offset += Vec3::X;
    }

    camera.translation += velocity.0 * time.delta_seconds();
    bounding.center = camera.translation - Vec3::new(3.0, 0.6, 0.0);

    const SPEED: f32 = 1.0;
    const GRAVITY: f32 = 1.0;

    velocity.0 *= 0.9;

    let mut acceleration = Vec3::ZERO;

    acceleration += camera.rotation * relative_offset * SPEED;
    acceleration += Vec3::NEG_Y * GRAVITY;

    velocity.0 += acceleration/* *time.delta_seconds() */;

    if keyboard.just_pressed(KeyCode::Space) {
        velocity.0.y = 20.0;
    }
}

pub fn aaaaa(query: Query<(&Transform), With<Camera>>, mut boudning_mesh: Query<&mut Transform, (With<AAAAAA>, Without<Camera>)>) {
    let player = query.single();
    let mut b_mesh = boudning_mesh.single_mut();

    b_mesh.translation = player.translation - Vec3::new(3.0, 0.6, 0.0);
}

pub fn collision(
    mut query: Query<(&BoundingBox, &mut Velocity), With<Camera>>,
    time: Res<Time>,
    manager: Res<ChunkManager>
) {
    let (bounding, mut velocity) = query.single_mut();

    let mut check_axis = |dir: Vec3| {
        let nullify = bounding.points().into_iter().any(|point| {
            let ( player_key, player_pos ) = ChunkManager::get_keys((point + velocity.0*dir*time.delta_seconds()).floor().as_ivec3());
            
            if let Some(c) = manager.chunks.get(&player_key) {
                c.get_unchecked(player_pos).full()
            }
            else {
                true
            }
        });

        if nullify {
            let response = velocity.0 * -dir;
            velocity.0 += response; // Nullify the axis given
        }
    };

    check_axis(Vec3::X);
    check_axis(Vec3::Y);
    check_axis(Vec3::Z);
}


