mod generation;
mod markers;
mod road_mesh;
mod scenery;
mod spawn;

pub use generation::{GeneratedTrackInfo, TrackRecipe};
pub use markers::{GeneratedRail, GeneratedRoadSurface, GeneratedTrigger};

use bevy::prelude::*;

pub struct TrackPlugin;

impl Plugin for TrackPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TrackRecipe::default());
    }
}

pub fn spawn_sandbox_track(
    commands: Commands,
    recipe: Res<TrackRecipe>,
    asset_server: Res<AssetServer>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn::spawn_generated_track(commands, &recipe, &asset_server, meshes, materials);
}
