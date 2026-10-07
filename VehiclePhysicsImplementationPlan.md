# Easy Vehicle Physics implementation plan

Replace the grounded car controller with Godot Easy Vehicle Physics (GEVP), using a native `RigidBody3D` and built-in Jolt. The result must support suspension, wheel unloading, airborne motion, landings, chassis collisions and rollover while preserving rally stages and hotseat timing. Complete the migration without compatibility adapters or a second handling model.

## Implementation status — 2026-10-08

The code migration is complete. The six sections below retain the implementation requirements; the live architecture is documented in [NativeGodotMigration.md](NativeGodotMigration.md) and tuning in [VehicleHandling.md](VehicleHandling.md).

- GEVP is pinned and vendored with its license; Jolt runs at 120 Hz. The library is the sole physics owner. Wheel startup history and force-offset fixes are recorded in `addons/gevp/UPSTREAM.txt`.
- The production rigid body has measured chassis/tire geometry, four independent wheel visuals, correct axle ordering, a proper spawn rotation and enabled continuous chassis collision.
- Asphalt, dirt and grass Resources supply all recognized tire dictionaries and collider groups. Grass permits re-entry and reaches 3.83 m/s after 20 s of flat-ground full throttle; no velocity cap is applied.
- Automatic AWD, natural flight/rollover, a gravity-up camera, swept airborne finish and explicit world-bound recovery are integrated. No ground-height snap or contact-loss reset remains.
- Tree pause freezes native motion and timing. R, retry and handoff recreate the complete vehicle, retain stage/bests and suppress held controls. A native handbrake holds the spawn slope until the first accepted drive input. Finish freezes chassis and wheel visuals.
- The old handling scripts, tuning Resource, spawn dictionary API and grounded-controller tests are removed. The full automated suite passed, including 42 generation/replay recipes and 500/1200/5000 m traversals. Final vehicle/session checks also passed after the slope-start and finish-visual fixes. Metal render checks covered settling, driving, steering, airborne suspension, pause and landing.

Remaining work is human handling calibration: the rendered pass was scripted, so it does not establish subjective steering/braking feel or competitive balance. Conservative generated-stage traversal stayed grounded; a separate road crest within the production grade limits verified takeoff, partial wheel unloading and landing at higher entry speed.

## Decision and evidence

Use [GEVP at commit `c392257f54f6ca537dc10bc5badad0c060f18982`](https://github.com/DAShoe1/Godot-Easy-Vehicle-Physics/tree/c392257f54f6ca537dc10bc5badad0c060f18982). Its vehicle exposes external input fields and configurable per-wheel surface behavior. The project documents Godot/Jolt support and recommends at least 120 physics ticks per second. Preserve its MIT license and source attribution when vendoring.

[Godot Advanced Vehicle](https://github.com/Dechode/Godot-Advanced-Vehicle) remains a reference for more detailed simulation. Its main branch was inspected at `e055cc37f374add4bcec4d8395888fa2f8ecc5a4`, and dev at `db1ea1d2c54d89fee52160f16d5bc5c634043134`. Its tire models, fuel, temperature and wear systems increase the integration and reset work for this game.

Both main-branch controllers passed isolated drop, acceleration, upward-impulse jump and landing checks in Godot 4.7.2/Jolt at 120 Hz. GEVP passed with stability assistance disabled. These checks establish initial compatibility; they do not establish handling quality or compatibility with the complete generated stage.

## 1. Establish one physics owner

- Vendor the required GEVP vehicle/wheel scripts and license under `addons/gevp`. Keep upstream code identifiable; record any necessary fixes separately. Exclude demo UI, controllers, cameras and effects from the production dependency.
- Set `project.godot` to 120 physics ticks/s and retain built-in Jolt. Tune at this fixed rate; the existing controller's internal substeps must disappear with it.
- Replace `scenes/cars/player_car.tscn` with the library vehicle and four suspension rays. The library owns forces, wheel spin, suspension, steering, drivetrain and body integration.
- Rewrite `scripts/car.gd` as a thin game integration script for inputs and telemetry. Never snap chassis height, project momentum onto a ground plane, assign yaw every frame, or apply another tire-force solver.

**Gate:** the production car scene falls, settles and accelerates on native road/terrain bodies with finite state and enabled chassis collision.

## 2. Build the physical car correctly

- Measure wheel centers, wheel radius and chassis dimensions from the existing SportsCar asset. Match suspension ray locations to those measurements; use a suitable chassis collider and physically reasonable center of mass.
- Split the combined rear-wheel visual into independent left/right wheels. Bind four separate wheel visuals to GEVP; remove the current synthetic compression, pitch, roll and spin updates.
- Resolve the coordinate convention explicitly: the stage uses forward `+Z`, while the inspected library scenes place the front axle toward `-Z`. Align spawn, model, steering, velocity telemetry and camera together. Verify W drives toward the first corner and A turns left on screen.
- Change spawn placement from a 5 cm ground offset to suspension/chassis clearance above the pitched road frame. Set the transform before adding the body to the scene tree so initialization starts at the correct pose.
- Give the chassis its car collision layer and road/terrain collision mask. Suspension rays must query both support layers and exclude their own vehicle.

**Gate:** all four wheels settle at plausible compression; the chassis clears the road and collides correctly during a hard landing.

## 3. Make surfaces one source of truth

- Keep `SurfaceProfile` as the shared material/surface catalog, but replace the old solver-specific fields with the GEVP parameters actually used: friction coefficient, stiffness, rolling resistance and grip ratios/assists.
- In `scripts/track_geometry.gd`, assign road and terrain bodies their recognized surface groups: `Road` for asphalt, `Dirt` and `Grass`. The inspected wheel code reads the collider's first group; keep that ordering unambiguous and every recognized key present in all parameter dictionaries.
- Populate GEVP's surface dictionaries from the catalog before vehicle initialization. Avoid a second copy of the same surface tuning in the car scene.
- Calibrate the values in the new model. Existing friction numbers and grass drag values are not interchangeable with the addon settings. Preserve a strong, progressive grass penalty and the ability to drive back onto the road using library tuning.

**Gate:** mixed road/grass wheel contacts work; surface changes produce no missing-key errors, velocity clamps or emergency resets. Measure grass speed rather than assuming the old 18 km/h behavior transfers.

## 4. Integrate airborne driving and rally controls

- Use automatic transmission and AWD as the initial rally setup. Reduce steering/countersteering assists and disable artificial airborne upright correction for the baseline. Tune suspension travel, damping, anti-roll and grip on the current sharper terrain.
- Map W/S, A/D and handbrake to the library's input fields. Preserve braking before reverse and clear held input on pause, retry and driver handoff.
- Replace contact-loss resets with explicit out-of-world/fall-below-world recovery. Zero wheel contacts means airborne; pitch, roll and angular momentum must continue through Jolt.
- Update `scripts/world.gd` to read native linear/angular velocity and wheel contact state. Remove calls to the old `step()` simulation and obsolete debug feedback fields.
- Update the chase camera to follow the agreed forward axis with a stable gravity-up reference during jumps and rollover. Keep contact loss from flipping the camera.
- Replace the narrow, height-sensitive finish check with a forward-crossing gate that includes expected airborne heights. Check previous/current positions against the gate so a fast crossing cannot skip it; reject reverse crossings and crossings outside the road width.

**Gate:** a crest jump retains forward motion, unloads the wheels, lands through suspension and continues the same run. An airborne finish records exactly once.

## 5. Preserve pause and hotseat fairness

- A `Game.paused` early return is insufficient for a live rigid body. Pause through `SceneTree.paused`; keep the vehicle/world nodes pausable and use a separate pause-input handler plus pause menu that process while paused. Clear tree pause on menu/setup transitions.
- Retry and driver handoff should recreate the car scene at the same stage spawn. Remove the previous body before introducing the new one. This resets engine, gearbox, wheel spin, suspension history and assists without a partial reset of hidden library state.
- Preserve the stage Resource and per-driver best times; reset only the current attempt's timer, progress and camera. Start timing on the first drive/brake input as before.
- At finish, clear inputs and freeze the body; zeroing linear velocity alone would still allow gravity and rotation to move it.

**Gate:** pausing in midair holds pose, velocities and time; resuming continues the trajectory. Every driver starts with identical vehicle state on the same stage.

## 6. Validate, then remove the old implementation

Retain generation, road/terrain seam, resource, pacenote and hotseat checks. Replace tests that assume planar motion, fixed height or the old tire-force equations with observable rigid-body behavior:

- Stationary settling, acceleration, braking/reverse, steering and handbrake.
- Dirt/asphalt transitions, mixed wheel surfaces, slow grass and road re-entry.
- Drop, crest takeoff, partial wheel contact, angled landing, chassis impact and rollover.
- No reset during ordinary flight; recovery beyond the world's bounds.
- Pause/resume in flight, retry, driver handoff and grounded/airborne finish crossings.
- Traversal of representative 500, 1200 and 5000 m stages at the fixed 120 Hz rate, including chunk seams and sharper crests. Record contact counts, reset count, finite body state and landing behavior.

Complete a rendered, hands-on driving pass for suspension movement, braking distances, steering feel, camera behavior and landing recovery. Isolated flat-ground probes are insufficient for this gate.

Once these checks pass, delete `scripts/vehicle_handling.gd`, `scripts/vehicle_tuning.gd`, `resources/vehicle_tuning.tres`, the old support/snap code and obsolete visual feedback. Update the architecture and handling documentation to describe the library-backed controller. The shipped game must have one vehicle simulation, one surface catalog and no legacy handling switch.
