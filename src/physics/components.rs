use avian3d::prelude::{Collider, CollisionLayers, RigidBody};
use bevy::prelude::*;

use super::layers::{TRACK_RAIL_LAYER, TRACK_ROAD_LAYER, VEHICLE_LAYER};
use crate::surface::SurfaceKind;

pub const VEHICLE_COLLISION_HALF_WIDTH: f32 = 1.00;
pub const VEHICLE_COLLISION_HALF_LENGTH: f32 = 2.48;
pub const VEHICLE_COLLISION_HEIGHT: f32 = 0.42;
pub const VEHICLE_COLLISION_CORNER_RADIUS: f32 = 0.12;

#[derive(Component)]
pub struct RailCollider;

#[derive(Component)]
pub struct RoadCollider {
    pub surface: SurfaceKind,
    pub boost_direction: Option<Vec3>,
}

#[derive(Component)]
pub struct VehicleCollider;

pub fn vehicle_collider() -> Collider {
    let inner_half_width = VEHICLE_COLLISION_HALF_WIDTH - VEHICLE_COLLISION_CORNER_RADIUS;
    let inner_half_length = VEHICLE_COLLISION_HALF_LENGTH - VEHICLE_COLLISION_CORNER_RADIUS;
    let inner_half_height = VEHICLE_COLLISION_HEIGHT * 0.5 - VEHICLE_COLLISION_CORNER_RADIUS;

    Collider::round_cuboid(
        inner_half_width * 2.0,
        inner_half_height.max(0.01) * 2.0,
        inner_half_length * 2.0,
        VEHICLE_COLLISION_CORNER_RADIUS,
    )
}

pub fn rail_path_collider(points: &[Vec3], radius: f32) -> Collider {
    Collider::compound(
        points
            .windows(2)
            .filter_map(|pair| {
                let [start, end] = [pair[0], pair[1]];
                (start.distance_squared(end) > f32::EPSILON).then(|| {
                    (
                        Vec3::ZERO,
                        Quat::IDENTITY,
                        Collider::capsule_endpoints(radius, start, end),
                    )
                })
            })
            .collect::<Vec<_>>(),
    )
}

pub fn road_mesh_collider(vertices: Vec<Vec3>, indices: Vec<[u32; 3]>) -> Collider {
    Collider::trimesh(vertices, indices)
}

pub fn static_rigid_body() -> RigidBody {
    RigidBody::Static
}

pub fn vehicle_rigid_body() -> RigidBody {
    RigidBody::Kinematic
}

pub fn road_collision_layers() -> CollisionLayers {
    CollisionLayers::new(TRACK_ROAD_LAYER, avian3d::prelude::LayerMask::ALL)
}

pub fn rail_collision_layers() -> CollisionLayers {
    CollisionLayers::new(TRACK_RAIL_LAYER, avian3d::prelude::LayerMask::ALL)
}

pub fn vehicle_collision_layers() -> CollisionLayers {
    CollisionLayers::new(VEHICLE_LAYER, TRACK_RAIL_LAYER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::SimpleCollider;

    #[test]
    fn rail_path_collider_uses_path_extents() {
        let collider =
            rail_path_collider(&[Vec3::ZERO, Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0)], 0.5);
        let aabb = collider.aabb(Vec3::ZERO, Quat::IDENTITY);

        assert!(aabb.max.z >= 10.0);
        assert!(aabb.max.x >= 0.5);
    }

    #[test]
    fn vehicle_collider_uses_centered_softened_footprint() {
        let collider = vehicle_collider();
        let aabb = collider.aabb(Vec3::ZERO, Quat::IDENTITY);

        assert!((aabb.max.x - VEHICLE_COLLISION_HALF_WIDTH).abs() < f32::EPSILON);
        assert!((aabb.min.x + VEHICLE_COLLISION_HALF_WIDTH).abs() < f32::EPSILON);
        assert!((aabb.max.z - VEHICLE_COLLISION_HALF_LENGTH).abs() < f32::EPSILON);
        assert!((aabb.min.z + VEHICLE_COLLISION_HALF_LENGTH).abs() < f32::EPSILON);
    }
}
