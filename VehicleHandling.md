# GEVP rally handling

The production car is a 1400 kg RigidBody3D driven by Godot Easy Vehicle Physics and built-in Jolt at 120 Hz. Its four suspension rays, brush tire model, automatic gearbox, 50/50 AWD and differential locking replace the removed grounded controller. The game wrapper supplies inputs and telemetry; it applies no competing driving forces.

## Tuning

`scenes/cars/rally_hatchback.tscn` inherits the shared vehicle rig and holds drivetrain, steering, suspension tuning and compound box collision. The current baseline has 380 Nm peak torque, 0.22 m suspension travel, 0.78/0.80 damping ratios, modest countersteering assistance and disabled stability/upright torque assistance. The labelled VOX source exports four independent hub-centred wheel meshes with 0.325 m tire radius, 200 mm width and 2.45 m wheelbase. There is no runtime geometry baking or model-specific setup. `Game.car_scene` selects the car; GEVP's front torque split also supports FWD and RWD. See [VoxelCars.md](VoxelCars.md) for the asset contract, player paint and working rear lamps.

The catalog under `resources/surfaces` is the sole tire/material source:

| Surface/group | Friction | Stiffness | Rolling multiplier | Longitudinal ratio | Lateral assist |
| --- | --- | --- | --- | --- | --- |
| Asphalt / Road | 2.20 | 6.0 | 1.0 | 0.55 | 0.02 |
| Dirt / Dirt | 1.45 | 2.4 | 2.4 | 0.42 | 0 |
| Grass / Grass | 0.85 | 1.0 | 18.0 | 0.28 | 0 |

These are library parameters, not coefficients interchangeable with the old custom solver. Four contacts independently choose their collider's surface group, including mixed road/grass contacts. Grass resistance rises through the library's rolling model, so high-speed entry decelerates progressively and the car can drive back onto the road. In the flat production-scene test, asphalt reaches 27.98 m/s after 6 seconds of throttle; grass reaches 4.20 m/s (15.1 km/h) after 20 seconds. This is a measured test result, not a universal speed cap; slopes and steering change it.

W/up drives, S/down brakes then selects reverse through the automatic gearbox, A/D steers, and Space/Shift operates the rear handbrake and clutch. R recreates the car at stage spawn; N hands over the same stage to the next driver. Pause suspends native physics and timing. Retries/handoffs preserve session bests and suppress held inputs until release. A native handbrake holds the car at spawn until the first accepted drive input, preventing untimed downhill rolling.

## Verification

Run the full production-scene regression suite:

```sh
godot --headless --path . --fixed-fps 120 --script scripts/smoke.gd --log-file /tmp/opentrack-native-smoke.log
```

`--fixed-fps 120` advances the harness without wall-clock throttling; physics still uses 1/120 s steps. Optional user arguments `-- --vehicle-only`, `-- --stages-only`, `-- --terrain-controls` or `-- --car-assets-only` isolate relevant checks.

Coverage includes generation/replay and native Resource round trips; terrain grade/crest limits, clearance and seams; four-wheel settling; acceleration, braking/reverse, left steering and handbrake; mixed surfaces, progressive grass entry and road re-entry; hard drop, impulse jump, angled landing, partial contact at a real road crest, chassis roof/side collision and natural rollover; gravity-up camera; airborne pause/resume; held-input suppression and slope-start parking; R, retry and hotseat lifecycle; swept grounded/airborne finishes, reverse/off-width rejection, chassis/wheel finish freezing and recovery beyond the finite world.

The conservative test driver completed 500, 1200 and 5000 m generated stages on the sharper default terrain with no resets and maximum centreline offset about 1.02 m. Its 12 m/s target did not take off on those stages. A separate analytic crest within the production grade/grade-change limits recorded 86 airborne ticks and 12 partial-contact ticks at a 55 m/s entry speed, followed by landing. The impulse-jump check recorded 131 airborne ticks and a 1.83 m peak chassis height. These tests establish integration behavior; they do not establish competitive handling balance or subjective driving feel.

Rendered driving checks are available through `tools/vehicle_preview.gd`. Manual driving should refine corner entry speed, brake distance, countersteering, grass recovery on slopes and high-speed crest behavior before adding more assists. The game has no damage, tire wear, fuel simulation or automatic rollover righting.

The voxel hatchback full regression run passed with zero assertion failures. Focused checks also cover material isolation, lamp states, wheel animation and correct driven axles for AWD/FWD/RWD. Metal renders cover player colors, front/rear/side views, lights and actual driving. Logs are `/tmp/opentrack-voxel-full.log`, `/tmp/opentrack-voxel-final-vehicle.log`, `/tmp/opentrack-car-assets-final.log`, `/tmp/opentrack-car-studio-final.log` and `/tmp/opentrack-voxel-driving-render-final.log`; rendered frames are `/tmp/opentrack-vehicle-{settled,driving,steering,airborne,paused,landed}.png`. Headless runs can emit a macOS certificate-store diagnostic; this is separate from the assertion result.
