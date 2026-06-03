use avian3d::prelude::{Collider, CollisionLayers, RigidBody};
use bevy::prelude::*;

use super::layers::{TRACK_RAIL_LAYER, TRACK_ROAD_LAYER, VEHICLE_LAYER};
use crate::surface::SurfaceKind;

pub const VEHICLE_COLLISION_LATERAL_HALF_EXTENT: f32 = 0.98;
pub const VEHICLE_COLLISION_LONGITUDINAL_HALF_EXTENT: f32 = 2.25;
pub const VEHICLE_COLLISION_HEIGHT: f32 = 0.35;

#[derive(Component)]
pub struct RailCollider;

#[derive(Component)]
pub struct RoadCollider {
    pub surface: SurfaceKind,
}

#[derive(Component)]
pub struct VehicleCollider;

pub fn vehicle_collider() -> Collider {
    Collider::cuboid(
        VEHICLE_COLLISION_LATERAL_HALF_EXTENT * 2.0,
        VEHICLE_COLLISION_HEIGHT,
        VEHICLE_COLLISION_LONGITUDINAL_HALF_EXTENT * 2.0,
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
    fn vehicle_collider_matches_controller_extents() {
        let collider = vehicle_collider();
        let aabb = collider.aabb(Vec3::ZERO, Quat::IDENTITY);

        assert!((aabb.max.x - VEHICLE_COLLISION_LATERAL_HALF_EXTENT).abs() < f32::EPSILON);
        assert!((aabb.max.z - VEHICLE_COLLISION_LONGITUDINAL_HALF_EXTENT).abs() < f32::EPSILON);
    }
}
