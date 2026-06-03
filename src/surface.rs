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
    pub longitudinal_friction: f32,
    pub lateral_friction: f32,
    pub rolling_resistance: f32,
    pub aerodynamic_drag: f32,
    pub boost_acceleration: f32,
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
                longitudinal_friction: 1.0,
                lateral_friction: 1.0,
                rolling_resistance: 0.018,
                aerodynamic_drag: 0.020,
                boost_acceleration: 0.0,
            },
            dirt: SurfaceParams {
                longitudinal_friction: 0.78,
                lateral_friction: 0.62,
                rolling_resistance: 0.035,
                aerodynamic_drag: 0.024,
                boost_acceleration: 0.0,
            },
            ice: SurfaceParams {
                longitudinal_friction: 0.28,
                lateral_friction: 0.16,
                rolling_resistance: 0.012,
                aerodynamic_drag: 0.016,
                boost_acceleration: 0.0,
            },
            boost: SurfaceParams {
                longitudinal_friction: 1.0,
                lateral_friction: 0.92,
                rolling_resistance: 0.014,
                aerodynamic_drag: 0.016,
                boost_acceleration: 24.0,
            },
            grass: SurfaceParams {
                longitudinal_friction: 0.50,
                lateral_friction: 0.42,
                rolling_resistance: 0.060,
                aerodynamic_drag: 0.034,
                boost_acceleration: 0.0,
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
