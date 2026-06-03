use bevy::prelude::*;

use crate::geometry::{forward_3d, right_3d};
use crate::surface::SurfaceParams;

pub const WHEEL_COUNT: usize = 4;
pub const FRONT_LEFT_WHEEL: usize = 0;
pub const FRONT_RIGHT_WHEEL: usize = 1;
pub const REAR_LEFT_WHEEL: usize = 2;
pub const REAR_RIGHT_WHEEL: usize = 3;
pub const REVERSE_ENTRY_SPEED: f32 = 0.15;
const REVERSE_ROLLING_SPEED: f32 = 0.4;

#[derive(Resource)]
pub struct DrivingTuning {
    pub mass: f32,
    pub gravity: f32,
    pub engine_force: f32,
    pub brake_force: f32,
    pub rear_brake_force: f32,
    pub reverse_force: f32,
    pub engine_speed_gain: f32,
    pub brake_speed_gain: f32,
    pub reverse_speed_gain: f32,
    pub drive_front_bias: f32,
    pub wheel_radius: f32,
    pub wheel_spin_response: f32,
    pub wheel_free_roll_response: f32,
    pub suspension_load_travel: f32,
    pub suspension_response: f32,
    pub suspension_visual_travel: f32,
    pub max_steer_angle: f32,
    pub wheel_steer_response: f32,
    pub high_speed_steer_fade: f32,
    pub yaw_rate_response: f32,
    pub yaw_rate_damping: f32,
    pub max_yaw_rate: f32,
    pub max_forward_speed: f32,
    pub max_reverse_speed: f32,
    pub reverse_steering_multiplier: f32,
    pub wheelbase: f32,
    pub track_width: f32,
    pub center_of_gravity_height: f32,
    pub front_weight_bias: f32,
    pub lateral_stiffness: f32,
    pub straight_line_settling: f32,
    pub passive_slide_yaw_response: f32,
    pub rear_brake_yaw_assist: f32,
    pub rear_brake_grip_loss: f32,
    pub drift_min_speed: f32,
    pub drift_min_steer: f32,
    pub drift_min_slip_angle: f32,
    pub slide_saturation_threshold: f32,
    pub slide_speed_threshold: f32,
    pub slide_slip_angle_threshold: f32,
}

const MIN_STEERING_SPEED: f32 = 0.5;

impl Default for DrivingTuning {
    fn default() -> Self {
        Self {
            mass: 1_180.0,
            gravity: 9.81,
            engine_force: 13_200.0,
            brake_force: 18_500.0,
            rear_brake_force: 8_500.0,
            reverse_force: 6_500.0,
            engine_speed_gain: 1_650.0,
            brake_speed_gain: 3_200.0,
            reverse_speed_gain: 2_100.0,
            drive_front_bias: 1.0,
            wheel_radius: 0.34,
            wheel_spin_response: 14.0,
            wheel_free_roll_response: 20.0,
            suspension_load_travel: 0.24,
            suspension_response: 12.0,
            suspension_visual_travel: 0.10,
            max_steer_angle: 0.30,
            wheel_steer_response: 16.0,
            high_speed_steer_fade: 0.035,
            yaw_rate_response: 9.0,
            yaw_rate_damping: 7.0,
            max_yaw_rate: 2.4,
            max_forward_speed: 58.0,
            max_reverse_speed: 14.0,
            reverse_steering_multiplier: 0.45,
            wheelbase: 3.44,
            track_width: 1.64,
            center_of_gravity_height: 0.42,
            front_weight_bias: 0.54,
            lateral_stiffness: 1.10,
            straight_line_settling: 0.45,
            passive_slide_yaw_response: 0.003,
            rear_brake_yaw_assist: 0.006,
            rear_brake_grip_loss: 0.22,
            drift_min_speed: 12.0,
            drift_min_steer: 0.35,
            drift_min_slip_angle: 0.24,
            slide_saturation_threshold: 1.95,
            slide_speed_threshold: 16.0,
            slide_slip_angle_threshold: 0.82,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriftAssistState {
    Inactive,
    RearBrake,
}

impl DriftAssistState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::RearBrake => "rear-brake",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlideReason {
    None,
    PassiveSlip,
    SurfaceSlip,
    RearBrakeAssist,
}

impl SlideReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PassiveSlip => "passive",
            Self::SurfaceSlip => "surface",
            Self::RearBrakeAssist => "rear-brake",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DriftAssist {
    pub state: DriftAssistState,
    pub rear_brake_lateral_cost: f32,
    pub yaw_assist: f32,
}

impl Default for DriftAssist {
    fn default() -> Self {
        Self {
            state: DriftAssistState::Inactive,
            rear_brake_lateral_cost: 0.0,
            yaw_assist: 0.0,
        }
    }
}

impl DriftAssist {
    pub fn from_input(
        tuning: &DrivingTuning,
        surface: &SurfaceParams,
        input: ControlInput,
        basis: &MotionBasis,
    ) -> Self {
        let rear_brake = input.rear_brake.clamp(0.0, 1.0);
        let speed = Vec2::new(basis.forward_speed, basis.lateral_speed).length();
        let slip_angle = basis.slip_angle();
        let speed_gate = threshold_gate(speed, tuning.drift_min_speed, tuning.drift_min_speed);
        let steer_gate = threshold_gate(
            input.steer.abs(),
            tuning.drift_min_steer,
            1.0 - tuning.drift_min_steer,
        );
        let slip_gate = threshold_gate(
            slip_angle,
            tuning.drift_min_slip_angle,
            tuning.drift_min_slip_angle,
        );
        let rotation_gate = steer_gate.max(slip_gate);
        let active_amount = rear_brake * speed_gate;
        let yaw_amount = rear_brake * speed_gate * rotation_gate;
        let state = if active_amount > 0.001 {
            DriftAssistState::RearBrake
        } else {
            DriftAssistState::Inactive
        };
        let rear_brake_lateral_cost = if state == DriftAssistState::RearBrake {
            let rotation_cost_gate = 0.30 + rotation_gate * 0.70;
            (0.34 + tuning.rear_brake_grip_loss * surface.rear_brake_grip_loss_scale)
                .clamp(0.20, 0.75)
                * speed_gate
                * rotation_cost_gate
        } else {
            0.0
        };
        let yaw_assist = rear_brake_yaw_assist(tuning, surface, input, basis, yaw_amount);

        Self {
            state,
            rear_brake_lateral_cost,
            yaw_assist,
        }
    }

    pub fn is_active(self) -> bool {
        self.state != DriftAssistState::Inactive
    }
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
    pub rear_brake: f32,
}

impl Default for ControlInput {
    fn default() -> Self {
        Self {
            throttle: 0.0,
            steer: 0.0,
            rear_brake: 0.0,
        }
    }
}

impl ControlInput {
    pub fn from_keys(keys: &ButtonInput<KeyCode>) -> Self {
        let rear_brake_keys = [KeyCode::Space, KeyCode::ShiftLeft, KeyCode::ShiftRight];
        Self {
            throttle: axis(
                keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]),
                keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]),
            ),
            steer: axis(
                keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]),
                keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]),
            ),
            rear_brake: if keys.any_pressed(rear_brake_keys) {
                1.0
            } else {
                0.0
            },
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
    pub fn from_input(tuning: &DrivingTuning, input: ControlInput, basis: &MotionBasis) -> Self {
        let drive_mode = drive_mode(input.throttle, basis.forward_speed);
        let mode_steering_multiplier = match drive_mode {
            DriveMode::Forward | DriveMode::Braking => 1.0,
            DriveMode::Reverse => tuning.reverse_steering_multiplier,
        };

        Self {
            input,
            drive_mode,
            wheel_steer_angle: input.steer * tuning.max_steer_angle,
            steering_motion_direction: steering_motion_direction(input, basis),
            mode_steering_multiplier,
        }
    }

    pub fn with_wheel_steer_angle(mut self, wheel_steer_angle: f32) -> Self {
        self.wheel_steer_angle = wheel_steer_angle;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TireForces {
    pub acceleration: Vec3,
    pub target_yaw_rate: f32,
    pub normal_load: f32,
    pub wheel_normal_loads: [f32; WHEEL_COUNT],
    pub front_normal_load: f32,
    pub rear_normal_load: f32,
    pub friction_limit: f32,
    pub target_speed: f32,
    pub longitudinal_force: f32,
    pub front_longitudinal_force: f32,
    pub rear_longitudinal_force: f32,
    pub wheel_longitudinal_forces: [f32; WHEEL_COUNT],
    pub lateral_force: f32,
    pub wheel_lateral_limits: [f32; WHEEL_COUNT],
    pub front_lateral_limit: f32,
    pub rear_lateral_limit: f32,
    pub wheel_lateral_forces: [f32; WHEEL_COUNT],
    pub front_lateral_force: f32,
    pub rear_lateral_force: f32,
    pub saturation: f32,
    pub wheel_saturations: [f32; WHEEL_COUNT],
    pub front_saturation: f32,
    pub rear_saturation: f32,
    pub handling_state: HandlingState,
    pub slide_reason: SlideReason,
    pub rear_brake_lateral_cost: f32,
    pub yaw_assist: f32,
}

pub struct TireForceInput<'a> {
    pub tuning: &'a DrivingTuning,
    pub surface: &'a SurfaceParams,
    pub intent: ControlIntent,
    pub basis: &'a MotionBasis,
    pub previous_handling_state: HandlingState,
    pub contact_friction: SurfaceFriction,
    pub boost_direction: Option<Vec3>,
    pub drift_assist: DriftAssist,
    pub gravity_acceleration: Vec3,
    pub normal_load_scale: f32,
}

impl Default for TireForces {
    fn default() -> Self {
        Self {
            acceleration: Vec3::ZERO,
            target_yaw_rate: 0.0,
            normal_load: 0.0,
            wheel_normal_loads: [0.0; WHEEL_COUNT],
            front_normal_load: 0.0,
            rear_normal_load: 0.0,
            friction_limit: 0.0,
            target_speed: 0.0,
            longitudinal_force: 0.0,
            front_longitudinal_force: 0.0,
            rear_longitudinal_force: 0.0,
            wheel_longitudinal_forces: [0.0; WHEEL_COUNT],
            lateral_force: 0.0,
            wheel_lateral_limits: [0.0; WHEEL_COUNT],
            front_lateral_limit: 0.0,
            rear_lateral_limit: 0.0,
            wheel_lateral_forces: [0.0; WHEEL_COUNT],
            front_lateral_force: 0.0,
            rear_lateral_force: 0.0,
            saturation: 0.0,
            wheel_saturations: [0.0; WHEEL_COUNT],
            front_saturation: 0.0,
            rear_saturation: 0.0,
            handling_state: HandlingState::Grip,
            slide_reason: SlideReason::None,
            rear_brake_lateral_cost: 0.0,
            yaw_assist: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WheelTelemetry {
    pub angular_speed: f32,
    pub target_angular_speed: f32,
    pub slip_ratio: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VehicleFeedback {
    pub motor_pitch: f32,
    pub motor_load: f32,
    pub wheel_speed_rpm: f32,
    pub target_wheel_speed_rpm: f32,
    pub slip_intensity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct WheelSuspension {
    pub compression: f32,
    pub visual_offset: f32,
}

impl Default for WheelSuspension {
    fn default() -> Self {
        Self {
            compression: 0.5,
            visual_offset: 0.0,
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
    #[cfg(test)]
    pub fn from_yaw(yaw: f32, velocity: Vec3) -> Self {
        Self::from_axes(forward_3d(yaw), right_3d(yaw), velocity)
    }

    pub fn from_ground(yaw: f32, normal: Vec3, velocity: Vec3) -> Self {
        let normal = normal.normalize_or(Vec3::Y);
        let flat_forward = forward_3d(yaw);
        let forward = (flat_forward - normal * flat_forward.dot(normal)).normalize_or(flat_forward);
        let right = normal.cross(forward).normalize_or(right_3d(yaw));

        Self::from_axes(forward, right, velocity)
    }

    fn from_axes(forward: Vec3, right: Vec3, velocity: Vec3) -> Self {
        let forward = forward.normalize_or(Vec3::Z);
        let right = right.normalize_or(Vec3::X);

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

pub fn gravity_acceleration(tuning: &DrivingTuning, normal: Vec3) -> Vec3 {
    let normal = normal.normalize_or(Vec3::Y);
    let gravity = Vec3::NEG_Y * tuning.gravity;
    gravity - normal * gravity.dot(normal)
}

pub fn normal_load_scale(tuning: &DrivingTuning, normal: Vec3) -> f32 {
    let normal = normal.normalize_or(Vec3::Y);
    let gravity = Vec3::NEG_Y * tuning.gravity;
    (-gravity.dot(normal) / tuning.gravity.max(0.001)).clamp(0.0, 1.25)
}

pub fn virtual_suspension(
    tuning: &DrivingTuning,
    previous: [WheelSuspension; WHEEL_COUNT],
    tire_forces: &TireForces,
    dt: f32,
) -> [WheelSuspension; WHEEL_COUNT] {
    let resting_loads = resting_wheel_loads(tuning);
    let blend = 1.0 - (-tuning.suspension_response * dt.max(0.0)).exp();

    std::array::from_fn(|index| {
        let load_ratio = tire_forces.wheel_normal_loads[index] / resting_loads[index].max(0.001);
        let target_compression =
            (0.5 + (load_ratio - 1.0) * tuning.suspension_load_travel).clamp(0.0, 1.0);
        let compression = previous[index].compression.lerp(target_compression, blend);
        let visual_offset = (0.5 - compression) * tuning.suspension_visual_travel;

        WheelSuspension {
            compression,
            visual_offset,
        }
    })
}

fn resting_wheel_loads(tuning: &DrivingTuning) -> [f32; WHEEL_COUNT] {
    let static_load = tuning.mass * tuning.gravity;
    let front = static_load * tuning.front_weight_bias * 0.5;
    let rear = static_load * (1.0 - tuning.front_weight_bias) * 0.5;

    [front, front, rear, rear]
}

pub fn resolved_wheel_steer_angle(
    tuning: &DrivingTuning,
    current_angle: f32,
    target_angle: f32,
    dt: f32,
) -> f32 {
    let blend = 1.0 - (-tuning.wheel_steer_response * dt.max(0.0)).exp();

    current_angle
        .lerp(target_angle, blend)
        .clamp(-tuning.max_steer_angle, tuning.max_steer_angle)
}

pub fn wheel_telemetry(
    tuning: &DrivingTuning,
    previous: [WheelTelemetry; WHEEL_COUNT],
    tire_forces: &TireForces,
    basis: &MotionBasis,
    dt: f32,
) -> [WheelTelemetry; WHEEL_COUNT] {
    let radius = tuning.wheel_radius.max(0.001);
    let rolling_angular_speed = basis.forward_speed / radius;

    std::array::from_fn(|index| {
        let wheel_force = tire_forces.wheel_longitudinal_forces[index];
        let target_linear_speed =
            wheel_target_linear_speed(wheel_force, tire_forces.target_speed, basis.forward_speed);
        let target_angular_speed = target_linear_speed / radius;
        let response = if wheel_force.abs() > 1.0 {
            tuning.wheel_spin_response
        } else {
            tuning.wheel_free_roll_response
        };
        let blend = 1.0 - (-response * dt.max(0.0)).exp();
        let angular_speed = previous[index]
            .angular_speed
            .lerp(target_angular_speed, blend);
        let wheel_linear_speed = angular_speed * radius;
        let slip_ratio = ((wheel_linear_speed - basis.forward_speed)
            / basis.forward_speed.abs().max(1.0))
        .clamp(-3.0, 3.0);

        WheelTelemetry {
            angular_speed: if wheel_force.abs() <= 1.0 {
                angular_speed.lerp(rolling_angular_speed, blend)
            } else {
                angular_speed
            },
            target_angular_speed,
            slip_ratio,
        }
    })
}

pub fn vehicle_feedback(
    tuning: &DrivingTuning,
    wheel_telemetry: &[WheelTelemetry; WHEEL_COUNT],
    tire_forces: &TireForces,
) -> VehicleFeedback {
    let front_actual_speed = axle_average_abs(wheel_telemetry, |wheel| wheel.angular_speed);
    let front_target_speed = axle_average_abs(wheel_telemetry, |wheel| wheel.target_angular_speed);
    let wheel_speed_rpm = radians_per_second_to_rpm(front_actual_speed);
    let target_wheel_speed_rpm = radians_per_second_to_rpm(front_target_speed);
    let motor_speed = front_actual_speed.max(front_target_speed);
    let motor_pitch = (0.65 + motor_speed * tuning.wheel_radius / tuning.max_forward_speed * 1.35)
        .clamp(0.55, 2.25);
    let motor_load = force_ratio(tire_forces.front_longitudinal_force, tuning.engine_force);
    let slip_intensity = wheel_telemetry
        .iter()
        .map(|wheel| (wheel.slip_ratio.abs() - 0.12).max(0.0) / 1.25)
        .fold(0.0, f32::max)
        .max((tire_forces.saturation - 1.0).max(0.0))
        .clamp(0.0, 1.0);

    VehicleFeedback {
        motor_pitch,
        motor_load,
        wheel_speed_rpm,
        target_wheel_speed_rpm,
        slip_intensity,
    }
}

fn axle_average_abs(
    wheel_telemetry: &[WheelTelemetry; WHEEL_COUNT],
    value: impl Fn(WheelTelemetry) -> f32,
) -> f32 {
    (value(wheel_telemetry[FRONT_LEFT_WHEEL]).abs()
        + value(wheel_telemetry[FRONT_RIGHT_WHEEL]).abs())
        * 0.5
}

fn radians_per_second_to_rpm(radians_per_second: f32) -> f32 {
    radians_per_second * 60.0 / std::f32::consts::TAU
}

fn wheel_target_linear_speed(wheel_force: f32, target_speed: f32, forward_speed: f32) -> f32 {
    if wheel_force.abs() <= 1.0 {
        forward_speed
    } else if target_speed.abs() > 0.01 {
        target_speed
    } else if forward_speed.abs() > 0.05 && wheel_force.signum() != forward_speed.signum() {
        0.0
    } else {
        forward_speed
    }
}

pub fn drive_mode(throttle: f32, forward_speed: f32) -> DriveMode {
    if forward_speed < -REVERSE_ROLLING_SPEED {
        DriveMode::Reverse
    } else if throttle < 0.0 && forward_speed > REVERSE_ENTRY_SPEED {
        DriveMode::Braking
    } else if throttle < 0.0 {
        DriveMode::Reverse
    } else {
        DriveMode::Forward
    }
}

pub fn resolved_yaw_rate(
    tuning: &DrivingTuning,
    current_yaw_rate: f32,
    target_yaw_rate: f32,
    dt: f32,
) -> f32 {
    let response = if target_yaw_rate.abs() > current_yaw_rate.abs() {
        tuning.yaw_rate_response
    } else {
        tuning.yaw_rate_damping
    };
    let blend = 1.0 - (-response * dt.max(0.0)).exp();

    current_yaw_rate
        .lerp(target_yaw_rate, blend)
        .clamp(-tuning.max_yaw_rate, tuning.max_yaw_rate)
}

pub fn tire_forces(input: TireForceInput<'_>) -> TireForces {
    let TireForceInput {
        tuning,
        surface,
        intent,
        basis,
        previous_handling_state,
        contact_friction,
        boost_direction,
        drift_assist,
        gravity_acceleration,
        normal_load_scale,
    } = input;

    let wheel_loads = wheel_loads(tuning, basis, normal_load_scale);
    let normal_load = wheel_loads.iter().sum::<f32>();
    let front_normal_load = wheel_loads[FRONT_LEFT_WHEEL] + wheel_loads[FRONT_RIGHT_WHEEL];
    let rear_normal_load = wheel_loads[REAR_LEFT_WHEEL] + wheel_loads[REAR_RIGHT_WHEEL];
    let wheel_longitudinal_limits = wheel_limits(wheel_loads, contact_friction, |friction| {
        friction.longitudinal
    });
    let base_wheel_lateral_limits =
        wheel_limits(wheel_loads, contact_friction, |friction| friction.lateral);
    let longitudinal_demand = longitudinal_demand(tuning, intent, basis, drift_assist);
    let wheel_longitudinal_forces =
        clamp_wheel_longitudinal(longitudinal_demand.wheel_forces, wheel_longitudinal_limits);
    let wheel_lateral_limits = combined_lateral_limits(
        base_wheel_lateral_limits,
        wheel_longitudinal_limits,
        wheel_longitudinal_forces,
        &longitudinal_demand,
    );
    let longitudinal_limit = wheel_longitudinal_limits.iter().sum::<f32>();
    let front_lateral_limit =
        wheel_lateral_limits[FRONT_LEFT_WHEEL] + wheel_lateral_limits[FRONT_RIGHT_WHEEL];
    let rear_lateral_limit =
        wheel_lateral_limits[REAR_LEFT_WHEEL] + wheel_lateral_limits[REAR_RIGHT_WHEEL];
    let longitudinal_force = wheel_longitudinal_forces.iter().sum::<f32>();
    let front_longitudinal_force =
        wheel_longitudinal_forces[FRONT_LEFT_WHEEL] + wheel_longitudinal_forces[FRONT_RIGHT_WHEEL];
    let rear_longitudinal_force =
        wheel_longitudinal_forces[REAR_LEFT_WHEEL] + wheel_longitudinal_forces[REAR_RIGHT_WHEEL];
    let requested_lateral = requested_lateral_force(tuning, surface, drift_assist, intent, basis);
    let front_share = front_lateral_demand_share(tuning, intent);
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
    let boost_acceleration = boost_acceleration(surface, boost_direction);
    let acceleration = basis.forward
        * ((longitudinal_force - rolling_force - drag_force) / tuning.mass)
        + boost_acceleration
        + gravity_acceleration
        - basis.right * (lateral_force / tuning.mass);
    let wheel_saturations = std::array::from_fn(|index| {
        wheel_saturation(
            wheel_longitudinal_forces[index],
            wheel_longitudinal_limits[index],
            longitudinal_demand.lateral_cost(index),
            wheel_lateral_requests[index],
            wheel_lateral_limits[index],
        )
    });
    let saturation = wheel_saturations
        .iter()
        .copied()
        .fold(0.0, f32::max)
        .clamp(0.0, 2.0);
    let front_saturation = force_ratio(front_request, front_lateral_limit);
    let rear_saturation = force_ratio(rear_request, rear_lateral_limit);
    let handling_state =
        saturated_handling_state(tuning, surface, basis, previous_handling_state, saturation);
    let slide_reason = slide_reason(surface, drift_assist, handling_state);
    let steering_yaw_rate = steering_target_yaw_rate(tuning, intent, basis, front_saturation);
    let slip_yaw = if handling_state == HandlingState::Sliding && basis.forward_speed > 1.0 {
        let rear_bias = (rear_saturation - front_saturation).clamp(-1.0, 1.0);
        -basis.lateral_speed.signum()
            * (saturation + rear_bias.abs() * 0.25)
            * tuning.passive_slide_yaw_response
            * basis.forward_speed.abs()
    } else {
        0.0
    };

    TireForces {
        acceleration,
        target_yaw_rate: steering_yaw_rate + slip_yaw + drift_assist.yaw_assist,
        normal_load,
        wheel_normal_loads: wheel_loads,
        front_normal_load,
        rear_normal_load,
        friction_limit: longitudinal_limit.min(front_lateral_limit + rear_lateral_limit),
        target_speed: longitudinal_demand.target_speed,
        longitudinal_force,
        front_longitudinal_force,
        rear_longitudinal_force,
        wheel_longitudinal_forces,
        lateral_force,
        wheel_lateral_limits,
        front_lateral_limit,
        rear_lateral_limit,
        wheel_lateral_forces,
        front_lateral_force,
        rear_lateral_force,
        saturation,
        wheel_saturations,
        front_saturation,
        rear_saturation,
        handling_state,
        slide_reason,
        rear_brake_lateral_cost: drift_assist.rear_brake_lateral_cost,
        yaw_assist: drift_assist.yaw_assist,
    }
}

fn wheel_loads(
    tuning: &DrivingTuning,
    basis: &MotionBasis,
    normal_load_scale: f32,
) -> [f32; WHEEL_COUNT] {
    let static_load = tuning.mass * tuning.gravity * normal_load_scale.max(0.0);
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

#[derive(Clone, Copy, Debug)]
struct LongitudinalDemand {
    wheel_forces: [f32; WHEEL_COUNT],
    weighted_lateral_costs: [f32; WHEEL_COUNT],
    target_speed: f32,
}

impl Default for LongitudinalDemand {
    fn default() -> Self {
        Self {
            wheel_forces: [0.0; WHEEL_COUNT],
            weighted_lateral_costs: [0.0; WHEEL_COUNT],
            target_speed: 0.0,
        }
    }
}

impl LongitudinalDemand {
    fn add_wheel_force(&mut self, index: usize, force: f32, lateral_cost: f32) {
        self.wheel_forces[index] += force;
        self.weighted_lateral_costs[index] += force.abs() * lateral_cost.max(0.0);
    }

    fn add_front_rear_split(&mut self, force: f32, front_bias: f32, lateral_cost: f32) {
        let front_force = force * front_bias.clamp(0.0, 1.0);
        let rear_force = force - front_force;
        self.add_wheel_force(FRONT_LEFT_WHEEL, front_force * 0.5, lateral_cost);
        self.add_wheel_force(FRONT_RIGHT_WHEEL, front_force * 0.5, lateral_cost);
        self.add_wheel_force(REAR_LEFT_WHEEL, rear_force * 0.5, lateral_cost);
        self.add_wheel_force(REAR_RIGHT_WHEEL, rear_force * 0.5, lateral_cost);
    }

    fn add_all_wheels(&mut self, force: f32, lateral_cost: f32) {
        for index in 0..WHEEL_COUNT {
            self.add_wheel_force(index, force * 0.25, lateral_cost);
        }
    }

    fn add_rear_wheels(&mut self, force: f32, lateral_cost: f32) {
        self.add_wheel_force(REAR_LEFT_WHEEL, force * 0.5, lateral_cost);
        self.add_wheel_force(REAR_RIGHT_WHEEL, force * 0.5, lateral_cost);
    }

    fn lateral_cost(&self, index: usize) -> f32 {
        let force = self.wheel_forces[index].abs();
        if force <= 0.001 {
            0.0
        } else {
            self.weighted_lateral_costs[index] / force
        }
    }
}

fn longitudinal_demand(
    tuning: &DrivingTuning,
    intent: ControlIntent,
    basis: &MotionBasis,
    drift_assist: DriftAssist,
) -> LongitudinalDemand {
    const DRIVE_LATERAL_COST: f32 = 0.35;
    const SERVICE_BRAKE_LATERAL_COST: f32 = 0.45;

    let mut demand = LongitudinalDemand::default();
    let input = intent.input.throttle;

    if input > 0.0 {
        let target_speed = input * tuning.max_forward_speed;
        let force = ((target_speed - basis.forward_speed) * tuning.engine_speed_gain)
            .clamp(0.0, tuning.engine_force * input);
        demand.target_speed = target_speed;
        demand.add_front_rear_split(force, tuning.drive_front_bias, DRIVE_LATERAL_COST);
    } else {
        match intent.drive_mode {
            DriveMode::Braking => {
                let brake = -input;
                let force = (-basis.forward_speed * tuning.brake_speed_gain * brake)
                    .clamp(-tuning.brake_force * brake, 0.0);
                demand.target_speed = 0.0;
                demand.add_all_wheels(force, SERVICE_BRAKE_LATERAL_COST);
            }
            DriveMode::Reverse => {
                let reverse = -input;
                let target_speed = input * tuning.max_reverse_speed;
                let force = ((target_speed - basis.forward_speed) * tuning.reverse_speed_gain)
                    .clamp(-tuning.reverse_force * reverse, 0.0);
                demand.target_speed = target_speed;
                demand.add_front_rear_split(force, tuning.drive_front_bias, DRIVE_LATERAL_COST);
            }
            DriveMode::Forward => {}
        }
    };

    let rear_brake = intent.input.rear_brake.clamp(0.0, 1.0);
    if rear_brake > 0.0 && basis.forward_speed.abs() > 0.05 {
        let force = (-basis.forward_speed * tuning.brake_speed_gain * rear_brake).clamp(
            -tuning.rear_brake_force * rear_brake,
            tuning.rear_brake_force * rear_brake,
        );
        demand.add_rear_wheels(force, drift_assist.rear_brake_lateral_cost);
    }

    demand
}

fn requested_lateral_force(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
    drift_assist: DriftAssist,
    intent: ControlIntent,
    basis: &MotionBasis,
) -> f32 {
    let compliance = surface.compliance.clamp(0.0, 0.85);
    let compliance_settling = 1.0 - compliance * 0.50;
    let settling = if drift_assist.is_active() || intent.input.rear_brake > 0.0 {
        1.0
    } else {
        1.0 + tuning.straight_line_settling
            * surface.recovery_scale
            * compliance_settling
            * (1.0 - intent.input.steer.abs()).clamp(0.0, 1.0)
    };
    let lateral_stiffness = tuning.lateral_stiffness * (1.0 - compliance * 0.35);

    basis.lateral_speed * tuning.mass * lateral_stiffness * settling
}

fn steering_target_yaw_rate(
    tuning: &DrivingTuning,
    intent: ControlIntent,
    basis: &MotionBasis,
    front_saturation: f32,
) -> f32 {
    let speed = basis.forward_speed.abs();
    if speed <= MIN_STEERING_SPEED || intent.input.steer.abs() <= f32::EPSILON {
        return 0.0;
    }

    let steer_fade = 1.0 / (1.0 + speed * tuning.high_speed_steer_fade);
    let steer_angle = intent.wheel_steer_angle * steer_fade * intent.mode_steering_multiplier;
    let front_grip = steering_front_grip(front_saturation);
    let yaw_rate = speed / tuning.wheelbase.max(0.001) * steer_angle.tan() * front_grip;

    intent.steering_motion_direction * yaw_rate
}

fn steering_front_grip(front_saturation: f32) -> f32 {
    (1.0 - (front_saturation - 1.0).max(0.0) * 0.55).clamp(0.35, 1.0)
}

fn front_lateral_demand_share(tuning: &DrivingTuning, intent: ControlIntent) -> f32 {
    let steering_fraction =
        (intent.wheel_steer_angle.abs() / tuning.max_steer_angle.max(0.001)).clamp(0.0, 1.0);
    (0.50 + steering_fraction * 0.16).clamp(0.46, 0.68)
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

fn clamp_wheel_longitudinal(
    requested: [f32; WHEEL_COUNT],
    limits: [f32; WHEEL_COUNT],
) -> [f32; WHEEL_COUNT] {
    std::array::from_fn(|index| requested[index].clamp(-limits[index], limits[index]))
}

fn combined_lateral_limits(
    base_limits: [f32; WHEEL_COUNT],
    longitudinal_limits: [f32; WHEEL_COUNT],
    longitudinal_forces: [f32; WHEEL_COUNT],
    demand: &LongitudinalDemand,
) -> [f32; WHEEL_COUNT] {
    std::array::from_fn(|index| {
        let longitudinal_usage = longitudinal_forces[index].abs()
            / longitudinal_limits[index].max(0.001)
            * demand.lateral_cost(index);
        let reserve = (1.0 - longitudinal_usage * longitudinal_usage)
            .max(0.0)
            .sqrt();
        base_limits[index] * reserve
    })
}

fn rolling_resistance_force(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
    forward_speed: f32,
) -> f32 {
    if forward_speed.abs() <= 0.01 {
        return 0.0;
    }

    forward_speed.signum() * tuning.mass * tuning.gravity * surface.rolling_resistance
}

fn aerodynamic_drag_force(surface: &SurfaceParams, forward_speed: f32) -> f32 {
    forward_speed * forward_speed.abs() * surface.aerodynamic_drag
}

fn boost_acceleration(surface: &SurfaceParams, direction: Option<Vec3>) -> Vec3 {
    direction
        .filter(|direction| direction.length_squared() > f32::EPSILON)
        .map(|direction| direction.normalize() * surface.boost_acceleration)
        .unwrap_or(Vec3::ZERO)
}

fn wheel_saturation(
    longitudinal_force: f32,
    longitudinal_limit: f32,
    longitudinal_lateral_cost: f32,
    lateral_force: f32,
    lateral_limit: f32,
) -> f32 {
    let longitudinal =
        longitudinal_force.abs() / longitudinal_limit.max(0.001) * longitudinal_lateral_cost;
    let lateral = lateral_force.abs() / lateral_limit.max(0.001);
    Vec2::new(longitudinal, lateral).length().clamp(0.0, 2.0)
}

fn force_ratio(force: f32, limit: f32) -> f32 {
    (force.abs() / limit.max(0.001)).clamp(0.0, 2.0)
}

fn saturated_handling_state(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
    basis: &MotionBasis,
    previous: HandlingState,
    saturation: f32,
) -> HandlingState {
    let speed = Vec2::new(basis.forward_speed, basis.lateral_speed).length();
    let compliance = surface.compliance.clamp(0.0, 0.85);
    let passive_slip_scale = (surface.passive_slip_scale * (1.0 + compliance * 0.20)).max(0.1);
    let recovery_scale = (surface.recovery_scale * (1.0 - compliance * 0.35)).max(0.1);
    let slip_threshold = tuning.slide_slip_angle_threshold / passive_slip_scale;
    let saturation_threshold = tuning.slide_saturation_threshold / passive_slip_scale;
    let slip_trigger = basis.slip_angle() >= slip_threshold;
    let breakaway = saturation >= saturation_threshold || slip_trigger;
    let recovery =
        saturation < recovery_scale && basis.slip_angle() < slip_threshold * 0.85 * recovery_scale;

    match previous {
        HandlingState::Grip if speed >= tuning.slide_speed_threshold && breakaway => {
            HandlingState::Sliding
        }
        HandlingState::Sliding if recovery => HandlingState::Grip,
        state => state,
    }
}

fn slide_reason(
    surface: &SurfaceParams,
    drift_assist: DriftAssist,
    handling_state: HandlingState,
) -> SlideReason {
    if handling_state == HandlingState::Grip {
        SlideReason::None
    } else if drift_assist.is_active() {
        SlideReason::RearBrakeAssist
    } else if surface.passive_slip_scale > 1.05 {
        SlideReason::SurfaceSlip
    } else {
        SlideReason::PassiveSlip
    }
}

fn threshold_gate(value: f32, threshold: f32, range: f32) -> f32 {
    ((value - threshold) / range.max(0.001)).clamp(0.0, 1.0)
}

fn rear_brake_yaw_assist(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
    input: ControlInput,
    basis: &MotionBasis,
    amount: f32,
) -> f32 {
    if amount <= 0.0 {
        return 0.0;
    }

    let speed = Vec2::new(basis.forward_speed, basis.lateral_speed).length();
    if speed < tuning.drift_min_speed {
        return 0.0;
    }

    let steer_gate = input.steer.abs() >= tuning.drift_min_steer;
    let slip_gate = basis.slip_angle() >= tuning.drift_min_slip_angle;
    if !steer_gate && !slip_gate {
        return 0.0;
    }

    let direction = if steer_gate {
        -input.steer.signum()
    } else {
        -basis.lateral_speed.signum()
    };

    direction * tuning.rear_brake_yaw_assist * surface.rear_brake_yaw_scale * amount * speed
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

    fn control_input(throttle: f32, steer: f32) -> ControlInput {
        ControlInput {
            throttle,
            steer,
            ..default()
        }
    }

    fn tire_forces(
        tuning: &DrivingTuning,
        surface: &SurfaceParams,
        intent: ControlIntent,
        basis: &MotionBasis,
        previous_handling_state: HandlingState,
        contact_friction: SurfaceFriction,
    ) -> TireForces {
        tire_forces_with_assist(
            tuning,
            surface,
            intent,
            basis,
            previous_handling_state,
            contact_friction,
            DriftAssist::default(),
        )
    }

    fn tire_forces_with_assist(
        tuning: &DrivingTuning,
        surface: &SurfaceParams,
        intent: ControlIntent,
        basis: &MotionBasis,
        previous_handling_state: HandlingState,
        contact_friction: SurfaceFriction,
        drift_assist: DriftAssist,
    ) -> TireForces {
        tire_forces_with_boost(
            tuning,
            surface,
            intent,
            basis,
            previous_handling_state,
            contact_friction,
            None,
            drift_assist,
        )
    }

    fn tire_forces_with_boost(
        tuning: &DrivingTuning,
        surface: &SurfaceParams,
        intent: ControlIntent,
        basis: &MotionBasis,
        previous_handling_state: HandlingState,
        contact_friction: SurfaceFriction,
        boost_direction: Option<Vec3>,
        drift_assist: DriftAssist,
    ) -> TireForces {
        super::tire_forces(TireForceInput {
            tuning,
            surface,
            intent,
            basis,
            previous_handling_state,
            contact_friction,
            boost_direction,
            drift_assist,
            gravity_acceleration: gravity_acceleration(tuning, Vec3::Y),
            normal_load_scale: normal_load_scale(tuning, Vec3::Y),
        })
    }

    #[test]
    fn keyboard_steer_axis_matches_runtime_vehicle_direction() {
        let mut keys = ButtonInput::<KeyCode>::default();

        keys.press(KeyCode::KeyD);
        assert_eq!(ControlInput::from_keys(&keys).steer, -1.0);

        keys.release(KeyCode::KeyD);
        keys.press(KeyCode::KeyA);
        assert_eq!(ControlInput::from_keys(&keys).steer, 1.0);
    }

    #[test]
    fn keyboard_rear_brake_maps_space_and_shift_without_reverse() {
        for key in [KeyCode::Space, KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            let mut keys = ButtonInput::<KeyCode>::default();

            keys.press(key);
            let input = ControlInput::from_keys(&keys);

            assert_eq!(input.rear_brake, 1.0);
            assert_eq!(input.throttle, 0.0);
        }
    }

    #[test]
    fn service_brake_does_not_trigger_rear_brake() {
        let mut keys = ButtonInput::<KeyCode>::default();

        keys.press(KeyCode::KeyS);
        let input = ControlInput::from_keys(&keys);

        assert_eq!(input.throttle, -1.0);
        assert_eq!(input.rear_brake, 0.0);
    }

    #[test]
    fn service_brake_enters_reverse_only_after_effective_stop() {
        assert_eq!(
            drive_mode(-1.0, REVERSE_ENTRY_SPEED + 0.01),
            DriveMode::Braking
        );
        assert_eq!(drive_mode(-1.0, REVERSE_ENTRY_SPEED), DriveMode::Reverse);
        assert_eq!(
            drive_mode(-1.0, -REVERSE_ROLLING_SPEED - 0.01),
            DriveMode::Reverse
        );
    }

    #[test]
    fn steering_requires_motion_or_drive_intent() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::ZERO);
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, -1.0), &basis);

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(1.0, 1.0),
        );
        assert_eq!(forces.target_yaw_rate, 0.0);
    }

    #[test]
    fn forward_steering_matches_input_direction() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 10.0);

        let right = ControlIntent::from_input(&tuning, control_input(1.0, 1.0), &basis);
        let left = ControlIntent::from_input(&tuning, control_input(1.0, -1.0), &basis);

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

        assert!(right_forces.target_yaw_rate > 0.0);
        assert!(left_forces.target_yaw_rate < 0.0);
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
        let input = control_input(-1.0, 1.0);
        let forward = ControlIntent::from_input(&tuning, input, &forward_basis);
        let reverse = ControlIntent::from_input(&tuning, input, &reverse_basis);
        let friction = SurfaceFriction::uniform(1.0, 1.0);
        let forward_yaw = tire_forces(
            &tuning,
            &surface,
            forward,
            &forward_basis,
            HandlingState::Grip,
            friction,
        )
        .target_yaw_rate;
        let reverse_yaw = tire_forces(
            &tuning,
            &surface,
            reverse,
            &reverse_basis,
            HandlingState::Grip,
            friction,
        )
        .target_yaw_rate;

        assert!(forward_yaw > 0.0);
        assert!(reverse_yaw < 0.0);
        assert!(reverse_yaw.abs() < forward_yaw.abs());
    }

    #[test]
    fn high_speed_steering_has_less_angle_authority() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let city_basis = MotionBasis::from_yaw(0.0, Vec3::Z * 12.0);
        let fast_basis = MotionBasis::from_yaw(0.0, Vec3::Z * 44.0);
        let city = ControlIntent::from_input(&tuning, control_input(1.0, 1.0), &city_basis);
        let fast = ControlIntent::from_input(&tuning, control_input(1.0, 1.0), &fast_basis);
        let friction = SurfaceFriction::uniform(10.0, 10.0);

        let city_forces = tire_forces(
            &tuning,
            &surface,
            city,
            &city_basis,
            HandlingState::Grip,
            friction,
        );
        let fast_forces = tire_forces(
            &tuning,
            &surface,
            fast,
            &fast_basis,
            HandlingState::Grip,
            friction,
        );

        assert!(fast_forces.target_yaw_rate.abs() > city_forces.target_yaw_rate.abs());
        assert!(
            fast_forces.target_yaw_rate.abs() / fast_basis.forward_speed.abs()
                < city_forces.target_yaw_rate.abs() / city_basis.forward_speed.abs()
        );
    }

    #[test]
    fn yaw_rate_response_is_not_instant_direct_yaw() {
        let tuning = DrivingTuning::default();
        let target = 1.5;
        let resolved = resolved_yaw_rate(&tuning, 0.0, target, 1.0 / 60.0);

        assert!(resolved > 0.0);
        assert!(resolved < target);
    }

    #[test]
    fn wheel_steer_servo_is_not_instant() {
        let tuning = DrivingTuning::default();
        let resolved = resolved_wheel_steer_angle(&tuning, 0.0, tuning.max_steer_angle, 1.0 / 60.0);

        assert!(resolved > 0.0);
        assert!(resolved < tuning.max_steer_angle);
    }

    #[test]
    fn tire_yaw_uses_resolved_wheel_steer_angle() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 16.0);
        let target = ControlIntent::from_input(&tuning, control_input(1.0, 1.0), &basis);
        let barely_turned = target.with_wheel_steer_angle(tuning.max_steer_angle * 0.1);
        let fully_turned = target.with_wheel_steer_angle(tuning.max_steer_angle);
        let friction = SurfaceFriction::uniform(10.0, 10.0);

        let small_yaw = tire_forces(
            &tuning,
            &surface,
            barely_turned,
            &basis,
            HandlingState::Grip,
            friction,
        )
        .target_yaw_rate
        .abs();
        let full_yaw = tire_forces(
            &tuning,
            &surface,
            fully_turned,
            &basis,
            HandlingState::Grip,
            friction,
        )
        .target_yaw_rate
        .abs();

        assert!(full_yaw > small_yaw * 4.0);
    }

    #[test]
    fn vehicle_feedback_reports_motor_and_slip_signals() {
        let tuning = DrivingTuning::default();
        let mut tire_forces = TireForces {
            front_longitudinal_force: tuning.engine_force * 0.5,
            saturation: 1.25,
            ..default()
        };
        tire_forces.wheel_longitudinal_forces[FRONT_LEFT_WHEEL] = tuning.engine_force * 0.25;
        tire_forces.wheel_longitudinal_forces[FRONT_RIGHT_WHEEL] = tuning.engine_force * 0.25;
        let wheel_telemetry = [
            WheelTelemetry {
                angular_speed: 80.0,
                target_angular_speed: 120.0,
                slip_ratio: 0.6,
            },
            WheelTelemetry {
                angular_speed: 70.0,
                target_angular_speed: 110.0,
                slip_ratio: 0.5,
            },
            WheelTelemetry::default(),
            WheelTelemetry::default(),
        ];

        let feedback = vehicle_feedback(&tuning, &wheel_telemetry, &tire_forces);

        assert!(feedback.motor_pitch > 0.65);
        assert!(feedback.motor_load > 0.0);
        assert!(feedback.target_wheel_speed_rpm > feedback.wheel_speed_rpm);
        assert!(feedback.slip_intensity > 0.0);
    }

    #[test]
    fn rear_brake_scrubs_speed_without_reverse_intent() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 18.0);
        let input = ControlInput {
            rear_brake: 1.0,
            ..control_input(0.0, 0.0)
        };
        let intent = ControlIntent::from_input(&tuning, input, &basis);

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(1.0, 1.0),
        );

        assert_eq!(intent.drive_mode, DriveMode::Forward);
        assert!(forces.longitudinal_force < 0.0);
    }

    #[test]
    fn throttle_uses_target_speed_and_fades_near_top_speed() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let slow_basis = MotionBasis::from_yaw(0.0, Vec3::Z * 8.0);
        let fast_basis = MotionBasis::from_yaw(0.0, Vec3::Z * tuning.max_forward_speed);
        let slow_intent = ControlIntent::from_input(&tuning, control_input(1.0, 0.0), &slow_basis);
        let fast_intent = ControlIntent::from_input(&tuning, control_input(1.0, 0.0), &fast_basis);
        let friction = SurfaceFriction::uniform(1.0, 1.0);

        let slow = tire_forces(
            &tuning,
            &surface,
            slow_intent,
            &slow_basis,
            HandlingState::Grip,
            friction,
        );
        let fast = tire_forces(
            &tuning,
            &surface,
            fast_intent,
            &fast_basis,
            HandlingState::Grip,
            friction,
        );

        assert_eq!(slow.target_speed, tuning.max_forward_speed);
        assert!(slow.longitudinal_force > 0.0);
        assert_eq!(fast.longitudinal_force, 0.0);
    }

    #[test]
    fn drive_force_is_front_axle_biased_by_default() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 12.0);
        let intent = ControlIntent::from_input(&tuning, control_input(1.0, 0.0), &basis);

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(1.0, 1.0),
        );

        assert!(forces.front_longitudinal_force > 0.0);
        assert_eq!(forces.rear_longitudinal_force, 0.0);
        assert!(forces.wheel_longitudinal_forces[FRONT_LEFT_WHEEL] > 0.0);
        assert_eq!(forces.wheel_longitudinal_forces[REAR_LEFT_WHEEL], 0.0);
    }

    #[test]
    fn wheel_telemetry_tracks_front_drive_target_speed() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 12.0);
        let intent = ControlIntent::from_input(&tuning, control_input(1.0, 0.0), &basis);
        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(1.0, 1.0),
        );

        let wheels = wheel_telemetry(
            &tuning,
            [WheelTelemetry::default(); WHEEL_COUNT],
            &forces,
            &basis,
            1.0,
        );

        let rolling_angular_speed = basis.forward_speed / tuning.wheel_radius;
        assert!(wheels[FRONT_LEFT_WHEEL].target_angular_speed > rolling_angular_speed);
        assert!(wheels[FRONT_LEFT_WHEEL].slip_ratio > 0.0);
        assert_eq!(
            wheels[REAR_LEFT_WHEEL].target_angular_speed,
            rolling_angular_speed
        );
    }

    #[test]
    fn wheel_telemetry_reports_rear_brake_lock_tendency() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 20.0);
        let input = ControlInput {
            rear_brake: 1.0,
            ..control_input(0.0, 0.0)
        };
        let intent = ControlIntent::from_input(&tuning, input, &basis);
        let drift_assist = DriftAssist::from_input(&tuning, &surface, input, &basis);
        let forces = tire_forces_with_assist(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            surface_friction(surface),
            drift_assist,
        );

        let previous = [WheelTelemetry {
            angular_speed: basis.forward_speed / tuning.wheel_radius,
            ..default()
        }; WHEEL_COUNT];
        let wheels = wheel_telemetry(&tuning, previous, &forces, &basis, 1.0);

        assert!(wheels[REAR_LEFT_WHEEL].target_angular_speed.abs() < f32::EPSILON);
        assert!(wheels[REAR_LEFT_WHEEL].slip_ratio < 0.0);
        assert!(wheels[FRONT_LEFT_WHEEL].slip_ratio.abs() < 0.01);
    }

    #[test]
    fn wheel_suspension_defaults_to_resting_position() {
        let suspension = WheelSuspension::default();

        assert_eq!(suspension.compression, 0.5);
        assert_eq!(suspension.visual_offset, 0.0);
    }

    #[test]
    fn virtual_suspension_compresses_loaded_wheels() {
        let tuning = DrivingTuning::default();
        let mut tire_forces = TireForces::default();
        tire_forces.wheel_normal_loads = resting_wheel_loads(&tuning);
        tire_forces.wheel_normal_loads[FRONT_LEFT_WHEEL] *= 1.6;

        let suspension = virtual_suspension(
            &tuning,
            [WheelSuspension::default(); WHEEL_COUNT],
            &tire_forces,
            1.0,
        );

        assert!(suspension[FRONT_LEFT_WHEEL].compression > 0.5);
        assert!(suspension[FRONT_LEFT_WHEEL].visual_offset < 0.0);
        assert_eq!(suspension[FRONT_RIGHT_WHEEL].compression, 0.5);
    }

    #[test]
    fn rear_brake_consumes_rear_lateral_reserve_through_combined_slip() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 22.0);
        let input = ControlInput {
            rear_brake: 1.0,
            ..control_input(0.0, 0.0)
        };
        let intent = ControlIntent::from_input(&tuning, input, &basis);
        let drift_assist = DriftAssist::from_input(&tuning, &surface, input, &basis);

        let coasting = tire_forces(
            &tuning,
            &surface,
            ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis),
            &basis,
            HandlingState::Grip,
            surface_friction(surface),
        );
        let braking = tire_forces_with_assist(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            surface_friction(surface),
            drift_assist,
        );

        assert!(braking.rear_longitudinal_force < 0.0);
        assert!(braking.rear_lateral_limit < coasting.rear_lateral_limit);
        assert_eq!(braking.front_lateral_limit, coasting.front_lateral_limit);
    }

    #[test]
    fn surface_friction_limits_lateral_force() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let asphalt = surfaces.get(SurfaceKind::Asphalt);
        let ice = surfaces.get(SurfaceKind::Ice);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(20.0, 0.0, 20.0));
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);
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
    fn surface_compliance_softens_lateral_correction() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let firm = surfaces.get(SurfaceKind::Asphalt);
        let soft = SurfaceParams {
            compliance: 0.80,
            ..firm
        };
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(8.0, 0.0, 20.0));
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);

        let firm_force = tire_forces(
            &tuning,
            &firm,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(10.0, 10.0),
        );
        let soft_force = tire_forces(
            &tuning,
            &soft,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(10.0, 10.0),
        );

        assert!(soft_force.lateral_force.abs() < firm_force.lateral_force.abs());
    }

    #[test]
    fn low_grip_surface_slides_before_asphalt() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let asphalt = surfaces.get(SurfaceKind::Asphalt);
        let ice = surfaces.get(SurfaceKind::Ice);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(12.0, 0.0, 22.0));
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);

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

        assert_eq!(asphalt_force.handling_state, HandlingState::Grip);
        assert_eq!(ice_force.handling_state, HandlingState::Sliding);
        assert_eq!(ice_force.slide_reason, SlideReason::SurfaceSlip);
    }

    #[test]
    fn rolling_resistance_slows_coasting() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Dirt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 10.0);
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);

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
    fn rolling_resistance_does_not_push_from_rest() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Dirt);

        assert_eq!(rolling_resistance_force(&tuning, &surface, 0.0), 0.0);
    }

    #[test]
    fn boost_surface_requires_track_direction() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Boost);
        let basis = MotionBasis::from_yaw(0.0, Vec3::ZERO);
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            surface_friction(surface),
        );

        assert_eq!(forces.acceleration, Vec3::ZERO);
    }

    #[test]
    fn boost_force_uses_track_direction_not_car_forward() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Boost);
        let basis = MotionBasis::from_yaw(0.0, Vec3::ZERO);
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);

        let forces = tire_forces_with_boost(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            surface_friction(surface),
            Some(Vec3::X),
            DriftAssist::default(),
        );

        assert!(forces.acceleration.x > 0.0);
        assert!(forces.acceleration.z.abs() < f32::EPSILON);
    }

    #[test]
    fn banked_ground_projects_gravity_and_reduces_normal_load() {
        let tuning = DrivingTuning::default();
        let normal = Vec3::new(
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
        );

        let gravity = gravity_acceleration(&tuning, normal);
        let load_scale = normal_load_scale(&tuning, normal);

        assert!(gravity.x > 0.0);
        assert!(gravity.y < 0.0);
        assert!(load_scale < 1.0);
        assert!(load_scale > 0.70);
    }

    #[test]
    fn axle_forces_report_split_loads_and_saturation() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(14.0, 0.0, 22.0));
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);

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
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(6.0, 0.0, 22.0));
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);

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
    fn rear_brake_physics_reduces_rear_grip_and_raises_rear_saturation() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(16.0, 0.0, 22.0));
        let input = ControlInput {
            steer: 1.0,
            rear_brake: 1.0,
            ..control_input(0.0, 0.0)
        };
        let intent = ControlIntent::from_input(&tuning, input, &basis);
        let drift_assist = DriftAssist::from_input(&tuning, &surface, input, &basis);

        let baseline = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            surface_friction(surface),
        );
        let assisted = tire_forces_with_assist(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            surface_friction(surface),
            drift_assist,
        );

        assert_eq!(drift_assist.state, DriftAssistState::RearBrake);
        assert!(assisted.rear_brake_lateral_cost > 0.0);
        assert!(assisted.rear_lateral_limit < baseline.rear_lateral_limit);
        assert!(assisted.rear_saturation > baseline.rear_saturation);
        assert!(assisted.rear_longitudinal_force < 0.0);
    }

    #[test]
    fn steering_angle_shifts_lateral_demand_to_front_wheels() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::new(10.0, 0.0, 20.0));
        let neutral = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &basis);
        let steering = ControlIntent::from_input(&tuning, control_input(0.0, 1.0), &basis);
        let friction = SurfaceFriction::uniform(10.0, 10.0);

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

        let neutral_front_share =
            neutral_forces.front_lateral_force.abs() / neutral_forces.lateral_force.abs();
        let steering_front_share =
            steering_forces.front_lateral_force.abs() / steering_forces.lateral_force.abs();

        assert!(steering_front_share > neutral_front_share);
        assert!(
            steering_forces.rear_lateral_force.abs() < steering_forces.front_lateral_force.abs()
        );
    }

    #[test]
    fn hard_steering_yaws_while_staying_in_grip_on_asphalt() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 22.0);
        let intent = ControlIntent::from_input(&tuning, control_input(1.0, 1.0), &basis);

        let forces = tire_forces(
            &tuning,
            &surface,
            intent,
            &basis,
            HandlingState::Grip,
            SurfaceFriction::uniform(1.0, 1.0),
        );

        assert_eq!(forces.handling_state, HandlingState::Grip);
        assert!(forces.target_yaw_rate > 0.0);
    }

    #[test]
    fn rear_brake_yaw_assist_requires_speed_and_steer_or_slip() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let low_speed_basis = MotionBasis::from_yaw(0.0, Vec3::Z * 3.0);
        let high_speed_basis = MotionBasis::from_yaw(0.0, Vec3::Z * 22.0);
        let steered = ControlInput {
            steer: 1.0,
            rear_brake: 1.0,
            ..control_input(0.0, 0.0)
        };
        let straight = ControlInput {
            rear_brake: 1.0,
            ..control_input(0.0, 0.0)
        };

        let low_speed = DriftAssist::from_input(&tuning, &surface, steered, &low_speed_basis);
        let straight_high_speed =
            DriftAssist::from_input(&tuning, &surface, straight, &high_speed_basis);
        let steered_high_speed =
            DriftAssist::from_input(&tuning, &surface, steered, &high_speed_basis);

        assert_eq!(low_speed.yaw_assist, 0.0);
        assert_eq!(straight_high_speed.yaw_assist, 0.0);
        assert!(steered_high_speed.yaw_assist < 0.0);
    }

    #[test]
    fn rear_brake_assist_is_physics_driven_without_windows() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Asphalt);
        let basis = MotionBasis::from_yaw(0.0, Vec3::Z * 22.0);
        let held_steering = ControlInput {
            steer: 1.0,
            rear_brake: 1.0,
            ..control_input(0.0, 0.0)
        };
        let released_steering = ControlInput {
            steer: 1.0,
            rear_brake: 0.0,
            ..control_input(0.0, 0.0)
        };
        let held_straight = ControlInput {
            rear_brake: 1.0,
            ..control_input(0.0, 0.0)
        };

        let active = DriftAssist::from_input(&tuning, &surface, held_steering, &basis);
        let released = DriftAssist::from_input(&tuning, &surface, released_steering, &basis);
        let straight = DriftAssist::from_input(&tuning, &surface, held_straight, &basis);

        assert_eq!(active.state, DriftAssistState::RearBrake);
        assert!(active.rear_brake_lateral_cost > 0.0);
        assert!(active.yaw_assist < 0.0);
        assert_eq!(released.state, DriftAssistState::Inactive);
        assert_eq!(released.rear_brake_lateral_cost, 0.0);
        assert_eq!(released.yaw_assist, 0.0);
        assert_eq!(straight.state, DriftAssistState::RearBrake);
        assert!(straight.rear_brake_lateral_cost > 0.0);
        assert_eq!(straight.yaw_assist, 0.0);
    }

    #[test]
    fn saturation_breakaway_and_recovery_have_hysteresis() {
        let tuning = DrivingTuning::default();
        let surfaces = SurfaceLibrary::default();
        let surface = surfaces.get(SurfaceKind::Ice);
        let sliding_basis = MotionBasis::from_yaw(0.0, Vec3::new(28.0, 0.0, 16.0));
        let stable_basis = MotionBasis::from_yaw(0.0, Vec3::Z * 8.0);
        let intent = ControlIntent::from_input(&tuning, control_input(0.0, 0.0), &sliding_basis);
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
