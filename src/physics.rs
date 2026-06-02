use bevy::prelude::*;

use crate::spatial::{OrientedRect, rotate_2d};
use crate::surface::{SurfaceKind, SurfaceZone};

const CAR_COLLISION_LATERAL_HALF_EXTENT: f32 = 0.98;
const CAR_COLLISION_LONGITUDINAL_HALF_EXTENT: f32 = 2.0;

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
    fn ground_at(&self, position: Vec3) -> GroundContact {
        self.surface_zones
            .iter()
            .find(|zone| zone.contains(position))
            .map(|zone| GroundContact {
                source: GroundSource::Road,
                surface: zone.kind,
            })
            .unwrap_or(GroundContact {
                source: GroundSource::OffTrack,
                surface: SurfaceKind::Grass,
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
        let local = self
            .bounds
            .pose
            .world_to_local(Vec2::new(position.x, position.z));
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

fn signum_or_one(value: f32) -> f32 {
    if value >= 0.0 { 1.0 } else { -1.0 }
}
