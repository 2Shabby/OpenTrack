# Arcade Precision Racer — Planning Design Doc

## Product Identity

A local-first, Trackmania Turbo-like arcade precision racer focused on generated tracks, hotseat time trials, fast retries, ghosts, surface mastery, and clean driving lines.

The game is about one active driver at a time, repeated attempts, generated course variety, and local competition.

It is not a realistic racing simulator, vehicle sandbox, car-combat game, or online racing platform.

## Core Product

Players take turns on generated tracks and compete for the best time.

Full tracks are not hand-authored. They are procedurally assembled from authored modular pieces.

The core loop is:

```text
Choose recipe → Generate track → Drive → Retry → Beat ghost → Climb leaderboard
```

## In Scope

* 3D arcade time-trial racing
* Bevy/Rust implementation
* local hotseat play
* effectively unlimited local players
* one active car at a time
* generated full tracks
* authored modular track pieces
* replayable recipes/seeds
* checkpoints, timer, leaderboard
* ghost replays
* multiple surfaces
* fast restart/retry
* simple car visuals
* debug/tuning tools

## Out of Scope

* hand-authored full tracks
* manual track editor
* online multiplayer
* simultaneous racing
* realistic sim racing
* damage
* car upgrades or parts
* tuning garage
* AI racers
* traffic
* combat
* open world
* campaign/story
* economy/progression grind
* soft-body physics
* destructible environments
* deep mechanical vehicle simulation

## Engine Direction

Use Bevy as the main engine.

Bevy owns:

* ECS/game state
* rendering
* input
* camera
* UI
* audio
* session state
* track entities
* ghost entities
* debug tooling

Use Avian or Rapier only for:

* static collision
* raycasts
* trigger volumes
* collision queries

The vehicle controller remains custom.

```text
Bevy ECS
+ custom arcade vehicle controller
+ Avian/Rapier collision queries
+ authored piece library
+ procedural track assembler
```

## Physics Backend

Prefer an ECS-native architecture, but avoid coupling gameplay logic directly to the physics crate.

Avian is attractive because it fits Bevy ECS well. Rapier is more mature and conservative.

Use a small project-owned physics query layer so the backend can be swapped later.

```rust
trait TrackPhysicsQueries {
    fn raycast_ground(...) -> Option<GroundHit>;
    fn cast_car_shape(...) -> Option<CarHit>;
    fn ground_at(...) -> GroundContact;
}
```

## Vehicle Controller

The car is a compact arcade controller, not a mechanical simulation.

Car state:

```text
transform
velocity
yaw/angular state
visual body pitch/roll state
grounded/airborne state
four wheel contact samples
current surface
throttle/brake/steer input
slip/drift state
surface effect state
reset state
```

Core behavior:

* acceleration
* braking
* steering
* grip/slip
* per-wheel surface/contact sampling for handling decisions
* visual body roll/pitch from steering, acceleration, braking, and impacts
* airborne control
* slope response
* wall/floor collision response
* reset/recover

Avoid:

* engine simulation
* gearbox simulation
* tire temperature
* brake heat
* damage
* upgrades
* part-based car systems

Design principle:

```text
Depth comes from tracks, surfaces, ghosts, retries, and player execution — not vehicle subsystem complexity.
```

Arcade drift direction:

* keep the custom gameplay controller instead of adopting a full tire simulator
* model drift as an explicit handling state/assist layered over the current velocity/yaw controller
* use lateral slip, speed, steering, throttle, and braking to enter/hold/exit drift
* evaluate drift cases from four wheel contacts: all-road, split-surface, front-only, rear-only, airborne/partial contact
* allow controlled counter-steer behavior instead of full spin-out simulation
* tune drift by changing lateral grip, yaw authority, and damping rather than adding drivetrain complexity
* keep body roll and wheel pose as visual/readability primitives unless a gameplay reason requires them to affect handling
* keep reverse steering separate from forward drift behavior so backing up stays predictable

## Surfaces

Surfaces are data-driven and affect handling.

Core surfaces:

* asphalt
* dirt
* ice
* boost

Surface parameters:

```text
longitudinal_grip
lateral_grip
rolling_resistance
acceleration_multiplier
steering_multiplier
drag
boost_force
```

Surface changes should be readable and should materially affect driving lines.

## Track Generation

Tracks are generated from authored pieces.

Track sources:

```text
fresh generated track
```

Saved/shared track mechanics are currently out of scope. Keep generation deterministic from recipe + seed so this can be revisited later without changing the generator contract.

No hand-authored full tracks.

Track recipe:

```text
seed
length
elevation/verticality bias
speed_bias
technicality
piece_set
theme
generation_rules
```

Track piece:

```text
entry transform
exit transform
collision mesh
visual mesh
surface type
connection rules
trigger/checkpoint data
```

Curved piece authoring direction:

* stop treating curves as rotated rectangles
* define a centerline/path first, then derive all other geometry from it
* sample the centerline at fixed arc-length intervals
* build road mesh vertices from sampled tangent/right vectors and track width
* build rails from the same samples and edge offsets
* build surface/collision zones from the same samples or segment rectangles
* place checkpoint/finish trigger lines perpendicular to the sampled tangent
* validate by checking adjacent samples, edge continuity, rail continuity, and trigger alignment

Initial curve implementation should use constant-curvature arcs or short sampled splines before clothoids. Clothoid-style transitions are a later quality upgrade for smoother high-speed turns, not a prerequisite for the first correct curve pieces.

Generation passes:

```text
route plan
piece assembly
surface pass
checkpoint pass
validation
```

Validation checks:

* valid piece connections
* checkpoint order
* reasonable gaps/slopes
* finish reachable by construction
* sufficient recovery after jumps/drops where used

## Verticality

Verticality is supported, but it is not the whole identity.

It appears through pieces like:

* slopes
* ramps
* drops
* elevated roads
* bridges/platforms

It should be controlled by recipe bias, not forced into every track.

## Hotseat

Hotseat is local and turn-based.

Session state:

```text
track recipe
track seed
player list
current player
leaderboard
```

No hard player cap. UI should handle many names pragmatically.

## Ghosts

Initial ghost system stores sampled transforms.

Replay data:

```text
player name
track recipe
track seed
generator version
physics tuning version
finish time
checkpoint times
sampled transforms
```

Input replay can be added later only if needed.

Ghost requirements:

* show session leader
* show own previous best
* optionally show last run
* visually non-blocking
* cheap to render

## Timing and Leaderboards

Timer starts when the run begins and stops at finish.

Track timing:

* total time
* checkpoint splits
* best split comparison
* invalid run state if needed

Leaderboard sorting:

```text
finish_time ascending
then checkpoint progress
then latest run order
```

Leaderboards are per track recipe, seed, and generator version.

## App Flow and Screens

The first playable shell should be simple and local-first.

Required screens:

* main menu
* track recipe/seed selection
* hotseat player setup
* car color selection
* in-run HUD
* pause menu
* post-run results
* leaderboard/results screen

Initial screen behavior:

* main menu starts a local hotseat session or quits
* setup screens collect player names, car colors, recipe length, and seed
* pause menu can resume, restart current run, return to setup/menu, or quit
* post-run screen shows finish time, checkpoint progress, current leaderboard, and next-player/retry actions

Keep save-game mechanics out of scope. Menus may keep session state in memory only.

## Camera

Camera is part of the driving feel.

Initial camera:

* chase camera
* speed-based distance
* mild lookahead
* smoothing
* reset-safe behavior

Camera must prioritize:

* readability
* speed feel
* surface visibility
* landing visibility where relevant
* minimal disorientation

## Debug and Tuning Tools

Debug tooling is required from the start.

Useful debug views:

* speed
* grounded state
* current surface
* grip values
* acceleration
* lateral velocity
* checkpoint status
* replay recording state
* track seed
* recipe values
* physics version
* generator version

Use a simple in-game debug UI for tuning constants.

## Suggested Crate Stack

```text
bevy                  engine
avian3d or rapier3d   physics queries/colliders
serde                 data serialization
ron/json              tuning and track definitions
rand_chacha           deterministic generation
bevy_egui             debug UI
leafwing-input-manager optional input handling
```

Keep dependencies modest until the car and track loop works.

## Current Implementation Dependency Decisions

Initial dependency set:

```text
bevy 0.18.1           engine/runtime
avian3d 0.6           preferred physics query backend, not gameplay-owned
bevy_egui 0.39.1      debug/tuning UI
serde 1               recipe/replay serialization, not save-game mechanics
ron 0.10              human-editable tuning/piece files
rand/rand_chacha 0.9  deterministic procedural generation
```

Dependency policy:

* Pin Bevy exactly enough to avoid accidental ecosystem skew during early development.
* Add Avian now so the physics-query seam can be designed, but do not let vehicle feel depend on Avian internals.
* Do not add asset, audio, input abstraction, or editor crates until the driving sandbox has a stable shape.
* Do not hand-roll road-path and mesh infrastructure indefinitely. Audit Bevy-compatible procedural/path crates before expanding beyond constant-radius arcs.
* Prefer plain Bevy input at first. Revisit `leafwing-input-manager` only when hotseat bindings or controller profiles become painful.

Current crate audit direction:

* Use Bevy's built-in curve/spline APIs first for centerline representation if they can replace custom arc sampling without fighting track validation.
* Use `bevy_procedural_meshes 0.18` for road mesh tessellation before adding more custom mesh builders.
* Evaluate `bevy_lookup_curve` for editable tuning curves once handling values move out of hard-coded constants.
* Avoid KCC-focused crates as vehicle-controller replacements unless the car becomes a kinematic-body problem; current arcade handling still needs a custom yaw/velocity model.

Crate swap audit:

* `bevy::math::cubic_splines` / `bevy::math::curve`: strong candidate for replacing bespoke centerline sampling once track pieces move from constant arcs to spline-authored paths. This should be tried before adding a separate spline crate.
* `bevy_procedural_meshes 0.18`: adopted for current road surface tessellation through its Lyon-backed `PMesh::fill` path.
* `kurbo`: strong candidate for non-Bevy 2D path vocabulary, arc/Bezier flattening, path simplification, and offset/stroke geometry. Prefer this if Bevy curves are insufficient for validation or offset-curve work.
* `lyon` / `lyon_tessellation`: useful as direct lower-level tools only if future road pieces need tessellation control that `bevy_procedural_meshes` does not expose.
* `rstar`: strong candidate for replacing O(n^2) overlap scans once piece counts grow. It does not replace SAT/geometry correctness, but it should narrow candidate overlap checks efficiently.
* `bevy_lookup_curve`: useful for tuning/feel curves, not track generation. Add when acceleration/grip/steering/drift values move from constants to data.
* KCC crates such as `bevy-ichun`/`bevy_ahoy`: not a current vehicle-controller replacement. They solve character movement over collision worlds, while this project needs arcade car yaw/velocity/surface/drift behavior.

Crate audit pass results:

* There is no obvious Bevy crate that replaces the whole generated racing-track problem end to end. The useful crates are lower-level: curve representation, path flattening/stroking, tessellation, spatial indexing, and tuning curves.
* The current code should therefore keep a small project-owned track contract and replace the most error-prone internals behind it instead of moving generation ownership wholesale to one crate.
* The first swap should be path representation, not mesh output. If the centerline abstraction is wrong, every mesh/collider/trigger crate integration will inherit that mismatch.
* Bevy built-in curves are the first candidate because they are already in the engine dependency and match Bevy 0.18.1. Use them to prototype `TrackPath` as sampled frames generated from curve primitives.
* `kurbo` is the next path-vocabulary candidate if Bevy curves do not give enough 2D path operations for road edge offsets, flattening tolerance, or future Bezier/arc authoring.
* `lyon_tessellation` is the robust low-level tessellation candidate when road surfaces stop being simple strips. It is most valuable for fills/strokes, joins, caps, and non-rectangular future pieces.
* `bevy_procedural_meshes` targets Bevy 0.18 and already depends optionally on `lyon`; current road surface generation now uses it as the single mesh path instead of maintaining hand-built strip mesh code.
* `rstar` should replace broad-phase overlap scanning once generator piece counts or candidate retries grow. It should not replace project geometry correctness; it only narrows which road spans reach SAT validation.
* `bevy_lookup_curve` belongs in driving/tuning once hard-coded grip, acceleration, steering, and drift values become data. It is not a generation crate.
* `bevy_mod_raycast` is not useful while Avian already owns physics queries. It targets older Bevy compatibility in its current docs and would duplicate the project-owned Avian query layer.
* Direct `parry3d` use is not the next step because Avian already wraps collision/query primitives. Revisit only if we need a query Avian cannot expose cleanly.

Crate swap order:

1. Introduce a project-owned `TrackPath` abstraction that produces `PathFrame` samples and can be backed first by current arcs, then by Bevy curves. Done.
2. Replace constant-arc generation with Bevy curve-backed sampling for the same straight/curve pieces, keeping validation and generated output shape unchanged. Done.
3. Use `bevy_procedural_meshes` as the current road-surface tessellation path.
4. If future curved/widened/branched road pieces need stronger path operations, spike `kurbo + lyon_tessellation` for path flattening, edge construction, and road-surface triangulation.
5. Add `rstar` broad-phase indexing after generation supports larger routes or if overlap validation starts dominating retries.
6. Move vehicle feel constants to tuning assets and evaluate `bevy_lookup_curve` for non-linear speed/grip/steering response curves.

Crate reject criteria:

* reject a crate if it forces gameplay code to depend on crate-specific geometry types across module boundaries
* reject a crate if it cannot preserve deterministic generation from recipe + seed
* reject a mesh crate if collider spans, rail spans, trigger normals, and visual mesh edges no longer derive from the same source path
* reject a crate if it increases Bevy version skew or brings in a heavy subsystem for a narrow helper job
* reject a vehicle/controller crate if it replaces arcade yaw/velocity/drift semantics with character-controller semantics

## Proposed Code Architecture

Source layout target:

```text
src/main.rs
src/geometry.rs
src/driving/
src/surface/
src/track/
src/physics/
src/run/
src/hotseat/
src/ghost/
src/debug/
assets/
assets/tuning/
assets/tracks/pieces/
```

Early module responsibilities:

* `driving`: custom car state, input-to-control mapping, fixed-step vehicle update.
* `surface`: `SurfaceKind`, `SurfaceParams`, lookup from ground hits and triggers.
* `track`: piece definitions, piece spawning, recipe and seed data.
* `physics`: project-owned query traits, collider components/layers, and backend adapter.
* `run`: timer, checkpoint order, finish state, retry/reset flow.
* `hotseat`: players, turn order, per-track leaderboard.
* `ghost`: transform sampling, playback entity spawning, replay serialization.
* `debug`: lightweight Bevy UI now; egui tuning panels later if the controls outgrow text.

## Implementation Status

Current working prototype:

Controls:

```text
W / ArrowUp       throttle
S / ArrowDown     brake/reverse
A / ArrowLeft     steer left
D / ArrowRight    steer right
R                 reset car
Escape            pause/resume
F3                debug overlay
P                 add local hotseat player while waiting in the driving sandbox
N                 next hotseat player after finish in the driving sandbox
```

Implemented systems:

* Bevy/Rust app scaffold with fixed timestep driving.
* Layered arcade driving model with explicit forward/braking/reverse modes.
* Data-driven surfaces: asphalt, dirt, ice, boost.
* Project-owned physics-query boundary for ground/surface and rail collision.
* Deterministic modular track generation from seed and length.
* Straight and sampled constant-curvature curve pieces.
* Modular track piece spawning with generated road meshes, rails, checkpoint triggers, and finish triggers.
* Track validation for finish count, checkpoint-before-finish order, piece frame counts, segment lengths, rail counts, trigger alignment, route yaw bounds, sector occupancy, and piece continuity.
* Session timer, checkpoint progress, restart, hotseat players, in-memory leaderboard, and session-only ghost replay.
* Realistic sports-car mesh from `~/Downloads/Realistic Car Pack - Nov 2018.zip`.
* Off-track grass/forest scenery using textures from `~/Downloads/LowpolyForestPack.zip`.
* Debug overlay for seed, pieces, generated entity counts, player state, run state, speed, signed speed, drive mode, surface, and tuning values.
* App state shell with main menu, setup screen, driving state, pause overlay, results screen, and clean spawned-scene teardown.
* Setup controls for player count, seed, track length, and car color.

Current constraints:

* No save-game mechanics, persisted best times, player profiles, or shared-track storage.
* Forest FBX files are not loaded directly; scenery uses generated low-poly geometry with the pack textures.
* Hotseat player setup currently chooses only player count; player names are generated as `Driver N`.
* Pause is represented by `PauseState` during `GameState::Driving`, not as a separate `Paused` app state.
* Track generation has flat straights and flat curves only; no slopes, ramps, jumps, bridges, or drops yet.
* Recipes expose only seed and length; speed bias, technicality, verticality, piece sets, themes, and rule sets are not modeled yet.
* Ghosts are session-only best ghosts and are not keyed by player, recipe, seed, generator version, or physics tuning version.
* Road and rail queries are backed by Avian static cuboid colliders, with project-owned metadata still used for surface lookup and gameplay response semantics.
* Road/rail colliders are on separate collision layers so future dynamic/query-only colliders do not pollute track queries.
* No audio, controller support, input rebinding, player profiles, or persisted settings.

Completed recent code changes:

1. Added Bevy app states for `MainMenu`, `Setup`, `Driving`, and `Results`.
2. Moved track, car, camera, lighting, ghost, and debug spawning behind state transitions.
3. Added main menu and local hotseat setup flow.
4. Added recipe controls for seed and length.
5. Added car color selection using the existing sports-car mesh.
6. Added pause overlay with resume, restart, setup/menu, and quit actions.
7. Added post-run/results screen with leaderboard and next-player/retry actions.
8. Added sampled curve generation and road mesh generation from path frames.
9. Fixed rail collision query selection to resolve the deepest overlapping rail contact.
10. Added Avian physics plugins and static rail colliders behind the project-owned physics-query boundary.
11. Reworked track generation into sector-occupancy planning with bounded backtracking.
12. Added sector occupancy checks and multi-seed generation validation tests.
13. Added static Avian road colliders and Avian-backed ground raycasts for surface lookup.
14. Added road/rail collision layers and layer-filtered spatial queries.
15. Added route-level generation checks for checkpoint order, route yaw bounds, and curve sample coherence.
16. Split physics into focused modules for components, layers, and Avian-backed queries.
17. Renamed `spatial` to `geometry` so shared pose/bounds helpers live under a clearer layer.
18. Split track generation into route assembly, primitive path generation, shared generation types, and validation modules.
19. Started a crate swap audit for Bevy curves, procedural mesh generation, path geometry, tessellation, spatial indexing, and tuning curves.
20. Identified path representation, mesh generation, and spatial broad-phase as the highest-value crate swap points.
21. Added a `TrackPath` sampling boundary so future Bevy curve or `kurbo` experiments can replace path internals without changing track pieces, meshes, colliders, rails, or triggers.
22. Replaced manual straight/arc frame sampling internals with Bevy's built-in curve API while preserving the current `PathFrame` output contract.
23. Replaced the hand-built road strip mesh indices with `bevy_procedural_meshes` / Lyon fill tessellation as the single road surface mesh path.
24. Split shell UI code by screen/flow: main menu, setup, results, and pause.
25. Grouped large Bevy system signatures with local `SystemParam` structs and query aliases so driving/debug systems are easier to read and clippy-clean without adding generic app layers.
26. Added a focused road mesh unit test that verifies procedural road meshes land on the X/Z ground plane.
27. Removed generation presets from the setup UI; there is one generation mode controlled by seed and length.
28. Moved overlap fitting to planner-owned occupied sectors instead of low-level road rectangle SAT checks.
29. Removed silent alternate track generation; planner failure is explicit during development.

Next code changes:

1. Replace hard-coded candidate lists with piece metadata, connection rules, and candidate weighting.
2. Audit whether `kurbo` should own future 2D path offset/flattening once road pieces need more than frame-derived polygons.
3. Add dedicated road/rail primitive validation that compares generated mesh edges, collider spans, and trigger normals for each segment.
5. Add `rstar` spatial indexing if overlap validation becomes a measurable bottleneck or piece counts increase substantially.
6. Add unreachable-finish validation once branching, verticality, or non-forward pieces exist.
7. Add vertical track pieces only after the generator can validate slope/ramp recovery and support placement.
8. Improve setup/results/pause UI polish within the current screen modules instead of rebuilding the shell architecture.
9. Move handling constants toward data/tuning assets, then evaluate `bevy_lookup_curve`.
10. Keep player-profile and persistence work deferred until generation and physics-query stability improve.

## Current Code Slice

Procedural assembly has started:

* `TrackRecipe` owns seed and piece count.
* Startup track pieces are generated deterministically from recipe + seed.
* Surfaces are assigned by deterministic RNG.
* Road colliders, rail colliders, and checkpoint/finish triggers now use oriented bounds.
* Track generation now stores explicit entry/exit transforms per piece to keep adjacent pieces and lines aligned.
* Track generation plans pieces with occupied sectors and bounded backtracking before emitting geometry.
* Straight pieces now derive center pose, length, road bounds, rails, and road colliders from entry/exit centerline frames.
* Road surface visuals are generated as meshes from path frames instead of spawned as rotated plane primitives.
* Generated tracks now include deterministic sampled arc curves.
* Road colliders and rails are generated per sampled path segment, not per whole piece rectangle.
* Each path segment now owns road surface bounds and optional rail bounds from one primitive.
* Track code is split into route generation, piece geometry, mesh spawning, validation, path primitives, shared generation types, and scenery modules.
* Track path generation now goes through `TrackPath`, which samples straight and constant-arc paths through Bevy's curve API into `PathFrame`s and is the intended seam for future `kurbo` replacement if Bevy curves become insufficient.
* Road surface visuals now use `bevy_procedural_meshes` fill tessellation from generated path-frame edge polygons instead of hand-built triangle indices.
* Generated scene entities are tagged by semantic role: environment, scenery, road surface, rail, trigger, player, camera, and lighting.
* Shared geometry types (`Pose2`, `OrientedRect`) are the single source of truth for X/Z poses and oriented bounds.
* Road colliders, rail colliders, triggers, and track pieces no longer carry parallel center/yaw/extent conventions.
* Shell code is split by screen/flow so future setup, results, and pause UI changes can stay local.
* Driving and debug systems use small local Bevy `SystemParam` groups where the system context had grown too wide.
* Physics query results, hotseat state, ghost samples, and car reset semantics have been narrowed to the minimum current API.
* Avian `PhysicsPlugins` are installed through the local physics plugin.
* Physics code is layered into components, collision layers, and Avian query adapters instead of one large module.
* Spawned road spans have static Avian cuboid colliders and project-owned `RoadCollider` metadata.
* Spawned rails have static Avian cuboid colliders and project-owned `RailCollider` metadata.
* Ground/surface lookup uses Avian downward raycasts filtered to the road collision layer.
* Car-vs-rail collision uses Avian shape intersection queries to find overlapping static rail colliders, then resolves through the project-owned response type.
* Generated tracks use straight modules and sampled flat curves; vertical modules are still pending.
* Rail collision resolves laterally rather than using generic rectangle end-cap normals.
* The generated track keeps one checkpoint and one finish for the current run-loop contract.
* Debug overlay shows generated seed, piece count, and actual/expected road/rail/trigger counts.
* Driving model reports slip angle and grip/sliding state, then applies first-pass drift lateral-grip and yaw assists.
* App now starts at a basic main menu and enters gameplay through `MainMenu -> Setup -> Driving`.
* Track/session spawning now happens on entering `Driving`; generated scene cleanup is state-owned.
* Escape opens an in-game pause overlay; resume preserves the active driving scene and main menu exits cleanly.
* Setup screen configures player count, track seed, track length, and car color before spawning gameplay.
* Finished runs move to a results screen with retry, next-player, main-menu, quit, and in-memory leaderboard display.
* Driving model samples four wheel surface contacts and reports them in debug for split-surface drift tuning.
* First-pass drift assist uses slip state to reduce lateral damping and add controlled yaw assist while sliding.
* Car visuals now use separate body and wheel primitives with body roll/pitch, front-wheel steering, and wheel spin.
* In-run HUD shows driver, timer, checkpoint progress, speed, best time, and ghost time; verbose debug is toggled with F3.
* Grass field, forest, and rocks are placed relative to generated track bounds instead of fixed world coordinates.
* Track validation now checks empty tracks, missing finish/checkpoint, checkpoint-before-finish order, short pieces, zero-length segments, route yaw bounds, connections, occupied sectors, and generated counts.
* Four-wheel contact samples now affect lateral grip, so split-surface cases change handling.
* Off-track surface is now grass instead of asphalt, with slower acceleration and higher drag.
* Wheel visuals tint by contact surface, and HUD shows handling/surface state.
* Setup recipe controls now expose one generation mode: player count, seed, length, and car color.
* `track/piece.rs` now exposes one piece geometry contract for road spans, rail spans, and checkpoint/finish trigger lines.
* Track spawning and validation now consume the same generated piece geometry instead of rebuilding road, rail, and trigger bounds separately.
* Curves are now explicit generated piece kinds instead of straight pieces with curved frames, and the generator tries curve candidates before straights when sectors fit.
* Pause flow now supports resume, restart, setup, main menu, and quit with run state reset on scene exits.
* Ground queries now distinguish road/off-track source from handling surface through Avian road raycasts; HUD/debug show both instead of treating lookup misses as just another road surface.

## Pending Work

### Product Shell

Current shell flow is prototype-complete for the local session loop, and the code is split by screen so setup/results/pause polish can happen locally. Player profiles and persistence remain deferred while physics queries and sequential generation are the active focus.

### Gameplay and Track

Pending:

* expand the piece-library contract with piece metadata and connection rules
* expand generated piece sequences beyond straight, curve, checkpoint, and finish pieces
* improve curve piece variety and primitive validation beyond constant-radius arcs
* checkpoint and finish line placement for every future piece type
* add metadata-driven connection rules and candidate weighting
* add unreachable-finish validation once routes become more complex

### Vehicle and Feel

Pending:

* continued tuning of reverse/brake/steering feel
* add true support/airborne states once vertical road/support geometry exists
* tune the current drift assist across surface and speed cases
* drift case matrix: brake-entered drift, throttle-held drift, counter-steer recovery, split-surface slide, ice slide, boost slide, reverse/no-drift
* counter-steer and spin-out damping assist tuned for Trackmania-like readability
* add airborne/grounded distinction to wheel contact hints
* tune body roll/pitch response for boost and wall impacts
* better collision response at high speed
* add stronger boost and impact visual feedback
* optional controller support once keyboard feel stabilizes

### UI/HUD

Pending:

* improve HUD layout styling beyond text-only display

### Assets and Presentation

Pending:

* improve track visuals beyond flat planes and cuboid rails
* add simple audio feedback after the core loop is stable

### Persistence

Out of scope for now:

* saved tracks
* saved profiles
* persisted leaderboards
* replay file export/import

## Milestones

### 1. Driving Sandbox — Mostly Complete

One car, flat plane, fixed timestep, steering, throttle, brake, camera, reset.

Success condition:

```text
The car is controllable and fun enough to keep testing.
```

### 2. Surface Handling — Prototype Complete

Asphalt, dirt, ice, boost, with debug tuning.

Success condition:

```text
Each surface changes driving lines clearly.
```

### 3. Track Pieces — In Progress

Basic pieces connected in code/data: straight, curve, checkpoint, finish. Slopes, ramps, drops, bridges, and elevated supports are still pending.

Success condition:

```text
A complete piece-chain run is possible from the procedural generator.
```

### 4. Complete Run — Prototype Complete

Timer, checkpoints, finish, restart flow.

Success condition:

```text
A full run can be completed, timed, and retried.
```

### 5. Hotseat — Prototype Complete, Setup Polish Pending

Player list, turn order, leaderboard.

Success condition:

```text
Many players can take turns on the same track.
```

### 6. Ghosts — Prototype Complete, Scope Narrow

Sampled transform replay for the current session best run.

Success condition:

```text
Players can chase visible previous runs.
```

### 7. Procedural Assembly — In Progress

Generate complete tracks from recipe + seed + piece library.

Success condition:

```text
Generated tracks are playable, repeatable, and fun enough to retry.
```

### 8. Product Shell — Prototype Complete

Main menu, setup, pause, results, and leaderboard screens.

Success condition:

```text
A local session can be configured, played, paused, completed, and repeated without debug-key workflows.
```

### 9. Vehicle Feel Upgrade — In Progress

Four wheel contact semantics, drift assist cases, visual wheel motion, and body roll/pitch.

Success condition:

```text
The car clearly communicates grip, slide, braking, acceleration, and recovery while staying arcade-readable.
```

## Main Risks

* car feel is not fun
* collision gets unstable at speed
* generated tracks are boring or invalid
* camera hurts readability
* scope creeps into sim racing or online multiplayer

## Design Rule

Prefer features that improve:

```text
fast retry
clean racing lines
surface mastery
ghost chasing
hotseat competition
generated track variety
```

Reject features that mainly add:

```text
simulation depth
vehicle subsystem complexity
online infrastructure
manual track authoring
progression grind
```
