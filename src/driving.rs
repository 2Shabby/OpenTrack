use bevy::prelude::*;

use crate::surface::{SurfaceKind, SurfaceLibrary, SurfaceZone, surface_at};

pub const CAR_START: Vec3 = Vec3::new(0.0, 0.35, -14.0);

pub struct DrivingPlugin;

impl Plugin for DrivingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DrivingTuning::default())
            .add_systems(FixedUpdate, drive_car)
            .add_systems(Update, chase_camera.after(drive_car));
    }
}

#[derive(Resource)]
pub struct DrivingTuning {
    acceleration: f32,
    brake_force: f32,
    reverse_force: f32,
    steer_rate: f32,
    lateral_grip: f32,
    drag: f32,
    max_forward_speed: f32,
    max_reverse_speed: f32,
}

impl Default for DrivingTuning {
    fn default() -> Self {
        Self {
            acceleration: 38.0,
            brake_force: 52.0,
            reverse_force: 24.0,
            steer_rate: 2.5,
            lateral_grip: 8.5,
            drag: 0.9,
            max_forward_speed: 58.0,
            max_reverse_speed: 14.0,
        }
    }
}

#[derive(Component)]
pub struct PlayerCar {
    pub velocity: Vec3,
    pub yaw: f32,
    pub current_surface: SurfaceKind,
    pub throttle: f32,
    pub steer: f32,
}

impl Default for PlayerCar {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            yaw: 0.0,
            current_surface: SurfaceKind::Asphalt,
            throttle: 0.0,
            steer: 0.0,
        }
    }
}

#[derive(Component)]
pub struct ChaseCamera;

fn drive_car(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<DrivingTuning>,
    surfaces: Res<SurfaceLibrary>,
    zones: Query<&SurfaceZone>,
    mut cars: Query<(&mut Transform, &mut PlayerCar)>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut car) in &mut cars {
        if keys.just_pressed(KeyCode::KeyR) {
            *car = PlayerCar::default();
            transform.translation = CAR_START;
            transform.rotation = Quat::IDENTITY;
        }

        let throttle = axis(
            keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]),
            keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]),
        );
        let steer = axis(
            keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]),
            keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]),
        );
        car.throttle = throttle;
        car.steer = steer;
        car.current_surface = surface_at(transform.translation, &zones);

        let surface = surfaces.get(car.current_surface);
        let forward = Vec3::new(car.yaw.sin(), 0.0, car.yaw.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        let forward_speed = car.velocity.dot(forward);
        let lateral_speed = car.velocity.dot(right);

        let speed_ratio = (forward_speed.abs() / tuning.max_forward_speed).clamp(0.0, 1.0);
        let steer_authority = 0.35 + speed_ratio * 0.65;
        car.yaw -= steer * tuning.steer_rate * steer_authority * surface.steering_multiplier * dt;

        let drive_force = if throttle >= 0.0 {
            tuning.acceleration * surface.acceleration_multiplier
        } else if forward_speed > 1.0 {
            tuning.brake_force * surface.longitudinal_grip
        } else {
            tuning.reverse_force * surface.acceleration_multiplier
        };

        car.velocity += forward * throttle * drive_force * dt;
        car.velocity += forward * surface.boost_force * dt;
        car.velocity -= right * lateral_speed * tuning.lateral_grip * surface.lateral_grip * dt;
        car.velocity *= 1.0 / (1.0 + tuning.drag * surface.drag * surface.rolling_resistance * dt);

        let capped_forward_speed = car
            .velocity
            .dot(forward)
            .clamp(-tuning.max_reverse_speed, tuning.max_forward_speed);
        let capped_lateral_speed = car.velocity.dot(right);
        car.velocity = forward * capped_forward_speed + right * capped_lateral_speed;

        transform.translation += car.velocity * dt;
        transform.translation.y = CAR_START.y;
        transform.rotation = Quat::from_rotation_y(car.yaw);
    }
}

fn chase_camera(
    time: Res<Time>,
    car: Single<(&Transform, &PlayerCar), With<PlayerCar>>,
    mut camera: Single<&mut Transform, (With<ChaseCamera>, Without<PlayerCar>)>,
) {
    let (car_transform, car_state) = *car;
    let speed = car_state.velocity.length();
    let forward = Vec3::new(car_state.yaw.sin(), 0.0, car_state.yaw.cos());
    let target = car_transform.translation + Vec3::Y * 1.0;
    let desired_position = target - forward * (7.5 + speed * 0.06) + Vec3::Y * 4.2;
    let smoothing = 1.0 - (-8.0 * time.delta_secs()).exp();

    camera.translation = camera.translation.lerp(desired_position, smoothing);
    camera.look_at(target + forward * 4.0, Vec3::Y);
}

fn axis(positive: bool, negative: bool) -> f32 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}
