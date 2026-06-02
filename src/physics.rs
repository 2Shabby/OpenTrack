use bevy::prelude::*;

use crate::surface::{SurfaceKind, SurfaceZone};

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
    center: Vec2,
    half_extents: Vec2,
}

pub struct EcsTrackPhysicsQueries {
    surface_zones: Vec<SurfaceZoneSample>,
}

impl EcsTrackPhysicsQueries {
    pub fn new(surface_zones: &Query<&SurfaceZone>) -> Self {
        Self {
            surface_zones: surface_zones
                .iter()
                .map(|zone| SurfaceZoneSample {
                    kind: zone.kind,
                    center: zone.center,
                    half_extents: zone.half_extents,
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

    fn cast_car_shape(&self, _position: Vec3, _velocity: Vec3) -> Option<CarHit> {
        None
    }
}

impl SurfaceZoneSample {
    fn contains(self, position: Vec3) -> bool {
        let dx = (position.x - self.center.x).abs();
        let dz = (position.z - self.center.y).abs();

        dx <= self.half_extents.x && dz <= self.half_extents.y
    }
}
