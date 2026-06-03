use avian3d::prelude::{Collider, CollisionLayers, RigidBody};
use bevy::prelude::*;

use super::layers::{TRACK_RAIL_LAYER, TRACK_ROAD_LAYER};
use crate::surface::SurfaceKind;

#[derive(Component)]
pub struct RailCollider;

#[derive(Component)]
pub struct RoadCollider {
    pub surface: SurfaceKind,
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

pub fn road_collision_layers() -> CollisionLayers {
    CollisionLayers::new(TRACK_ROAD_LAYER, avian3d::prelude::LayerMask::ALL)
}

pub fn rail_collision_layers() -> CollisionLayers {
    CollisionLayers::new(TRACK_RAIL_LAYER, avian3d::prelude::LayerMask::ALL)
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
}
