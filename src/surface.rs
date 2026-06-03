use bevy::prelude::*;

pub struct SurfacePlugin;

impl Plugin for SurfacePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SurfaceLibrary::default());
    }
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub enum SurfaceKind {
    Asphalt,
    Dirt,
    Ice,
    Boost,
}

impl SurfaceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Asphalt => "asphalt",
            Self::Dirt => "dirt",
            Self::Ice => "ice",
            Self::Boost => "boost",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceParams {
    pub longitudinal_friction: f32,
    pub lateral_friction: f32,
    pub rolling_resistance: f32,
    pub aerodynamic_drag: f32,
    pub boost_acceleration: f32,
    pub passive_slip_scale: f32,
    pub recovery_scale: f32,
    pub compliance: f32,
    pub rear_brake_grip_loss_scale: f32,
    pub rear_brake_yaw_scale: f32,
}

#[derive(Resource)]
pub struct SurfaceLibrary {
    asphalt: SurfaceParams,
    dirt: SurfaceParams,
    ice: SurfaceParams,
    boost: SurfaceParams,
}

impl Default for SurfaceLibrary {
    fn default() -> Self {
        Self {
            asphalt: SurfaceParams {
                longitudinal_friction: 1.04,
                lateral_friction: 1.08,
                rolling_resistance: 0.018,
                aerodynamic_drag: 0.020,
                boost_acceleration: 0.0,
                passive_slip_scale: 0.95,
                recovery_scale: 1.35,
                compliance: 0.0,
                rear_brake_grip_loss_scale: 0.35,
                rear_brake_yaw_scale: 0.55,
            },
            dirt: SurfaceParams {
                longitudinal_friction: 0.76,
                lateral_friction: 0.58,
                rolling_resistance: 0.035,
                aerodynamic_drag: 0.024,
                boost_acceleration: 0.0,
                passive_slip_scale: 1.12,
                recovery_scale: 0.82,
                compliance: 0.34,
                rear_brake_grip_loss_scale: 0.85,
                rear_brake_yaw_scale: 0.82,
            },
            ice: SurfaceParams {
                longitudinal_friction: 0.28,
                lateral_friction: 0.16,
                rolling_resistance: 0.012,
                aerodynamic_drag: 0.016,
                boost_acceleration: 0.0,
                passive_slip_scale: 1.45,
                recovery_scale: 0.45,
                compliance: 0.10,
                rear_brake_grip_loss_scale: 0.50,
                rear_brake_yaw_scale: 0.35,
            },
            boost: SurfaceParams {
                longitudinal_friction: 1.04,
                lateral_friction: 1.02,
                rolling_resistance: 0.014,
                aerodynamic_drag: 0.016,
                boost_acceleration: 24.0,
                passive_slip_scale: 0.98,
                recovery_scale: 1.20,
                compliance: 0.0,
                rear_brake_grip_loss_scale: 0.35,
                rear_brake_yaw_scale: 0.50,
            },
        }
    }
}

impl SurfaceLibrary {
    pub fn get(&self, kind: SurfaceKind) -> SurfaceParams {
        match kind {
            SurfaceKind::Asphalt => self.asphalt,
            SurfaceKind::Dirt => self.dirt,
            SurfaceKind::Ice => self.ice,
            SurfaceKind::Boost => self.boost,
        }
    }
}
