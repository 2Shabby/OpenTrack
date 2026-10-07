# Open Track rally direction

Local hotseat rally stages on dirt and asphalt. The active pass builds readable roads from distances, corner grades and corner lengths. Unbanked roads follow gently generated terrain, replacing the old modular Trackmania layout. Ice and boost are removed from generation, handling resources, materials and texture generation.

## Implemented foundation

- Seeded stages from 500 to 5000 metres; default 1200 metres.
- Explicit corner lengths and grades from 1 (tight) to 6 (fast).
- Groups of linked corners, including changes in direction and grade, interspersed with deliberate short and long straights. Straights are not required between individual corners.
- Sustained dirt/asphalt sections, rather than random surface changes per road slice.
- One native stage Resource drives draped mesh geometry, collision, progress and visible pacenotes.
- Widened 12 m roads over a generated green terrain with blended shoulders, with no guardrails. Grass slows the car and permits driving back onto the road without resetting the run.
- Native Godot/Jolt chassis collisions and road/ground wheel queries with vendored GEVP in GDScript; no C++ gameplay dependency.
- All-wheel drive, automatic transmission, suspension, tire slip forces, jumps, landings and rollover. Steering redirects momentum through tire forces; handbrake rotation comes from rear-wheel braking and lost lateral reserve.
- One active driver at a time on an identical stage, timed from the first throttle/brake input to the finish. Scene-tree pause stops native physics and timing. Retries clear the run; driver handoff retains each driver's best in that stage session.
- Shared setup/pause menus and a small driving HUD for driver, speed, distance, notes and times. R retries; N hands over; pause also exposes both actions.

## Next priorities

1. Drive repeatable seeds to tune entry speeds, braking distance, grade/radius mapping and corner length distribution.
2. Tune linked bend shapes and note lookahead for readability. The current roads have smooth curvature entry/exit, but do not yet carry continuous nonzero curvature through a tightening bend.
3. Tune terrain relief, crest profiles and cut-and-fill shoulders through playtesting. Validate high-speed crest takeoff and landing recovery while tuning suspension.
4. Add audio pacenotes and session comparison/results once the stage and handling feel are settled.
5. Consider ghosts and persistent leaderboards after the run state stabilizes.

## Architecture and limits

Godot 4.7.2, built-in Jolt, GDScript and native Resources/meshes/bodies. GEVP owns suspension, tire forces and drivetrain; Jolt integrates the rigid body. Imported SportsCar visuals have four independent suspension/spin pivots. No banking, ice, boost, second physics backend, manual track editor, damage, online multiplayer or simultaneous racing is included.

`RallyStages.md` documents generation and its reproducibility boundary. `NativeGodotMigration.md` documents ownership, vehicle integration, session lifecycle and dependency maintenance. Older handling/collision audits are historical snapshots and must not be treated as the current surface catalog or generation contract.

`VehicleHandling.md` documents the native vehicle, grass measurements, verification and tuning limits.
