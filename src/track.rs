mod generation;
mod markers;
mod piece;
mod road_mesh;
mod scenery;
mod spawn;

pub use generation::{GeneratedTrackInfo, SurfaceMix, TrackRecipe};
pub use markers::{GeneratedRail, GeneratedRoadSurface, GeneratedTrigger, SpawnedSceneEntity};

use bevy::prelude::*;

use crate::driving::CarPaint;
use crate::game_state::GameState;

pub struct TrackPlugin;

impl Plugin for TrackPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TrackRecipe::default())
            .add_systems(OnEnter(GameState::Driving), spawn_sandbox_track)
            .add_systems(OnExit(GameState::Driving), despawn_spawned_scene);
    }
}

pub fn spawn_sandbox_track(
    commands: Commands,
    recipe: Res<TrackRecipe>,
    asset_server: Res<AssetServer>,
    car_paint: Res<CarPaint>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn::spawn_generated_track(
        commands,
        &recipe,
        &asset_server,
        &car_paint,
        meshes,
        materials,
    );
}

fn despawn_spawned_scene(
    mut commands: Commands,
    entities: Query<Entity, With<SpawnedSceneEntity>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}
