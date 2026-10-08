# Voxel cars

The first car is an original, boxy Golf-inspired AWD hatchback. Its editable source is `assets/cars/rally_hatchback.vox`; the game uses the generated GLB and native Godot scenes. No second car or runtime voxel library is included.

## Source and export

The source contains 13 named parts: Body, four wheels, left/right tail lamps, brake lamps, reverse lamps and headlamps. Palette NOTE labels identify Paint, Glass, Rubber, Trim, Metal, TailLamp, BrakeLamp, ReverseLamp, Headlamp and Plate. Windows are opaque dark glazing. Paint colors export as neutral luminance shades, multiplied by player color, so source palette hues cannot tint another driver's paint. Every animated or illuminated part is separate in the source.

`tools/voxel_car.py` uses Python's standard library to read static MagicaVoxel 150/200 scene graphs, including nested translations and voxel rotations. It validates required part names and palette roles, removes interior faces and greedily merges adjacent faces of matching color. The initial 49,060 voxels become 607 quads / 1,214 triangles. Constant palette colors become material albedo, while paint shades use vertex colors, avoiding Godot's constant-vertex-color import optimization.

```sh
# After editing the VOX source, rebuild its GLB and visual scene.
python3 tools/voxel_car.py assets/cars/rally_hatchback.vox

# Verify the committed generated assets still match the source.
python3 tools/voxel_car.py assets/cars/rally_hatchback.vox --check
python3 -m unittest tools.test_voxel_car
```

`--author-hatchback` recreates the initial source and exports it; it overwrites source edits, so ordinary rebuilds must omit that flag. Generated GLB/visual scenes should not be edited by hand. The VOX source, generated assets and GLB import settings are committed together. Godot imports the GLB normally; the Python tool never runs during gameplay.

The lattice uses 5 cm voxels. Export rotates VOX Z-up coordinates into Godot +Y-up, -Z-forward coordinates. Driver-left is -X. Transforms use positive unit scale, with no runtime model flip or wheel splitting. Wheel mesh origins are centred on their hubs. The hatchback has a 2.45 m wheelbase, 1.60 m track, 0.325 m tire radius and 0.20 m tire width.

## Car interface

Each car is an inherited `base_car.tscn` scene with its own GEVP tuning, primitive/convex chassis collision and a child named Visual using `CarVisual`. The base contains shared rigid-body configuration and four GEVP suspension rays. The hatchback adds two box colliders for the lower body and cabin, excluding tires and mirrors. Rendering meshes never become dynamic concave collision.

| Binding | Contract |
| --- | --- |
| `wheels` | Four distinct MeshInstance3D pivots in FL, FR, RL, RR order; local hub positions define mounts at suspension rest |
| `wheel_radii`, `wheel_widths` | Four positive values in metres, exported from source bounds; each axle has matching left/right dimensions |
| `tail_lamps`, `brake_lamps`, `reverse_lamps` | Explicit distinct lamp mesh references with StandardMaterial3D materials |
| Paint | StandardMaterial3D with the semantic resource name Paint; other materials are untouched |
| Chassis | Enabled CollisionShape3D children directly on the rigid body, using primitives or convex shapes |

`RallyCar.prepare()` validates bindings before GEVP initializes. It moves each existing wheel mesh under its suspension ray without baking new geometry. GEVP owns steering, spring travel and spin. Preparation works before tree entry and computes spawn clearance from every tire at full suspension extension. `RallyCar.configure(color)` sets paint independently of handling and preserves existing lamp state when called again. Scene contract errors return to setup through `Game.setup_error`.

GEVP's `front_torque_split` is the drivetrain source: 0 selects RWD, 1 selects FWD, and values between select AWD. The hatchback uses 0.5, fixed split, automatic gears and the existing 1400 kg / 380 Nm baseline. Mass is configured once through `vehicle_mass`; GEVP applies it to the rigid body. Wheel dimensions are owned by the visual asset rather than duplicated in tuning. Convert width to millimetres only at the GEVP boundary.

To add a later car, author its labelled VOX, run the exporter, create an inherited base-car scene with its collision and tuning, then assign it to `Game.car_scene`. Shared driving, terrain, camera, input and session scripts need no model-specific branches. The current interface targets four-wheeled cars with front steering and a rear handbrake; different axle counts or steering layouts would need an explicit extension.

## Paint and lamps

`Game.player_color(index)` deterministically distributes all 16 driver colors around the hue wheel. Spawning, retry, recovery and handoff use the selected `Game.car_scene` and current driver's color. Each instance duplicates only mutable paint and lamp materials; imported mesh resources remain shared.

Tail lamps emit dim red continuously. Service brake emission follows GEVP's smoothed `brake_amount`, and white reverse emission follows `current_gear == -1`. The spawn parking handbrake does not activate brake lamps. Pause and finish preserve the displayed states. Lamps are emissive surfaces, with no projected headlights, turn signals, damage or bloom dependency in this pass.

## Verification

```sh
godot --headless --path . --fixed-fps 120 --script scripts/smoke.gd -- --car-assets-only
godot --headless --path . --fixed-fps 120 --script scripts/smoke.gd
godot --path . --script tools/car_asset_preview.gd
godot --path . --script tools/vehicle_preview.gd
```

Asset tests cover export reproducibility, labels, wheel axes/dimensions, greedy-mesh area and semantic palette edits. Native checks cover material isolation, actual brake/gear lamp states, spin/suspension/steering, invalid rigs, and correct driven axles plus acceleration for RWD, AWD and FWD using the same model. Existing tests cover complete stages, grass, jump/landing, rollover and session lifecycle. Renderer previews save front, rear, side and lamp states to `/tmp/opentrack-car-*.png`, and actual driving frames to `/tmp/opentrack-vehicle-*.png`.

The full native suite and final vehicle checks passed with zero failures, including 42 generation/replay cases and complete 500/1200/5000 m traversals. Five Python pipeline tests passed. A packed RWD configuration of the same hatchback verified scene replacement through setup, retry, recovery and hotseat. Metal previews confirmed isolated paint, glazing, lamp states and actual driving/steering/flight; the jump preview waited for native landing contact and completed with zero recoveries. Final editor import completed without script errors.
