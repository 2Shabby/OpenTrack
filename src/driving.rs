mod model;
mod support;

use avian3d::prelude::MoveAndSlide;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub use model::{DriftAssist, DriveMode, DrivingTuning, HandlingState};
pub use support::{CAR_GROUND_OFFSET, VehicleSupportFrame, WheelContacts};

use crate::game_state::{GameState, not_paused};
use crate::geometry::rotation_from_yaw_and_up;
use crate::physics::{
    AvianTrackPhysicsQueries, CarCollisionDebug, CarPose, CollisionState, GroundContact,
    GroundSource, RoadCollider, TrackPhysicsQueries,
};
use crate::surface::{SurfaceKind, SurfaceLibrary};
use support::sample_vehicle_support;

const DEFAULT_CAR_START: Vec3 = Vec3::new(0.0, CAR_GROUND_OFFSET, -26.0);
const BODY_SUSPENSION_ROLL_SCALE: f32 = 0.16;
const BODY_SUSPENSION_PITCH_SCALE: f32 = 0.12;
const BODY_VISUAL_HEIGHT: f32 = 0.0;
const SPORTS_CAR_ASSET_SCALE: f32 = 0.01;
const SPORTS_CAR_FRONT_WHEEL_OUTSET: f32 = 0.18;
const SPORTS_CAR_REAR_WHEEL_WIDTH_SCALE: f32 = 1.16;
const SPORTS_CAR_VISUAL_STEER_MULTIPLIER: f32 = 2.0;
const SPORTS_CAR_MAX_VISUAL_STEER: f32 = 0.62;
const DRIVE_DEBUG_LOG_INTERVAL: f32 = 0.5;
const WHEEL_CONTACT_COUNT: usize = model::WHEEL_COUNT;

type VehicleMaterialNodes<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static MeshMaterial3d<StandardMaterial>),
    (With<Mesh3d>, Without<VehicleMaterialChecked>),
>;
type VehicleWheelCandidates<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), (With<Mesh3d>, Without<AssetWheelVisual>)>;
type VehicleSceneRoots<'w, 's> = Query<'w, 's, (), With<VehicleSceneRoot>>;
type SceneParents<'w, 's> = Query<'w, 's, &'static ChildOf>;

#[derive(Default)]
struct DriveDebugLogState {
    elapsed: f32,
}

struct DriveDebugSnapshot<'a> {
    input: model::ControlInput,
    force_basis: &'a model::MotionBasis,
    movement_basis: &'a model::MotionBasis,
    ground: GroundContact,
    resolved_ground: GroundContact,
    support_frame: VehicleSupportFrame,
    desired_yaw: f32,
    car: &'a PlayerCar,
    transform: &'a Transform,
}

#[derive(Clone, Copy, Resource)]
pub struct CarSpawn {
    pub translation: Vec3,
    pub yaw: f32,
    pub up: Vec3,
}

impl Default for CarSpawn {
    fn default() -> Self {
        Self {
            translation: DEFAULT_CAR_START,
            yaw: 0.0,
            up: Vec3::Y,
        }
    }
}

impl CarSpawn {
    pub fn rotation(self) -> Quat {
        rotation_from_yaw_and_up(self.yaw, self.up)
    }

    pub fn transform(self) -> Transform {
        Transform::from_translation(self.translation).with_rotation(self.rotation())
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
                    normalize_vehicle_materials,
                    update_imported_wheel_visuals,
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
    pub support_frame: VehicleSupportFrame,
    pub throttle: f32,
    pub steer: f32,
    pub rear_brake: f32,
    pub wheel_steer_target_angle: f32,
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
    pub vehicle_feedback: model::VehicleFeedback,
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
            support_frame: VehicleSupportFrame::default(),
            throttle: 0.0,
            steer: 0.0,
            rear_brake: 0.0,
            wheel_steer_target_angle: 0.0,
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
            vehicle_feedback: model::VehicleFeedback::default(),
            tire_forces: model::TireForces::default(),
        }
    }
}

impl PlayerCar {
    pub fn reset_to_spawn(&mut self, transform: &mut Transform, car_spawn: CarSpawn) {
        *self = Self::default();
        self.yaw = car_spawn.yaw;
        self.support_frame = VehicleSupportFrame::from_spawn(car_spawn);
        self.previous_translation = car_spawn.translation;
        *transform = car_spawn.transform();
    }
}

#[derive(Component)]
pub struct VehicleSceneRoot;

#[derive(Component)]
struct VehicleMaterialChecked;

#[derive(Component)]
pub struct AssetWheelVisual {
    base_rotation: Quat,
    base_translation: Vec3,
    base_scale: Vec3,
    visual_translation_offset: Vec3,
    visual_scale: Vec3,
    role: AssetWheelRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetWheelRole {
    FrontLeft,
    FrontRight,
    Rear,
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

fn drive_car(
    ctx: DrivingContext,
    mut debug_log: Local<DriveDebugLogState>,
    mut cars: Query<(Entity, &mut Transform, &mut PlayerCar)>,
) {
    let dt = ctx.time.delta_secs();
    let physics = AvianTrackPhysicsQueries::new(&ctx.move_and_slide, &ctx.roads);
    debug_log.elapsed += dt;
    let should_log = debug_log.elapsed >= DRIVE_DEBUG_LOG_INTERVAL;
    if should_log {
        debug_log.elapsed = 0.0;
    }

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
        let support_sample = sample_vehicle_support(
            &physics,
            car.yaw,
            transform.translation,
            car.velocity,
            car.support_frame.normal,
            car.support_frame,
        );
        if support_sample.is_airborne_offtrack() {
            log_offtrack_reset(
                "center",
                entity,
                transform.translation,
                support_sample.center_ground,
            );
            car.reset_to_spawn(&mut transform, *ctx.car_spawn);
            continue;
        }
        let support_frame = car
            .support_frame
            .resolved_towards(support_sample.target, car.yaw, dt);
        car.support_frame = support_frame;
        car.wheel_contacts = support_sample.contacts;
        car.current_surface = support_frame.surface;
        car.ground_source = support_frame.ground_source(support_sample.center_ground);
        car.boost_direction = support_frame.boost_direction;

        let surface = ctx.surfaces.get(car.current_surface);
        let contact_friction = car.wheel_contacts.friction(&ctx.surfaces);
        let force_basis =
            model::MotionBasis::from_ground(car.yaw, car.support_frame.normal, car.velocity);
        car.signed_speed = force_basis.forward_speed;
        car.slip_angle = force_basis.slip_angle();
        let target_intent = model::ControlIntent::from_input(&ctx.tuning, input, &force_basis);
        car.drift_assist =
            model::DriftAssist::from_input(&ctx.tuning, &surface, input, &force_basis);
        car.drive_mode = target_intent.drive_mode;
        car.wheel_steer_target_angle = target_intent.wheel_steer_angle;
        car.wheel_steer_angle = model::resolved_wheel_steer_angle(
            &ctx.tuning,
            car.wheel_steer_angle,
            car.wheel_steer_target_angle,
            dt,
        );
        let intent = target_intent.with_wheel_steer_angle(car.wheel_steer_angle);
        let tire_forces = model::tire_forces(model::TireForceInput {
            tuning: &ctx.tuning,
            surface: &surface,
            intent,
            basis: &force_basis,
            previous_handling_state: car.handling_state,
            contact_friction,
            boost_direction: support_frame.boost_direction,
            drift_assist: car.drift_assist,
            gravity_acceleration: model::gravity_acceleration(&ctx.tuning, support_frame.normal),
            normal_load_scale: model::normal_load_scale(&ctx.tuning, support_frame.normal),
        });
        car.tire_forces = tire_forces;
        car.handling_state = tire_forces.handling_state;
        car.yaw_rate =
            model::resolved_yaw_rate(&ctx.tuning, car.yaw_rate, tire_forces.target_yaw_rate, dt);
        let desired_yaw = car.yaw + car.yaw_rate * dt;
        car.velocity += tire_forces.acceleration * dt;

        let movement_basis =
            model::MotionBasis::from_ground(desired_yaw, support_frame.normal, car.velocity);
        let capped_forward_speed = car
            .velocity
            .dot(movement_basis.forward)
            .clamp(-ctx.tuning.max_reverse_speed, ctx.tuning.max_forward_speed);
        let capped_lateral_speed = car.velocity.dot(movement_basis.right);
        car.velocity = movement_basis.forward * capped_forward_speed
            + movement_basis.right * capped_lateral_speed;
        let wheel_basis =
            model::MotionBasis::from_ground(desired_yaw, support_frame.normal, car.velocity);
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
        car.vehicle_feedback =
            model::vehicle_feedback(&ctx.tuning, &car.wheel_telemetry, &tire_forces);

        let current_translation = transform.translation;
        let desired_translation = current_translation + car.velocity * dt;
        let desired_sample = sample_vehicle_support(
            &physics,
            desired_yaw,
            desired_translation,
            car.velocity,
            support_frame.normal,
            support_frame,
        );
        let resolution = physics.resolve_car_pose(
            CarPose {
                translation: current_translation,
                yaw: car.yaw,
                up: support_frame.normal,
            },
            CarPose {
                translation: desired_translation,
                yaw: desired_yaw,
                up: desired_sample.target.normal,
            },
            car.velocity,
            ctx.time.delta(),
            entity,
        );
        let resolved_sample = sample_vehicle_support(
            &physics,
            resolution.pose.yaw,
            resolution.pose.translation,
            resolution.velocity,
            resolution.pose.up,
            support_frame,
        );
        if resolved_sample.is_airborne_offtrack() {
            log_offtrack_reset(
                "resolved",
                entity,
                resolution.pose.translation,
                resolved_sample.center_ground,
            );
            car.reset_to_spawn(&mut transform, *ctx.car_spawn);
            continue;
        }

        let resolved_support =
            support_frame.resolved_towards(resolved_sample.target, resolution.pose.yaw, dt);
        let next_translation = resolved_support.supported_center(resolution.pose.translation);
        let next_velocity = project_onto_plane(resolution.velocity, resolved_support.normal);
        car.yaw = resolution.pose.yaw;
        car.yaw_rate = accepted_yaw_rate(&resolution.debug, dt);
        car.velocity = next_velocity;
        car.collision_state = resolution.state;
        car.collision_debug = resolution.debug;
        car.support_frame = resolved_support;
        car.wheel_contacts = resolved_sample.contacts;
        car.current_surface = resolved_support.surface;
        car.ground_source = resolved_support.ground_source(resolved_sample.center_ground);
        car.boost_direction = resolved_support.boost_direction;
        transform.translation = next_translation;
        transform.rotation = resolved_support.rotation;
        if should_log {
            log_drive_debug(DriveDebugSnapshot {
                input,
                force_basis: &force_basis,
                movement_basis: &movement_basis,
                ground: support_sample.center_ground,
                resolved_ground: resolved_sample.center_ground,
                support_frame: resolved_support,
                desired_yaw,
                car: &car,
                transform: &transform,
            });
        }
    }
}

fn log_drive_debug(snapshot: DriveDebugSnapshot<'_>) {
    let car_up = snapshot.transform.rotation * Vec3::Y;
    info!(
        target: "drive_debug",
        "ad_map=A:+1,D:-1 input(throttle={:+.0},steer={:+.0},rear={:+.0}) \
    pos=({:+.2},{:+.2},{:+.2}) vel=({:+.2},{:+.2},{:+.2}) speed={:.2} signed={:+.2} \
    yaw(current={:+.3},desired={:+.3},rate_target={:+.3},rate_actual={:+.3}) \
    steer(target={:+.3},actual={:+.3}) basis_force(f=({:+.2},{:+.2},{:+.2}),r=({:+.2},{:+.2},{:+.2})) \
    basis_move(f=({:+.2},{:+.2},{:+.2}),r=({:+.2},{:+.2},{:+.2})) \
    ground={} n=({:+.2},{:+.2},{:+.2}) resolved={} n=({:+.2},{:+.2},{:+.2}) support={} contacts={} n=({:+.2},{:+.2},{:+.2}) f=({:+.2},{:+.2},{:+.2}) r=({:+.2},{:+.2},{:+.2}) car_up=({:+.2},{:+.2},{:+.2}) \
    handling={} slide={} collision={} yaw_req/ok={:+.3}/{:+.3} move_req/ok={:.2}/{:.2} wheels={} split={}",
        snapshot.input.throttle,
        snapshot.input.steer,
        snapshot.input.rear_brake,
        snapshot.transform.translation.x,
        snapshot.transform.translation.y,
        snapshot.transform.translation.z,
        snapshot.car.velocity.x,
        snapshot.car.velocity.y,
        snapshot.car.velocity.z,
        snapshot.car.velocity.length(),
        snapshot.car.signed_speed,
        snapshot.car.yaw,
        snapshot.desired_yaw,
        snapshot.car.tire_forces.target_yaw_rate,
        snapshot.car.yaw_rate,
        snapshot.car.wheel_steer_target_angle,
        snapshot.car.wheel_steer_angle,
        snapshot.force_basis.forward.x,
        snapshot.force_basis.forward.y,
        snapshot.force_basis.forward.z,
        snapshot.force_basis.right.x,
        snapshot.force_basis.right.y,
        snapshot.force_basis.right.z,
        snapshot.movement_basis.forward.x,
        snapshot.movement_basis.forward.y,
        snapshot.movement_basis.forward.z,
        snapshot.movement_basis.right.x,
        snapshot.movement_basis.right.y,
        snapshot.movement_basis.right.z,
        snapshot.ground.label(),
        snapshot.ground.normal.x,
        snapshot.ground.normal.y,
        snapshot.ground.normal.z,
        snapshot.resolved_ground.label(),
        snapshot.resolved_ground.normal.x,
        snapshot.resolved_ground.normal.y,
        snapshot.resolved_ground.normal.z,
        snapshot.support_frame.state_label(),
        snapshot.support_frame.contact_count,
        snapshot.support_frame.normal.x,
        snapshot.support_frame.normal.y,
        snapshot.support_frame.normal.z,
        snapshot.support_frame.forward.x,
        snapshot.support_frame.forward.y,
        snapshot.support_frame.forward.z,
        snapshot.support_frame.right.x,
        snapshot.support_frame.right.y,
        snapshot.support_frame.right.z,
        car_up.x,
        car_up.y,
        car_up.z,
        snapshot.car.handling_state.label(),
        snapshot.car.tire_forces.slide_reason.label(),
        snapshot.car.collision_state.label(),
        snapshot.car.collision_debug.requested_yaw_delta,
        snapshot.car.collision_debug.accepted_yaw_delta,
        snapshot.car.collision_debug.requested_translation_delta.length(),
        snapshot.car.collision_debug.accepted_translation_delta.length(),
        snapshot.car.wheel_contacts.summary(),
        snapshot.car.wheel_contacts.split_surface(),
    );
}

fn log_offtrack_reset(phase: &str, entity: Entity, position: Vec3, ground: GroundContact) {
    info!(
        target: "drive_debug",
        "offtrack_reset phase={} entity={:?} pos=({:+.2},{:+.2},{:+.2}) ground={} n=({:+.2},{:+.2},{:+.2})",
        phase,
        entity,
        position.x,
        position.y,
        position.z,
        ground.label(),
        ground.normal.x,
        ground.normal.y,
        ground.normal.z,
    );
}

fn project_onto_plane(value: Vec3, normal: Vec3) -> Vec3 {
    let normal = normal.normalize_or(Vec3::Y);
    value - normal * value.dot(normal)
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

    body.translation =
        car_transform.translation + car_state.support_frame.normal * BODY_VISUAL_HEIGHT;
    body.rotation =
        car_transform.rotation * Quat::from_rotation_z(roll) * Quat::from_rotation_x(pitch);
}

fn bind_imported_vehicle_wheels(
    mut commands: Commands,
    wheels: VehicleWheelCandidates,
    vehicle_roots: VehicleSceneRoots,
    parents: SceneParents,
) {
    for (entity, transform) in &wheels {
        if !is_vehicle_scene_descendant(entity, &vehicle_roots, &parents) {
            continue;
        }

        let Some(role) = sports_car_wheel_role_from_transform(transform) else {
            continue;
        };

        commands.entity(entity).insert(AssetWheelVisual {
            base_rotation: transform.rotation,
            base_translation: transform.translation,
            base_scale: transform.scale,
            visual_translation_offset: asset_wheel_translation_offset(role),
            visual_scale: asset_wheel_role_scale(role),
            role,
        });
    }
}

fn normalize_vehicle_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    material_nodes: VehicleMaterialNodes,
    vehicle_roots: VehicleSceneRoots,
    parents: SceneParents,
) {
    for (entity, material_handle) in &material_nodes {
        if !is_vehicle_scene_descendant(entity, &vehicle_roots, &parents) {
            continue;
        }

        let Some(material) = materials.get_mut(&material_handle.0) else {
            continue;
        };
        material.base_color.set_alpha(1.0);
        material.alpha_mode = AlphaMode::Opaque;

        commands.entity(entity).insert(VehicleMaterialChecked);
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
        transform.translation =
            wheel.base_translation + wheel.visual_translation_offset + Vec3::Y * suspension_offset;
        transform.rotation = asset_wheel_rotation(wheel.base_rotation, steer_angle, spin);
        transform.scale =
            wheel.base_scale * wheel.visual_scale * asset_wheel_load_scale(&car, wheel.role);
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
        AssetWheelRole::FrontLeft | AssetWheelRole::FrontRight => (wheel_steer_angle
            * SPORTS_CAR_VISUAL_STEER_MULTIPLIER)
            .clamp(-SPORTS_CAR_MAX_VISUAL_STEER, SPORTS_CAR_MAX_VISUAL_STEER),
        AssetWheelRole::Rear => 0.0,
    }
}

fn asset_wheel_translation_offset(role: AssetWheelRole) -> Vec3 {
    match role {
        AssetWheelRole::FrontLeft => Vec3::X * SPORTS_CAR_FRONT_WHEEL_OUTSET,
        AssetWheelRole::FrontRight => Vec3::NEG_X * SPORTS_CAR_FRONT_WHEEL_OUTSET,
        AssetWheelRole::Rear => Vec3::ZERO,
    }
}

fn asset_wheel_role_scale(role: AssetWheelRole) -> Vec3 {
    match role {
        AssetWheelRole::FrontLeft | AssetWheelRole::FrontRight => Vec3::ONE,
        AssetWheelRole::Rear => Vec3::new(SPORTS_CAR_REAR_WHEEL_WIDTH_SCALE, 1.0, 1.0),
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

fn asset_wheel_load_scale(car: &PlayerCar, role: AssetWheelRole) -> Vec3 {
    let load = (asset_wheel_suspension_compression(car, role) - 0.5).clamp(-0.35, 0.45);
    let slip = asset_wheel_slip(car, role).clamp(0.0, 1.0);
    let sidewall = (1.0 - load * 0.08).clamp(0.94, 1.04);
    let radius = (1.0 + load * 0.04 + slip * 0.015).clamp(0.97, 1.05);

    Vec3::new(1.0, sidewall, radius)
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

fn asset_wheel_suspension_compression(car: &PlayerCar, role: AssetWheelRole) -> f32 {
    match role {
        AssetWheelRole::FrontLeft => car.wheel_suspension[model::FRONT_LEFT_WHEEL].compression,
        AssetWheelRole::FrontRight => car.wheel_suspension[model::FRONT_RIGHT_WHEEL].compression,
        AssetWheelRole::Rear => {
            (car.wheel_suspension[model::REAR_LEFT_WHEEL].compression
                + car.wheel_suspension[model::REAR_RIGHT_WHEEL].compression)
                * 0.5
        }
    }
}

fn asset_wheel_slip(car: &PlayerCar, role: AssetWheelRole) -> f32 {
    match role {
        AssetWheelRole::FrontLeft => car.wheel_telemetry[model::FRONT_LEFT_WHEEL]
            .slip_ratio
            .abs(),
        AssetWheelRole::FrontRight => car.wheel_telemetry[model::FRONT_RIGHT_WHEEL]
            .slip_ratio
            .abs(),
        AssetWheelRole::Rear => {
            (car.wheel_telemetry[model::REAR_LEFT_WHEEL].slip_ratio.abs()
                + car.wheel_telemetry[model::REAR_RIGHT_WHEEL]
                    .slip_ratio
                    .abs())
                * 0.5
        }
    }
}

fn asset_wheel_rotation(base_rotation: Quat, steer_angle: f32, spin: f32) -> Quat {
    Quat::from_rotation_y(steer_angle) * base_rotation * Quat::from_rotation_x(spin)
}

fn sports_car_wheel_role_from_transform(transform: &Transform) -> Option<AssetWheelRole> {
    if !is_sports_car_import_scale(transform.scale) {
        return None;
    }

    let position = transform.translation;
    if position.z > 0.8 && position.x > 0.3 {
        Some(AssetWheelRole::FrontLeft)
    } else if position.z > 0.8 && position.x < -0.3 {
        Some(AssetWheelRole::FrontRight)
    } else if position.z < -0.8 && position.x.abs() < 0.25 {
        Some(AssetWheelRole::Rear)
    } else {
        None
    }
}

fn is_vehicle_scene_descendant(
    entity: Entity,
    vehicle_roots: &VehicleSceneRoots,
    parents: &SceneParents,
) -> bool {
    let mut current = entity;
    for _ in 0..32 {
        if vehicle_roots.get(current).is_ok() {
            return true;
        }

        let Ok(parent) = parents.get(current) else {
            return false;
        };
        current = parent.parent();
    }
    false
}

fn is_sports_car_import_scale(scale: Vec3) -> bool {
    (scale.x - SPORTS_CAR_ASSET_SCALE).abs() <= 0.002
        && (scale.y - SPORTS_CAR_ASSET_SCALE).abs() <= 0.002
        && (scale.z - SPORTS_CAR_ASSET_SCALE).abs() <= 0.002
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
    fn imported_front_wheels_exaggerate_steer_visually() {
        let visual_steer = asset_wheel_steer(0.2, AssetWheelRole::FrontLeft);
        let clamped_steer = asset_wheel_steer(2.0, AssetWheelRole::FrontLeft);

        assert!(visual_steer.abs() > 0.2);
        assert_eq!(clamped_steer, SPORTS_CAR_MAX_VISUAL_STEER);
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
    fn sports_car_front_wheels_are_pushed_outward() {
        assert!(asset_wheel_translation_offset(AssetWheelRole::FrontLeft).x > 0.0);
        assert!(asset_wheel_translation_offset(AssetWheelRole::FrontRight).x < 0.0);
        assert_eq!(
            asset_wheel_translation_offset(AssetWheelRole::Rear),
            Vec3::ZERO
        );
    }

    #[test]
    fn sports_car_rear_wheel_mesh_is_widened_without_rescaling_body() {
        assert_eq!(asset_wheel_role_scale(AssetWheelRole::FrontLeft), Vec3::ONE);
        assert!(asset_wheel_role_scale(AssetWheelRole::Rear).x > 1.0);
        assert_eq!(asset_wheel_role_scale(AssetWheelRole::Rear).y, 1.0);
    }

    #[test]
    fn wheel_load_scale_squashes_loaded_tires_lightly() {
        let mut car = PlayerCar::default();
        car.wheel_suspension[model::FRONT_LEFT_WHEEL].compression = 0.8;
        car.wheel_telemetry[model::FRONT_LEFT_WHEEL].slip_ratio = 0.5;

        let scale = asset_wheel_load_scale(&car, AssetWheelRole::FrontLeft);

        assert!(scale.y < 1.0);
        assert!(scale.z > 1.0);
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
    fn sports_car_wheel_meshes_bind_from_imported_local_transform() {
        let front_left = Transform::from_translation(Vec3::new(0.719, 0.250, 1.188))
            .with_scale(Vec3::splat(SPORTS_CAR_ASSET_SCALE));
        let front_right = Transform::from_translation(Vec3::new(-0.714, 0.250, 1.188))
            .with_scale(Vec3::splat(SPORTS_CAR_ASSET_SCALE));
        let rear = Transform::from_translation(Vec3::new(0.003, 0.250, -1.255))
            .with_scale(Vec3::splat(SPORTS_CAR_ASSET_SCALE));
        let body =
            Transform::from_translation(Vec3::ZERO).with_scale(Vec3::splat(SPORTS_CAR_ASSET_SCALE));

        assert_eq!(
            sports_car_wheel_role_from_transform(&front_left),
            Some(AssetWheelRole::FrontLeft)
        );
        assert_eq!(
            sports_car_wheel_role_from_transform(&front_right),
            Some(AssetWheelRole::FrontRight)
        );
        assert_eq!(
            sports_car_wheel_role_from_transform(&rear),
            Some(AssetWheelRole::Rear)
        );
        assert_eq!(sports_car_wheel_role_from_transform(&body), None);
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
}
