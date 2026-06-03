mod components;
mod layers;
mod queries;

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;

pub use components::{
    RailCollider, RoadCollider, rail_collider, rail_collision_layers, road_collider,
    road_collision_layers, static_rigid_body,
};
pub use queries::{AvianTrackPhysicsQueries, GroundContact, GroundSource, TrackPhysicsQueries};

pub struct PhysicsQueriesPlugin;

impl Plugin for PhysicsQueriesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default());
    }
}
