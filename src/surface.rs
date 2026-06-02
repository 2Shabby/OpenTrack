use bevy::prelude::*;

use crate::spatial::{OrientedRect, Pose2};

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
    Grass,
}

impl SurfaceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Asphalt => "asphalt",
            Self::Dirt => "dirt",
            Self::Ice => "ice",
            Self::Boost => "boost",
            Self::Grass => "grass",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceParams {
    pub longitudinal_grip: f32,
    pub lateral_grip: f32,
    pub rolling_resistance: f32,
    pub acceleration_multiplier: f32,
    pub steering_multiplier: f32,
    pub drag: f32,
    pub boost_force: f32,
}

#[derive(Resource)]
pub struct SurfaceLibrary {
    asphalt: SurfaceParams,
    dirt: SurfaceParams,
    ice: SurfaceParams,
    boost: SurfaceParams,
    grass: SurfaceParams,
}

impl Default for SurfaceLibrary {
    fn default() -> Self {
        Self {
            asphalt: SurfaceParams {
                longitudinal_grip: 1.0,
                lateral_grip: 1.0,
                rolling_resistance: 1.0,
                acceleration_multiplier: 1.0,
                steering_multiplier: 1.0,
                drag: 1.0,
                boost_force: 0.0,
            },
            dirt: SurfaceParams {
                longitudinal_grip: 0.82,
                lateral_grip: 0.62,
                rolling_resistance: 1.3,
                acceleration_multiplier: 0.86,
                steering_multiplier: 0.82,
                drag: 1.15,
                boost_force: 0.0,
            },
            ice: SurfaceParams {
                longitudinal_grip: 0.55,
                lateral_grip: 0.16,
                rolling_resistance: 0.8,
                acceleration_multiplier: 0.62,
                steering_multiplier: 0.42,
                drag: 0.72,
                boost_force: 0.0,
            },
            boost: SurfaceParams {
                longitudinal_grip: 1.0,
                lateral_grip: 0.92,
                rolling_resistance: 0.85,
                acceleration_multiplier: 1.08,
                steering_multiplier: 0.95,
                drag: 0.75,
                boost_force: 32.0,
            },
            grass: SurfaceParams {
                longitudinal_grip: 0.62,
                lateral_grip: 0.48,
                rolling_resistance: 1.8,
                acceleration_multiplier: 0.58,
                steering_multiplier: 0.7,
                drag: 1.65,
                boost_force: 0.0,
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
            SurfaceKind::Grass => self.grass,
        }
    }
}

#[derive(Component)]
pub struct SurfaceZone {
    pub kind: SurfaceKind,
    pub bounds: OrientedRect,
}

impl SurfaceZone {
    pub fn new(kind: SurfaceKind, pose: Pose2, half_extents: Vec2) -> Self {
        Self {
            kind,
            bounds: OrientedRect::new(pose, half_extents),
        }
    }
}
