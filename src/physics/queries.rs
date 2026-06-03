use avian3d::prelude::{Collider, ShapeCastConfig, SpatialQuery};
use bevy::prelude::*;

use super::components::RoadCollider;
use super::layers::{rail_query_filter, road_query_filter};
use crate::geometry::yaw_rotation;
use crate::surface::SurfaceKind;

const CAR_COLLISION_LATERAL_HALF_EXTENT: f32 = 0.98;
const CAR_COLLISION_LONGITUDINAL_HALF_EXTENT: f32 = 2.0;
const CAR_COLLISION_TARGET_DISTANCE: f32 = 0.03;
const GROUND_RAY_START_HEIGHT: f32 = 3.0;
const GROUND_RAY_DISTANCE: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct CarHit {
    pub normal: Vec3,
    pub travel: f32,
}

pub trait TrackPhysicsQueries {
    fn cast_car_motion(&self, start: Vec3, end: Vec3, yaw: f32) -> Option<CarHit>;
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
struct RoadColliderSample {
    surface: SurfaceKind,
}

pub struct AvianTrackPhysicsQueries<'a, 'w, 's> {
    spatial_query: &'a SpatialQuery<'w, 's>,
    roads: Vec<(Entity, RoadColliderSample)>,
}

impl<'a, 'w, 's> AvianTrackPhysicsQueries<'a, 'w, 's> {
    pub fn new(
        spatial_query: &'a SpatialQuery<'w, 's>,
        roads: &Query<(Entity, &RoadCollider)>,
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

    fn cast_car_motion(&self, start: Vec3, end: Vec3, yaw: f32) -> Option<CarHit> {
        let motion = end - start;
        let (direction, distance) = Dir3::new_and_length(motion).ok()?;
        let shape = car_collider();
        let rotation = yaw_rotation(yaw);
        let config = ShapeCastConfig::from_max_distance(distance)
            .with_target_distance(CAR_COLLISION_TARGET_DISTANCE);
        let filter = rail_query_filter();

        self.spatial_query
            .cast_shape(&shape, start, rotation, direction, &config, &filter)
            .map(|hit| CarHit {
                normal: hit.normal1.normalize_or_zero(),
                travel: hit.distance,
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

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::SimpleCollider;

    #[test]
    fn car_collider_matches_controller_extents() {
        let collider = car_collider();
        let aabb = collider.aabb(Vec3::ZERO, Quat::IDENTITY);

        assert!((aabb.max.x - CAR_COLLISION_LATERAL_HALF_EXTENT).abs() < f32::EPSILON);
        assert!((aabb.max.z - CAR_COLLISION_LONGITUDINAL_HALF_EXTENT).abs() < f32::EPSILON);
    }
}
