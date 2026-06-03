use bevy::prelude::*;

use crate::geometry::{forward_3d, right_3d};
use crate::surface::SurfaceParams;

pub const WHEEL_COUNT: usize = 4;
pub const FRONT_LEFT_WHEEL: usize = 0;
pub const FRONT_RIGHT_WHEEL: usize = 1;
pub const REAR_LEFT_WHEEL: usize = 2;
pub const REAR_RIGHT_WHEEL: usize = 3;

#[derive(Resource)]
pub struct DrivingTuning {
    pub mass: f32,
    pub gravity: f32,
    pub engine_force: f32,
    pub brake_force: f32,
    pub reverse_force: f32,
    pub steer_rate: f32,
    pub min_steer_authority: f32,
    pub max_forward_speed: f32,
    pub max_reverse_speed: f32,
    pub reverse_steering_multiplier: f32,
    pub wheelbase: f32,
    pub track_width: f32,
    pub center_of_gravity_height: f32,
    pub front_weight_bias: f32,
    pub lateral_stiffness: f32,
    pub drift_yaw_response: f32,
    pub slide_speed_threshold: f32,
    pub slide_slip_angle_threshold: f32,
}

const MIN_STEERING_SPEED: f32 = 0.5;

impl Default for DrivingTuning {
    fn default() -> Self {
        Self {
            mass: 1_180.0,
            gravity: 9.81,
            engine_force: 13_800.0,
            brake_force: 18_500.0,
            reverse_force: 6_500.0,
            steer_rate: 2.5,
            min_steer_authority: 0.22,
            max_forward_speed: 58.0,
            max_reverse_speed: 14.0,
            reverse_steering_multiplier: 0.45,
            wheelbase: 3.44,
            track_width: 1.64,
            center_of_gravity_height: 0.42,
            front_weight_bias: 0.54,
            lateral_stiffness: 0.26,
            drift_yaw_response: 0.035,
            slide_speed_threshold: 10.0,
            slide_slip_angle_threshold: 0.48,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveMode {
    Forward,
    Braking,
    Reverse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandlingState {
    Grip,
    Sliding,
}

impl HandlingState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Grip => "grip",
            Self::Sliding => "sliding",
        }
    }
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

#[derive(Clone, Copy)]
pub struct ControlInput {
    pub throttle: f32,
    pub steer: f32,
}

impl ControlInput {
    pub fn from_keys(keys: &ButtonInput<KeyCode>) -> Self {
        Self {
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
}

#[derive(Clone, Copy)]
pub struct ControlIntent {
    pub input: ControlInput,
    pub drive_mode: DriveMode,
    pub wheel_steer_angle: f32,
    steering_motion_direction: f32,
    pub mode_steering_multiplier: f32,
}

impl ControlIntent {
    pub fn from_input(
        tuning: &DrivingTuning,
        input: ControlInput,
        basis: &MotionBasis,
        max_visual_angle: f32,
    ) -> Self {
        let drive_mode = drive_mode(input.throttle, basis.forward_speed);
        let mode_steering_multiplier = match drive_mode {
            DriveMode::Forward | DriveMode::Braking => 1.0,
            DriveMode::Reverse => tuning.reverse_steering_multiplier,
        };

        Self {
            input,
            drive_mode,
            wheel_steer_angle: input.steer * max_visual_angle,
            steering_motion_direction: steering_motion_direction(input, basis),
            mode_steering_multiplier,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TireForces {
    pub acceleration: Vec3,
    pub yaw_delta: f32,
    pub normal_load: f32,
    pub wheel_normal_loads: [f32; WHEEL_COUNT],
    pub front_normal_load: f32,
    pub rear_normal_load: f32,
    pub friction_limit: f32,
    pub longitudinal_force: f32,
    pub lateral_force: f32,
    pub wheel_lateral_forces: [f32; WHEEL_COUNT],
    pub front_lateral_force: f32,
    pub rear_lateral_force: f32,
    pub saturation: f32,
    pub wheel_saturations: [f32; WHEEL_COUNT],
    pub front_saturation: f32,
    pub rear_saturation: f32,
    pub handling_state: HandlingState,
}

impl Default for TireForces {
    fn default() -> Self {
        Self {
            acceleration: Vec3::ZERO,
            yaw_delta: 0.0,
            normal_load: 0.0,
            wheel_normal_loads: [0.0; WHEEL_COUNT],
            front_normal_load: 0.0,
            rear_normal_load: 0.0,
            friction_limit: 0.0,
            longitudinal_force: 0.0,
            lateral_force: 0.0,
            wheel_lateral_forces: [0.0; WHEEL_COUNT],
            front_lateral_force: 0.0,
            rear_lateral_force: 0.0,
            saturation: 0.0,
            wheel_saturations: [0.0; WHEEL_COUNT],
            front_saturation: 0.0,
            rear_saturation: 0.0,
            handling_state: HandlingState::Grip,
        }
    }
}

#[derive(Clone, Copy)]
pub struct MotionBasis {
    pub forward: Vec3,
    pub right: Vec3,
    pub forward_speed: f32,
    pub lateral_speed: f32,
}

impl MotionBasis {
    pub fn from_yaw(yaw: f32, velocity: Vec3) -> Self {
        let forward = forward_3d(yaw);
        let right = right_3d(yaw);

        Self {
            forward,
            right,
            forward_speed: velocity.dot(forward),
            lateral_speed: velocity.dot(right),
        }
    }

    pub fn slip_angle(self) -> f32 {
        self.lateral_speed
            .abs()
            .atan2(self.forward_speed.abs().max(0.001))
    }
}

pub fn drive_mode(throttle: f32, forward_speed: f32) -> DriveMode {
    if throttle < 0.0 && forward_speed > 1.0 {
        DriveMode::Braking
    } else if forward_speed < -0.4 || throttle < 0.0 {
        DriveMode::Reverse
    } else {
        DriveMode::Forward
    }
}

pub fn tire_forces(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
    intent: ControlIntent,
    basis: &MotionBasis,
    previous_handling_state: HandlingState,
    contact_friction: SurfaceFriction,
) -> TireForces {
    let wheel_loads = wheel_loads(tuning, basis);
    let normal_load = wheel_loads.iter().sum::<f32>();
    let front_normal_load = wheel_loads[FRONT_LEFT_WHEEL] + wheel_loads[FRONT_RIGHT_WHEEL];
    let rear_normal_load = wheel_loads[REAR_LEFT_WHEEL] + wheel_loads[REAR_RIGHT_WHEEL];
    let wheel_longitudinal_limits = wheel_limits(wheel_loads, contact_friction, |friction| {
        friction.longitudinal
    });
    let wheel_lateral_limits =
        wheel_limits(wheel_loads, contact_friction, |friction| friction.lateral);
    let longitudinal_limit = wheel_longitudinal_limits.iter().sum::<f32>();
    let front_lateral_limit =
        wheel_lateral_limits[FRONT_LEFT_WHEEL] + wheel_lateral_limits[FRONT_RIGHT_WHEEL];
    let rear_lateral_limit =
        wheel_lateral_limits[REAR_LEFT_WHEEL] + wheel_lateral_limits[REAR_RIGHT_WHEEL];
    let requested_longitudinal = requested_longitudinal_force(tuning, intent, basis);
    let longitudinal_force = requested_longitudinal.clamp(-longitudinal_limit, longitudinal_limit);
    let requested_lateral = requested_lateral_force(tuning, basis);
    let front_share = front_lateral_demand_share(intent);
    let front_request = requested_lateral * front_share;
    let rear_request = requested_lateral - front_request;
    let wheel_lateral_requests = split_lateral_requests(front_request, rear_request);
    let wheel_lateral_forces = std::array::from_fn(|index| {
        wheel_lateral_requests[index]
            .clamp(-wheel_lateral_limits[index], wheel_lateral_limits[index])
    });
    let front_lateral_force =
        wheel_lateral_forces[FRONT_LEFT_WHEEL] + wheel_lateral_forces[FRONT_RIGHT_WHEEL];
    let rear_lateral_force =
        wheel_lateral_forces[REAR_LEFT_WHEEL] + wheel_lateral_forces[REAR_RIGHT_WHEEL];
    let lateral_force = front_lateral_force + rear_lateral_force;
    let rolling_force = rolling_resistance_force(tuning, surface, basis.forward_speed);
    let drag_force = aerodynamic_drag_force(surface, basis.forward_speed);
    let boost_force = tuning.mass * surface.boost_acceleration;
    let acceleration = basis.forward
        * ((longitudinal_force + boost_force - rolling_force - drag_force) / tuning.mass)
        - basis.right * (lateral_force / tuning.mass);
    let saturation = friction_saturation(
        requested_longitudinal,
        longitudinal_limit,
        requested_lateral,
        front_lateral_limit + rear_lateral_limit,
    );
    let front_saturation = force_ratio(front_request, front_lateral_limit);
    let rear_saturation = force_ratio(rear_request, rear_lateral_limit);
    let wheel_saturations = std::array::from_fn(|index| {
        force_ratio(wheel_lateral_requests[index], wheel_lateral_limits[index])
    });
    let handling_state =
        saturated_handling_state(tuning, basis, previous_handling_state, saturation);
    let speed_ratio = (basis.forward_speed.abs() / tuning.max_forward_speed).clamp(0.0, 1.0);
    let steer_authority =
        tuning.min_steer_authority + speed_ratio * (1.0 - tuning.min_steer_authority);
    let steering_yaw = -intent.input.steer
        * intent.steering_motion_direction
        * tuning.steer_rate
        * steer_authority
        * intent.mode_steering_multiplier;
    let slip_yaw = if handling_state == HandlingState::Sliding && basis.forward_speed > 1.0 {
        let rear_bias = (rear_saturation - front_saturation).clamp(-1.0, 1.0);
        -basis.lateral_speed.signum()
            * (saturation + rear_bias.abs() * 0.25)
            * tuning.drift_yaw_response
            * basis.forward_speed.abs()
    } else {
        0.0
    };

    TireForces {
        acceleration,
        yaw_delta: steering_yaw + slip_yaw,
        normal_load,
        wheel_normal_loads: wheel_loads,
        front_normal_load,
        rear_normal_load,
        friction_limit: longitudinal_limit.min(front_lateral_limit + rear_lateral_limit),
        longitudinal_force,
        lateral_force,
        wheel_lateral_forces,
        front_lateral_force,
        rear_lateral_force,
        saturation,
        wheel_saturations,
        front_saturation,
        rear_saturation,
        handling_state,
    }
}

fn wheel_loads(tuning: &DrivingTuning, basis: &MotionBasis) -> [f32; WHEEL_COUNT] {
    let static_load = tuning.mass * tuning.gravity;
    let static_front = static_load * tuning.front_weight_bias;
    let static_rear = static_load - static_front;
    let longitudinal_transfer =
        tuning.mass * basis.forward_speed.abs() * tuning.center_of_gravity_height
            / tuning.wheelbase.max(0.001)
            * 0.02;
    let lateral_transfer =
        tuning.mass * basis.lateral_speed.abs() * tuning.center_of_gravity_height
            / tuning.track_width.max(0.001)
            * 0.02;
    let front = (static_front - longitudinal_transfer).max(0.0);
    let rear = (static_rear + longitudinal_transfer).max(0.0);
    let left_transfer = if basis.lateral_speed < 0.0 {
        lateral_transfer
    } else {
        -lateral_transfer
    };
    let right_transfer = -left_transfer;

    [
        (front * 0.5 + left_transfer * 0.5).max(0.0),
        (front * 0.5 + right_transfer * 0.5).max(0.0),
        (rear * 0.5 + left_transfer * 0.5).max(0.0),
        (rear * 0.5 + right_transfer * 0.5).max(0.0),
    ]
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceFriction {
    pub wheels: [WheelFriction; WHEEL_COUNT],
}

impl SurfaceFriction {
    #[cfg(test)]
    fn uniform(longitudinal: f32, lateral: f32) -> Self {
        Self {
            wheels: [WheelFriction {
                longitudinal,
                lateral,
            }; WHEEL_COUNT],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WheelFriction {
    pub longitudinal: f32,
    pub lateral: f32,
}

fn requested_longitudinal_force(
    tuning: &DrivingTuning,
    intent: ControlIntent,
    basis: &MotionBasis,
) -> f32 {
    let input = intent.input.throttle;
    if input >= 0.0 {
        input * tuning.engine_force
    } else if basis.forward_speed > 1.0 {
        input * tuning.brake_force
    } else {
        input * tuning.reverse_force
    }
}

fn requested_lateral_force(tuning: &DrivingTuning, basis: &MotionBasis) -> f32 {
    basis.lateral_speed * tuning.mass * tuning.lateral_stiffness
}

fn front_lateral_demand_share(intent: ControlIntent) -> f32 {
    (0.50 + intent.input.steer.abs() * 0.16).clamp(0.46, 0.68)
}

fn split_lateral_requests(front_request: f32, rear_request: f32) -> [f32; WHEEL_COUNT] {
    [
        front_request * 0.5,
        front_request * 0.5,
        rear_request * 0.5,
        rear_request * 0.5,
    ]
}

fn wheel_limits(
    loads: [f32; WHEEL_COUNT],
    friction: SurfaceFriction,
    component: impl Fn(WheelFriction) -> f32,
) -> [f32; WHEEL_COUNT] {
    std::array::from_fn(|index| loads[index] * component(friction.wheels[index]))
}

fn rolling_resistance_force(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
    forward_speed: f32,
) -> f32 {
    forward_speed.signum() * tuning.mass * tuning.gravity * surface.rolling_resistance
}

fn aerodynamic_drag_force(surface: &SurfaceParams, forward_speed: f32) -> f32 {
    forward_speed * forward_speed.abs() * surface.aerodynamic_drag
}

fn friction_saturation(
    longitudinal_force: f32,
    longitudinal_limit: f32,
    lateral_force: f32,
    lateral_limit: f32,
) -> f32 {
    let longitudinal = longitudinal_force.abs() / longitudinal_limit.max(0.001);
    let lateral = lateral_force.abs() / lateral_limit.max(0.001);
    Vec2::new(longitudinal, lateral).length().clamp(0.0, 2.0)
}

fn force_ratio(force: f32, limit: f32) -> f32 {
    (force.abs() / limit.max(0.001)).clamp(0.0, 2.0)
}

fn saturated_handling_state(
    tuning: &DrivingTuning,
    basis: &MotionBasis,
    previous: HandlingState,
    saturation: f32,
) -> HandlingState {
    let speed = Vec2::new(basis.forward_speed, basis.lateral_speed).length();
    let slip_trigger = basis.slip_angle() >= tuning.slide_slip_angle_threshold;
    let breakaway = saturation >= 1.18 || slip_trigger;
    let recovery =
        saturation < 0.92 && basis.slip_angle() < tuning.slide_slip_angle_threshold * 0.8;

    match previous {
        HandlingState::Grip if speed >= tuning.slide_speed_threshold && breakaway => {
            HandlingState::Sliding
        }
        HandlingState::Sliding if recovery => HandlingState::Grip,
        state => state,
    }
}

fn axis(positive: bool, negative: bool) -> f32 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}

fn steering_motion_direction(input: ControlInput, basis: &MotionBasis) -> f32 {
    if basis.forward_speed.abs() > MIN_STEERING_SPEED {
        signed_axis(basis.forward_speed)
    } else {
        signed_axis(input.throttle)
    }
}

fn signed_axis(value: f32) -> f32 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::{SurfaceKind, SurfaceLibrary};

    fn surface_friction(surface: SurfaceParams) -> SurfaceFriction {
        SurfaceFriction::uniform(surface.longitudinal_friction, surface.lateral_friction)
    }

    #[test]
    fn keyboard_steer_axis_maps_a_left_and_d_right() {
        let mut keys = ButtonInput::<KeyCode>::default();

        keys.press(KeyCode::KeyD);
        assert_eq!(ControlInput::from_keys(&keys).steer, 1.0);

        keys.release(KeyCode::KeyD);
        keys.press(KeyCode::KeyA);
        assert_eq!(ControlInput::from_keys(&keys).steer, -1.0);
    }

    #[test]
    fn steering_requires_motion_or_drive_intent() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::ZERO);
        let intent = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 0.0,
                steer: -1.0,
            },
            &basis,
            0.42,
        );

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(1.0, 1.0),
        );
        assert_eq!(forces.yaw_delta, 0.0);
    }

    #[test]
    fn forward_steering_matches_input_direction() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 10.0);

        let right = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 1.0,
                steer: 1.0,
            },
            &basis,
            0.42,
        );
        let left = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 1.0,
                steer: -1.0,
            },
            &basis,
            0.42,
        );

        let friction = SurfaceFriction::uniform(1.0, 1.0);
        let right_forces = tire_forces(
            &tuning,
            &surface,
            right,
            &basis,
            HandlingState::Grip,
            friction,
        );
        let left_forces = tire_forces(
            &tuning,
            &surface,
            left,
            &basis,
            HandlingState::Grip,
            friction,
        );

        assert!(right_forces.yaw_delta < 0.0);
        assert!(left_forces.yaw_delta > 0.0);
        assert!(right.wheel_steer_angle > 0.0);
        assert!(left.wheel_steer_angle < 0.0);
    }

    #[test]
    fn reverse_steering_uses_reverse_motion_direction() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let forward_basis = MotionBasis::from_yaw(0.0, Vec3::Z * 10.0);
        let reverse_basis = MotionBasis::from_yaw(0.0, -Vec3::Z * 10.0);
        let input = ControlInput {
            throttle: -1.0,
            steer: 1.0,
        };
        let forward = ControlIntent::from_input(&tuning, input, &forward_basis, 0.42);
        let reverse = ControlIntent::from_input(&tuning, input, &reverse_basis, 0.42);
        let friction = SurfaceFriction::uniform(1.0, 1.0);
        let forward_yaw = tire_forces(
            &tuning,
            &surface,
            forward,
            &forward_basis,
            HandlingState::Grip,
            friction,
        )
        .yaw_delta;
        let reverse_yaw = tire_forces(
            &tuning,
            &surface,
            reverse,
            &reverse_basis,
            HandlingState::Grip,
            friction,
        )
        .yaw_delta;

        assert!(forward_yaw < 0.0);
        assert!(reverse_yaw > 0.0);
        assert!(reverse_yaw.abs() < forward_yaw.abs());
    }

    #[test]
    fn surface_friction_limits_lateral_force() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let asphalt = surfaces.get(SurfaceKind::Asphalt);
        let ice = surfaces.get(SurfaceKind::Ice);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(20.0, 0.0, 20.0));
        let intent = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 0.0,
                steer: 0.0,
            },
            &basis,
            0.42,
        );
        let asphalt_force = tire_forces(
            &tuning,
            &asphalt,
            intent,
            &basis,
            HandlingState::Grip,
            surface_friction(asphalt),
        );
        let ice_force = tire_forces(
            &tuning,
            &ice,
            intent,
            &basis,
            HandlingState::Grip,
            surface_friction(ice),
        );

        assert!(asphalt_force.lateral_force.abs() > ice_force.lateral_force.abs());
        assert!(ice_force.saturation >= asphalt_force.saturation);
    }

    #[test]
    fn rolling_resistance_slows_coasting() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Grass);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 10.0);
        let intent = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 0.0,
                steer: 0.0,
            },
            &basis,
            0.42,
        );

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(1.0, 1.0),
        );

        assert!(forces.acceleration.dot(basis.forward) < 0.0);
    }

    #[test]
    fn axle_forces_report_split_loads_and_saturation() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(14.0, 0.0, 22.0));
        let intent = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 0.0,
                steer: 0.0,
            },
            &basis,
            0.42,
        );

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(1.0, 1.0),
        );

        assert!(
            (forces.front_normal_load + forces.rear_normal_load - forces.normal_load).abs() < 0.01
        );
        assert!(forces.front_lateral_force.abs() > 0.0);
        assert!(forces.rear_lateral_force.abs() > 0.0);
        assert!(forces.front_saturation > 0.0);
        assert!(forces.rear_saturation > 0.0);
    }

    #[test]
    fn rear_surface_loss_increases_rear_saturation() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(18.0, 0.0, 22.0));
        let intent = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 0.0,
                steer: 0.0,
            },
            &basis,
            0.42,
        );

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction {
                wheels: [
                    WheelFriction {
                        longitudinal: 1.0,
                        lateral: 1.0,
                    },
                    WheelFriction {
                        longitudinal: 1.0,
                        lateral: 1.0,
                    },
                    WheelFriction {
                        longitudinal: 1.0,
                        lateral: 0.25,
                    },
                    WheelFriction {
                        longitudinal: 1.0,
                        lateral: 0.25,
                    },
                ],
            },
        );

        assert!(forces.rear_saturation > forces.front_saturation);
    }

    #[test]
    fn steering_angle_shifts_lateral_demand_to_front_wheels() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(10.0, 0.0, 20.0));
        let neutral = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 0.0,
                steer: 0.0,
            },
            &basis,
            0.42,
        );
        let steering = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 0.0,
                steer: 1.0,
            },
            &basis,
            0.42,
        );
        let friction = SurfaceFriction::uniform(1.0, 1.0);

        let neutral_forces = tire_forces(
            &tuning,
            &surface,
            neutral,
            &basis,
            HandlingState::Grip,
            friction,
        );
        let steering_forces = tire_forces(
            &tuning,
            &surface,
            steering,
            &basis,
            HandlingState::Grip,
            friction,
        );

        assert!(
            steering_forces.front_lateral_force.abs() > neutral_forces.front_lateral_force.abs()
        );
        assert!(steering_forces.rear_lateral_force.abs() < neutral_forces.rear_lateral_force.abs());
    }

    #[test]
    fn saturation_breakaway_and_recovery_have_hysteresis() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Ice);
        let sliding_basis = MotionBasis::from_yaw(0.0, Vec3::new(28.0, 0.0, 16.0));
        let stable_basis = MotionBasis::from_yaw(0.0, Vec3::Z * 8.0);
        let intent = ControlIntent::from_input(
            &tuning,
            ControlInput {
                throttle: 0.0,
                steer: 0.0,
            },
            &sliding_basis,
            0.42,
        );
        let low_friction = SurfaceFriction::uniform(1.0, 0.35);

        let breakaway = tire_forces(
            &tuning,
            &surface,
            intent,
            &sliding_basis,
            HandlingState::Grip,
            low_friction,
        );
        let recovery = tire_forces(
            &tuning,
            &surface,
            intent,
            &stable_basis,
            HandlingState::Sliding,
            SurfaceFriction::uniform(1.0, 1.0),
        );

        assert_eq!(breakaway.handling_state, HandlingState::Sliding);
        assert_eq!(recovery.handling_state, HandlingState::Grip);
    }
}
