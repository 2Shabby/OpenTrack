# Voxel cars

The default car is an original, boxy Golf-inspired AWD hatchback. A slower, rear-driven auto rickshaw is also selectable in setup. Their editable sources are `assets/cars/rally_hatchback.vox` and `assets/cars/auto_rickshaw.vox`; the game uses generated GLBs and native Godot scenes. No runtime voxel library is included.

## Source and export

The hatchback source contains 16 named parts: separate paint, glass, trim and plate, four wheels, and left/right tail, brake, reverse and headlamps. Each scene part declares its material `_role`; palette slots contain colors only. The root declares `_opentrack=car-v2` and `_voxel_size=0.1`. Missing metadata, incorrect bindings and noncanonical palettes are authoring errors.

`tools/voxel_car.py` reads static MagicaVoxel 150/200 scene graphs with translations and voxel rotations. Interior faces are removed and matching exposed faces merge greedily. The hatchback’s 6,328 source voxels export as 486 quads / 972 triangles. Paint has one palette albedo; there is no grayscale multiplication or RGB inference.

```sh
# After editing the VOX source, rebuild its GLB and visual scene.
python3 tools/voxel_car.py assets/cars/rally_hatchback.vox

# Verify the committed generated assets still match the source.
python3 tools/voxel_car.py assets/cars/rally_hatchback.vox --check

# Author the initial three-wheeler, then rebuild/check normally after edits.
python3 tools/voxel_car.py --author-rickshaw
python3 tools/voxel_car.py assets/cars/auto_rickshaw.vox
python3 tools/voxel_car.py assets/cars/auto_rickshaw.vox --check
```

`--author-hatchback` and `--author-rickshaw` recreate their initial sources and export them; they overwrite source edits, so ordinary rebuilds must omit these flags. Generated GLB/visual scenes should not be edited by hand. The VOX source, generated assets and GLB import settings belong together. Godot imports the GLB normally; the Python tool never runs during gameplay.

The lattice uses 10 cm voxels. Export rotates VOX Z-up coordinates into Godot +Y-up, -Z-forward coordinates. Driver-left is -X. Transforms use positive unit scale, with no runtime model flip or wheel splitting. Wheel mesh origins are centred on their hubs. The hatchback has a 2.40 m wheelbase, 1.60 m track, 0.35 m tire radius and 0.20 m tire width.

## Car interface

`base_vehicle.tscn` owns shared rigid-body configuration and the controller. `base_car.tscn` extends it with four GEVP suspension rays; `base_trike.tscn` extends it with a centered front ray and two rear rays. Each authored vehicle inherits its layout, adds its own GEVP tuning, primitive/convex chassis collision and a child named Visual using `CarVisual`. The hatchback adds two box colliders for the lower body and cabin, excluding tires and mirrors. The rickshaw adds lower-body, nose and canopy boxes, leaving the open passenger cabin clear. Rendering meshes never become dynamic concave collision.

| Binding | Contract |
| --- | --- |
| `wheels` | Distinct MeshInstance3D pivots in FL, FR, RL, RR order or Front, RL, RR order; local hub positions define mounts at suspension rest |
| `wheel_radii`, `wheel_widths` | One positive value per wheel in metres, exported from source bounds; paired axles have matching left/right dimensions |
| `tail_lamps`, `brake_lamps`, `reverse_lamps` | Explicit distinct lamp mesh references with StandardMaterial3D materials |
| Paint | StandardMaterial3D with the semantic resource name Paint; other materials are untouched |
| Chassis | Enabled CollisionShape3D children directly on the rigid body, using primitives or convex shapes |

`RallyCar.prepare()` validates bindings before GEVP initializes. It moves each existing wheel mesh under its suspension ray without baking new geometry. GEVP owns steering, spring travel and spin. Preparation works before tree entry and computes spawn clearance from every tire at full suspension extension. `RallyCar.configure(palette_index)` sets paint independently of handling and preserves existing lamp state when called again. Scene contract errors return to setup through `Game.setup_error`.

GEVP's `front_torque_split` is the drivetrain source: 0 selects RWD, 1 selects FWD, and values between select AWD. The hatchback uses 0.5, fixed split, automatic gears and the existing 1400 kg / 380 Nm baseline. Mass is configured once through `vehicle_mass`; GEVP applies it to the rigid body. Wheel dimensions are owned by the visual asset rather than duplicated in tuning. Convert width to millimetres only at the GEVP boundary.

To add a vehicle, author its labelled VOX, run the exporter, create a scene inheriting `base_car.tscn` or `base_trike.tscn` with its collision and tuning, and add its name/scene to `Game.VEHICLE_NAMES` and `Game.VEHICLE_SCENES`. `Game.car_scene` holds the selected scene; the hatchback remains the default. Shared driving, terrain, camera, input and session scripts need no model-specific branches. Supported layouts are four wheels or one centered front wheel plus a rear pair, with front steering and a rear handbrake. Other layouts need an explicit extension.

The rickshaw source has 2,202 voxels in 15 named parts and exports 290 quads / 580 triangles. It uses a 2.0 m wheelbase, 1.2 m rear track, 0.30 m tire radius and 0.20 m tire width. `WheelFront`, `WheelRL` and `WheelRR` are separate hub-centered meshes for suspension, steering and spin. Body paint, glass, trim, plate and rear lamp roles follow the same contract as the hatchback. The single front suspension carries the complete front axle weight and has no opposite wheel, antiroll, camber, toe or Ackermann correction. Brake and drivetrain torque use one wheel at the front and a differential at the rear.

## Paint and lamps

`Game.player_paint_index(index)` selects one of 16 fixed ENDE​SGA-32 colors. `assets/endesga-32.pal` is the sole palette source; run `python3 tools/palette.py` after editing it. Spawning, retry, recovery and handoff use the selected `Game.car_scene` and current driver's color. Each instance duplicates only mutable paint and lamp materials; imported mesh resources remain shared.

Tail lamps emit dim red continuously. Service brake emission follows GEVP's smoothed `brake_amount`, and white reverse emission follows `current_gear == -1`. The spawn parking handbrake does not activate brake lamps. Pause and finish preserve the displayed states. Lamps are emissive surfaces, with no projected headlights, turn signals, damage or bloom dependency in this pass.

## Manual verification

```sh
python3 tools/palette.py --check
python3 tools/voxel_car.py assets/cars/rally_hatchback.vox --check
godot --path . --script tools/car_asset_preview.gd
godot --path . --script tools/vehicle_preview.gd

# Preview the optional vehicle without changing the default selection.
godot --path . --script tools/car_asset_preview.gd -- --vehicle=res://scenes/cars/auto_rickshaw.tscn
godot --path . --script tools/vehicle_preview.gd -- --stage=wales-slate-mountain-17119 --vehicle=res://scenes/cars/auto_rickshaw.tscn
```

Regression suites have been removed. Preview the front/rear/side views, all player paints, brake and reverse lamps, steering, wheel spin, suspension, jumps and landing. The preview saves images to `/tmp/opentrack-car-*.png` and `/tmp/opentrack-vehicle-*.png`. Inspect R/retry/recovery/hotseat and optional AWD/FWD/RWD tuning in the actual game. Exporter and rig validations remain production safeguards.
