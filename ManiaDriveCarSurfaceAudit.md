# ManiaDrive Car And Surface Inspiration Audit

## Scope

This pass is intentionally limited to car feel, car presentation, and surface/material behavior. Track authoring, remote track repositories, score posting, multiplayer, menu flow, and ManiaDrive's `.mni` generation pipeline are out of scope unless they directly affect car feel.

Source inspected locally:

- ManiaDrive source release: `/Users/shaik/Code/ManiaDrive-1.2-src`
- Main vehicle/game loop: [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:1967)
- Track/game constants: [mania.h](/Users/shaik/Code/ManiaDrive-1.2-src/mania.h:1)
- Raydium ODE bridge: [raydium/ode.c](/Users/shaik/Code/ManiaDrive-1.2-src/raydium/ode.c:3473)
- Raydium ODE constants and license notice: [raydium/ode.h](/Users/shaik/Code/ManiaDrive-1.2-src/raydium/ode.h:1)

License constraint: Raydium/ManiaDrive code is GPL. We should not copy implementation code or import assets without a separate license decision. Treat this as a behavioral study and reimplement ideas in our Rust model.

## What ManiaDrive Actually Does

ManiaDrive builds the car as a small physical rig, not as one abstract vehicle body:

- A body box named `corps`, with visual mesh selected by story progression: base `clio.tri` / `roue5.tri`, upgraded `clio_sp.tri` / `roue6.tri` [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:1939).
- A hidden lower ballast named `balancier`, fixed under the body at `z = -0.5`, heavier than the body itself [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:1997).
- Four wheel spheres, attached with ODE hinge2 joints and suspension softness [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:2003).
- Front wheels steer and receive engine motor torque; rear wheels are hinge-blocked for steering [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:2023), [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:2037).
- Engine control is target wheel speed plus max motor power, not a raw forward force [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:2239).
- Steering is also target angle plus max steering motor power [raydium/ode.c](/Users/shaik/Code/ManiaDrive-1.2-src/raydium/ode.c:2111).

ManiaDrive's surface model lives in contact parameters:

- Raydium stores per-element slip and rotational friction [raydium/ode.c](/Users/shaik/Code/ManiaDrive-1.2-src/raydium/ode.c:666).
- Each contact averages the two participants' ERP, CFM, and slip, then sets both contact slip axes [raydium/ode.c](/Users/shaik/Code/ManiaDrive-1.2-src/raydium/ode.c:3563).
- Contact friction is effectively very high, while slip/softness decide how forgiving the contact is [raydium/ode.c](/Users/shaik/Code/ManiaDrive-1.2-src/raydium/ode.c:3588).
- The car body is deliberately assigned an "ice" slip value [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:1993). That is likely a snag-prevention measure for chassis contacts, not a driving-surface statement.

Car presentation also has useful details:

- Wheel visuals are independent from the body mesh, with front wheels steering and rear wheels staying locked.
- Camera look target is pulled toward current velocity, not only car forward [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:2354).
- Engine audio pitch is derived from motor/wheel speed, with noise added for life [mania_drive.c](/Users/shaik/Code/ManiaDrive-1.2-src/mania_drive.c:2397).

The source release does not include the actual `.tri`, `.tga`, `.wav`, or `.ogg` data files. It only references them by name. Do not plan on importing ManiaDrive car models unless we separately fetch the data repository and accept the asset license.

## Current OpenTrackTurbo State

Our current car model is a deterministic kinematic controller:

- Tuning is centralized in [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:14).
- Rear brake assist is physics-derived from held input, speed, steering, and slip, with no timer/window state [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:143).
- Tire forces have wheel loads, target-speed longitudinal demand, per-wheel friction, front/rear saturation, and handling states [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:369).
- Longitudinal demand is now source/axle-aware: drive and reverse are front-biased, service braking is all-wheel, rear brake is rear-wheel, and lateral reserve comes from combined tire usage [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:573).
- Surface parameters already expose longitudinal/lateral friction, rolling resistance, drag, passive slip, recovery, and rear-brake scales [src/surface.rs](/Users/shaik/Code/OpenTrackTurbo/src/surface.rs:32).
- Wheel contacts are sampled per wheel and mapped into per-wheel friction [src/driving.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving.rs:166).
- Visual wheels already bind into front/rear roles, with front steering and rear lock behavior [src/driving.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving.rs:325).
- Vehicle collision is a centered rounded kinematic collider, which is the right direction for avoiding chassis snagging [src/physics/components.rs](/Users/shaik/Code/OpenTrackTurbo/src/physics/components.rs:23).

## Main Adaptation Decisions

Decision: do not port ManiaDrive's ODE multi-body vehicle. Our controller is already deterministic, testable, and compatible with the current rail collision pass.

Decision: adopt ManiaDrive's front-drive structure as a handling inspiration. Engine force should primarily consume front-tire traction; service braking can use all tires; rear brake should primarily consume rear-tire traction.

Decision: direct rear-brake grip loss has been replaced by combined tire usage. A rear brake slide should happen because rear longitudinal braking demand consumes rear tire budget, leaving less lateral grip.

Decision: keep surfaces as game data in `SurfaceParams`. Raydium's contact slip maps cleanly to our `passive_slip_scale`, `recovery_scale`, and lateral/longitudinal friction values. Add new surface knobs only after we prove the existing ones are insufficient.

Decision: do not import ManiaDrive car assets yet. Use their body/wheel separation and visual behavior as inspiration, while continuing with our licensed SportsCar assets under [assets/cars/License.txt](/Users/shaik/Code/OpenTrackTurbo/assets/cars/License.txt:1).

## Useful Ideas To Reimplement

### 1. Target-Speed Motor, Not Constant Engine Force

ManiaDrive sets a wheel motor target speed and motor power cap. OpenTrackTurbo now mirrors this concept with speed-error longitudinal demand rather than a constant throttle force.

Implemented Rust behavior:

- Add target-speed fields to `DrivingTuning`: forward target speed, reverse target speed, engine speed gain, brake speed gain.
- Compute throttle force from speed error: target forward speed minus current forward speed.
- Clamp by engine power and axle traction.
- Let force fade naturally near top speed instead of relying only on drag or `max_forward_speed`.
- Keep `S`/down behavior: brake until below `REVERSE_ENTRY_SPEED`, then reverse.

Primary code target: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:625).

Expected feel: straighter acceleration, less endless tire saturation, less accidental slide at high speed, and easier top-speed tuning.

### 2. Axle-Aware Longitudinal Traction

ManiaDrive's engine is attached to the front wheel joints only. OpenTrackTurbo now uses source-aware wheel demand rather than a single global longitudinal clamp.

Implemented Rust behavior:

- Split longitudinal demand by source:
  - engine/reverse: front-biased, initially 100 percent front or a tunable `drive_front_bias`
  - service brake: all wheels
  - rear brake: rear wheels
- Clamp each wheel's longitudinal demand by that wheel's friction limit.
- Feed the consumed longitudinal fraction into lateral limit calculation, forming a simple friction ellipse per wheel.

Primary code targets:

- Existing wheel loads and limits: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:378)
- Existing wheel friction mapping: [src/driving.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving.rs:196)
- Existing rear lateral scaling: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:587)

Expected feel: rear brake creates rotation only when the rear tires are actually loaded and moving, asphalt remains grippy, and dirt/grass/ice slide because their lower tire budgets are physically consumed faster.

### 3. Rear Brake As Secondary Drift Trigger

The current model avoids explicit timer windows and uses combined tire budget. Keep that. The next polish should make rear brake feel secondary and legible:

- Rear brake should first scrub speed.
- If speed and steering/slip are present, it should reduce rear lateral reserve through combined slip.
- Yaw assist should become a small nudge, not the main cause of rotation.
- Asphalt should mostly stay controlled unless rear brake is held or tapped while steering.
- Dirt/grass should become more sideways from lower lateral friction and slower recovery, not larger arbitrary yaw.
- Ice should slide mainly because both longitudinal and lateral grip are low; rear-brake yaw assist can stay muted.

Current code targets:

- Rear brake input and no-reverse behavior: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:208)
- Drift assist derivation: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:143)
- Rear brake force: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:662)
- Rear brake yaw assist: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:669)

Expected feel: rear brake slide exits by reducing rear brake input, restoring rear tire lateral reserve, straightening steering, and letting recovery pull lateral velocity down. It should not need an explicit "exit window."

### 4. Surface Slip And Recovery Pass

ManiaDrive's slip is a contact property, not a named "drift mode." Our current `SurfaceParams` can represent the same idea.

Recommended interpretation of existing knobs:

- `longitudinal_friction`: acceleration/brake tire budget.
- `lateral_friction`: cornering tire budget.
- `passive_slip_scale`: how easily the surface enters slide from saturation/slip angle.
- `recovery_scale`: how strongly the car returns to grip when demand drops.
- `rear_brake_grip_loss_scale`: how much rear brake consumes rear lateral reserve.
- `rear_brake_yaw_scale`: only a small steering-yaw helper, not the slide source.

Surface intent:

- Asphalt: high friction, high recovery, low rear-brake amplification.
- Dirt: moderate longitudinal friction, lower lateral friction, slower recovery.
- Grass: low/moderate grip, high rolling resistance, slow recovery.
- Ice: very low lateral/longitudinal friction, low recovery, muted yaw assist.
- Boost: keep its surface friction close to asphalt. If boost behavior feels wrong later, make boost a trigger/event rather than a surface-wide forward acceleration.

Primary code target: [src/surface.rs](/Users/shaik/Code/OpenTrackTurbo/src/surface.rs:54).

### 5. Low Ballast Without Dynamic Bodies

ManiaDrive uses a hidden low ballast to make the car less tippy and less twitchy. In our kinematic model, this should be represented through tuning, not an extra rigid body.

Recommended Rust behavior:

- Keep `center_of_gravity_height` conservative.
- Add yaw inertia/damping only if target-speed and axle traction are still too twitchy.
- Use visual pitch/roll for feel, not for physics authority.

Current code targets:

- Load transfer: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:472)
- Body visual attitude from suspension compression: [src/driving.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving.rs:341)

### 6. Car Model Presentation

The important car-model lesson is structural, not asset copying:

- Body mesh and wheel meshes should remain separate.
- Front wheels steer visually; rear wheels do not.
- Wheel spin comes from wheel angular state, not `elapsed_time * signed_speed`.
- Engine pitch should use motor/wheel speed when an audio module exists, not raw car linear speed.
- Virtual suspension compression per sampled wheel drives body attitude and wheel placement.

Current code covers body/wheel separation, front-wheel steer, wheel spin, and suspension offsets: [src/driving.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving.rs:341).

### 7. Chassis Contact Material

ManiaDrive gives the car body a high-slip contact material, likely to prevent chassis snagging when the body contacts world geometry. OpenTrackTurbo should express the same concept in its kinematic collision response, not by adding an ODE contact solver.

Implemented Rust behavior:

- Keep the centered rounded collider and MoveAndSlide rail resolution.
- Chassis/rail contact tuning preserves tangential scrape velocity and removes inward velocity.
- Collision yaw is damped on scrape/depenetration instead of carrying the full requested yaw through rail contact.
- When a candidate rail pose intersects, translation is binary-searched back to the largest clear partial movement instead of immediately falling back to the previous full slice pose.
- Keep trigger/road contacts separate from rail/body contacts.

Primary code targets:

- Vehicle collider shape: [src/physics/components.rs](/Users/shaik/Code/OpenTrackTurbo/src/physics/components.rs:23)
- Rail resolution: [src/physics/queries.rs](/Users/shaik/Code/OpenTrackTurbo/src/physics/queries.rs:1)
- Collision telemetry: [src/debug.rs](/Users/shaik/Code/OpenTrackTurbo/src/debug.rs:97)

### 8. Wheel RPM And Slip Ratio

ManiaDrive gets engine sound and visual behavior from motor/wheel speed. OpenTrackTurbo now tracks lightweight wheel angular state and uses it for imported wheel visuals.

Implemented baseline Rust behavior:

- Per-wheel angular speed is derived from target motor speed, applied longitudinal force, wheel radius, and braking lock tendency.
- Slip ratio telemetry is exposed for drive spin and brake lock.
- Imported wheel spin uses front/rear axle wheel state instead of elapsed-time signed-speed fallback.
- Motor/wheel speed is available for engine pitch later.
- Keep this lightweight; do not add gearboxes, clutches, or full drivetrain simulation.

Primary code targets:

- Tire force output: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:278)
- Player car state: [src/driving.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving.rs:74)
- Wheel visuals: [src/driving.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving.rs:341)

### 9. Directional Boost Pads

ManiaDrive applies boost as a world-direction force from a trigger-like box. OpenTrackTurbo now derives boost direction from the generated boost piece path frame and applies acceleration in that direction.

Implemented Rust behavior:

- Keep boost surface friction close to asphalt.
- Road colliders carry an optional boost direction when the piece surface is `Boost`.
- Ground contacts expose that boost direction to driving.
- Tire force integration applies boost as acceleration in pad direction.
- Boost without a valid pad direction produces no hidden car-forward acceleration.
- Keep boost readable rather than slide-prone.

Primary code targets:

- Surface boost fields: [src/surface.rs](/Users/shaik/Code/OpenTrackTurbo/src/surface.rs:90)
- Track trigger or road-surface spawn data: [src/track](/Users/shaik/Code/OpenTrackTurbo/src/track)
- Driving force integration: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:369)

### 10. Surface Softness / Compliance

Raydium uses ERP/CFM softness in addition to slip. OpenTrackTurbo now has an explicit `compliance` surface parameter layered into the existing tire model.

Implemented Rust behavior:

- Treat compliance as recovery/settling modulation, not an independent drift mode.
- Keep asphalt firm and fast to recover.
- Make dirt/grass feel soft through slower lateral recovery and slightly delayed settling.
- Preserve one tire-force code path across asphalt, dirt, ice, boost, and grass.

Primary code target: [src/surface.rs](/Users/shaik/Code/OpenTrackTurbo/src/surface.rs:32).

### 11. Velocity-Based Camera And Motor-Speed Feedback

ManiaDrive's chase camera looks toward velocity, and engine pitch follows motor speed. OpenTrackTurbo can use the same player-facing idea without adopting the old engine stack.

Implemented Rust behavior:

- Blend camera look target between car forward and velocity direction.
- Increase chase distance or lookahead from speed without losing corner readability.
- Keep wheel/motor RPM available for audio once an audio module exists.

Primary code targets:

- Camera: [src/driving.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving.rs:374)
- Wheel telemetry: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:442)

## Recommended Implementation Order

1. Completed: replace direct engine force with target-speed force.
   - Code: [src/driving/model.rs](/Users/shaik/Code/OpenTrackTurbo/src/driving/model.rs:625).
   - Tests cover throttle force fading near target speed, reverse entry, and service brake behavior.

2. Completed: add axle-aware longitudinal distribution.
   - Engine/reverse front-biased.
   - Rear brake rear-biased.
   - Service brake all-wheel.
   - Tests cover default front-drive bias and rear-brake reserve consumption.

3. Completed: convert rear brake grip loss to combined-slip reserve.
   - Direct rear lateral scaling is no longer the primary mechanism.
   - Tests cover rear brake no-reverse behavior, yaw gating, and physics-derived activation without windows.

4. Completed baseline: retune surfaces using the same knobs.
   - Asphalt should recover most aggressively.
   - Dirt/grass should slide because lateral friction and recovery are lower.
   - Ice should be low-grip without large yaw assist.
   - Tests cover asphalt staying grip-biased under hard steering and ice entering slide earlier.

5. Completed baseline: add motor/wheel telemetry.
   - Debug overlay should show target speed, applied engine/brake force by axle, and front/rear tire reserve.
   - This makes the next tuning pass empirical instead of guesswork.

6. Completed baseline: chassis contact material.
   - Add/tune slippery chassis contact behavior for rail/body impacts.
   - Preserve scrape velocity and reduce rear-corner snagging/yaw spikes.

7. Completed baseline: wheel RPM and slip ratio.
   - Add lightweight angular wheel state.
   - Wheel spin from axle speed.
   - Debug target/actual RPM and slip ratio.

8. Completed baseline: directional boost pads.
   - Boost acceleration uses road/pad-frame direction.
   - Boost no longer falls back to car-forward acceleration.
   - Debug overlay exposes boost direction.

9. Completed baseline: virtual suspension presentation.
   - Use wheel samples for compression/rebound and body attitude.
   - Avoid dynamic joint simulation.
   - Imported wheels move by sampled suspension offsets.

10. Completed baseline: surface compliance.
   - Dirt/grass softness comes from compliance-modulated lateral correction and recovery.
   - Asphalt and boost remain firm.

11. Completed baseline: velocity-based camera and motor-speed feedback prep.
   - Blend camera lookahead toward velocity.
   - Use motor/wheel speed for audio when an audio module exists.

## Remaining Implementation Scope

The next code pass should be implementation-only only if in-game verification identifies a concrete gap. Current remaining scope is tuning and optional diagnostics, not adding parallel physics systems.

Keep the order:

- Verify asphalt clean cornering first.
- Verify rear brake as secondary rotation.
- Verify boost pads on straights and curves.
- Verify dirt/grass compliance and ice readability.
- Verify rail scrape after the new velocity camera and suspension presentation are active.

This continues the same ManiaDrive lesson: wheels, axles, and contact material matter even in an arcade model.

## Open Decisions To Confirm

Adopt front-wheel-drive bias by default. Recommendation: yes, because it matches the reference and should reduce rear-driven driftiness.

Do not import ManiaDrive car meshes/audio yet. Recommendation: yes, because the inspected source release does not include those files and GPL/asset license handling needs a separate decision.

Ignore ManiaDrive track authoring. Recommendation: yes, for now. Track block generation is not needed for the current car/surface polish.

Keep automatic fall/off-track recovery out of scope. Recommendation: yes, for now. Manual reset is enough until verticality/off-track states are deliberate gameplay.
