use bevy::prelude::*;

use crate::geometry::{forward_3d, right_3d};
use crate::surface::SurfaceParams;

#[derive(Resource)]
pub struct DrivingTuning {
    pub acceleration: f32,
    pub brake_force: f32,
    pub reverse_force: f32,
    pub steer_rate: f32,
    pub min_steer_authority: f32,
    pub lateral_grip: f32,
    pub drag: f32,
    pub max_forward_speed: f32,
    pub max_reverse_speed: f32,
    pub reverse_steering_multiplier: f32,
    pub slide_speed_threshold: f32,
    pub slide_slip_angle_threshold: f32,
    pub slide_lateral_grip_multiplier: f32,
    pub slide_yaw_assist_rate: f32,
}

const MIN_STEERING_SPEED: f32 = 0.5;

impl Default for DrivingTuning {
    fn default() -> Self {
        Self {
            acceleration: 38.0,
            brake_force: 52.0,
            reverse_force: 24.0,
            steer_rate: 2.5,
            min_steer_authority: 0.22,
            lateral_grip: 8.5,
            drag: 0.9,
            max_forward_speed: 58.0,
            max_reverse_speed: 14.0,
            reverse_steering_multiplier: 0.45,
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
    mode_steering_multiplier: f32,
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
    intent: ControlIntent,
    basis: &MotionBasis,
    handling_state: HandlingState,
) -> f32 {
    if handling_state != HandlingState::Sliding || basis.forward_speed <= 1.0 {
        return 0.0;
    }

    let speed_ratio = (basis.forward_speed / tuning.max_forward_speed).clamp(0.0, 1.0);

    -intent.input.steer * tuning.slide_yaw_assist_rate * speed_ratio
}

pub fn steering_yaw_delta(
    tuning: &DrivingTuning,
    surface: &SurfaceParams,
    intent: ControlIntent,
    basis: &MotionBasis,
) -> f32 {
    let speed_ratio = (basis.forward_speed.abs() / tuning.max_forward_speed).clamp(0.0, 1.0);
    let steer_authority =
        tuning.min_steer_authority + speed_ratio * (1.0 - tuning.min_steer_authority);

    -intent.input.steer
        * intent.steering_motion_direction
        * tuning.steer_rate
        * steer_authority
        * surface.steering_multiplier
        * intent.mode_steering_multiplier
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

        assert_eq!(steering_yaw_delta(&tuning, &surface, intent, &basis), 0.0);
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

        assert!(steering_yaw_delta(&tuning, &surface, right, &basis) < 0.0);
        assert!(steering_yaw_delta(&tuning, &surface, left, &basis) > 0.0);
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
        let forward_yaw = steering_yaw_delta(&tuning, &surface, forward, &forward_basis);
        let reverse_yaw = steering_yaw_delta(&tuning, &surface, reverse, &reverse_basis);

        assert!(forward_yaw < 0.0);
        assert!(reverse_yaw > 0.0);
        assert!(reverse_yaw.abs() < forward_yaw.abs());
    }
}
