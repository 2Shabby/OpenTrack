use bevy::prelude::*;

use crate::spatial::OrientedRect;
use crate::surface::{SurfaceKind, SurfaceZone};

const CAR_COLLISION_LATERAL_HALF_EXTENT: f32 = 0.98;
const RAIL_END_TOLERANCE: f32 = 0.15;

pub struct PhysicsQueriesPlugin;

impl Plugin for PhysicsQueriesPlugin {
    fn build(&self, _app: &mut App) {}
}

#[derive(Clone, Copy, Debug)]
pub struct CarHit {
    pub normal: Vec3,
    pub penetration: f32,
}

pub trait TrackPhysicsQueries {
    fn cast_car_shape(&self, _position: Vec3, _velocity: Vec3) -> Option<CarHit>;
    fn surface_at(&self, position: Vec3) -> SurfaceKind;
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
    fn surface_at(&self, position: Vec3) -> SurfaceKind {
        self.surface_zones
            .iter()
            .find(|zone| zone.contains(position))
            .map(|zone| zone.kind)
            .unwrap_or(SurfaceKind::Asphalt)
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
            Vec2::new(CAR_COLLISION_LATERAL_HALF_EXTENT, RAIL_END_TOLERANCE),
        )?;

        Some(CarHit {
            normal: Vec3::new(overlap.normal.x, 0.0, overlap.normal.y),
            penetration: overlap.penetration,
        })
    }
}
