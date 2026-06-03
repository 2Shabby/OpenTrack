use std::time::Duration;

use avian3d::character_controller::move_and_slide::{
    MoveAndSlide, MoveAndSlideConfig, MoveAndSlideHitResponse,
};
use avian3d::math::AdjustPrecision as _;
use bevy::prelude::*;

use super::components::{RoadCollider, vehicle_collider};
use super::layers::{rail_query_filter_excluding, road_query_filter};
use crate::geometry::rotation_from_yaw_and_up;
use crate::surface::SurfaceKind;

const GROUND_RAY_START_HEIGHT: f32 = 3.0;
const GROUND_RAY_DISTANCE: f32 = 8.0;
const CAR_COLLISION_SKIN_WIDTH: f32 = 0.035;
const CAR_MOVE_AND_SLIDE_ITERATIONS: usize = 5;
const CAR_DEPENETRATION_ITERATIONS: usize = 5;
const MAX_CAR_TRANSLATION_PER_SLICE: f32 = 0.75;
const MAX_CAR_YAW_PER_SLICE: f32 = 0.08;
const MAX_CAR_COLLISION_SLICES: usize = 8;
const YAW_LIMIT_ITERATIONS: usize = 8;
const TRANSLATION_LIMIT_ITERATIONS: usize = 8;
const CHASSIS_SCRAPE_TANGENT_RETENTION: f32 = 0.985;
const CHASSIS_BLOCKED_TANGENT_RETENTION: f32 = 0.58;
const CHASSIS_SCRAPE_YAW_RETENTION: f32 = 0.62;
const CHASSIS_DEPENETRATION_YAW_RETENTION: f32 = 0.25;
const BLOCKED_SPEED_THRESHOLD: f32 = 0.25;

#[derive(Clone, Copy, Debug, Default)]
pub struct CarCollisionDebug {
    pub requested_translation_delta: Vec3,
    pub accepted_translation_delta: Vec3,
    pub requested_yaw_delta: f32,
    pub accepted_yaw_delta: f32,
    pub hit_count: u8,
    pub last_hit_normal: Vec3,
    pub depenetration: Vec3,
    pub yaw_limited: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollisionState {
    Clear,
    Scraping,
    Depenetrated,
    Blocked,
}

impl CollisionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Scraping => "scrape",
            Self::Depenetrated => "depenetrated",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CarPose {
    pub translation: Vec3,
    pub yaw: f32,
    pub up: Vec3,
}

impl CarPose {
    fn rotation(self) -> Quat {
        rotation_from_yaw_and_up(self.yaw, self.up)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CarCollisionResolution {
    pub pose: CarPose,
    pub velocity: Vec3,
    pub state: CollisionState,
    pub debug: CarCollisionDebug,
}

pub trait TrackPhysicsQueries {
    fn resolve_car_pose(
        &self,
        start: CarPose,
        requested: CarPose,
        velocity: Vec3,
        delta_time: Duration,
        car_entity: Entity,
    ) -> CarCollisionResolution;
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundContact {
    pub source: GroundSource,
    pub surface: SurfaceKind,
    pub boost_direction: Option<Vec3>,
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct RoadColliderSample {
    surface: SurfaceKind,
    boost_direction: Option<Vec3>,
}

pub struct AvianTrackPhysicsQueries<'a, 'w, 's> {
    move_and_slide: &'a MoveAndSlide<'w, 's>,
    roads: Vec<(Entity, RoadColliderSample)>,
}

impl<'a, 'w, 's> AvianTrackPhysicsQueries<'a, 'w, 's> {
    pub fn new(
        move_and_slide: &'a MoveAndSlide<'w, 's>,
        roads: &Query<(Entity, &RoadCollider)>,
    ) -> Self {
        Self {
            move_and_slide,
            roads: roads
                .iter()
                .map(|(entity, road)| {
                    (
                        entity,
                        RoadColliderSample {
                            surface: road.surface,
                            boost_direction: road.boost_direction,
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
        let Some(hit) = self.move_and_slide.spatial_query.cast_ray(
            origin,
            Dir3::NEG_Y,
            GROUND_RAY_DISTANCE,
            false,
            &filter,
        ) else {
            return road_miss_contact(position);
        };

        self.roads
            .iter()
            .find(|(entity, _)| *entity == hit.entity)
            .map(|(_, road)| GroundContact {
                source: GroundSource::Road,
                surface: road.surface,
                boost_direction: road.boost_direction,
                point: origin + Vec3::NEG_Y * hit.distance,
                normal: hit.normal.adjust_precision().normalize_or(Vec3::Y),
            })
            .unwrap_or_else(|| road_miss_contact(position))
    }

    fn resolve_car_pose(
        &self,
        start: CarPose,
        requested: CarPose,
        velocity: Vec3,
        delta_time: Duration,
        car_entity: Entity,
    ) -> CarCollisionResolution {
        let requested_translation_delta = requested.translation - start.translation;
        let requested_yaw_delta = requested.yaw - start.yaw;
        let slice_count =
            car_collision_slice_count(requested_translation_delta, requested_yaw_delta);
        let slice_duration = duration_div(delta_time, slice_count);
        let yaw_step = requested_yaw_delta / slice_count as f32;
        let shape = vehicle_collider();
        let config = car_move_and_slide_config();
        let filter = rail_query_filter_excluding(car_entity);

        let mut pose = start;
        let mut resolved_velocity = velocity;
        let mut debug = CarCollisionDebug {
            requested_translation_delta,
            requested_yaw_delta,
            ..default()
        };
        let mut started_intersecting = false;

        for _ in 0..slice_count {
            let slice_start = pose;
            let candidate_yaw = pose.yaw + yaw_step;
            let candidate_up = pose
                .up
                .lerp(requested.up, 1.0 / slice_count as f32)
                .normalize_or(pose.up);
            let was_intersecting = self.car_intersects_rail(
                slice_start.translation,
                candidate_yaw,
                candidate_up,
                car_entity,
            );
            started_intersecting |= was_intersecting;

            let mut slice_hit_count = 0u8;
            let output = self.move_and_slide.move_and_slide(
                &shape,
                slice_start.translation,
                rotation_from_yaw_and_up(candidate_yaw, candidate_up),
                resolved_velocity,
                slice_duration,
                &config,
                &filter,
                |hit| {
                    slice_hit_count = slice_hit_count.saturating_add(1);
                    debug.hit_count = debug.hit_count.saturating_add(1);
                    debug.last_hit_normal = hit.normal.adjust_precision();
                    MoveAndSlideHitResponse::Accept
                },
            );

            let mut accepted_translation = output.position;
            resolved_velocity = output.projected_velocity;
            let mut accepted_yaw = chassis_contact_yaw_response(
                slice_start.yaw,
                candidate_yaw,
                slice_hit_count,
                was_intersecting,
            );

            if self.car_intersects_rail(
                accepted_translation,
                accepted_yaw,
                candidate_up,
                car_entity,
            ) {
                debug.yaw_limited = true;
                accepted_yaw = self.largest_clear_yaw(
                    accepted_translation,
                    slice_start.yaw,
                    candidate_yaw,
                    candidate_up,
                    car_entity,
                );

                if self.car_intersects_rail(
                    accepted_translation,
                    accepted_yaw,
                    candidate_up,
                    car_entity,
                ) {
                    accepted_translation = self.largest_clear_translation(
                        slice_start.translation,
                        accepted_translation,
                        accepted_yaw,
                        candidate_up,
                        car_entity,
                    );
                }

                if self.car_intersects_rail(
                    accepted_translation,
                    accepted_yaw,
                    candidate_up,
                    car_entity,
                ) {
                    accepted_yaw = slice_start.yaw;
                    accepted_translation = self.largest_clear_translation(
                        slice_start.translation,
                        accepted_translation,
                        accepted_yaw,
                        candidate_up,
                        car_entity,
                    );
                    resolved_velocity =
                        chassis_blocked_velocity_response(resolved_velocity, debug.last_hit_normal);
                    pose = CarPose {
                        translation: accepted_translation,
                        yaw: accepted_yaw,
                        up: candidate_up,
                    };
                    debug.depenetration += pose.translation - slice_start.translation;
                }
            }

            pose = CarPose {
                translation: accepted_translation,
                yaw: accepted_yaw,
                up: candidate_up,
            };

            if was_intersecting {
                debug.depenetration += pose.translation - slice_start.translation;
            }

            if slice_hit_count > 0 {
                resolved_velocity =
                    chassis_scrape_velocity_response(resolved_velocity, debug.last_hit_normal);
            }
        }

        let mut resolution = finalized_resolution(start, requested, pose, resolved_velocity, debug);
        if resolution.state == CollisionState::Clear && started_intersecting {
            resolution.state = CollisionState::Depenetrated;
        }
        resolution
    }
}

impl AvianTrackPhysicsQueries<'_, '_, '_> {
    fn car_intersects_rail(&self, position: Vec3, yaw: f32, up: Vec3, car_entity: Entity) -> bool {
        let shape = vehicle_collider();
        let rotation = CarPose {
            translation: position,
            yaw,
            up,
        }
        .rotation();
        let filter = rail_query_filter_excluding(car_entity);

        !self
            .move_and_slide
            .spatial_query
            .shape_intersections(&shape, position, rotation, &filter)
            .is_empty()
    }

    fn largest_clear_yaw(
        &self,
        position: Vec3,
        clear_yaw: f32,
        blocked_yaw: f32,
        up: Vec3,
        car_entity: Entity,
    ) -> f32 {
        if !self.car_intersects_rail(position, blocked_yaw, up, car_entity) {
            return blocked_yaw;
        }
        if self.car_intersects_rail(position, clear_yaw, up, car_entity) {
            return clear_yaw;
        }

        let mut clear = clear_yaw;
        let mut blocked = blocked_yaw;
        for _ in 0..YAW_LIMIT_ITERATIONS {
            let midpoint = clear + (blocked - clear) * 0.5;
            if self.car_intersects_rail(position, midpoint, up, car_entity) {
                blocked = midpoint;
            } else {
                clear = midpoint;
            }
        }
        clear
    }

    fn largest_clear_translation(
        &self,
        clear_position: Vec3,
        blocked_position: Vec3,
        yaw: f32,
        up: Vec3,
        car_entity: Entity,
    ) -> Vec3 {
        if !self.car_intersects_rail(blocked_position, yaw, up, car_entity) {
            return blocked_position;
        }
        if self.car_intersects_rail(clear_position, yaw, up, car_entity) {
            return clear_position;
        }

        let mut clear = clear_position;
        let mut blocked = blocked_position;
        for _ in 0..TRANSLATION_LIMIT_ITERATIONS {
            let midpoint = clear.lerp(blocked, 0.5);
            if self.car_intersects_rail(midpoint, yaw, up, car_entity) {
                blocked = midpoint;
            } else {
                clear = midpoint;
            }
        }
        clear
    }
}

fn road_miss_contact(position: Vec3) -> GroundContact {
    GroundContact {
        source: GroundSource::OffTrack,
        surface: SurfaceKind::Asphalt,
        boost_direction: None,
        point: position,
        normal: Vec3::Y,
    }
}

fn car_move_and_slide_config() -> MoveAndSlideConfig {
    MoveAndSlideConfig {
        skin_width: CAR_COLLISION_SKIN_WIDTH,
        move_and_slide_iterations: CAR_MOVE_AND_SLIDE_ITERATIONS,
        depenetration_iterations: CAR_DEPENETRATION_ITERATIONS,
        ..default()
    }
}

fn car_collision_slice_count(translation_delta: Vec3, yaw_delta: f32) -> usize {
    let translation_slices =
        (translation_delta.length() / MAX_CAR_TRANSLATION_PER_SLICE).ceil() as usize;
    let yaw_slices = (yaw_delta.abs() / MAX_CAR_YAW_PER_SLICE).ceil() as usize;
    translation_slices
        .max(yaw_slices)
        .clamp(1, MAX_CAR_COLLISION_SLICES)
}

fn duration_div(duration: Duration, divisor: usize) -> Duration {
    Duration::from_secs_f32(duration.as_secs_f32() / divisor.max(1) as f32)
}

fn finalized_resolution(
    start: CarPose,
    requested: CarPose,
    pose: CarPose,
    velocity: Vec3,
    mut debug: CarCollisionDebug,
) -> CarCollisionResolution {
    debug.requested_translation_delta = requested.translation - start.translation;
    debug.accepted_translation_delta = pose.translation - start.translation;
    debug.requested_yaw_delta = requested.yaw - start.yaw;
    debug.accepted_yaw_delta = pose.yaw - start.yaw;

    let state = collision_state_from_debug(&debug, velocity);

    CarCollisionResolution {
        pose,
        velocity,
        state,
        debug,
    }
}

fn collision_state_from_debug(debug: &CarCollisionDebug, velocity: Vec3) -> CollisionState {
    if debug.hit_count == 0 && !debug.yaw_limited && debug.depenetration.length_squared() == 0.0 {
        CollisionState::Clear
    } else if velocity.length() <= BLOCKED_SPEED_THRESHOLD || movement_collapsed(debug) {
        CollisionState::Blocked
    } else if debug.depenetration.length_squared() > 0.0 {
        CollisionState::Depenetrated
    } else {
        CollisionState::Scraping
    }
}

fn movement_collapsed(debug: &CarCollisionDebug) -> bool {
    let requested = debug.requested_translation_delta.length();
    requested > 0.1 && debug.accepted_translation_delta.length() < requested * 0.1
}

fn chassis_contact_yaw_response(
    start_yaw: f32,
    requested_yaw: f32,
    hit_count: u8,
    was_intersecting: bool,
) -> f32 {
    if hit_count == 0 {
        return requested_yaw;
    }

    let retention = if was_intersecting {
        CHASSIS_DEPENETRATION_YAW_RETENTION
    } else {
        CHASSIS_SCRAPE_YAW_RETENTION
    };
    start_yaw + (requested_yaw - start_yaw) * retention
}

fn chassis_scrape_velocity_response(velocity: Vec3, normal: Vec3) -> Vec3 {
    chassis_contact_velocity_response(velocity, normal, CHASSIS_SCRAPE_TANGENT_RETENTION)
}

fn chassis_blocked_velocity_response(velocity: Vec3, normal: Vec3) -> Vec3 {
    chassis_contact_velocity_response(velocity, normal, CHASSIS_BLOCKED_TANGENT_RETENTION)
}

fn chassis_contact_velocity_response(velocity: Vec3, normal: Vec3, tangent_retention: f32) -> Vec3 {
    if normal.length_squared() <= f32::EPSILON {
        return velocity * tangent_retention;
    }

    let normal = normal.normalize();
    let normal_speed = velocity.dot(normal);
    let normal_velocity = normal * normal_speed;
    let tangent_velocity = velocity - normal_velocity;
    let retained_normal = if normal_speed < 0.0 {
        Vec3::ZERO
    } else {
        normal_velocity
    };
    tangent_velocity * tangent_retention + retained_normal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_slice_count_scales_with_translation_and_yaw() {
        assert_eq!(car_collision_slice_count(Vec3::ZERO, 0.0), 1);
        assert!(car_collision_slice_count(Vec3::Z * 3.0, 0.0) > 1);
        assert!(car_collision_slice_count(Vec3::ZERO, 0.3) > 1);
    }

    #[test]
    fn scrape_velocity_preserves_tangential_direction() {
        let velocity = Vec3::new(-3.0, 0.0, 20.0);
        let scraped = chassis_scrape_velocity_response(velocity, Vec3::X);

        assert!(scraped.x.abs() < f32::EPSILON);
        assert!(scraped.z > velocity.z * 0.98);
    }

    #[test]
    fn blocked_velocity_removes_inward_component() {
        let velocity = Vec3::new(-3.0, 0.0, 4.0);
        let normal = Vec3::X;
        let blocked = chassis_blocked_velocity_response(velocity, normal);

        assert!(blocked.x.abs() < f32::EPSILON);
        assert!(blocked.z > 0.0);
        assert!(blocked.z < velocity.z);
    }

    #[test]
    fn chassis_contact_yaw_damps_scrape_rotation_without_reversing_it() {
        let yaw = chassis_contact_yaw_response(1.0, 1.5, 1, false);

        assert!(yaw > 1.0);
        assert!(yaw < 1.5);
    }

    #[test]
    fn chassis_contact_yaw_damps_depenetration_more_than_scrape() {
        let scrape = chassis_contact_yaw_response(1.0, 1.5, 1, false);
        let depenetrating = chassis_contact_yaw_response(1.0, 1.5, 1, true);

        assert!(depenetrating < scrape);
    }

    #[test]
    fn old_pose_overlap_state_is_not_part_of_collision_labels() {
        let labels = [
            CollisionState::Clear.label(),
            CollisionState::Scraping.label(),
            CollisionState::Depenetrated.label(),
            CollisionState::Blocked.label(),
        ];

        assert!(!labels.contains(&"overlap"));
    }

    #[test]
    fn yaw_limited_motion_can_still_be_scraping() {
        let debug = CarCollisionDebug {
            requested_translation_delta: Vec3::Z * 4.0,
            accepted_translation_delta: Vec3::Z * 3.4,
            yaw_limited: true,
            ..default()
        };

        assert_eq!(
            collision_state_from_debug(&debug, Vec3::Z * 12.0),
            CollisionState::Scraping
        );
    }
}
