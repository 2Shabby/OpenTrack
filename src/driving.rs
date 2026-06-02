mod model;

use bevy::prelude::*;

pub use model::{DriveMode, DrivingTuning, HandlingState};

use crate::game_state::{GameState, not_paused};
use crate::physics::{EcsTrackPhysicsQueries, RailCollider, TrackPhysicsQueries};
use crate::surface::{SurfaceKind, SurfaceLibrary, SurfaceZone};

const DEFAULT_CAR_START: Vec3 = Vec3::new(0.0, 0.05, -26.0);
const WHEEL_SAMPLE_HALF_WIDTH: f32 = 0.82;
const WHEEL_SAMPLE_HALF_LENGTH: f32 = 1.72;
const BODY_ROLL_RATE: f32 = 0.18;
const BODY_PITCH_RATE: f32 = 0.05;
const BODY_VISUAL_HEIGHT: f32 = 0.0;

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
        Quat::from_rotation_y(self.yaw)
    }
}

pub struct DrivingPlugin;

impl Plugin for DrivingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DrivingTuning::default())
            .insert_resource(CarSpawn::default())
            .insert_resource(CarPaint::default())
            .add_systems(
                FixedUpdate,
                drive_car.run_if(in_state(GameState::Driving).and(not_paused)),
            )
            .add_systems(
                Update,
                (update_car_visuals, chase_camera)
                    .chain()
                    .after(drive_car)
                    .run_if(in_state(GameState::Driving).and(not_paused)),
            );
    }
}

#[derive(Clone, Copy, Resource)]
pub struct CarPaint {
    pub color: Color,
}

impl Default for CarPaint {
    fn default() -> Self {
        Self {
            color: Color::srgb(0.92, 0.08, 0.05),
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
    pub slip_angle: f32,
    pub drive_mode: DriveMode,
    pub handling_state: HandlingState,
    pub wheel_contacts: WheelContacts,
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
            slip_angle: 0.0,
            drive_mode: DriveMode::Forward,
            handling_state: HandlingState::Grip,
            wheel_contacts: WheelContacts::default(),
        }
    }
}

impl PlayerCar {
    pub fn reset_to_spawn(&mut self, transform: &mut Transform, car_spawn: CarSpawn) {
        *self = Self::default();
        self.yaw = car_spawn.yaw;
        transform.translation = car_spawn.translation;
        transform.rotation = car_spawn.rotation();
    }
}

#[derive(Component)]
pub struct ChaseCamera;

#[derive(Component)]
pub struct CarBodyVisual;

#[derive(Component)]
pub struct WheelVisual {
    pub local_offset: Vec3,
    pub front: bool,
    pub corner: WheelCorner,
}

#[derive(Clone, Copy)]
pub enum WheelCorner {
    FrontLeft,
    FrontRight,
    RearLeft,
    RearRight,
}

#[derive(Clone, Copy)]
pub struct WheelContacts {
    pub front_left: SurfaceKind,
    pub front_right: SurfaceKind,
    pub rear_left: SurfaceKind,
    pub rear_right: SurfaceKind,
}

impl Default for WheelContacts {
    fn default() -> Self {
        Self {
            front_left: SurfaceKind::Asphalt,
            front_right: SurfaceKind::Asphalt,
            rear_left: SurfaceKind::Asphalt,
            rear_right: SurfaceKind::Asphalt,
        }
    }
}

impl WheelContacts {
    pub fn summary(self) -> String {
        format!(
            "FL:{} FR:{} RL:{} RR:{}",
            self.front_left.label(),
            self.front_right.label(),
            self.rear_left.label(),
            self.rear_right.label()
        )
    }

    pub fn average_lateral_grip(self, surfaces: &SurfaceLibrary) -> f32 {
        let contacts = [
            self.front_left,
            self.front_right,
            self.rear_left,
            self.rear_right,
        ];

        contacts
            .iter()
            .map(|surface| surfaces.get(*surface).lateral_grip)
            .sum::<f32>()
            / contacts.len() as f32
    }

    pub fn split_surface(self) -> bool {
        self.front_left != self.front_right
            || self.front_left != self.rear_left
            || self.front_left != self.rear_right
    }

    pub fn at(self, corner: WheelCorner) -> SurfaceKind {
        match corner {
            WheelCorner::FrontLeft => self.front_left,
            WheelCorner::FrontRight => self.front_right,
            WheelCorner::RearLeft => self.rear_left,
            WheelCorner::RearRight => self.rear_right,
        }
    }
}

fn drive_car(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    car_spawn: Res<CarSpawn>,
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
            car.reset_to_spawn(&mut transform, *car_spawn);
        }

        let controls = model::DriverControls::from_keys(&keys);
        car.throttle = controls.throttle;
        car.steer = controls.steer;
        car.current_surface = physics.surface_at(transform.translation);

        let surface = surfaces.get(car.current_surface);
        let basis = model::MotionBasis::from_yaw(car.yaw, car.velocity);
        car.wheel_contacts = sample_wheel_contacts(&physics, transform.translation, &basis);
        let contact_lateral_grip = car.wheel_contacts.average_lateral_grip(&surfaces);
        car.signed_speed = basis.forward_speed;
        car.slip_angle = basis.slip_angle();
        car.drive_mode = model::drive_mode(controls.throttle, basis.forward_speed);
        car.handling_state = model::handling_state(&tuning, &basis);

        car.yaw += model::steering_yaw_delta(&tuning, &surface, controls, &basis) * dt;
        car.yaw += model::slide_yaw_assist(&tuning, controls, &basis, car.handling_state) * dt;

        let basis = model::MotionBasis::from_yaw(car.yaw, car.velocity);
        let drive_force =
            model::drive_force(&tuning, &surface, controls.throttle, basis.forward_speed);
        let lateral_grip_multiplier = model::lateral_grip_multiplier(&tuning, car.handling_state);

        car.velocity += basis.forward * controls.throttle * drive_force * dt;
        car.velocity += basis.forward * surface.boost_force * dt;
        car.velocity -= basis.right
            * basis.lateral_speed
            * tuning.lateral_grip
            * contact_lateral_grip
            * lateral_grip_multiplier
            * dt;
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

        next_translation.y = car_spawn.translation.y;
        transform.translation = next_translation;
        transform.rotation = Quat::from_rotation_y(car.yaw);
    }
}

fn sample_wheel_contacts(
    physics: &impl TrackPhysicsQueries,
    center: Vec3,
    basis: &model::MotionBasis,
) -> WheelContacts {
    let front = basis.forward * WHEEL_SAMPLE_HALF_LENGTH;
    let rear = -basis.forward * WHEEL_SAMPLE_HALF_LENGTH;
    let left = -basis.right * WHEEL_SAMPLE_HALF_WIDTH;
    let right = basis.right * WHEEL_SAMPLE_HALF_WIDTH;

    WheelContacts {
        front_left: physics.surface_at(center + front + left),
        front_right: physics.surface_at(center + front + right),
        rear_left: physics.surface_at(center + rear + left),
        rear_right: physics.surface_at(center + rear + right),
    }
}

fn update_car_visuals(
    time: Res<Time>,
    car: Single<(&Transform, &PlayerCar)>,
    mut body: Single<&mut Transform, (With<CarBodyVisual>, Without<PlayerCar>)>,
    mut wheels: Query<
        (
            &mut Transform,
            &WheelVisual,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<PlayerCar>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (car_transform, car_state) = *car;
    let forward = Vec3::new(car_state.yaw.sin(), 0.0, car_state.yaw.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let roll = car_state.steer * car_state.velocity.length() * BODY_ROLL_RATE * -0.01;
    let pitch = car_state.throttle * BODY_PITCH_RATE;

    body.translation = car_transform.translation + Vec3::Y * BODY_VISUAL_HEIGHT;
    body.rotation = Quat::from_rotation_y(car_state.yaw)
        * Quat::from_rotation_z(roll)
        * Quat::from_rotation_x(pitch);

    let spin = time.elapsed_secs_wrapped() * car_state.signed_speed * 2.0;
    for (mut transform, wheel, material) in &mut wheels {
        let world_offset = right * wheel.local_offset.x
            + Vec3::Y * wheel.local_offset.y
            + forward * wheel.local_offset.z;
        let steer_angle = if wheel.front {
            car_state.steer * 0.42
        } else {
            0.0
        };

        transform.translation = car_transform.translation + world_offset;
        transform.rotation =
            Quat::from_rotation_y(car_state.yaw + steer_angle) * Quat::from_rotation_x(spin);

        if let Some(material) = materials.get_mut(material) {
            material.base_color = wheel_color(car_state.wheel_contacts.at(wheel.corner));
        }
    }
}

fn wheel_color(surface: SurfaceKind) -> Color {
    match surface {
        SurfaceKind::Grass => Color::srgb(0.13, 0.35, 0.09),
        SurfaceKind::Ice => Color::srgb(0.45, 0.75, 0.82),
        SurfaceKind::Boost => Color::srgb(0.85, 0.54, 0.05),
        SurfaceKind::Dirt => Color::srgb(0.34, 0.2, 0.1),
        SurfaceKind::Asphalt => Color::srgb(0.02, 0.02, 0.018),
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
