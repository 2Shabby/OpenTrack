use bevy::prelude::*;

use crate::surface::SurfaceParams;

#[derive(Resource)]
pub struct DrivingTuning {
    pub acceleration: f32,
    pub brake_force: f32,
    pub reverse_force: f32,
    pub steer_rate: f32,
    pub reverse_steer_rate: f32,
    pub min_steer_authority: f32,
    pub lateral_grip: f32,
    pub drag: f32,
    pub max_forward_speed: f32,
    pub max_reverse_speed: f32,
    pub reverse_steering_threshold: f32,
    pub slide_speed_threshold: f32,
    pub slide_slip_angle_threshold: f32,
    pub slide_lateral_grip_multiplier: f32,
    pub slide_yaw_assist_rate: f32,
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
            slide_speed_threshold: 8.0,
            slide_slip_angle_threshold: 0.35,
            slide_lateral_grip_multiplier: 0.58,
            slide_yaw_assist_rate: 0.9,
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
pub struct DriverControls {
    pub throttle: f32,
    pub steer: f32,
}

impl DriverControls {
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
pub struct MotionBasis {
    pub forward: Vec3,
    pub right: Vec3,
    pub forward_speed: f32,
    pub lateral_speed: f32,
}

impl MotionBasis {
    pub fn from_yaw(yaw: f32, velocity: Vec3) -> Self {
        let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);

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

pub fn handling_state(tuning: &DrivingTuning, basis: &MotionBasis) -> HandlingState {
    let speed = Vec2::new(basis.forward_speed, basis.lateral_speed).length();

    if speed >= tuning.slide_speed_threshold
        && basis.slip_angle() >= tuning.slide_slip_angle_threshold
    {
        HandlingState::Sliding
    } else {
        HandlingState::Grip
    }
}

pub fn lateral_grip_multiplier(tuning: &DrivingTuning, handling_state: HandlingState) -> f32 {
    match handling_state {
        HandlingState::Grip => 1.0,
        HandlingState::Sliding => tuning.slide_lateral_grip_multiplier,
    }
}

pub fn slide_yaw_assist(
    tuning: &DrivingTuning,
    controls: DriverControls,
    basis: &MotionBasis,
    handling_state: HandlingState,
) -> f32 {
    if handling_state != HandlingState::Sliding || basis.forward_speed <= 1.0 {
        return 0.0;
    }

    let speed_ratio = (basis.forward_speed / tuning.max_forward_speed).clamp(0.0, 1.0);

    -controls.steer * tuning.slide_yaw_assist_rate * speed_ratio
}

pub fn steering_yaw_delta(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
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

pub fn drive_force(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
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

fn axis(positive: bool, negative: bool) -> f32 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}
