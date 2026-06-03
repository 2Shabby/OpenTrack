use bevy::prelude::*;

pub struct CarAssetPlugin;

impl Plugin for CarAssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleSelection>();
    }
}

#[derive(Clone, Copy, Resource)]
pub struct VehicleSelection {
    pub vehicle: VehicleKind,
}

impl Default for VehicleSelection {
    fn default() -> Self {
        Self {
            vehicle: VehicleKind::SportsCar,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VehicleKind {
    SportsCar,
    SportsCar2,
}

impl VehicleKind {
    pub const fn from_index(index: usize) -> Self {
        match index {
            0 => Self::SportsCar,
            _ => Self::SportsCar2,
        }
    }

    pub const fn count() -> usize {
        2
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::SportsCar => "SportsCar",
            Self::SportsCar2 => "SportsCar2",
        }
    }

    pub const fn fbx_scene_path(self) -> &'static str {
        match self {
            Self::SportsCar => "cars/fbx/SportsCar.fbx#Scene0",
            Self::SportsCar2 => "cars/fbx/SportsCar2.fbx#Scene0",
        }
    }
}
