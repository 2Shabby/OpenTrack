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
positive steer    yaw right in the current +Z-forward car basis
```

The imported car assets can have their own node axes. Current implementation keeps imported front-wheel steering aligned with gameplay steer. If another asset disagrees, add a debug-only vehicle node/axis inspector and fix the asset binding at the visual layer, not the gameplay input layer.

## Track Generation Direction

Current generator:

* deterministic from recipe seed and piece count
* shape and surface are separate
* candidates are validated against occupied road-footprint sectors
* failure is explicit during development
* road surface meshes use `bevy_procedural_meshes`
* 2D path vocabulary and boundary extraction use `kurbo`
* Bevy curve APIs sample straight and constant-arc pieces into `PathFrame`s

Generation validator should stay focused on correctness:

* valid piece continuity
* finish/checkpoint ordering
* sector occupancy and no overlap
* coherent curve samples
* trigger alignment
* generated count consistency

Do not reject tracks for subjective variety or boringness in validation.

## Vehicle Direction

The vehicle controller remains custom and arcade-focused.

Keep:

* throttle/brake/reverse input
* signed speed
* yaw/velocity basis
* surface multipliers
* four logical wheel contacts
* lateral grip and first-pass slide assist
* predictable reverse steering with reduced authority
* imported wheel visuals as readability only

Avoid:

* drivetrain simulation
* tire heat/wear
* part systems
* realistic suspension
* deriving physics from imported art mesh topology

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

* `rstar`: broad-phase indexing if piece counts or validation retries grow
* `bevy_lookup_curve`: tuning curves for steering/grip/drift once constants move to data
* direct `lyon_tessellation`: only if `bevy_procedural_meshes` stops exposing enough control

Reject crates that force gameplay to depend on crate-specific geometry across module boundaries, break deterministic generation, increase Bevy version skew, or replace the arcade yaw/velocity controller with character-controller semantics.

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
* four wheel contact samples affecting lateral grip
* HUD and debug overlay
* session-only ghosts and leaderboard

Recent fixes:

* removed spawned cuboid wheel visuals
* removed per-surface wheel recoloring
* pinned A/D steering semantics with tests
* aligned imported wheel visual steering with gameplay steer
* fixed pause-menu cleanup on `Driving` exit
* changed shell UI cleanup to despawn root entities only, avoiding duplicate child despawn warnings

## Pending Work

Highest priority:

1. Verify the A/D steering behavior in-game after the visual-axis correction.
2. Add a debug-only imported vehicle node/axis inspector if wheel behavior is still ambiguous.
3. Replace per-segment cuboid rail colliders with path-derived continuous edge collision primitives for curves.
4. Classify true boundaries versus internal seams so rails never appear at surface-transition seams.
5. Add road/rail primitive validation comparing mesh edges, boundary paths, collider spans, and trigger normals.

Next:

6. Move the fixed shape catalog into piece metadata with connection rules and candidate weighting.
7. Add banked track frames after the flat road/rail/contact pipeline is coherent.
8. Add `rstar` broad-phase indexing only if validation performance needs it.
9. Add unreachable-finish validation once branching, verticality, or non-forward pieces exist.
10. Add vertical pieces only after slope/ramp recovery and placement validation exist.
11. Improve setup/results/pause UI polish inside the current shell modules.
12. Move handling constants toward data/tuning assets, then evaluate `bevy_lookup_curve`.
