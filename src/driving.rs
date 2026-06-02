use bevy::prelude::*;

use crate::physics::{EcsTrackPhysicsQueries, RailCollider, TrackPhysicsQueries};
use crate::surface::{SurfaceKind, SurfaceLibrary, SurfaceZone};

pub const CAR_START: Vec3 = Vec3::new(0.0, 0.05, -26.0);

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
    reverse_steer_rate: f32,
    min_steer_authority: f32,
    lateral_grip: f32,
    drag: f32,
    max_forward_speed: f32,
    max_reverse_speed: f32,
    reverse_steering_threshold: f32,
}

impl Default for DrivingTuning {
    fn default() -> Self {
        Self {
            acceleration: 38.0,
            brake_force: 52.0,
            reverse_force: 24.0,
            steer_rate: 2.5,
            reverse_steer_rate: 1.8,
            min_steer_authority: 0.22,
            lateral_grip: 8.5,
            drag: 0.9,
            max_forward_speed: 58.0,
            max_reverse_speed: 14.0,
            reverse_steering_threshold: 0.8,
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
    pub signed_speed: f32,
    pub drive_mode: DriveMode,
}

impl Default for PlayerCar {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            yaw: 0.0,
            current_surface: SurfaceKind::Asphalt,
            throttle: 0.0,
            steer: 0.0,
            signed_speed: 0.0,
            drive_mode: DriveMode::Forward,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveMode {
    Forward,
    Braking,
    Reverse,
}

impl DriveMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Braking => "braking",
            Self::Reverse => "reverse",
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
    rails: Query<&RailCollider>,
    mut cars: Query<(&mut Transform, &mut PlayerCar)>,
) {
    let dt = time.delta_secs();
    let physics = EcsTrackPhysicsQueries::new(&zones, &rails);

    for (mut transform, mut car) in &mut cars {
        if keys.just_pressed(KeyCode::KeyR) {
            *car = PlayerCar::default();
            transform.translation = CAR_START;
            transform.rotation = Quat::IDENTITY;
        }

        let controls = read_controls(&keys);
        car.throttle = controls.throttle;
        car.steer = controls.steer;
        car.current_surface = physics.surface_at(transform.translation);

        let surface = surfaces.get(car.current_surface);
        let basis = MotionBasis::from_yaw(car.yaw, car.velocity);
        car.signed_speed = basis.forward_speed;
        car.drive_mode = drive_mode(controls.throttle, basis.forward_speed);

        car.yaw += steering_yaw_delta(&tuning, &surface, controls, &basis) * dt;

        let basis = MotionBasis::from_yaw(car.yaw, car.velocity);
        let drive_force = drive_force(&tuning, &surface, controls.throttle, basis.forward_speed);

        car.velocity += basis.forward * controls.throttle * drive_force * dt;
        car.velocity += basis.forward * surface.boost_force * dt;
        car.velocity -=
            basis.right * basis.lateral_speed * tuning.lateral_grip * surface.lateral_grip * dt;
        car.velocity *= 1.0 / (1.0 + tuning.drag * surface.drag * surface.rolling_resistance * dt);

        let capped_forward_speed = car
            .velocity
            .dot(basis.forward)
            .clamp(-tuning.max_reverse_speed, tuning.max_forward_speed);
        let capped_lateral_speed = car.velocity.dot(basis.right);
        car.velocity = basis.forward * capped_forward_speed + basis.right * capped_lateral_speed;

        let mut next_translation = transform.translation + car.velocity * dt;
        if let Some(hit) = physics.cast_car_shape(next_translation, car.velocity) {
            next_translation += hit.normal * (hit.penetration + 0.01);

            let inward_speed = car.velocity.dot(hit.normal);
            if inward_speed < 0.0 {
                car.velocity -= hit.normal * inward_speed * 1.35;
                car.velocity *= 0.78;
            }
        }

        next_translation.y = CAR_START.y;
        transform.translation = next_translation;
        transform.rotation = Quat::from_rotation_y(car.yaw);
    }
}

#[derive(Clone, Copy)]
struct DriverControls {
    throttle: f32,
    steer: f32,
}

#[derive(Clone, Copy)]
struct MotionBasis {
    forward: Vec3,
    right: Vec3,
    forward_speed: f32,
    lateral_speed: f32,
}

impl MotionBasis {
    fn from_yaw(yaw: f32, velocity: Vec3) -> Self {
        let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);

        Self {
            forward,
            right,
            forward_speed: velocity.dot(forward),
            lateral_speed: velocity.dot(right),
        }
    }
}

fn read_controls(keys: &ButtonInput<KeyCode>) -> DriverControls {
    DriverControls {
        throttle: axis(
            keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]),
            keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]),
        ),
        steer: axis(
            keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]),
            keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]),
        ),
    }
}

fn drive_mode(throttle: f32, forward_speed: f32) -> DriveMode {
    if throttle < 0.0 && forward_speed > 1.0 {
        DriveMode::Braking
    } else if forward_speed < -0.4 || throttle < 0.0 {
        DriveMode::Reverse
    } else {
        DriveMode::Forward
    }
}

fn steering_yaw_delta(
    tuning: &DrivingTuning,
    surface: &crate::surface::SurfaceParams,
    controls: DriverControls,
    basis: &MotionBasis,
) -> f32 {
    let speed_ratio = (basis.forward_speed.abs() / tuning.max_forward_speed).clamp(0.0, 1.0);
    let steer_authority =
        tuning.min_steer_authority + speed_ratio * (1.0 - tuning.min_steer_authority);
    let reversing = basis.forward_speed < -tuning.reverse_steering_threshold;
    let rate = if reversing {
        tuning.reverse_steer_rate
    } else {
        tuning.steer_rate
    };
    let direction = if reversing { 1.0 } else { -1.0 };

    controls.steer * rate * steer_authority * surface.steering_multiplier * direction
}

fn drive_force(
    tuning: &DrivingTuning,
    surface: &crate::surface::SurfaceParams,
    throttle: f32,
    forward_speed: f32,
) -> f32 {
    if throttle >= 0.0 {
        tuning.acceleration * surface.acceleration_multiplier
    } else if forward_speed > 1.0 {
        tuning.brake_force * surface.longitudinal_grip
    } else {
        tuning.reverse_force * surface.acceleration_multiplier
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
