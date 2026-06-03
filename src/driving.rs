mod model;

use avian3d::prelude::SpatialQuery;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub use model::{DriveMode, DrivingTuning, HandlingState};

use crate::game_state::{GameState, not_paused};
use crate::geometry::{forward_3d, yaw_rotation};
use crate::physics::{
    AvianTrackPhysicsQueries, GroundContact, GroundSource, RoadCollider, TrackPhysicsQueries,
};
use crate::surface::{SurfaceKind, SurfaceLibrary};

const DEFAULT_CAR_START: Vec3 = Vec3::new(0.0, 0.05, -26.0);
const WHEEL_SAMPLE_HALF_WIDTH: f32 = 0.82;
const WHEEL_SAMPLE_HALF_LENGTH: f32 = 1.72;
const BODY_ROLL_RATE: f32 = 0.18;
const BODY_PITCH_RATE: f32 = 0.05;
const BODY_VISUAL_HEIGHT: f32 = 0.0;
const FRONT_WHEEL_MAX_STEER: f32 = 0.42;
const ASSET_WHEEL_SPIN_RATE: f32 = 2.0;
const CAR_COLLISION_SKIN: f32 = 0.02;
const WHEEL_CONTACT_COUNT: usize = 4;
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
    pub current_surface: SurfaceKind,
    pub ground_source: GroundSource,
    pub throttle: f32,
    pub steer: f32,
    pub wheel_steer_angle: f32,
    pub signed_speed: f32,
    pub slip_angle: f32,
    pub drive_mode: DriveMode,
    pub handling_state: HandlingState,
    pub wheel_contacts: WheelContacts,
    pub tire_forces: model::TireForces,
}

impl Default for PlayerCar {
    fn default() -> Self {
        Self {
            previous_translation: Vec3::ZERO,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            current_surface: SurfaceKind::Asphalt,
            ground_source: GroundSource::Road,
            throttle: 0.0,
            steer: 0.0,
            wheel_steer_angle: 0.0,
            signed_speed: 0.0,
            slip_angle: 0.0,
            drive_mode: DriveMode::Forward,
            handling_state: HandlingState::Grip,
            wheel_contacts: WheelContacts::default(),
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
    role: AssetWheelRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetWheelRole {
    Front,
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
        let (longitudinal, lateral) =
            self.contacts
                .iter()
                .fold((0.0, 0.0), |(longitudinal, lateral), contact| {
                    let surface = surfaces.get(contact.surface);
                    (
                        longitudinal + surface.longitudinal_friction,
                        lateral + surface.lateral_friction,
                    )
                });
        let count = self.contacts.len() as f32;
        model::SurfaceFriction {
            longitudinal: longitudinal / count,
            lateral: lateral / count,
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
    spatial_query: SpatialQuery<'w, 's>,
    roads: Query<'w, 's, (Entity, &'static RoadCollider)>,
}

fn drive_car(ctx: DrivingContext, mut cars: Query<(&mut Transform, &mut PlayerCar)>) {
    let dt = ctx.time.delta_secs();
    let physics = AvianTrackPhysicsQueries::new(&ctx.spatial_query, &ctx.roads);

    for (mut transform, mut car) in &mut cars {
        if ctx.keys.just_pressed(KeyCode::KeyR) {
            car.reset_to_spawn(&mut transform, *ctx.car_spawn);
        }
        car.previous_translation = transform.translation;

        let input = model::ControlInput::from_keys(&ctx.keys);
        car.throttle = input.throttle;
        car.steer = input.steer;
        let ground = physics.ground_at(transform.translation);
        car.current_surface = ground.surface;
        car.ground_source = ground.source;

        let surface = ctx.surfaces.get(car.current_surface);
        let basis = model::MotionBasis::from_yaw(car.yaw, car.velocity);
        car.wheel_contacts = WheelContacts::sample(&physics, transform.translation, &basis);
        let contact_friction = car.wheel_contacts.friction(&ctx.surfaces);
        car.signed_speed = basis.forward_speed;
        car.slip_angle = basis.slip_angle();
        let intent =
            model::ControlIntent::from_input(&ctx.tuning, input, &basis, FRONT_WHEEL_MAX_STEER);
        car.drive_mode = intent.drive_mode;
        car.handling_state = model::handling_state(&ctx.tuning, &basis);
        car.wheel_steer_angle = intent.wheel_steer_angle;
        let tire_forces = model::tire_forces(
            &ctx.tuning,
            &surface,
            intent,
            &basis,
            car.handling_state,
            contact_friction,
        );
        car.tire_forces = tire_forces;
        car.yaw += tire_forces.yaw_delta * dt;
        car.velocity += tire_forces.acceleration * dt;

        let basis = model::MotionBasis::from_yaw(car.yaw, car.velocity);
        let capped_forward_speed = car
            .velocity
            .dot(basis.forward)
            .clamp(-ctx.tuning.max_reverse_speed, ctx.tuning.max_forward_speed);
        let capped_lateral_speed = car.velocity.dot(basis.right);
        car.velocity = basis.forward * capped_forward_speed + basis.right * capped_lateral_speed;

        let current_translation = transform.translation;
        let mut next_translation = current_translation + car.velocity * dt;
        if let Some(hit) = physics.cast_car_motion(current_translation, next_translation, car.yaw) {
            let motion = next_translation - current_translation;
            let travel = (hit.travel - CAR_COLLISION_SKIN).max(0.0);
            next_translation = current_translation + motion.normalize_or_zero() * travel;
            let inward_speed = car.velocity.dot(hit.normal);
            if inward_speed < 0.0 {
                car.velocity -= hit.normal * inward_speed * 1.35;
                car.velocity *= 0.78;
            }
        }

        next_translation.y = ctx.car_spawn.translation.y;
        transform.translation = next_translation;
        transform.rotation = yaw_rotation(car.yaw);
    }
}

fn update_car_body_visual(
    car: Single<(&Transform, &PlayerCar)>,
    mut body: Single<&mut Transform, (With<VehicleSceneRoot>, Without<PlayerCar>)>,
) {
    let (car_transform, car_state) = *car;
    let roll = car_state.steer * car_state.velocity.length() * BODY_ROLL_RATE * -0.01;
    let pitch = car_state.throttle * BODY_PITCH_RATE;

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
            role,
        });
    }
}

fn update_imported_wheel_visuals(
    time: Res<Time>,
    car: Single<&PlayerCar>,
    mut wheels: Query<(&mut Transform, &AssetWheelVisual), Without<PlayerCar>>,
) {
    let spin = time.elapsed_secs_wrapped() * car.signed_speed * ASSET_WHEEL_SPIN_RATE;
    for (mut transform, wheel) in &mut wheels {
        let steer_angle = asset_wheel_steer(car.wheel_steer_angle, wheel.role);
        transform.rotation = asset_wheel_rotation(wheel.base_rotation, steer_angle, spin);
    }
}

fn asset_wheel_steer(wheel_steer_angle: f32, role: AssetWheelRole) -> f32 {
    match role {
        AssetWheelRole::Front => wheel_steer_angle,
        AssetWheelRole::Rear => 0.0,
    }
}

fn asset_wheel_rotation(base_rotation: Quat, steer_angle: f32, spin: f32) -> Quat {
    base_rotation * Quat::from_rotation_y(steer_angle) * Quat::from_rotation_x(spin)
}

fn imported_wheel_role(name: &str) -> Option<AssetWheelRole> {
    if name.contains("FrontLeftWheel") || name.contains("FrontRightWheel") {
        Some(AssetWheelRole::Front)
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
    let target = car_transform.translation + Vec3::Y * 1.0;
    let desired_position = target - forward * (7.5 + speed * 0.06) + Vec3::Y * 4.2;
    let smoothing = 1.0 - (-8.0 * time.delta_secs()).exp();

    camera.translation = camera.translation.lerp(desired_position, smoothing);
    camera.look_at(target + forward * 4.0, Vec3::Y);
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
        assert!(asset_wheel_steer(0.42, AssetWheelRole::Front) > 0.0);
        assert!(asset_wheel_steer(-0.42, AssetWheelRole::Front) < 0.0);
        assert_eq!(asset_wheel_steer(1.0, AssetWheelRole::Rear), 0.0);
    }

    #[test]
    fn imported_wheel_rotation_points_with_steer_direction() {
        let right_steer = asset_wheel_steer(0.42, AssetWheelRole::Front);
        let left_steer = asset_wheel_steer(-0.42, AssetWheelRole::Front);
        let right = asset_wheel_rotation(Quat::IDENTITY, right_steer, 0.0) * Vec3::Z;
        let left = asset_wheel_rotation(Quat::IDENTITY, left_steer, 0.0) * Vec3::Z;

        assert!(right.x > 0.0);
        assert!(left.x < 0.0);
    }

    #[test]
    fn imported_vehicle_wheel_names_bind_to_visual_roles() {
        assert_eq!(
            imported_wheel_role("SportsCar_FrontLeftWheel_Cylinder.013"),
            Some(AssetWheelRole::Front)
        );
        assert_eq!(
            imported_wheel_role("SportsCar2_FrontRightWheel_Cylinder.018"),
            Some(AssetWheelRole::Front)
        );
        assert_eq!(
            imported_wheel_role("SportsCar_BackWheels_Cylinder.004"),
            Some(AssetWheelRole::Rear)
        );
        assert_eq!(imported_wheel_role("SportsCar_Cube.005"), None);
    }
}
