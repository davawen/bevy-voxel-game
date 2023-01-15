use std::f32::consts::PI;

use bevy::{prelude::*, input::mouse::MouseMotion, window::CursorGrabMode};

#[derive(Resource)]
pub struct CameraDisabled(pub bool);

const SPEED: f32 = 1.0;

pub fn move_camera(
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
            let pitch = pitch.clamp(-PI / 2.0 + 0.1, PI / 2.0 - 0.1);
            camera.rotation = Quat::from_euler(
                EulerRot::YXZ,
                yaw - event.delta.x / 500.0,
                pitch - event.delta.y / 500.0,
                0.0,
            );
        }
    }

    let mut relative_offset = Vec3::ZERO;
    let mut offset = Vec3::ZERO;

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
    if keyboard.pressed(KeyCode::C) {
        offset += Vec3::NEG_Y;
    }
    if keyboard.pressed(KeyCode::Space) {
        offset += Vec3::Y;
    }

    let relative_offset = camera.rotation * relative_offset * SPEED;
    let offset = offset * SPEED;
    camera.translation += relative_offset + offset;
}

