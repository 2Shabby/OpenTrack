# Native Godot rally architecture

The live game uses GDScript, Godot 4.7.2 and built-in Jolt at 120 physics ticks/s. Godot Easy Vehicle Physics (GEVP) is the only vehicle simulation. No gameplay C++, GDExtension, separate physics world, compatibility adapter or alternate handling mode is required.

## Ownership

- `rally_generator.gd` builds a seeded native stage Resource with distance-based straight/corner features and shared cross-sections.
- `terrain_builder.gd` drapes those cross-sections over constrained native noise and stamps a shared terrain heightfield. The route has no splines or banking.
- `rally_stage.gd` derives mesh sections, distance projection, pitched spawn and swept finish gate, and pacenotes from that Resource.
- `road_shoulders.gd` joins the road's exact cross-section edges to terrain triangles with closed grass strips, including the start and finish borders.
- `track_geometry.gd` builds native road, shoulder and green terrain bodies using identical rendering/collision triangles. Every body has exactly one recognized surface group. There are no rails.
- `addons/gevp/scripts/vehicle.gd` and `wheel.gd` own suspension, brush tire forces, wheel rotation, steering, AWD drivetrain, automatic transmission and force application. Jolt integrates the rigid body and resolves chassis collisions.
- `car.gd` validates the shared car rig and binds game inputs, material profiles, exported wheel geometry, spawn and telemetry to the library. It never anchors height, projects velocity or sets body yaw during driving.
- `car_visual.gd` binds four authored wheel pivots, instance paint and rear lamps. GEVP controls suspension, steering and spin; lamp state follows actual braking and gear. Local transform composition works before scene entry.
- `base_car.tscn` supplies the reusable vehicle rig. Per-car inherited scenes supply GEVP tuning, convex/primitive chassis collision and a conforming visual scene. `Game.car_scene` selects the model and hotseat supplies a stable driver color.
- `tools/voxel_car.py` exports labelled VOX sources into GLB/visual scenes offline. The original hatchback replaces the FBX model and all SportsCar-specific repairs.
- `world.gd` owns timing, progress, recovery, retry, hotseat bests and camera updates. The HUD consumes this state.
- Setup/pause share native menu controls. Scene transitions clear tree pause and detach the old world before deletion.

## Physics and session lifecycle

The stage points forward along local +Z; the vehicle/model point along local -Z. Spawn applies a proper 180-degree rotation and measured full-extension clearance before adding the car to the tree. Wheel rays exclude the chassis and query road layer 1 and terrain layer 2. The chassis uses layer 4, mask 3 and continuous collision detection.

SurfaceProfile Resources contain the GEVP tire parameters and the rendering material. Their `Road`, `Dirt` and `Grass` keys populate every library dictionary before initialization. Grass uses low grip and elevated native rolling resistance, without a speed clamp or additional drag solver.

Airborne motion preserves gravity, pitch, roll and angular momentum. Stability/upright torque assistance is disabled. Zero wheel contacts is an airborne state, never a recovery trigger. Recovery recreates the car only outside the finite heightfield, below terrain by 30 m, or with nonfinite body state. The camera uses gravity-up during flight/rollover.

Pause uses SceneTree.paused, with an always-processing pause menu/input handler. Retry, R and driver handoff recreate the complete car, resetting gearbox, engine, wheels and suspension history while preserving the stage and session bests. Held driving/steering inputs must all be released before controls resume. The native handbrake holds the spawn slope until the first accepted throttle/brake input starts timing. The swept forward finish gate accepts heights -1.5 to 20 m in its pitched frame, rejects reverse/off-width crossings and freezes the chassis and wheel visuals after recording once.

## Dependency maintenance

GEVP is vendored at `c392257f54f6ca537dc10bc5badad0c060f18982`. `addons/gevp/UPSTREAM.txt` records local wheel fixes for initial position/spring history, body-relative force offsets and rolling drag separated from drivetrain reaction torque. The MIT license and upstream README are retained. Demo controllers, scenes and effects are excluded.

See [VehicleHandling.md](VehicleHandling.md) for tuning and verification, [VoxelCars.md](VoxelCars.md) for the asset contract, and [RallyStages.md](RallyStages.md) for generation. Historical Rust handling/collision audits do not describe the live implementation.
