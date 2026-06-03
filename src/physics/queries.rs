use avian3d::prelude::{ShapeCastConfig, SpatialQuery};
use bevy::prelude::*;

use super::components::{RoadCollider, vehicle_collider};
use super::layers::{rail_query_filter, road_query_filter};
use crate::geometry::yaw_rotation;
use crate::surface::SurfaceKind;

const CAR_COLLISION_TARGET_DISTANCE: f32 = 0.03;
const GROUND_RAY_START_HEIGHT: f32 = 3.0;
const GROUND_RAY_DISTANCE: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct CarHit {
    pub normal: Vec3,
    pub travel: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CarMotion {
    pub translation: Vec3,
    pub hit: Option<CarHit>,
}

pub trait TrackPhysicsQueries {
    fn cast_car_motion(&self, start: Vec3, end: Vec3, yaw: f32) -> Option<CarHit>;
    fn move_car_with_collisions(&self, start: Vec3, desired_end: Vec3, yaw: f32) -> CarMotion;
    fn car_overlaps_rail(&self, position: Vec3, yaw: f32) -> bool;
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
        let shape = vehicle_collider();
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

    fn move_car_with_collisions(&self, start: Vec3, desired_end: Vec3, yaw: f32) -> CarMotion {
        let requested_motion = desired_end - start;
        let Some(hit) = self.cast_car_motion(start, desired_end, yaw) else {
            return CarMotion {
                translation: desired_end,
                hit: None,
            };
        };

        let travel = (hit.travel - CAR_COLLISION_TARGET_DISTANCE).max(0.0);
        let first_stop = start + requested_motion.normalize_or_zero() * travel;
        let remaining = desired_end - first_stop;
        let slide = slide_motion(remaining, hit.normal);
        let slide_end = first_stop + slide;

        let translation = if slide.length_squared() > 0.0001 {
            if let Some(slide_hit) = self.cast_car_motion(first_stop, slide_end, yaw) {
                let slide_travel = (slide_hit.travel - CAR_COLLISION_TARGET_DISTANCE).max(0.0);
                first_stop + slide.normalize_or_zero() * slide_travel
            } else {
                slide_end
            }
        } else {
            first_stop
        };

        CarMotion {
            translation,
            hit: Some(hit),
        }
    }

    fn car_overlaps_rail(&self, position: Vec3, yaw: f32) -> bool {
        let shape = vehicle_collider();
        let rotation = yaw_rotation(yaw);
        let filter = rail_query_filter();

        !self
            .spatial_query
            .shape_intersections(&shape, position, rotation, &filter)
            .is_empty()
    }
}

fn off_track_contact() -> GroundContact {
    GroundContact {
        source: GroundSource::OffTrack,
        surface: SurfaceKind::Grass,
    }
}

fn slide_motion(remaining: Vec3, normal: Vec3) -> Vec3 {
    remaining - normal * remaining.dot(normal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_motion_removes_inward_normal_component() {
        let slide = slide_motion(Vec3::new(3.0, 0.0, 4.0), Vec3::X);

        assert!(slide.x.abs() < f32::EPSILON);
        assert_eq!(slide.z, 4.0);
    }
}
