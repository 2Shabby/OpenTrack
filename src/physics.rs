use avian3d::prelude::{
    Collider, CollisionLayers, LayerMask, PhysicsPlugins, RigidBody, SpatialQuery,
    SpatialQueryFilter,
};
use bevy::prelude::*;

use crate::spatial::{OrientedRect, rotate_2d};
use crate::surface::SurfaceKind;

const CAR_COLLISION_LATERAL_HALF_EXTENT: f32 = 0.98;
const CAR_COLLISION_LONGITUDINAL_HALF_EXTENT: f32 = 2.0;
const GROUND_RAY_START_HEIGHT: f32 = 3.0;
const GROUND_RAY_DISTANCE: f32 = 8.0;
const TRACK_ROAD_LAYER: LayerMask = LayerMask(1 << 0);
const TRACK_RAIL_LAYER: LayerMask = LayerMask(1 << 1);

pub struct PhysicsQueriesPlugin;

impl Plugin for PhysicsQueriesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default());
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CarHit {
    pub normal: Vec3,
    pub penetration: f32,
}

pub trait TrackPhysicsQueries {
    fn cast_car_shape(&self, _position: Vec3, _yaw: f32, _velocity: Vec3) -> Option<CarHit>;
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

#[derive(Component)]
pub struct RailCollider {
    pub bounds: OrientedRect,
}

#[derive(Component)]
pub struct RoadCollider {
    pub bounds: OrientedRect,
    pub surface: SurfaceKind,
}

#[derive(Clone, Copy, Debug)]
struct RailColliderSample {
    bounds: OrientedRect,
}

#[derive(Clone, Copy, Debug)]
struct RoadColliderSample {
    surface: SurfaceKind,
    bounds: OrientedRect,
}

pub struct EcsTrackPhysicsQueries {
    roads: Vec<RoadColliderSample>,
    rails: Vec<RailColliderSample>,
}

pub struct AvianTrackPhysicsQueries<'a, 'w, 's> {
    spatial_query: &'a SpatialQuery<'w, 's>,
    roads: Vec<(Entity, RoadColliderSample)>,
    rails: Vec<(Entity, RailColliderSample)>,
}

impl EcsTrackPhysicsQueries {
    #[cfg(test)]
    fn from_rails(rails: Vec<RailColliderSample>) -> Self {
        Self {
            roads: Vec::new(),
            rails,
        }
    }
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
                            bounds: road.bounds,
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

impl TrackPhysicsQueries for EcsTrackPhysicsQueries {
    fn ground_at(&self, position: Vec3) -> GroundContact {
        self.roads
            .iter()
            .find(|zone| zone.contains(position))
            .map(|zone| GroundContact {
                source: GroundSource::Road,
                surface: zone.surface,
            })
            .unwrap_or(GroundContact {
                source: GroundSource::OffTrack,
                surface: SurfaceKind::Grass,
            })
    }

    fn cast_car_shape(&self, position: Vec3, yaw: f32, _velocity: Vec3) -> Option<CarHit> {
        self.rails
            .iter()
            .filter_map(|rail| rail.collide_car(position, yaw))
            .max_by(|a, b| a.penetration.total_cmp(&b.penetration))
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
            return GroundContact {
                source: GroundSource::OffTrack,
                surface: SurfaceKind::Grass,
            };
        };

        self.roads
            .iter()
            .find(|(entity, _)| *entity == hit.entity)
            .map(|(_, road)| GroundContact {
                source: GroundSource::Road,
                surface: road.surface,
            })
            .unwrap_or(GroundContact {
                source: GroundSource::OffTrack,
                surface: SurfaceKind::Grass,
            })
    }

    fn cast_car_shape(&self, position: Vec3, yaw: f32, _velocity: Vec3) -> Option<CarHit> {
        let shape = car_collider();
        let rotation = Quat::from_rotation_y(yaw);
        let filter = rail_query_filter();
        let intersections = self
            .spatial_query
            .shape_intersections(&shape, position, rotation, &filter);

        self.rails
            .iter()
            .filter(|(entity, _)| intersections.contains(entity))
            .filter_map(|(_, rail)| rail.collide_car(position, yaw))
            .max_by(|a, b| a.penetration.total_cmp(&b.penetration))
    }
}

impl RoadColliderSample {
    fn contains(self, position: Vec3) -> bool {
        self.bounds.contains_xz(position)
    }
}

impl RailColliderSample {
    fn collide_car(self, position: Vec3, _yaw: f32) -> Option<CarHit> {
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

pub fn rail_collider(width: f32, height: f32, length: f32) -> Collider {
    Collider::cuboid(width, height, length)
}

pub fn road_collider(width: f32, height: f32, length: f32) -> Collider {
    Collider::cuboid(width, height, length)
}

pub fn static_rigid_body() -> RigidBody {
    RigidBody::Static
}

pub fn road_collision_layers() -> CollisionLayers {
    CollisionLayers::new(TRACK_ROAD_LAYER, LayerMask::ALL)
}

pub fn rail_collision_layers() -> CollisionLayers {
    CollisionLayers::new(TRACK_RAIL_LAYER, LayerMask::ALL)
}

fn road_query_filter() -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask(TRACK_ROAD_LAYER)
}

fn rail_query_filter() -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask(TRACK_RAIL_LAYER)
}

fn car_collider() -> Collider {
    Collider::cuboid(
        CAR_COLLISION_LATERAL_HALF_EXTENT * 2.0,
        0.3,
        CAR_COLLISION_LONGITUDINAL_HALF_EXTENT * 2.0,
    )
}

fn signum_or_one(value: f32) -> f32 {
    if value >= 0.0 { 1.0 } else { -1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial::Pose2;

    #[test]
    fn cast_car_shape_returns_deepest_rail_hit() {
        let queries = EcsTrackPhysicsQueries::from_rails(vec![
            RailColliderSample {
                bounds: OrientedRect::new(Pose2::new(Vec2::ZERO, 0.0), Vec2::ZERO),
            },
            RailColliderSample {
                bounds: OrientedRect::new(
                    Pose2::new(Vec2::new(0.5, 0.0), 0.0),
                    Vec2::new(1.0, 0.0),
                ),
            },
        ]);

        let hit = queries
            .cast_car_shape(Vec3::ZERO, 0.0, Vec3::ZERO)
            .expect("car overlaps both rails");

        assert!((hit.penetration - 1.48).abs() < f32::EPSILON);
    }
}
