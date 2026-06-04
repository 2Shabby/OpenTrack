use bevy::prelude::*;
use third_person_camera::{
    CameraOffset, DampingFactor, TargetOffset, TargetPoint, ThirdPersonCamera,
    ThirdPersonCameraPlugin, ThirdPersonCameraSettings,
};

use crate::driving::{CarSpawn, PlayerCar};
use crate::game_state::{GameState, not_paused};

const CAMERA_BASE_DISTANCE: f32 = 11.8;
const CAMERA_MAX_EXTRA_DISTANCE: f32 = 4.4;
const CAMERA_SPEED_DISTANCE_SCALE: f32 = 0.044;
const CAMERA_HEIGHT: f32 = 5.8;
const CAMERA_TARGET_HEIGHT: f32 = 1.2;
const CAMERA_LOOKAHEAD: f32 = 6.8;
const CAMERA_FOV: f32 = 66.0_f32.to_radians();
const CAMERA_POSITION_DAMPING: f32 = 4.5;
const CAMERA_ROTATION_DAMPING: f32 = 3.2;
const CAMERA_VELOCITY_BLEND_SPEED: f32 = 34.0;
const CAMERA_MAX_VELOCITY_BLEND: f32 = 0.35;
pub const CAMERA_TARGET_DAMPING: f32 = 2.8;

pub struct RaceCameraPlugin;

impl Plugin for RaceCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ThirdPersonCameraPlugin::new(ThirdPersonCameraSettings {
            default_camera_offset: race_camera_pose(Vec3::ZERO, Vec3::Z, Vec3::Y, 0.0).offset,
            default_target_offset: Vec3::ZERO,
            default_damping: Some(CAMERA_TARGET_DAMPING),
            local_cam: None,
            ..default()
        }))
        .add_systems(
            PostUpdate,
            update_race_camera
                .before(TransformSystems::Propagate)
                .run_if(in_state(GameState::Driving).and(not_paused)),
        );
    }
}

#[derive(Component)]
pub struct RaceCamera;

#[derive(Bundle)]
pub struct RaceCameraBundle {
    camera: Camera3d,
    projection: Projection,
    transform: Transform,
    third_person: ThirdPersonCamera,
    camera_offset: CameraOffset,
    target_offset: TargetOffset,
    target_point: TargetPoint,
    damping: DampingFactor,
    marker: RaceCamera,
}

impl RaceCameraBundle {
    pub fn new(target: Entity, car_spawn: CarSpawn) -> Self {
        let rotation = car_spawn.rotation();
        let target_point = car_spawn.translation;
        let pose = race_camera_pose(target_point, rotation * Vec3::Z, rotation * Vec3::Y, 0.0);

        Self {
            camera: Camera3d::default(),
            projection: Projection::from(PerspectiveProjection {
                fov: CAMERA_FOV,
                ..default()
            }),
            transform: Transform::from_translation(pose.translation).with_rotation(pose.rotation),
            third_person: ThirdPersonCamera::aimed_at(target),
            camera_offset: CameraOffset(pose.offset),
            target_offset: TargetOffset(Vec3::ZERO),
            target_point: TargetPoint(target_point),
            damping: DampingFactor(CAMERA_TARGET_DAMPING),
            marker: RaceCamera,
        }
    }
}

fn update_race_camera(
    time: Res<Time>,
    car: Single<(&Transform, &PlayerCar), With<PlayerCar>>,
    camera: Single<
        (&mut Transform, &mut CameraOffset, &TargetPoint),
        (With<RaceCamera>, Without<PlayerCar>),
    >,
) {
    let (car_transform, car_state) = *car;
    let (mut camera_transform, mut camera_offset, target_point) = camera.into_inner();
    let forward = car_transform.rotation * Vec3::Z;
    let up = car_transform.rotation * Vec3::Y;
    let tracking_direction = camera_tracking_direction(forward, car_state.velocity);
    let desired_pose = race_camera_pose(
        target_point.0,
        tracking_direction,
        up,
        car_state.velocity.length(),
    );
    let dt = time.delta_secs();
    let position_smoothing = exponential_smoothing(CAMERA_POSITION_DAMPING, dt);
    let rotation_smoothing = exponential_smoothing(CAMERA_ROTATION_DAMPING, dt);

    camera_transform.translation = camera_transform
        .translation
        .lerp(desired_pose.translation, position_smoothing);
    camera_transform.rotation = camera_transform
        .rotation
        .slerp(desired_pose.rotation, rotation_smoothing);
    camera_offset.0 =
        camera_transform.rotation.inverse() * (target_point.0 - camera_transform.translation);
}

#[derive(Clone, Copy)]
struct RaceCameraPose {
    translation: Vec3,
    rotation: Quat,
    offset: Vec3,
}

fn race_camera_pose(target_point: Vec3, forward: Vec3, up: Vec3, speed: f32) -> RaceCameraPose {
    let forward = forward.normalize_or(Vec3::Z);
    let up = up.normalize_or(Vec3::Y);
    let distance = race_camera_distance(speed);
    let translation = target_point - forward * distance + up * CAMERA_HEIGHT;
    let look_target = target_point + up * CAMERA_TARGET_HEIGHT + forward * CAMERA_LOOKAHEAD;
    let rotation = Transform::from_translation(translation)
        .looking_at(look_target, up)
        .rotation;
    let offset = rotation.inverse() * (target_point - translation);

    RaceCameraPose {
        translation,
        rotation,
        offset,
    }
}

fn race_camera_distance(speed: f32) -> f32 {
    let extra_distance =
        (speed * CAMERA_SPEED_DISTANCE_SCALE).clamp(0.0, CAMERA_MAX_EXTRA_DISTANCE);
    CAMERA_BASE_DISTANCE + extra_distance
}

fn exponential_smoothing(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

fn camera_tracking_direction(forward: Vec3, velocity: Vec3) -> Vec3 {
    let forward = forward.normalize_or(Vec3::Z);
    let speed = velocity.length();
    if speed <= 0.5 {
        return forward;
    }

    let velocity_direction = velocity / speed;
    let blend = (speed / CAMERA_VELOCITY_BLEND_SPEED).clamp(0.0, CAMERA_MAX_VELOCITY_BLEND);
    let blended = forward.lerp(velocity_direction, blend);
    blended.normalize_or(forward)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_tracking_blends_toward_velocity_direction() {
        let tracking = camera_tracking_direction(Vec3::Z, Vec3::X * 34.0);

        assert!(tracking.x > 0.0);
        assert!(tracking.z > 0.0);
        assert!((tracking.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn camera_offset_moves_back_as_speed_rises() {
        let stopped = race_camera_distance(0.0);
        let fast = race_camera_distance(120.0);

        assert!(fast > stopped);
        assert_eq!(fast, CAMERA_BASE_DISTANCE + CAMERA_MAX_EXTRA_DISTANCE);
    }

    #[test]
    fn camera_pose_uses_banked_up_vector() {
        let banked_up = Vec3::new(0.0, 0.707, 0.707).normalize();
        let pose = race_camera_pose(Vec3::ZERO, Vec3::X, banked_up, 0.0);
        let camera_right = pose.rotation * Vec3::X;
        let camera_up = pose.rotation * Vec3::Y;

        assert!(camera_right.dot(banked_up).abs() < 0.001);
        assert!(camera_up.dot(banked_up) > 0.8);
    }

    #[test]
    fn camera_pose_is_behind_above_and_looking_down_at_car() {
        let pose = race_camera_pose(Vec3::ZERO, Vec3::Z, Vec3::Y, 0.0);
        let view_direction = pose.rotation * Vec3::NEG_Z;

        assert!(pose.translation.z < -CAMERA_BASE_DISTANCE + 0.001);
        assert!(pose.translation.y > CAMERA_HEIGHT - 0.001);
        assert!(view_direction.y < -0.1);
        assert!(view_direction.z > 0.0);
    }

    #[test]
    fn camera_smoothing_damps_transition_steps() {
        let smoothing = exponential_smoothing(CAMERA_POSITION_DAMPING, 1.0 / 60.0);

        assert!(smoothing > 0.0);
        assert!(smoothing < 0.1);
    }

    #[test]
    fn third_person_camera_relationship_updates_target_point() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::transform::TransformPlugin,
        ))
        .add_plugins(ThirdPersonCameraPlugin::default());

        let target = app
            .world_mut()
            .spawn((
                Transform::from_translation(Vec3::ZERO),
                GlobalTransform::default(),
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((
                Transform::default(),
                ThirdPersonCamera::aimed_at(target),
                CameraOffset(Vec3::ZERO),
                TargetOffset(Vec3::ZERO),
                TargetPoint(Vec3::ZERO),
            ))
            .id();

        app.update();
        app.world_mut()
            .entity_mut(target)
            .insert(Transform::from_translation(Vec3::new(6.0, 2.0, -3.0)));
        app.update();
        app.update();

        let target_point = app.world().get::<TargetPoint>(camera).unwrap().0;
        assert_eq!(target_point, Vec3::new(6.0, 2.0, -3.0));
    }
}
