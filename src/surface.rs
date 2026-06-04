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
                lateral_friction: 1.16,
                rolling_resistance: 0.018,
                aerodynamic_drag: 0.020,
                boost_acceleration: 0.0,
                passive_slip_scale: 0.84,
                recovery_scale: 1.55,
                compliance: 0.0,
                rear_brake_grip_loss_scale: 0.28,
                rear_brake_yaw_scale: 0.45,
            },
            dirt: SurfaceParams {
                longitudinal_friction: 0.80,
                lateral_friction: 0.66,
                rolling_resistance: 0.035,
                aerodynamic_drag: 0.024,
                boost_acceleration: 0.0,
                passive_slip_scale: 1.02,
                recovery_scale: 0.96,
                compliance: 0.28,
                rear_brake_grip_loss_scale: 0.72,
                rear_brake_yaw_scale: 0.68,
            },
            ice: SurfaceParams {
                longitudinal_friction: 0.32,
                lateral_friction: 0.20,
                rolling_resistance: 0.012,
                aerodynamic_drag: 0.016,
                boost_acceleration: 0.0,
                passive_slip_scale: 1.30,
                recovery_scale: 0.56,
                compliance: 0.10,
                rear_brake_grip_loss_scale: 0.42,
                rear_brake_yaw_scale: 0.28,
            },
            boost: SurfaceParams {
                longitudinal_friction: 1.04,
                lateral_friction: 1.10,
                rolling_resistance: 0.014,
                aerodynamic_drag: 0.016,
                boost_acceleration: 28.8,
                passive_slip_scale: 0.88,
                recovery_scale: 1.40,
                compliance: 0.0,
                rear_brake_grip_loss_scale: 0.30,
                rear_brake_yaw_scale: 0.42,
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
