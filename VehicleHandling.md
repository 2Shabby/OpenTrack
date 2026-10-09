# GEVP rally handling

The default car is a 1400 kg RigidBody3D driven by Godot Easy Vehicle Physics and built-in Jolt at 120 Hz. Its four suspension rays, brush tire model, automatic gearbox, 50/50 AWD and differential locking replace the removed grounded controller. Setup also offers a slower three-wheeled auto rickshaw. The game wrapper supplies inputs and telemetry; it applies no competing driving forces.

## Tuning

`scenes/cars/rally_hatchback.tscn` inherits the shared vehicle rig and holds drivetrain, steering, suspension tuning and compound box collision. The current baseline has 380 Nm peak torque, 0.22 m suspension travel, 0.78/0.80 damping ratios, modest countersteering assistance and disabled stability/upright torque assistance. The labelled VOX source exports four independent hub-centred wheel meshes with 0.35 m tire radius, 200 mm width and 2.40 m wheelbase. There is no runtime geometry baking or model-specific setup. `Game.car_scene` selects the car; GEVP's front torque split also supports FWD and RWD. See [VoxelCars.md](VoxelCars.md) for the asset contract, player paint and working rear lamps.

`scenes/cars/auto_rickshaw.tscn` inherits `base_trike.tscn`, sharing the same controller and rigid-body base as the hatchback. It has one front steering/support wheel, a rear-driven pair with an open differential, 420 kg mass, 30 Nm peak torque, 4,500 RPM redline, three forward gears (4.6 / 2.7 / 1.6) and a 5.0 final drive. Its 0.30 m tires and gearing target roughly 60 km/h on level ground, with no velocity clamp; hills can change that speed. On flat asphalt it reaches 27.80 km/h after six seconds and 59.29 km/h after thirty seconds. The lower center of mass and gentle steering make it suitable for testing, but sharp turns can still tip the physical three-wheel support triangle. The original hatchback tuning and assets are preserved.

The catalog under `resources/surfaces` is the sole tire/material source:

| Surface/group | Friction | Stiffness | Rolling multiplier | Longitudinal ratio | Lateral assist |
| --- | --- | --- | --- | --- | --- |
| Asphalt / Road | 2.20 | 6.0 | 1.0 | 0.55 | 0.02 |
| Dirt / Dirt | 1.45 | 2.4 | 2.4 | 0.42 | 0 |
| Grass / Grass | 0.85 | 1.0 | 5.0 | 0.40 | 0 |

These are library parameters, not coefficients interchangeable with the old custom solver. Four contacts independently choose their collider's surface group, including mixed road/grass contacts. Rolling resistance applies at each supported contact, separately from the brush tire force used for drivetrain reaction torque. Feeding drag back into wheel torque lets a freely rolling tire cancel that drag. Resistance is bounded by the contact's stopping impulse to avoid reversing motion near a standstill. Grass has less drive/braking grip and more rolling drag than dirt, while remaining driveable without a speed cap.

On a flat surface with the production car, six seconds of throttle reaches 27.57 m/s on asphalt, 25.69 m/s on dirt and 12.95 m/s on grass. With the clutch disengaged, a 25 m/s coast slows to 23.64, 23.28 and 22.45 m/s respectively after three seconds. Grass also supports mixed-contact entry, automatic reverse and climbing a 6-degree slope (7.13 m/s after six seconds of throttle). These are controlled measurements using the current 10 cm car assets, rather than universal speeds; terrain, steering and engine braking change the result.

W/up drives, S/down brakes then selects reverse through the automatic gearbox, A/D steers, and Space/Shift operates the rear handbrake and clutch. R recreates the car at stage spawn; N hands over the same stage to the next driver. Pause suspends native physics and timing. Retries/handoffs preserve session bests and suppress held inputs until release. A native handbrake holds the car at spawn until the first accepted drive input, preventing untimed downhill rolling.

## Manual verification

Run `godot --path . --script tools/vehicle_preview.gd` for driving, steering, jump, pause and landing captures. Add `-- --stage=wales-slate-mountain-17119` to use a saved world instead of authoring a procedural test. It reports telemetry rather than asserting pass/fail; headless runs skip image capture. Drive asphalt/dirt/grass transitions, braking/reverse, handbrake turns, sharp crests, rollover, pause/resume, retry and hotseat in the actual game. Airborne motion remains native GEVP/Jolt behavior.

The regression harness and Python test suite have been removed; production scene/asset validation remains.

## Chase camera

The chase sits 7.2 m behind the vehicle and 2.8 m above its origin, looking 2.8 m ahead. Speed smoothly adds up to 1.2 m of distance and widens the field of view from 68 to 74 degrees. It follows the interpolated car position directly in the horizontal plane, avoiding accumulated lag from smoothing both the target and the camera position. Vertical motion and yaw have separate damping, with bounded yaw speed and a small, limited contribution from forward travel during slides. Reverse retains the rear view, and gravity-up keeps the horizon level during jumps and rollover.

A 0.3 m sphere sweep against road/terrain brings the camera forward immediately when obstructed and smoothly returns it once clear. Retry/hotseat reset all follow history. Physics interpolation is enabled for the vehicle; the render-driven camera disables its own interpolation to avoid an additional tick of lag.
