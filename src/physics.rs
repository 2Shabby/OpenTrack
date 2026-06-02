use bevy::prelude::*;

use crate::spatial::OrientedRect;
use crate::surface::{SurfaceKind, SurfaceZone};

const CAR_COLLISION_HALF_EXTENTS: Vec2 = Vec2::new(0.98, 2.05);

pub struct PhysicsQueriesPlugin;

impl Plugin for PhysicsQueriesPlugin {
    fn build(&self, _app: &mut App) {}
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct GroundHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub surface: SurfaceKind,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct CarHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
}

#[allow(dead_code)]
pub trait TrackPhysicsQueries {
    fn raycast_ground(&self, position: Vec3) -> Option<GroundHit>;
    fn cast_car_shape(&self, _position: Vec3, _velocity: Vec3) -> Option<CarHit>;
    fn surface_at(&self, position: Vec3) -> SurfaceKind {
        self.raycast_ground(position)
            .map(|hit| hit.surface)
            .unwrap_or(SurfaceKind::Asphalt)
    }
}

#[derive(Clone, Copy, Debug)]
struct SurfaceZoneSample {
    kind: SurfaceKind,
    bounds: OrientedRect,
}

#[derive(Component)]
pub struct RailCollider {
    pub bounds: OrientedRect,
}

#[derive(Clone, Copy, Debug)]
struct RailColliderSample {
    bounds: OrientedRect,
}

pub struct EcsTrackPhysicsQueries {
    surface_zones: Vec<SurfaceZoneSample>,
    rails: Vec<RailColliderSample>,
}

impl EcsTrackPhysicsQueries {
    pub fn new(surface_zones: &Query<&SurfaceZone>, rails: &Query<&RailCollider>) -> Self {
        Self {
            surface_zones: surface_zones
                .iter()
                .map(|zone| SurfaceZoneSample {
                    kind: zone.kind,
                    bounds: zone.bounds,
                })
                .collect(),
            rails: rails
                .iter()
                .map(|rail| RailColliderSample {
                    bounds: rail.bounds,
                })
                .collect(),
        }
    }
}

impl TrackPhysicsQueries for EcsTrackPhysicsQueries {
    fn raycast_ground(&self, position: Vec3) -> Option<GroundHit> {
        let surface = self
            .surface_zones
            .iter()
            .find(|zone| zone.contains(position))
            .map(|zone| zone.kind)?;

        Some(GroundHit {
            point: Vec3::new(position.x, 0.0, position.z),
            normal: Vec3::Y,
            surface,
        })
    }

    fn cast_car_shape(&self, position: Vec3, _velocity: Vec3) -> Option<CarHit> {
        self.rails
            .iter()
            .filter_map(|rail| rail.collide_car(position))
            .min_by(|a, b| a.penetration.total_cmp(&b.penetration))
    }
}

impl SurfaceZoneSample {
    fn contains(self, position: Vec3) -> bool {
        self.bounds.contains_xz(position)
    }
}

impl RailColliderSample {
    fn collide_car(self, position: Vec3) -> Option<CarHit> {
        let overlap = self.bounds.overlap_point(
            Vec2::new(position.x, position.z),
            CAR_COLLISION_HALF_EXTENTS,
        )?;

        Some(CarHit {
            point: position,
            normal: Vec3::new(overlap.normal.x, 0.0, overlap.normal.y),
            penetration: overlap.penetration,
        })
    }
}
