# Open Track Turbo Plan

## Product Shape

Local-first arcade precision racer inspired by Trackmania-style hotseat time trials.

The game is about generated tracks, fast retries, readable handling, surface mastery, and local hotseat competition. Ghosts, timing, results, and leaderboard presentation remain product goals, but they are deferred until the vehicle physics path is stable. It is not a realistic vehicle simulator, online racer, track editor, open world, or progression game.

## Current Scope

In scope:

* one active car at a time
* local hotseat sessions
* deterministic generated tracks from seed + length
* fixed modular piece shapes: straight, double straight, 45-degree turn, 90-degree turn, 180-degree turn
* multiple handling surfaces
* checkpoint/finish trigger geometry owned by track spawning and bound to the road frame it marks
* imported sports-car visuals
* forgiving rail contact and recovery
* secondary rear-brake drift control
* target-speed motor forces with axle-aware tire budgets
* lightweight wheel RPM/slip-ratio state for physics, debug, and wheel visuals; engine audio can consume it later
* virtual wheel suspension for per-corner compression, presentation, and load-feel tuning
* chassis contact material behavior for slippery, non-snagging body/rail impacts
* directional boost pads that apply track/pad-frame acceleration
* velocity-aware camera and wheel/motor telemetry for future audio feedback
* single repaired SportsCar FBX asset with scoped material normalization and transform-based wheel binding for the current importer output
* automatic reset to start when the car center leaves valid road contact; there is no off-road driving/collider fallback
* debug/tuning overlays

Out of scope for now:

* hand-authored full tracks
* manual track editor
* online multiplayer
* simultaneous racing
* runtime timing, results, HUD, session leaderboard, and ghost replay until the vehicle physics swap settles
* save-game/profile persistence
* car upgrades, damage, full drivetrain simulation
* traffic, combat, open world, campaign
* off-road terrain driving or recovery to anything except the current start spawn

## Current Architecture

Main layers:

* `driving`: arcade car state, input mapping, yaw/velocity handling, wheel contact sampling, body and imported-wheel visuals.
* `surface`: data-driven handling parameters for asphalt, dirt, ice, and boost road pieces.
* `track`: recipe, generated pieces, path frames, road mesh, rail/road/trigger spawning, scene cleanup.
* `physics`: project-owned query trait plus Avian-backed ground queries and MoveAndSlide rail collision resolution.
* `hotseat`: player order and manual next-driver reset.
* `shell`: main menu, setup, and pause.
* `debug`: runtime tuning visibility.

Important contracts:

* Track geometry should derive from sampled path frames.
* Visual road meshes, road contact spans, rails, triggers, and future banked contact frames should share the same source path data.
* Bank changes must happen through dedicated `BankTransition` pieces with dense eased `PathFrame` samples; held-bank and flat straights can stay endpoint-only.
* Start pose, checkpoint strips, and finish strips must use the relevant `PathFrame` center/normal/yaw rather than a fixed world height or yaw-only transform.
* Gameplay wheel contacts are logical sampled contact patches, not imported mesh polygons.
* The active car pose is yaw plus a wheel-derived support normal. Tire forces, vehicle transform, visual root, camera, and rail collision must consume that same support frame rather than falling back to a yaw-only/world-up path.
* Wheel RPM, slip ratio, and virtual suspension are lightweight gameplay state layered onto those logical contacts, not a solver-owned vehicle rig.
* Imported vehicle meshes are visual-only; wheel animation must not own vehicle physics.
* The active vehicle asset is `assets/cars/fbx/SportsCar.fbx`. There is no vehicle selector or old OBJ/SportsCar2 compatibility path.
* Avian is used for static colliders and spatial queries, not as the vehicle controller.

## Steering Semantics

Pinned behavior:

```text
A / ArrowLeft     steer left
D / ArrowRight    steer right
W + D             yaw right while moving forward
W + A             yaw left while moving forward
A or D alone      no idle yaw
S / ArrowDown     service brake while rolling forward; reverse only once effectively stopped
Space / Shift     rear brake / drift assist, never reverse
```

Yaw is derived from steering plus signed movement/drive intent, not raw A/D input alone. Reverse uses reverse motion direction with reduced authority. Holding service brake should not command reverse until forward speed has decayed to an explicit stop threshold. Rear brake is a separate intent from service brake/reverse, but it is secondary to normal steering: it should loosen the rear only while current speed, steering/slip, surface, and held rear-brake input justify it. There should be no artificial tap window, sustain timer, or recovery timer. Asphalt cornering should normally work without rear brake. The imported car assets can have their own node axes; if another asset disagrees visually, add a debug-only vehicle node/axis inspector and fix the asset binding at the visual layer, not the gameplay input layer.

## Track Generation Direction

Current generator:

* deterministic from recipe seed and piece count
* shape and surface are separate
* candidates are validated against occupied road-footprint sectors
* failure is explicit during development
* road surface meshes use `bevy_procedural_meshes`
* 2D path vocabulary and boundary extraction use `kurbo`
* Bevy curve APIs sample straight and constant-arc pieces into `PathFrame`s
* bank transitions use dense smootherstep samples so road mesh, road collider, rails, and wheel support raycasts see the same gradual crossfall
* road contact colliders are piece-level Avian triangle meshes from sampled path edges
* rail collision colliders are track-level Avian compound capsules from merged sampled boundary paths

Generation validator should stay focused on correctness:

* valid piece continuity
* finish/checkpoint ordering
* sector occupancy and no overlap
* coherent curve samples
* coherent dense bank-transition samples
* trigger alignment
* generated count consistency
* rail boundary paths have coherent points and nonzero segments

Do not reject tracks for subjective variety or boringness in validation.

## Vehicle Physics Direction

The deterministic tire-force controller is now the right foundation. Future passes should tune player-facing feel around it, not replace it with a solver-owned multi-body car.

Keep:

* throttle/brake/reverse input
* dedicated rear-brake input
* signed speed
* yaw/velocity basis
* four logical wheel contacts
* target-speed longitudinal demand
* front-biased drive/reverse force, all-wheel service brake, rear-wheel rear brake
* combined longitudinal/lateral tire budget per wheel
* predictable reverse steering with reduced authority
* imported SportsCar wheel visuals as readability only
* Avian static collider and spatial query integration

Implemented in this physics polish line:

* Chassis contact material: rail/body impacts behave like a slippery chassis contact, preserving tangential velocity and damping rear-corner snag/yaw spikes.
* Wheel angular state: track wheel RPM, target RPM, slip ratio, and braking lock tendency for physics telemetry and visual spin.
* Directional boost pads: boost is authored/applied in pad or track-frame direction, not car-forward.
* Virtual suspension: use per-wheel ground samples to derive compression/rebound and presentation/body attitude without introducing dynamic ODE-style joints.
* Surface compliance: softness/recovery modulation is a surface parameter, layered into the same tire model rather than a separate drift mode.
* Velocity-based camera: camera lookahead blends toward current velocity while wheel/motor telemetry remains available for a future audio module.
* Tire-supported steering yaw: front steer angle now produces a target yaw rate, yaw-rate state follows through response/damping, high-speed steering fades, front tire saturation limits steering support, and collision resolution feeds back the accepted yaw rate.
* Steering servo state: player input sets a target front-wheel angle, the resolved wheel angle follows through a response knob, and tire yaw uses the resolved angle rather than raw input.
* Non-asset vehicle feedback: motor pitch/load, front wheel target/actual RPM, and slip intensity are computed from wheel telemetry for future audio or UI without adding playback assets yet.
* SportsCar asset cleanup: Blender CLI repairs FBX material alpha, canonical mesh names, and front hub orientation by geometry; runtime code scopes importer opacity normalization and wheel binding to descendants of the active car scene only.

Avoid:

* full drivetrain simulation
* tire heat/wear
* part systems
* solver-owned realistic suspension
* deriving physics from imported art mesh topology
* delegating the full vehicle controller to the physics solver before the deterministic tire model is solid

## Handling Direction

The target feel is grip-first arcade handling with drift as a skillful secondary layer. The player should be able to run clean asphalt corners with steering, throttle modulation, and service brake alone. Rear brake is for optional rotation, style, recovery from over-entry, and certain low-grip surfaces; it should not be required for every turn.

Implementation state and tuning handoff: see `HandlingImplementationAudit.md` for the current rear-brake/drift code path, verified behavior, and remaining feel-tuning order.

Current collision status:

* Rail response has moved from one-hit shapecast plus last-clear rewind to Avian `MoveAndSlide` resolution with yaw limiting, projected velocity, and collision telemetry.
* The vehicle collider is now centered and rounded instead of rear-biased and sharp.
* `PoseOverlap`, `last_clear_car_pose`, and global overlap velocity loss are removed from normal source code.
* Remaining collision work is in-game scrape verification and scalar tuning, not reverting to last-clear rewinds.

Grip/drift split:

* Asphalt baseline: high lateral authority, strong recovery, no spontaneous rear breakaway in ordinary steering.
* Rear-brake assist: rear wheel braking consumes rear tire reserve through combined slip, with only a small yaw nudge when speed and steering/slip make it plausible.
* Dirt: lower lateral authority and slower recovery, but still driveable without forcing constant drift.
* Ice: the exception surface; slip can be frequent, but should still feel legible and recoverable.
* Boost: fast and grippy/readable; acceleration follows the road/pad frame direction rather than car forward.

## Dependencies

Current useful crates:

* `bevy`: engine, ECS, rendering, input, UI
* `avian3d`: static colliders and spatial queries
* `bevy_egui`: debug/tuning UI candidate
* `bevy_procedural_meshes`: road surface tessellation
* `kurbo`: 2D path/edge/polygon vocabulary
* `rand_chacha`: deterministic generation
* `ron`/`serde`: future tuning and recipe data
* `bevy_ufbx`: imported FBX vehicle assets
* Blender CLI: repeatable SportsCar FBX inspection/repair through `tools/sports_car_fbx_tool.py`, including outward front-wheel hub verification

Future crate candidates:

* `bevy_lookup_curve`: slip/grip/steering tuning curves once tire forces replace direct damping
* `rstar`: broad-phase indexing if piece counts or validation retries grow
* direct `lyon_tessellation`: only if `bevy_procedural_meshes` stops exposing enough control

Do not add a vehicle-controller crate unless it clearly supports Bevy 0.18, deterministic fixed-step use, custom tire/surface tuning, and does not force a solver-driven drivetrain/suspension model. Reject crates that force gameplay to depend on crate-specific geometry across module boundaries, break deterministic generation, increase Bevy version skew, or replace the custom yaw/velocity controller with character-controller semantics.

## Implemented Snapshot

Current foundation:

* app flow, setup, pause, hotseat player order, and debug overlay
* deterministic generated tracks with straights, sampled curves, dense eased bank transitions, banked frames, road meshes, road colliders, merged rail colliders, surface-bound checkpoints, and surface-bound finish triggers
* single imported SportsCar visual with front-wheel steering, outward-facing front hubs, wheel spin, wheel load scaling, virtual suspension offsets, and importer-scoped opaque materials
* custom deterministic controller with signed speed, four logical wheel contacts, wheel-derived support frame, target-speed motor force, front-biased drive, steering-servo tire yaw, rear-wheel rear brake, combined-slip tire budgets, per-wheel friction, load/saturation debug, and vehicle feedback signals
* Avian-backed ground raycasts plus full-orientation MoveAndSlide rail collision resolution, used as static track queries rather than a vehicle controller
* centered rounded vehicle collider, yaw-limited collision pose resolution, projected rail scrape velocity, and collision debug telemetry
* chassis contact velocity/yaw material response that removes inward rail speed, preserves scrape speed, and avoids full old-pose fallbacks where a clear partial translation exists
* lightweight wheel angular-speed/slip telemetry with imported wheel spin driven from per-axle wheel state instead of elapsed-time signed-speed fallback
* directional boost pads driven by generated piece path direction
* virtual suspension compression/rebound driving wheel offsets and visual body attitude
* surface compliance tuning for dirt softness without a separate surface physics path
* velocity-biased chase camera direction
* validation around piece continuity, generated counts, trigger ordering/alignment, and coherent rail boundary paths

Explicitly removed for the vehicle-physics migration:

* `run` timing/checkpoint state, finish flow, and results screen
* session leaderboard recording
* session-best ghost recording/playback
* gameplay HUD

These should be rebuilt later against the final vehicle state shape rather than preserved through compatibility shims.

## Pending Work

Active gameplay order:

1. In-game feel verification. Recheck asphalt grip, loose surfaces, boost pads, suspension presentation, velocity camera, and rail scrape on repeatable seeds.
2. If the car is still too drifty, tune the single yaw/tire path in this order: `max_steer_angle`, `high_speed_steer_fade`, `yaw_rate_response`, `yaw_rate_damping`, `lateral_stiffness`, `straight_line_settling`, `slide_saturation_threshold`, `slide_slip_angle_threshold`, then rear-brake cost/yaw scalars.
3. Keep rear brake secondary. It may consume rear tire reserve and add a small gated yaw assist, but asphalt steering should not need it for ordinary corners.
4. Rebuild runtime timing, HUD/results, leaderboard, and ghost replay only after the vehicle physics path is stable.
5. Audio feedback. Add an audio module only when there is an actual sound path; the current motor/load/slip/RPM feedback values are already available.

Debug and tuning support:

* Keep the existing text debug overlay as the source of truth for current metrics.
* Add tuning controls only for values actively being tuned: chassis contact slip, yaw damping, wheel RPM/slip ratio, boost force, virtual suspension response/travel, surface compliance/recovery, and camera response.
* Add visual wheel-contact, force, suspension, boost-direction, and collider overlays only if text output is not enough to diagnose a concrete handling or collision issue.
* Add a debug-only imported vehicle node/axis inspector only if future assets are intentionally brought back into scope and make wheel axes ambiguous.

Parked gameplay/system work:

* Improve setup/pause UI polish inside the current shell modules after driving feel is fun.
* Move the fixed shape catalog into piece metadata with connection rules and candidate weighting when generation needs more control.

Parked until scope expands:

* Evaluate `bevy_lookup_curve` only when tune curves are actually needed.
* Add `rstar` only if validation performance needs it.
* Add airtime/loops only after the grounded support-frame path feels stable on banked pieces.
* Add unreachable-finish validation once branching, verticality, or non-forward pieces exist.
* Add vertical pieces only after slope/ramp recovery and placement validation exist.
