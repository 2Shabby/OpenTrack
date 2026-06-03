use bevy::prelude::*;

pub struct CarAssetPlugin;

impl Plugin for CarAssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleSelection>();
    }
}

#[derive(Clone, Copy, Resource, Default)]
pub struct VehicleSelection;

impl VehicleSelection {
    pub const fn fbx_scene_path(self) -> &'static str {
        "cars/fbx/SportsCar.fbx#Scene0"
    }
}
