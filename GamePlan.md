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
    fn surface_at(...) -> SurfaceKind;
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
difficulty
surface_mix
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
difficulty rating
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
* Prefer plain Bevy input at first. Revisit `leafwing-input-manager` only when hotseat bindings or controller profiles become painful.

## Proposed Code Architecture

Source layout target:

```text
src/main.rs
src/app.rs
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
* `physics`: project-owned query traits and backend adapter.
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
P                 add local hotseat player while waiting
N                 next hotseat player after finish
```

Implemented systems:

* Bevy/Rust app scaffold with fixed timestep driving.
* Layered arcade driving model with explicit forward/braking/reverse modes.
* Data-driven surfaces: asphalt, dirt, ice, boost.
* Project-owned physics-query boundary for ground/surface and rail collision.
* Modular track piece spawning with rails, checkpoint triggers, and finish triggers.
* Session timer, checkpoint progress, restart, hotseat players, in-memory leaderboard, and session-only ghost replay.
* Realistic sports-car mesh from `~/Downloads/Realistic Car Pack - Nov 2018.zip`.
* Off-track grass/forest scenery using textures from `~/Downloads/LowpolyForestPack.zip`.
* Debug overlay for seed, pieces, generated entity counts, player state, run state, speed, signed speed, drive mode, surface, and tuning values.

Current constraints:

* No save-game mechanics, persisted best times, player profiles, or shared-track storage.
* Forest FBX files are not loaded directly; scenery uses generated low-poly geometry with the pack textures.
* Generated tracks are currently straight piece chains only.
* No app state shell yet: gameplay starts directly at the driving sandbox.
* No main menu, pause menu, setup screens, car color selection, or results screen yet.

Next code changes:

1. Add Bevy app states for `MainMenu`, `Setup`, `Driving`, `Paused`, and `Results`.
2. Move direct startup spawning behind state transitions so a session can be created, torn down, and recreated cleanly.
3. Add basic main menu and hotseat setup screen.
4. Add recipe/seed/length selection.
5. Add car color selection using the existing sports-car mesh.
6. Add pause menu with resume, restart, setup/menu, and quit actions.
7. Add post-run/results screen with leaderboard and next-player/retry actions.
8. Expand procedural assembly from fixed straight chain to generated piece sequences.
9. Add simple curve pieces only after curved road visuals and colliders share one geometry contract.
10. Split tuning values into editable resources or RON assets once values settle.

## Current Code Slice

Procedural assembly has started:

* `TrackRecipe` owns seed and piece count.
* Startup track pieces are generated deterministically from recipe + seed.
* Surfaces are assigned by deterministic RNG.
* Surface zones, rail colliders, and checkpoint/finish triggers now use oriented bounds.
* Track generation now stores explicit entry/exit transforms per piece to keep adjacent pieces and lines aligned.
* Straight pieces now derive center pose, length, road bounds, rails, and surface zones from entry/exit centerline frames.
* Road surface visuals are generated as meshes from path frames instead of spawned as rotated plane primitives.
* Generated tracks now include deterministic sampled arc curves.
* Road surface zones and rails are generated per sampled path segment, not per whole piece rectangle.
* Each path segment now owns road surface bounds and optional rail bounds from one primitive.
* Track code is split into `track/generation.rs`, `track/spawn.rs`, and `track/scenery.rs`.
* Generated scene entities are tagged by semantic role: environment, scenery, road surface, rail, trigger, player, camera, and lighting.
* Shared spatial types (`Pose2`, `OrientedRect`) are the single source of truth for X/Z poses and oriented bounds.
* Surface zones, rail colliders, triggers, and track pieces no longer carry parallel center/yaw/extent conventions.
* Physics query results, hotseat state, ghost samples, and car reset semantics have been narrowed to the minimum current API.
* Generated tracks intentionally use straight modules only until real curved road geometry/colliders are implemented.
* Rail collision resolves laterally rather than using generic rectangle end-cap normals.
* The generated track keeps one checkpoint and one finish for the current run-loop contract.
* Debug overlay shows generated seed, piece count, and actual/expected road/rail/trigger counts.
* Driving model now reports slip angle and grip/sliding state for drift tuning without changing handling yet.
* App now starts at a basic main menu and enters gameplay through one `MainMenu -> Driving` state transition.
* Track/session spawning now happens on entering `Driving`; generated scene cleanup is state-owned.
* Escape opens an in-game pause overlay; resume preserves the active driving scene and main menu exits cleanly.
* Setup screen configures player count, track seed, track length, and car color before spawning gameplay.
* Finished runs move to a results screen with retry, next-player, main-menu, quit, and in-memory leaderboard display.
* Driving model samples four wheel surface contacts and reports them in debug for split-surface drift tuning.
* First-pass drift assist uses slip state to reduce lateral damping and add controlled yaw assist while sliding.
* Car visuals now use separate body and wheel primitives with body roll/pitch, front-wheel steering, and wheel spin.
* In-run HUD shows driver, timer, checkpoint progress, speed, best time, and ghost time; verbose debug is toggled with F3.
* Grass field, forest, and rocks are placed relative to generated track bounds instead of fixed world coordinates.
* Track validation now checks empty tracks, missing finish, short pieces, zero-length segments, connections, and generated counts.
* Four-wheel contact samples now affect lateral grip, so split-surface cases change handling.

## Pending Work

### Product Shell

Pending:

* richer scene teardown/rebuild flows from setup/results back into driving

This is the next sensible phase before adding more track complexity, because runtime track changes and setup screens both need explicit scene ownership and state transitions.

### Gameplay and Track

Pending:

* a real piece-library contract for visuals, road surface zones, rails, triggers, and colliders
* move more per-segment spawning into reusable piece-library data
* expand generated piece sequences beyond simple straight/arc choices
* improve curve piece variety and validation
* better off-track behavior once border collision is stable
* checkpoint and finish line placement for every future piece type
* recipe controls for seed, length, surface mix, and difficulty
* add overlap and unreachable-finish validation once routes become more complex

### Vehicle and Feel

Pending:

* continued tuning of reverse/brake/steering feel
* use four wheel contacts for airborne checks
* tune the current drift assist across surface and speed cases
* drift case matrix: brake-entered drift, throttle-held drift, counter-steer recovery, split-surface slide, ice slide, boost slide, reverse/no-drift
* counter-steer and spin-out damping assist tuned for Trackmania-like readability
* add grounded/contact visual hints to the current wheel primitives
* tune body roll/pitch response for boost and wall impacts
* better collision response at high speed
* visual feedback for surface transitions and boost
* optional controller support once keyboard feel stabilizes

### UI/HUD

Pending:

* improve HUD visual design and layout polish

### Assets and Presentation

Pending:

* support multiple car colors from the same mesh
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

Basic pieces connected in code/data: straight, curve, slope, ramp/drop, checkpoint, finish.

Success condition:

```text
A complete piece-chain run is possible before procedural generation exists.
```

### 4. Complete Run — Prototype Complete

Timer, checkpoints, finish, restart flow.

Success condition:

```text
A full run can be completed, timed, and retried.
```

### 5. Hotseat — Prototype Complete, UI Pending

Player list, turn order, leaderboard.

Success condition:

```text
Many players can take turns on the same track.
```

### 6. Ghosts — Prototype Complete, UI Pending

Sampled transform replay for best/previous runs.

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

### 8. Product Shell — Pending

Main menu, setup, pause, results, and leaderboard screens.

Success condition:

```text
A local session can be configured, played, paused, completed, and repeated without debug-key workflows.
```

### 9. Vehicle Feel Upgrade — Pending

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
