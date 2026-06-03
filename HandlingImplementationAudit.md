# Handling Implementation Audit

## Goal

Normal driving is primary. Rear-brake drift is a secondary rotation tool, not the default steering model. Asphalt should stay grippy and self-settling unless the player asks for rotation with rear brake or the car enters an actual low-grip/high-slip state.

## Implemented State

`src/driving/model.rs`:

* `ControlInput` separates service brake/reverse from rear brake with a held `rear_brake` axis.
* `Space`, `ShiftLeft`, and `ShiftRight` map to rear brake only.
* `S` / `ArrowDown` remain service brake/reverse input. `drive_mode` uses `REVERSE_ENTRY_SPEED = 0.15`, so reverse engages only after an effective stop, or when already rolling backward meaningfully.
* `DriftAssist` is physics-derived each fixed step. It has no tap window, sustain timer, recovery timer, or cooldown.
* Rear-brake assist has three separate effects:
  * non-reversing longitudinal speed scrub through `rear_brake_force`
  * rear-only lateral reserve consumption through combined longitudinal/lateral tire usage
  * gated yaw assist through `rear_brake_yaw_assist`
* Rear-brake tire reserve cost and yaw amounts are continuous functions of held rear brake, speed, steering/slip, and surface scale.
* Passive slide yaw is `passive_slide_yaw_response`; it is weaker than explicit rear-brake yaw assist.
* `SlideReason` distinguishes `PassiveSlip`, `SurfaceSlip`, and `RearBrakeAssist` for debug.
* Straight-line settling increases lateral cleanup when rear brake is not active.
* `ControlIntent::from_input` derives wheel steer angle from `DrivingTuning::max_steer_angle`; steering is no longer a direct body-yaw delta.
* `tire_forces` emits `target_yaw_rate`, combining front-steering yaw support, passive slide yaw, and rear-brake yaw assist in one path.
* `steering_target_yaw_rate` derives yaw from front steer angle, wheelbase, speed, high-speed steering fade, and front tire saturation.
* `resolved_yaw_rate` applies yaw inertia/damping and clamps to `DrivingTuning::max_yaw_rate`.
* The less-drifty baseline currently uses stronger lateral cleanup, conservative passive slide response, higher slide thresholds, and reduced rear-brake grip/yaw effect.

`src/driving.rs`:

* `PlayerCar` stores rear-brake input and current physics-derived `DriftAssist`.
* `PlayerCar` stores `yaw_rate` as actual vehicle state.
* `PlayerCar` stores target and resolved front-wheel steer angles; tire yaw uses the resolved angle.
* `PlayerCar` stores a wheel-derived support frame: support state, contact count, support normal, support axes, dominant surface, and boost direction.
* `drive_car` derives drift assist before tire force calculation and feeds it into `tire_forces`.
* `drive_car` advances yaw from `resolved_yaw_rate`, not raw steering input.
* `drive_car` now uses the same support frame for force basis, car transform rotation, visual-root rotation, camera up, and rail collision pose. The transform no longer gets overwritten with yaw-only world-up rotation on banked road.
* Collision resolution feeds back accepted yaw into `yaw_rate`, so rail contact cannot leave stale free rotation.
* The single active SportsCar FBX is visual-only. Wheel mesh fragments bind by the current imported local transform pattern, the FBX repair script canonicalizes mesh names and verifies outward front hub orientation, and car-scene material normalization is scoped to descendants of `VehicleSceneRoot`.
* Service brake does not trigger drift assist.

`src/surface.rs`:

* `SurfaceParams` includes `passive_slip_scale`, `recovery_scale`, `rear_brake_grip_loss_scale`, and `rear_brake_yaw_scale`.
* Asphalt is the grippy baseline with faster recovery and conservative rear-brake scaling.
* Dirt loosens sooner and recovers slower than asphalt, but less aggressively than the earlier drift-first pass.
* Ice has the earliest passive breakaway and weakest recovery.
* Boost remains fast and close to asphalt rather than a drift surface.

`src/debug.rs`:

* The overlay shows rear-brake input, drift state, slide reason, target/actual steer and yaw, support state/count/normal, motor/load/slip feedback, wheel RPM/slip, suspension compression/offsets, boost direction, and collision telemetry.

## Resolved Contradictions

* The plan said grip-first, while code treated every slide as drift. Code now splits passive slide response from rear-brake yaw assist.
* The plan pinned `Space / Shift` as rear brake, while code had no rear-brake input. Implemented.
* The plan pinned brake-until-stop reverse, while code entered reverse too early. Implemented with `REVERSE_ENTRY_SPEED`.
* The plan required surface-scaled slip/recovery, while surfaces only had friction and drag. Implemented.
* The user rejected explicit drift windows. Tap/sustain/recovery timers are removed; assist is now derived from current physics/input state.
* The car felt drift-first because direct steering yaw could rotate the body faster than tire cleanup. Steering now targets yaw rate through front wheel angle, speed, tire saturation, and yaw inertia/damping.
* The car did not right itself to banked surfaces because physics used road normals while transform and collider pose stayed yaw-only/world-up. The car pose now uses yaw plus a wheel-derived support normal.
* Checkpoint/finish strips and the start pose were not coherently bound to banked surfaces. They now use the relevant `PathFrame` center/normal/yaw instead of fixed world height plus yaw-only placement.
* Bank transitions were too jerky because transition pieces were sampled as one large twisted quad, leaving the road collider with coarse triangle normals. `BankTransition` now uses dense smootherstep samples while ordinary held-bank straights remain endpoint-only.
* Imported wheel visuals should not be corrected with runtime per-wheel flips. The Blender repair script now checks front hub center/normal geometry and mirrors only inward-facing front hubs in the FBX mesh data.

No user-blocking decision is open for the next implementation pass.

## Current Behavior Decisions

Rear-brake entry:

* There is no artificial tap window.
* A short tap only applies rear-brake effects for the physics frames where the input is held.
* Any slide that persists after release persists because velocity, slip, saturation, and surface state still justify it.
* Holding rear brake with enough speed and steering/slip lowers rear lateral authority and can add yaw assist.
* Holding rear brake straight scrubs speed and lightly reduces rear stability, but does not add free yaw.

Rear-brake exit:

* Releasing rear brake immediately removes rear-brake grip loss and rear-brake yaw assist.
* Straightening below the steering/slip gates removes yaw assist while rear brake is held; rear braking can still scrub speed and consume rear tire reserve.
* Recovery is handled by tire forces, straight-line settling, friction saturation, and surface recovery scale.

Service brake:

* Service brake can add longitudinal braking saturation.
* Service brake does not reduce rear lateral grip.
* Service brake does not add rear-brake yaw assist.
* Service brake cannot command reverse until the explicit stop threshold is reached.

Surface behavior:

* Surface profiles scale the same tire model.
* Low-grip surfaces can trigger `SurfaceSlip` earlier than asphalt.
* Rear-brake assist remains rear-wheel-biased on every surface.

## Tests Added

`cargo test` now covers:

* `Space`, `ShiftLeft`, and `ShiftRight` rear-brake mapping
* service brake not triggering rear brake
* service brake entering reverse only after effective stop
* rear brake scrubbing speed without reverse intent
* rear-brake physics reducing rear grip and raising rear saturation
* rear-brake yaw assist requiring speed and steering/slip
* rear-brake assist having no explicit windows after release
* low-grip surface breakaway before asphalt
* high-speed steering losing authority versus low-speed steering
* yaw-rate response not snapping instantly to target yaw
* steering moving lateral demand toward the front axle by share rather than by stale absolute-force assumptions
* existing asphalt hard-steering grip behavior
* target/resolved steering servo behavior and repaired SportsCar front hub orientation
* SportsCar wheel binding from the supported imported mesh transform pattern
* yaw-plus-support-normal rotation aligning local car up to banked road normals
* dense eased bank-transition sampling and validation

Current verification: `cargo test` passes with 76 tests.

## Remaining Work

The architecture pass is complete. The next pass should be implementation-only tuning against actual feel:

1. Playtest repeatable seeds and classify bad behavior using target yaw rate, actual yaw rate, slide reason, rear-brake tire cost, yaw assist, saturation, boost direction, suspension compression, and collision telemetry.
2. If asphalt is still too drifty while steering normally, tune `max_steer_angle`, `high_speed_steer_fade`, `yaw_rate_response`, `yaw_rate_damping`, `lateral_stiffness`, `straight_line_settling`, `slide_saturation_threshold`, and `slide_slip_angle_threshold` before touching rear brake.
3. If rear brake still over-rotates, tune `rear_brake_force`, `rear_brake_grip_loss`, `rear_brake_yaw_assist`, `drift_min_speed`, `drift_min_steer`, and `drift_min_slip_angle`.
4. If rail contact still creates a rear-snag feel, tune collision yaw limits/chassis contact response in the collision path, not tire/drift settings.

## Do Not Do

* Do not make rear brake the primary steering input.
* Do not reintroduce explicit tap, sustain, recovery, or cooldown windows.
* Do not let service brake trigger rear-grip loss or drift yaw.
* Do not lower all wheel grip for rear-brake drift; keep it rear-biased.
* Do not fork a separate physics path per surface.
* Do not reintroduce direct body-yaw steering.
* Do not add tire heat, wear, damage, drivetrain, or suspension systems.
