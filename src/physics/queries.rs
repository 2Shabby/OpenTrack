use avian3d::prelude::{Collider, SpatialQuery};
use bevy::prelude::*;

use super::components::{RailCollider, RoadCollider};
use super::layers::{rail_query_filter, road_query_filter};
use crate::geometry::{OrientedRect, rotate_2d, xz_position, yaw_rotation};
use crate::surface::SurfaceKind;

const CAR_COLLISION_LATERAL_HALF_EXTENT: f32 = 0.98;
const CAR_COLLISION_LONGITUDINAL_HALF_EXTENT: f32 = 2.0;
const GROUND_RAY_START_HEIGHT: f32 = 3.0;
const GROUND_RAY_DISTANCE: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct CarHit {
    pub normal: Vec3,
    pub penetration: f32,
}

pub trait TrackPhysicsQueries {
    fn cast_car_shape(&self, position: Vec3, yaw: f32, velocity: Vec3) -> Option<CarHit>;
    fn ground_at(&self, position: Vec3) -> GroundContact;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroundSource {
    Road,
    OffTrack,
}

impl GroundSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Road => "road",
            Self::OffTrack => "offtrack",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroundContact {
    pub source: GroundSource,
    pub surface: SurfaceKind,
}

#[derive(Clone, Copy, Debug)]
struct RailColliderSample {
    bounds: OrientedRect,
}

#[derive(Clone, Copy, Debug)]
struct RoadColliderSample {
    surface: SurfaceKind,
}

pub struct AvianTrackPhysicsQueries<'a, 'w, 's> {
    spatial_query: &'a SpatialQuery<'w, 's>,
    roads: Vec<(Entity, RoadColliderSample)>,
    rails: Vec<(Entity, RailColliderSample)>,
}

impl<'a, 'w, 's> AvianTrackPhysicsQueries<'a, 'w, 's> {
    pub fn new(
        spatial_query: &'a SpatialQuery<'w, 's>,
        roads: &Query<(Entity, &RoadCollider)>,
        rails: &Query<(Entity, &RailCollider)>,
    ) -> Self {
        Self {
            spatial_query,
            roads: roads
                .iter()
                .map(|(entity, road)| {
                    (
                        entity,
                        RoadColliderSample {
                            surface: road.surface,
                        },
                    )
                })
                .collect(),
            rails: rails
                .iter()
                .map(|(entity, rail)| {
                    (
                        entity,
                        RailColliderSample {
                            bounds: rail.bounds,
                        },
                    )
                })
                .collect(),
        }
    }
}

impl TrackPhysicsQueries for AvianTrackPhysicsQueries<'_, '_, '_> {
    fn ground_at(&self, position: Vec3) -> GroundContact {
        let origin = position + Vec3::Y * GROUND_RAY_START_HEIGHT;
        let filter = road_query_filter();
        let Some(hit) =
            self.spatial_query
                .cast_ray(origin, Dir3::NEG_Y, GROUND_RAY_DISTANCE, false, &filter)
        else {
            return off_track_contact();
        };

        self.roads
            .iter()
            .find(|(entity, _)| *entity == hit.entity)
            .map(|(_, road)| GroundContact {
                source: GroundSource::Road,
                surface: road.surface,
            })
            .unwrap_or_else(off_track_contact)
    }

    fn cast_car_shape(&self, position: Vec3, yaw: f32, _velocity: Vec3) -> Option<CarHit> {
        let shape = car_collider();
        let rotation = yaw_rotation(yaw);
        let filter = rail_query_filter();
        let intersections = self
            .spatial_query
            .shape_intersections(&shape, position, rotation, &filter);

        self.rails
            .iter()
            .filter(|(entity, _)| intersections.contains(entity))
            .filter_map(|(_, rail)| rail.collide_car(position))
            .max_by(|a, b| a.penetration.total_cmp(&b.penetration))
    }
}

impl RailColliderSample {
    fn collide_car(self, position: Vec3) -> Option<CarHit> {
        let local = self.bounds.pose.world_to_local(xz_position(position));
        let expanded = self.bounds.half_extents
            + Vec2::new(
                CAR_COLLISION_LATERAL_HALF_EXTENT,
                CAR_COLLISION_LONGITUDINAL_HALF_EXTENT,
            );

        if local.x.abs() > expanded.x || local.y.abs() > expanded.y {
            return None;
        }

        let local_normal = Vec2::new(signum_or_one(local.x), 0.0);
        let normal = rotate_2d(local_normal, self.bounds.pose.yaw);

        Some(CarHit {
            normal: Vec3::new(normal.x, 0.0, normal.y),
            penetration: expanded.x - local.x.abs(),
        })
    }
}

fn car_collider() -> Collider {
    Collider::cuboid(
        CAR_COLLISION_LATERAL_HALF_EXTENT * 2.0,
        0.3,
        CAR_COLLISION_LONGITUDINAL_HALF_EXTENT * 2.0,
    )
}

fn off_track_contact() -> GroundContact {
    GroundContact {
        source: GroundSource::OffTrack,
        surface: SurfaceKind::Grass,
    }
}

fn signum_or_one(value: f32) -> f32 {
    if value >= 0.0 { 1.0 } else { -1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Pose2;

    #[test]
    fn rail_collision_returns_deepest_hit() {
        let rails = [
            RailColliderSample {
                bounds: OrientedRect::new(Pose2::new(Vec2::ZERO, 0.0), Vec2::ZERO),
            },
            RailColliderSample {
                bounds: OrientedRect::new(
                    Pose2::new(Vec2::new(0.5, 0.0), 0.0),
                    Vec2::new(1.0, 0.0),
                ),
            },
        ];

        let hit = rails
            .iter()
            .filter_map(|rail| rail.collide_car(Vec3::ZERO))
            .max_by(|a, b| a.penetration.total_cmp(&b.penetration))
            .expect("car overlaps both rails");

        assert!((hit.penetration - 1.48).abs() < f32::EPSILON);
    }
}
