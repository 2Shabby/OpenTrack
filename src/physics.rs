use bevy::prelude::*;

use crate::surface::{SurfaceKind, SurfaceZone};

const CAR_COLLISION_HALF_EXTENTS: Vec2 = Vec2::new(0.78, 1.05);

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
    center: Vec2,
    half_extents: Vec2,
}

#[derive(Component)]
pub struct RailCollider {
    pub center: Vec2,
    pub half_extents: Vec2,
}

#[derive(Clone, Copy, Debug)]
struct RailColliderSample {
    center: Vec2,
    half_extents: Vec2,
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
                    center: zone.center,
                    half_extents: zone.half_extents,
                })
                .collect(),
            rails: rails
                .iter()
                .map(|rail| RailColliderSample {
                    center: rail.center,
                    half_extents: rail.half_extents,
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
        let dx = (position.x - self.center.x).abs();
        let dz = (position.z - self.center.y).abs();

        dx <= self.half_extents.x && dz <= self.half_extents.y
    }
}

impl RailColliderSample {
    fn collide_car(self, position: Vec3) -> Option<CarHit> {
        let center = Vec2::new(position.x, position.z);
        let delta = center - self.center;
        let expanded = self.half_extents + CAR_COLLISION_HALF_EXTENTS;

        if delta.x.abs() > expanded.x || delta.y.abs() > expanded.y {
            return None;
        }

        let penetration_x = expanded.x - delta.x.abs();
        let penetration_z = expanded.y - delta.y.abs();

        let normal = if penetration_x <= penetration_z {
            Vec3::new(delta.x.signum_or_one(), 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, delta.y.signum_or_one())
        };

        Some(CarHit {
            point: position,
            normal,
            penetration: penetration_x.min(penetration_z),
        })
    }
}

trait SignumOrOne {
    fn signum_or_one(self) -> f32;
}

impl SignumOrOne for f32 {
    fn signum_or_one(self) -> f32 {
        if self >= 0.0 { 1.0 } else { -1.0 }
    }
}
