mod components;
mod layers;
mod queries;

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;

pub use components::{
    RailCollider, RoadCollider, VehicleCollider, rail_collision_layers, rail_path_collider,
    road_collision_layers, road_mesh_collider, static_rigid_body, vehicle_collider,
    vehicle_collision_layers, vehicle_rigid_body,
};
#[cfg(test)]
pub use queries::CarHit;
#[cfg(test)]
pub use queries::CarMotion;
pub use queries::{AvianTrackPhysicsQueries, GroundContact, GroundSource, TrackPhysicsQueries};

pub struct PhysicsQueriesPlugin;

impl Plugin for PhysicsQueriesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default());
    }
}
