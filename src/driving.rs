mod model;

use avian3d::prelude::MoveAndSlide;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub use model::{DriftAssist, DriveMode, DrivingTuning, HandlingState};

use crate::game_state::{GameState, not_paused};
use crate::geometry::{forward_3d, yaw_rotation};
use crate::physics::{
    AvianTrackPhysicsQueries, CarCollisionDebug, CarPose, CollisionState, GroundContact,
    GroundSource, RoadCollider, TrackPhysicsQueries,
};
use crate::surface::{SurfaceKind, SurfaceLibrary};

const DEFAULT_CAR_START: Vec3 = Vec3::new(0.0, 0.05, -26.0);
const WHEEL_SAMPLE_HALF_WIDTH: f32 = 0.82;
const WHEEL_SAMPLE_HALF_LENGTH: f32 = 1.72;
const BODY_SUSPENSION_ROLL_SCALE: f32 = 0.16;
const BODY_SUSPENSION_PITCH_SCALE: f32 = 0.12;
const BODY_VISUAL_HEIGHT: f32 = 0.0;
const CAMERA_VELOCITY_BLEND_SPEED: f32 = 24.0;
const CAMERA_MAX_VELOCITY_BLEND: f32 = 0.55;
const WHEEL_CONTACT_COUNT: usize = model::WHEEL_COUNT;
const WHEEL_CONTACT_LABELS: [&str; WHEEL_CONTACT_COUNT] = ["FL", "FR", "RL", "RR"];

#[derive(Clone, Copy, Resource)]
pub struct CarSpawn {
    pub translation: Vec3,
    pub yaw: f32,
}

impl Default for CarSpawn {
    fn default() -> Self {
        Self {
            translation: DEFAULT_CAR_START,
            yaw: 0.0,
        }
    }
}

impl CarSpawn {
    pub fn rotation(self) -> Quat {
        yaw_rotation(self.yaw)
    }
}

pub struct DrivingPlugin;

impl Plugin for DrivingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DrivingTuning::default())
            .insert_resource(CarSpawn::default())
            .add_systems(
                FixedUpdate,
                drive_car.run_if(in_state(GameState::Driving).and(not_paused)),
            )
            .add_systems(
                Update,
                (
                    update_car_body_visual,
                    bind_imported_vehicle_wheels,
                    update_imported_wheel_visuals,
                    chase_camera,
                )
                    .chain()
                    .after(drive_car)
                    .run_if(in_state(GameState::Driving).and(not_paused)),
            );
    }
}

#[derive(Component)]
pub struct PlayerCar {
    pub previous_translation: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub yaw_rate: f32,
    pub current_surface: SurfaceKind,
    pub ground_source: GroundSource,
    pub boost_direction: Option<Vec3>,
    pub throttle: f32,
    pub steer: f32,
    pub rear_brake: f32,
    pub wheel_steer_angle: f32,
    pub signed_speed: f32,
    pub slip_angle: f32,
    pub drive_mode: DriveMode,
    pub drift_assist: DriftAssist,
    pub handling_state: HandlingState,
    pub collision_state: CollisionState,
    pub collision_debug: CarCollisionDebug,
    pub wheel_contacts: WheelContacts,
    pub wheel_telemetry: [model::WheelTelemetry; WHEEL_CONTACT_COUNT],
    pub wheel_suspension: [model::WheelSuspension; WHEEL_CONTACT_COUNT],
    pub wheel_spin_angles: [f32; WHEEL_CONTACT_COUNT],
    pub tire_forces: model::TireForces,
}

impl Default for PlayerCar {
    fn default() -> Self {
        Self {
            previous_translation: Vec3::ZERO,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            yaw_rate: 0.0,
            current_surface: SurfaceKind::Asphalt,
            ground_source: GroundSource::Road,
            boost_direction: None,
            throttle: 0.0,
            steer: 0.0,
            rear_brake: 0.0,
            wheel_steer_angle: 0.0,
            signed_speed: 0.0,
            slip_angle: 0.0,
            drive_mode: DriveMode::Forward,
            drift_assist: DriftAssist::default(),
            handling_state: HandlingState::Grip,
            collision_state: CollisionState::Clear,
            collision_debug: CarCollisionDebug::default(),
            wheel_contacts: WheelContacts::default(),
            wheel_telemetry: [model::WheelTelemetry::default(); WHEEL_CONTACT_COUNT],
            wheel_suspension: [model::WheelSuspension::default(); WHEEL_CONTACT_COUNT],
            wheel_spin_angles: [0.0; WHEEL_CONTACT_COUNT],
            tire_forces: model::TireForces::default(),
        }
    }
}

impl PlayerCar {
    pub fn reset_to_spawn(&mut self, transform: &mut Transform, car_spawn: CarSpawn) {
        *self = Self::default();
        self.yaw = car_spawn.yaw;
        self.previous_translation = car_spawn.translation;
        transform.translation = car_spawn.translation;
        transform.rotation = car_spawn.rotation();
    }
}

#[derive(Component)]
pub struct ChaseCamera;

#[derive(Component)]
pub struct VehicleSceneRoot;

#[derive(Component)]
pub struct AssetWheelVisual {
    base_rotation: Quat,
    base_translation: Vec3,
    role: AssetWheelRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetWheelRole {
    FrontLeft,
    FrontRight,
    Rear,
}

#[derive(Clone, Copy)]
pub struct WheelContacts {
    contacts: [GroundContact; WHEEL_CONTACT_COUNT],
}

impl Default for WheelContacts {
    fn default() -> Self {
        let contact = GroundContact {
            source: GroundSource::Road,
            surface: SurfaceKind::Asphalt,
            boost_direction: None,
        };
        Self {
            contacts: [contact; WHEEL_CONTACT_COUNT],
        }
    }
}

impl WheelContacts {
    fn sample(
        physics: &impl TrackPhysicsQueries,
        center: Vec3,
        basis: &model::MotionBasis,
    ) -> Self {
        let front = basis.forward * WHEEL_SAMPLE_HALF_LENGTH;
        let rear = -basis.forward * WHEEL_SAMPLE_HALF_LENGTH;
        let left = -basis.right * WHEEL_SAMPLE_HALF_WIDTH;
        let right = basis.right * WHEEL_SAMPLE_HALF_WIDTH;

        Self {
            contacts: [
                physics.ground_at(center + front + left),
                physics.ground_at(center + front + right),
                physics.ground_at(center + rear + left),
                physics.ground_at(center + rear + right),
            ],
        }
    }

    pub fn summary(self) -> String {
        WHEEL_CONTACT_LABELS
            .iter()
            .zip(self.contacts)
            .map(|(label, contact)| format!("{label}:{}", contact.label()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn friction(self, surfaces: &SurfaceLibrary) -> model::SurfaceFriction {
        model::SurfaceFriction {
            wheels: self.contacts.map(|contact| {
                let surface = surfaces.get(contact.surface);
                model::WheelFriction {
                    longitudinal: surface.longitudinal_friction,
                    lateral: surface.lateral_friction,
                }
            }),
        }
    }

    pub fn split_surface(self) -> bool {
        self.contacts[1..]
            .iter()
            .any(|contact| *contact != self.contacts[0])
    }
}

impl GroundContact {
    fn label(self) -> String {
        format!("{}:{}", self.source.label(), self.surface.label())
    }
}

#[derive(SystemParam)]
struct DrivingContext<'w, 's> {
    time: Res<'w, Time>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    car_spawn: Res<'w, CarSpawn>,
    tuning: Res<'w, DrivingTuning>,
    surfaces: Res<'w, SurfaceLibrary>,
    move_and_slide: MoveAndSlide<'w, 's>,
    roads: Query<'w, 's, (Entity, &'static RoadCollider)>,
}

fn drive_car(ctx: DrivingContext, mut cars: Query<(Entity, &mut Transform, &mut PlayerCar)>) {
    let dt = ctx.time.delta_secs();
    let physics = AvianTrackPhysicsQueries::new(&ctx.move_and_slide, &ctx.roads);

    for (entity, mut transform, mut car) in &mut cars {
        if ctx.keys.just_pressed(KeyCode::KeyR) {
            car.reset_to_spawn(&mut transform, *ctx.car_spawn);
        }
        car.previous_translation = transform.translation;
        car.collision_state = CollisionState::Clear;
        car.collision_debug = CarCollisionDebug::default();

        let input = model::ControlInput::from_keys(&ctx.keys);
        car.throttle = input.throttle;
        car.steer = input.steer;
        car.rear_brake = input.rear_brake;
        let ground = physics.ground_at(transform.translation);
        car.current_surface = ground.surface;
        car.ground_source = ground.source;
        car.boost_direction = ground.boost_direction;

        let surface = ctx.surfaces.get(car.current_surface);
        let basis = model::MotionBasis::from_yaw(car.yaw, car.velocity);
        car.wheel_contacts = WheelContacts::sample(&physics, transform.translation, &basis);
        let contact_friction = car.wheel_contacts.friction(&ctx.surfaces);
        car.signed_speed = basis.forward_speed;
        car.slip_angle = basis.slip_angle();
        let intent = model::ControlIntent::from_input(&ctx.tuning, input, &basis);
        car.drift_assist = model::DriftAssist::from_input(&ctx.tuning, &surface, input, &basis);
        car.drive_mode = intent.drive_mode;
        car.wheel_steer_angle = intent.wheel_steer_angle;
        let tire_forces = model::tire_forces(model::TireForceInput {
            tuning: &ctx.tuning,
            surface: &surface,
            intent,
            basis: &basis,
            previous_handling_state: car.handling_state,
            contact_friction,
            boost_direction: ground.boost_direction,
            drift_assist: car.drift_assist,
        });
        car.tire_forces = tire_forces;
        car.handling_state = tire_forces.handling_state;
        car.yaw_rate =
            model::resolved_yaw_rate(&ctx.tuning, car.yaw_rate, tire_forces.target_yaw_rate, dt);
        let desired_yaw = car.yaw + car.yaw_rate * dt;
        car.velocity += tire_forces.acceleration * dt;

        let basis = model::MotionBasis::from_yaw(desired_yaw, car.velocity);
        let capped_forward_speed = car
            .velocity
            .dot(basis.forward)
            .clamp(-ctx.tuning.max_reverse_speed, ctx.tuning.max_forward_speed);
        let capped_lateral_speed = car.velocity.dot(basis.right);
        car.velocity = basis.forward * capped_forward_speed + basis.right * capped_lateral_speed;
        let wheel_basis = model::MotionBasis::from_yaw(desired_yaw, car.velocity);
        car.wheel_telemetry = model::wheel_telemetry(
            &ctx.tuning,
            car.wheel_telemetry,
            &tire_forces,
            &wheel_basis,
            dt,
        );
        car.wheel_suspension =
            model::virtual_suspension(&ctx.tuning, car.wheel_suspension, &tire_forces, dt);
        for index in 0..WHEEL_CONTACT_COUNT {
            car.wheel_spin_angles[index] += car.wheel_telemetry[index].angular_speed * dt;
        }

        let current_translation = transform.translation;
        let desired_translation = current_translation + car.velocity * dt;
        let resolution = physics.resolve_car_pose(
            CarPose {
                translation: current_translation,
                yaw: car.yaw,
            },
            CarPose {
                translation: desired_translation,
                yaw: desired_yaw,
            },
            car.velocity,
            ctx.time.delta(),
            entity,
        );
        let mut next_translation = resolution.pose.translation;

        next_translation.y = ctx.car_spawn.translation.y;
        car.yaw = resolution.pose.yaw;
        car.yaw_rate = accepted_yaw_rate(&resolution.debug, dt);
        car.velocity = resolution.velocity;
        car.collision_state = resolution.state;
        car.collision_debug = resolution.debug;
        transform.translation = next_translation;
        transform.rotation = yaw_rotation(car.yaw);
    }
}

fn accepted_yaw_rate(debug: &CarCollisionDebug, dt: f32) -> f32 {
    if dt <= f32::EPSILON {
        0.0
    } else {
        debug.accepted_yaw_delta / dt
    }
}

fn update_car_body_visual(
    car: Single<(&Transform, &PlayerCar)>,
    mut body: Single<&mut Transform, (With<VehicleSceneRoot>, Without<PlayerCar>)>,
) {
    let (car_transform, car_state) = *car;
    let (roll, pitch) = body_suspension_attitude(car_state);

    body.translation = car_transform.translation + Vec3::Y * BODY_VISUAL_HEIGHT;
    body.rotation =
        yaw_rotation(car_state.yaw) * Quat::from_rotation_z(roll) * Quat::from_rotation_x(pitch);
}

fn bind_imported_vehicle_wheels(
    mut commands: Commands,
    wheels: Query<(Entity, &Name, &Transform), Without<AssetWheelVisual>>,
) {
    for (entity, name, transform) in &wheels {
        let Some(role) = imported_wheel_role(name.as_str()) else {
            continue;
        };

        commands.entity(entity).insert(AssetWheelVisual {
            base_rotation: transform.rotation,
            base_translation: transform.translation,
            role,
        });
    }
}

fn update_imported_wheel_visuals(
    car: Single<&PlayerCar>,
    mut wheels: Query<(&mut Transform, &AssetWheelVisual), Without<PlayerCar>>,
) {
    for (mut transform, wheel) in &mut wheels {
        let steer_angle = asset_wheel_steer(car.wheel_steer_angle, wheel.role);
        let spin = asset_wheel_spin(&car, wheel.role);
        let suspension_offset = asset_wheel_suspension_offset(&car, wheel.role);
        transform.translation = wheel.base_translation + Vec3::Y * suspension_offset;
        transform.rotation = asset_wheel_rotation(wheel.base_rotation, steer_angle, spin);
    }
}

fn body_suspension_attitude(car: &PlayerCar) -> (f32, f32) {
    let left_compression = (car.wheel_suspension[model::FRONT_LEFT_WHEEL].compression
        + car.wheel_suspension[model::REAR_LEFT_WHEEL].compression)
        * 0.5;
    let right_compression = (car.wheel_suspension[model::FRONT_RIGHT_WHEEL].compression
        + car.wheel_suspension[model::REAR_RIGHT_WHEEL].compression)
        * 0.5;
    let front_compression = (car.wheel_suspension[model::FRONT_LEFT_WHEEL].compression
        + car.wheel_suspension[model::FRONT_RIGHT_WHEEL].compression)
        * 0.5;
    let rear_compression = (car.wheel_suspension[model::REAR_LEFT_WHEEL].compression
        + car.wheel_suspension[model::REAR_RIGHT_WHEEL].compression)
        * 0.5;

    (
        (left_compression - right_compression) * BODY_SUSPENSION_ROLL_SCALE,
        (rear_compression - front_compression) * BODY_SUSPENSION_PITCH_SCALE,
    )
}

fn asset_wheel_steer(wheel_steer_angle: f32, role: AssetWheelRole) -> f32 {
    match role {
        AssetWheelRole::FrontLeft | AssetWheelRole::FrontRight => wheel_steer_angle,
        AssetWheelRole::Rear => 0.0,
    }
}

fn asset_wheel_spin(car: &PlayerCar, role: AssetWheelRole) -> f32 {
    match role {
        AssetWheelRole::FrontLeft => car.wheel_spin_angles[model::FRONT_LEFT_WHEEL],
        AssetWheelRole::FrontRight => car.wheel_spin_angles[model::FRONT_RIGHT_WHEEL],
        AssetWheelRole::Rear => {
            (car.wheel_spin_angles[model::REAR_LEFT_WHEEL]
                + car.wheel_spin_angles[model::REAR_RIGHT_WHEEL])
                * 0.5
        }
    }
}

fn asset_wheel_suspension_offset(car: &PlayerCar, role: AssetWheelRole) -> f32 {
    match role {
        AssetWheelRole::FrontLeft => car.wheel_suspension[model::FRONT_LEFT_WHEEL].visual_offset,
        AssetWheelRole::FrontRight => car.wheel_suspension[model::FRONT_RIGHT_WHEEL].visual_offset,
        AssetWheelRole::Rear => {
            (car.wheel_suspension[model::REAR_LEFT_WHEEL].visual_offset
                + car.wheel_suspension[model::REAR_RIGHT_WHEEL].visual_offset)
                * 0.5
        }
    }
}

fn asset_wheel_rotation(base_rotation: Quat, steer_angle: f32, spin: f32) -> Quat {
    base_rotation * Quat::from_rotation_y(steer_angle) * Quat::from_rotation_x(spin)
}

fn imported_wheel_role(name: &str) -> Option<AssetWheelRole> {
    if name.contains("FrontLeftWheel") {
        Some(AssetWheelRole::FrontLeft)
    } else if name.contains("FrontRightWheel") {
        Some(AssetWheelRole::FrontRight)
    } else if name.contains("BackWheels") {
        Some(AssetWheelRole::Rear)
    } else {
        None
    }
}

fn chase_camera(
    time: Res<Time>,
    car: Single<(&Transform, &PlayerCar), With<PlayerCar>>,
    mut camera: Single<&mut Transform, (With<ChaseCamera>, Without<PlayerCar>)>,
) {
    let (car_transform, car_state) = *car;
    let speed = car_state.velocity.length();
    let forward = forward_3d(car_state.yaw);
    let tracking_direction = camera_tracking_direction(forward, car_state.velocity);
    let target = car_transform.translation + Vec3::Y * 1.0;
    let desired_position = target - tracking_direction * (7.5 + speed * 0.06) + Vec3::Y * 4.2;
    let smoothing = 1.0 - (-8.0 * time.delta_secs()).exp();

    camera.translation = camera.translation.lerp(desired_position, smoothing);
    camera.look_at(target + tracking_direction * 4.0, Vec3::Y);
}

fn camera_tracking_direction(forward: Vec3, velocity: Vec3) -> Vec3 {
    let horizontal_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
    let speed = horizontal_velocity.length();
    if speed <= 0.5 {
        return forward;
    }

    let velocity_direction = horizontal_velocity / speed;
    let blend = (speed / CAMERA_VELOCITY_BLEND_SPEED).clamp(0.0, CAMERA_MAX_VELOCITY_BLEND);
    let blended = forward.lerp(velocity_direction, blend);
    if blended.length_squared() > f32::EPSILON {
        blended.normalize()
    } else {
        forward
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    use crate::game_state::GameState;
    #[test]
    fn driving_plugin_registers_without_query_conflicts() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.add_plugins(DrivingPlugin);

        app.update();
    }

    #[test]
    fn imported_front_wheels_use_gameplay_steer_direction() {
        assert!(asset_wheel_steer(0.42, AssetWheelRole::FrontLeft) > 0.0);
        assert!(asset_wheel_steer(-0.42, AssetWheelRole::FrontRight) < 0.0);
        assert_eq!(asset_wheel_steer(1.0, AssetWheelRole::Rear), 0.0);
    }

    #[test]
    fn imported_wheel_rotation_points_with_steer_direction() {
        let right_steer = asset_wheel_steer(0.42, AssetWheelRole::FrontLeft);
        let left_steer = asset_wheel_steer(-0.42, AssetWheelRole::FrontRight);
        let right = asset_wheel_rotation(Quat::IDENTITY, right_steer, 0.0) * Vec3::Z;
        let left = asset_wheel_rotation(Quat::IDENTITY, left_steer, 0.0) * Vec3::Z;

        assert!(right.x > 0.0);
        assert!(left.x < 0.0);
    }

    #[test]
    fn imported_wheel_spin_uses_matching_axle_state() {
        let mut car = PlayerCar::default();
        car.wheel_spin_angles[model::FRONT_LEFT_WHEEL] = 2.0;
        car.wheel_spin_angles[model::FRONT_RIGHT_WHEEL] = 4.0;
        car.wheel_spin_angles[model::REAR_LEFT_WHEEL] = 6.0;
        car.wheel_spin_angles[model::REAR_RIGHT_WHEEL] = 8.0;

        assert_eq!(asset_wheel_spin(&car, AssetWheelRole::FrontLeft), 2.0);
        assert_eq!(asset_wheel_spin(&car, AssetWheelRole::FrontRight), 4.0);
        assert_eq!(asset_wheel_spin(&car, AssetWheelRole::Rear), 7.0);
    }

    #[test]
    fn imported_wheel_suspension_uses_matching_sample_state() {
        let mut car = PlayerCar::default();
        car.wheel_suspension[model::FRONT_LEFT_WHEEL].visual_offset = -0.02;
        car.wheel_suspension[model::FRONT_RIGHT_WHEEL].visual_offset = -0.04;
        car.wheel_suspension[model::REAR_LEFT_WHEEL].visual_offset = 0.06;
        car.wheel_suspension[model::REAR_RIGHT_WHEEL].visual_offset = 0.08;

        assert_eq!(
            asset_wheel_suspension_offset(&car, AssetWheelRole::FrontLeft),
            -0.02
        );
        assert_eq!(
            asset_wheel_suspension_offset(&car, AssetWheelRole::FrontRight),
            -0.04
        );
        assert_eq!(
            asset_wheel_suspension_offset(&car, AssetWheelRole::Rear),
            0.07
        );
    }

    #[test]
    fn imported_vehicle_wheel_names_bind_to_visual_roles() {
        assert_eq!(
            imported_wheel_role("SportsCar_FrontLeftWheel_Cylinder.013"),
            Some(AssetWheelRole::FrontLeft)
        );
        assert_eq!(
            imported_wheel_role("SportsCar2_FrontRightWheel_Cylinder.018"),
            Some(AssetWheelRole::FrontRight)
        );
        assert_eq!(
            imported_wheel_role("SportsCar_BackWheels_Cylinder.004"),
            Some(AssetWheelRole::Rear)
        );
        assert_eq!(imported_wheel_role("SportsCar_Cube.005"), None);
    }

    #[test]
    fn body_attitude_comes_from_suspension_compression() {
        let mut car = PlayerCar::default();
        car.wheel_suspension[model::FRONT_LEFT_WHEEL].compression = 0.7;
        car.wheel_suspension[model::FRONT_RIGHT_WHEEL].compression = 0.7;
        car.wheel_suspension[model::REAR_LEFT_WHEEL].compression = 0.4;
        car.wheel_suspension[model::REAR_RIGHT_WHEEL].compression = 0.4;

        let (roll, pitch) = body_suspension_attitude(&car);

        assert_eq!(roll, 0.0);
        assert!(pitch < 0.0);
    }

    #[test]
    fn camera_tracking_blends_toward_velocity_direction() {
        let forward = Vec3::Z;
        let tracking = camera_tracking_direction(forward, Vec3::X * 24.0);

        assert!(tracking.x > 0.0);
        assert!(tracking.z > 0.0);
        assert!((tracking.length() - 1.0).abs() < 0.001);
    }
}
