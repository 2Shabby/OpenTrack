use avian3d::prelude::{Collider, CollisionLayers, RigidBody};
use bevy::prelude::*;

use super::layers::{TRACK_RAIL_LAYER, TRACK_ROAD_LAYER};
use crate::geometry::OrientedRect;
use crate::surface::SurfaceKind;

#[derive(Component)]
pub struct RailCollider {
    pub bounds: OrientedRect,
}

#[derive(Component)]
pub struct RoadCollider {
    pub surface: SurfaceKind,
}

pub fn rail_collider(width: f32, height: f32, length: f32) -> Collider {
    Collider::cuboid(width, height, length)
}

pub fn road_collider(width: f32, height: f32, length: f32) -> Collider {
    Collider::cuboid(width, height, length)
}

pub fn static_rigid_body() -> RigidBody {
    RigidBody::Static
}

pub fn road_collision_layers() -> CollisionLayers {
    CollisionLayers::new(TRACK_ROAD_LAYER, avian3d::prelude::LayerMask::ALL)
}

pub fn rail_collision_layers() -> CollisionLayers {
    CollisionLayers::new(TRACK_RAIL_LAYER, avian3d::prelude::LayerMask::ALL)
}
