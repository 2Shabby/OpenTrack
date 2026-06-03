# Open Track Turbo Plan

## Product Shape

Local-first arcade precision racer inspired by Trackmania-style hotseat time trials.

The game is about generated tracks, fast retries, readable handling, surface mastery, ghosts, and local leaderboard competition. It is not a realistic vehicle simulator, online racer, track editor, open world, or progression game.

## Current Scope

In scope:

* one active car at a time
* local hotseat sessions
* deterministic generated tracks from seed + length
* fixed modular piece shapes: straight, 45-degree turn, 90-degree turn, 180-degree turn
* multiple handling surfaces
* checkpoint/finish triggers
* session leaderboard
* session-only ghost replay
* imported sports-car visuals
* debug/tuning overlays

Out of scope for now:

* hand-authored full tracks
* manual track editor
* online multiplayer
* simultaneous racing
* save-game/profile persistence
* car upgrades, damage, drivetrain simulation
* traffic, combat, open world, campaign

## Current Architecture

Main layers:

* `driving`: arcade car state, input mapping, yaw/velocity handling, wheel contact sampling, body and imported-wheel visuals.
* `surface`: data-driven handling parameters for asphalt, dirt, ice, boost, and grass/off-track.
* `track`: recipe, generated pieces, path frames, road mesh, rail/road/trigger spawning, scene cleanup.
* `physics`: project-owned query trait plus Avian-backed ground and rail collision queries.
* `run`: timer, checkpoint progress, finish state, reset.
* `hotseat`: player order and in-memory leaderboard.
* `ghost`: session-best transform replay.
* `shell`: main menu, setup, pause, results.
* `hud` and `debug`: runtime status and tuning visibility.

Important contracts:

* Track geometry should derive from sampled path frames.
* Visual road meshes, road contact spans, rails, triggers, and future banked contact frames should share the same source path data.
* Gameplay wheel contacts are logical sampled contact patches, not imported mesh polygons.
* Imported vehicle meshes are visual-only; wheel node animation must not own vehicle physics.
* Avian is used for static colliders and spatial queries, not as the vehicle controller.

## Steering Semantics

Pinned behavior:

```text
A / ArrowLeft     steer left
D / ArrowRight    steer right
W + D             yaw right while moving forward
W + A             yaw left while moving forward
A or D alone      no idle yaw
```

Yaw is derived from steering plus signed movement/drive intent, not raw A/D input alone. Reverse uses reverse motion direction with reduced authority. The imported car assets can have their own node axes; if another asset disagrees visually, add a debug-only vehicle node/axis inspector and fix the asset binding at the visual layer, not the gameplay input layer.

## Track Generation Direction

Current generator:

* deterministic from recipe seed and piece count
* shape and surface are separate
* candidates are validated against occupied road-footprint sectors
* failure is explicit during development
* road surface meshes use `bevy_procedural_meshes`
* 2D path vocabulary and boundary extraction use `kurbo`
* Bevy curve APIs sample straight and constant-arc pieces into `PathFrame`s
* road contact colliders are piece-level Avian triangle meshes from sampled path edges
* rail collision colliders are track-level Avian compound capsules from merged sampled boundary paths

Generation validator should stay focused on correctness:

* valid piece continuity
* finish/checkpoint ordering
* sector occupancy and no overlap
* coherent curve samples
* trigger alignment
* generated count consistency
* rail boundary paths have coherent points and nonzero segments

Do not reject tracks for subjective variety or boringness in validation.

## Vehicle Physics Direction

The vehicle controller remains deterministic and custom, but should move away from direct arcade damping toward a simple coherent tire-force model.

Keep:

* throttle/brake/reverse input
* signed speed
* yaw/velocity basis
* four logical wheel contacts
* predictable reverse steering with reduced authority
* imported wheel visuals as readability only
* Avian static collider and spatial query integration

Replace:

* direct lateral velocity damping with lateral tire force derived from slip angle
* direct throttle acceleration with longitudinal tire force capped by available grip
* standalone drag/rolling multipliers with rolling resistance, aerodynamic drag, and boost force terms
* binary slide assist with slip-ratio/slip-angle driven yaw response
* global grip multipliers with per-wheel contact aggregation and normal-load weighting

Add:

* deterministic mass, gravity, wheelbase, track width, center-of-gravity height, and front/rear weight distribution
* static and first-pass axle load transfer for acceleration, braking, and cornering
* combined longitudinal/lateral friction budget per surface
* explicit debug values for normal load, axle tire force, friction limit, slip angle, and saturation
* optional tune curves for slip-to-force and drift breakaway/recovery

Avoid:

* drivetrain simulation
* tire heat/wear
* part systems
* realistic suspension
* deriving physics from imported art mesh topology
* delegating the full vehicle controller to the physics solver before the deterministic tire model is solid

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

Future crate candidates:

* `bevy_lookup_curve`: slip/grip/steering tuning curves once tire forces replace direct damping
* `rstar`: broad-phase indexing if piece counts or validation retries grow
* direct `lyon_tessellation`: only if `bevy_procedural_meshes` stops exposing enough control

Do not add a vehicle-controller crate unless it clearly supports Bevy 0.18, deterministic fixed-step use, custom tire/surface tuning, and does not force a solver-driven drivetrain/suspension model. Reject crates that force gameplay to depend on crate-specific geometry across module boundaries, break deterministic generation, increase Bevy version skew, or replace the custom yaw/velocity controller with character-controller semantics.

## Implemented Snapshot

Working now:

* app states: main menu, setup, driving, results
* pause overlay during driving
* generated scene cleanup on driving exit
* setup controls for player count, seed, length, and vehicle
* deterministic generated flat tracks with straights and sampled curves
* generated road meshes, road colliders, rails, checkpoint/finish triggers
* Avian-backed ground raycasts and rail collision queries
* imported SportsCar/SportsCar2 visuals
* imported wheel-node steering/spin where nodes are present
* fixed logical wheel contact samples affecting lateral grip
* swept checkpoint/finish trigger detection
* HUD and debug overlay
* session-only ghosts and leaderboard

Recent fixes:

* removed spawned cuboid wheel visuals
* removed per-surface wheel recoloring
* pinned A/D steering semantics with tests
* removed idle yaw from raw A/D steering
* aligned imported wheel visual steering with gameplay steer
* fixed pause-menu cleanup on `Driving` exit
* changed shell UI cleanup to despawn root entities only, avoiding duplicate child despawn warnings
* centralized shell button spawning in a shared shell UI helper
* collapsed wheel contact storage to one fixed sample array
* replaced per-segment road cuboid colliders with path-derived Avian road mesh colliders
* replaced handrolled car-vs-rail rectangle collision with Avian shape casts against path-derived rail colliders
* merged rail collision into track-level left/right boundary paths to remove piece seam gaps
* changed checkpoint/finish detection from point containment to swept trigger crossing
* tightened primitive validation around rail boundary paths
* replaced direct acceleration/lateral damping with a deterministic tire-force integration result
* converted surface handling fields from arcade multipliers to friction, rolling resistance, aerodynamic drag, and boost acceleration
* added tire-force debug output and tests for surface grip and coasting resistance
* split tire-force output into front/rear axle load, lateral force, and saturation terms
* changed grip/sliding transitions to saturation breakaway/recovery with hysteresis
* extended debug output and tests for axle loads, axle saturation, and rear-surface grip loss
* changed wheel contacts from axle-average friction to four explicit FL/FR/RL/RR friction inputs
* added per-wheel normal load, lateral force, and saturation outputs
* replaced the fixed lateral-force split with steering-angle-aware front/rear demand

## Pending Work

Active gameplay:

* Tune per-surface friction, rolling resistance, drift breakaway/recovery, and lateral stiffness against actual gameplay feel.
* Add slip-to-force and drift-yaw tune curves only if the current scalar knobs are too coarse.
* Improve setup/results/pause UI polish inside the current shell modules.
* Move the fixed shape catalog into piece metadata with connection rules and candidate weighting when generation needs more control.

Debug and tuning support:

* Add a compact debug/tuning UI for tire forces and surface parameters, likely via `bevy_egui`.
* Add a visual wheel-contact and force overlay if text debug output is not enough for tuning.
* Add a debug-only imported vehicle node/axis inspector only if future assets make wheel axes ambiguous.

Parked until scope expands:

* Evaluate `bevy_lookup_curve` only when tune curves are actually needed.
* Add `rstar` only if validation performance needs it.
* Add banked track frames after the flat road/rail/contact pipeline is coherent.
* Add unreachable-finish validation once branching, verticality, or non-forward pieces exist.
* Add vertical pieces only after slope/ramp recovery and placement validation exist.
