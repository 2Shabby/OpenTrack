# Collision Polish Audit

## Goal

Rail contact should feel like a fast arcade scrape, not a rear-corner catch or snap-back recovery.

This layer is no longer the main blocker for handling work. The old collision path has been replaced; the remaining collision work is in-game verification and tuning.

## Avian Behavior Accounted For

Primary references:

* [Avian spatial queries](https://docs.rs/avian3d/latest/avian3d/spatial_query/index.html)
* [SpatialQuery shape queries](https://docs.rs/avian3d/latest/avian3d/spatial_query/struct.SpatialQuery.html)
* [MoveAndSlide](https://docs.rs/avian3d/latest/avian3d/character_controller/move_and_slide/struct.MoveAndSlide.html)
* [Collider shape notes](https://docs.rs/avian3d/latest/avian3d/collision/collider/struct.Collider.html)

Decisions already reflected in code:

* Avian is still a static track query/collision library, not the vehicle controller.
* Rail collision uses Avian `MoveAndSlide` because a single shapecast does not handle multi-plane scrape or depenetration well.
* The resolver handles pose/yaw safety; gameplay code should not call raw overlap tests and then rewind.
* Rail colliders stay path-derived capsules. Road triangle meshes are relevant to ground raycasts, not the rear-rail clipping issue.

## Current Code State

Implemented source of truth:

* `src/physics/queries.rs` owns collision resolution through `TrackPhysicsQueries::resolve_car_pose`.
* `src/driving.rs` requests a desired pose and accepts only the resolver output.
* `CarPose` carries yaw plus support up, so rail shape casts and overlap checks use the same banked orientation as the visible vehicle.
* `src/physics/components.rs` uses a centered rounded vehicle collider instead of the old rear-biased sharp cuboid.
* `src/physics/layers.rs` exposes `rail_query_filter_excluding` so the car does not collide with itself during `MoveAndSlide`.
* `src/debug.rs` shows collision telemetry: requested/accepted move, requested/accepted yaw, hit count, hit normal, yaw limiting, and depenetration length.

Removed behavior:

* no `move_car_with_collisions`
* no `last_clear_car_pose`
* no `PoseOverlap` / `SweepHit` states
* no normal-use boolean overlap rewind in `drive_car`
* no global `velocity *= 0.45` overlap penalty

Current collision states:

* `Clear`: no rail contact or correction.
* `Scraping`: rail contact with useful projected movement preserved.
* `Depenetrated`: local correction happened but motion remains viable.
* `Blocked`: projected movement collapsed or the resolver could not accept the requested pose.

## Remaining Collision Checks

Before deeper handling tuning, verify these in-game with repeatable tight-corner seeds:

* shallow rail brush preserves most speed and continues along the rail
* holding steer into the inner rail scrapes and slows, not snaps backward
* rear-corner contact does not rotate the car deeper into the rail
* `Blocked` is rare during normal driving
* yaw limiting appears as a small accepted-yaw reduction, not an abrupt stop
* the rounded collider does not visibly pass through rails

Only tune these values if the checks fail:

* `VEHICLE_COLLISION_HALF_WIDTH`
* `VEHICLE_COLLISION_HALF_LENGTH`
* `VEHICLE_COLLISION_CORNER_RADIUS`
* `CAR_COLLISION_SKIN_WIDTH`
* `MAX_CAR_TRANSLATION_PER_SLICE`
* `MAX_CAR_YAW_PER_SLICE`
* scrape and blocked speed-retention factors

## Non-Goals

* Do not reintroduce last-clear rewind.
* Do not replace rail capsules with triangle meshes.
* Do not delegate tire forces, steering, or drift behavior to Avian.
* Do not tune secondary drift until collision scrape behavior is acceptable in-game.
