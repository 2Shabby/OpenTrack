# Rally stage and terrain generation

The stage recipe contains straights and corners with explicit start/end distances in metres. Corners specify direction, grade and length. A grade is an internal geometry convention, not a calibrated Dirt Rally pacenote system. Each corner's length can vary independently of its grade; longer corners sustain the bend for longer.

| Grade | Peak radius |
| --- | --- |
| 1 | 14 m |
| 2 | 22 m |
| 3 | 35 m |
| 4 | 55 m |
| 5 | 90 m |
| 6 | 140 m |

Corner heading follows an eased curve with zero curvature at entry/exit. Stations integrate that heading at no more than 2 m spacing, and peak curvature controls the grade. Adjacent corners share the same endpoint and heading with no inserted straight. Different grades and directions allow opening, tightening and reversing sequences at the phrase level; continuous nonzero-curvature tightening bends remain a later refinement.

The first straight is 80 m. The generator chooses groups of one to three corners, then planned straights of roughly 50–130 m, with 150–220 m straights when a stage has gone far enough without one. It reserves at least 40 m at the finish and merges adjacent straight features with the same surface. The requested length describes the horizontal route; the displayed road length is measured again in 3D after elevation is applied. Features and pacenotes use that same 3D distance; shorter stages need not contain every kind of phrase.

Dirt starts the stage. Surface blocks switch between dirt and asphalt at phrase boundaries, with longer dirt targets. Grade and corner length produce the actual geometry; pacenotes are derived from those same features and projected car distance. A note reports distance to a corner, direction/grade, total corner length and the following linked corner or straight. During a corner it reports distance remaining to exit.

Road width is 12 m (previously 8 m). A spatial index rejects new geometry within 14 m of distant earlier centerline stations, with nearby connected stations excluded. Heading stays within ±1.45 radians of the stage's initial direction, keeping the route generally forward. This is a bounded first-stage generator, not a generator for arbitrary looping stages or true 180-degree hairpins. Candidate selection and whole-stage retries are bounded; failures return a setup error rather than emitting overlapping roads.

## Native data and reproducibility

`scripts/rally_stage.gd` is a Resource with exported features, centerline stations, headings, distances, shared road edges/normals, terrain settings, final heightfield, generator version and engine version. Rendered road sections, road collision, finish marker, progress and pacenotes derive from this data. The Resource can be saved through ResourceSaver as a binary `.res` asset and reloaded with exactly its stored geometry. Editable `.tres` files are also supported, but their text serialization rounds numeric values.

`rally-terrain-v2` uses a private Godot RandomNumberGenerator seeded independently of global randomness and native FastNoiseLite terrain. Seed, requested length and terrain settings repeat a complete stage within the same generator/Godot version. Save the Resource for exact preservation across engine versions. The generator requires terrain settings, and the geometry builder requires a complete stage Resource. There are no old-format readers, flat-world branches, seed compatibility guarantees or custom C++ dependencies.

## Terrain draping and stamping

The route remains segment-based, sampled every 2 m, with no splines. FastNoiseLite generates low-frequency smooth Simplex fBm with three octaves. Elevations sampled at cross-section centers are smoothed over neighboring stations, then constrained by maximum road gradient and gradient changes. The grade bound also applies to the inside and outside edges of bends. Nearby nonadjacent stations constrain their height differences according to the available space between roads, preventing steep steps between hairpin legs. The existing layout generator still does not produce arbitrary looping routes or full hairpins.

Left and right edges share each station's elevation, so the road gains climbs, crests and descents without banking. Consecutive stations rebuild the triangles. Adjacent pieces reuse identical boundary vertices and area-weighted normals. Spawn and finish use pitched road frames; the finish gate checks the local road plane instead of a horizontal strip.

The surrounding green heightfield uses a 4 m grid, split into 128 m chunks, with a 256 m margin beyond route bounds. A quintic distance blend stamps the road's height into the terrain. A small grid-sized apron protects coarse-cell interpolation, and intersecting cells are conservatively kept at least 8 cm below road triangles. A lowering-only slope limiter bounds terrain gradients to 35%, extending local cut-and-fill when necessary to avoid cliffs. Rendering and native ConcavePolygonShape3D collision use identical triangles and diagonals. Normals sample the global lattice across chunk boundaries. No separate collision height source exists.

Editable defaults live in `resources/terrain_settings.tres`, and setup exposes these controls:

| Control | Default | Range |
| --- | --- | --- |
| Terrain relief (noise multiplier) | 16 m | 0–20 m |
| Terrain wavelength | 220 m | 150–1000 m |
| Maximum road gradient | 10% | 1–10% |
| Shoulder blend distance | 16 m | 16–64 m |

The last control sets the nominal blend width; the grid apron and necessary slope-limited cut-and-fill can extend the final shoulder. Road gradient changes are bounded internally at 0.006 per metre for sharper crests and dips. Difficult nearby-road constraints may reduce road relief to preserve the grade/crest limits. Terrain amplitude zero is a setting in the same pipeline, not a separate legacy generator.

## Driving and hotseat

The stage follows the green terrain with blended grass shoulders. Road clearance keeps wheel queries on the road while driving on it. Guardrail geometry, materials and texture generation are removed. Four wheel queries select asphalt, dirt or grass independently; straddling the edge uses mixed grip and resistance. Going onto grass keeps the car supported and preserves the current run's timer.

The car uses GEVP suspension and tire forces on a native Jolt rigid body, with automatic transmission and 50/50 AWD. Road/grass wheel contacts are independent. Gravity and angular momentum continue through jumps and rollover; there is no height snapping or artificial airborne upright correction. Grass uses the library's rolling resistance rather than a speed cap. See [VehicleHandling.md](VehicleHandling.md) for current tuning and measurements.

Timing starts with the driver's first accepted throttle/brake input. A swept forward crossing of the pitched finish gate within road width finishes the run, including airborne crossings up to 20 m above the road. Reverse/off-width crossings are rejected. R retries; N selects the next driver on the same Resource. Both recreate the car and suppress held controls until release. Best times survive retries/handoffs in the current world, but are not persisted after leaving that stage session. Scene-tree pause stops native physics and time. Recovery is limited to leaving the finite world, falling below it or nonfinite body state; wheel contact loss alone never resets.

Validation is in `scripts/smoke.gd`. Automated geometry and physics checks do not replace driving the stages to judge their rhythm, landing behavior or corner entry speeds.

The terrain pass verified 42 generation/replay cases across 500–5000 m, native resource round trips, both terrain cell triangles and chunk seams, shared road vertices/normals, grade/crest bounds, terrain clearance and a synthetic hairpin conflict. The default 1200 m stage has 5.72 m of road relief, a 4.53% peak centerline grade and a 3.30% mean absolute grade, up from 2.53 m, 2.73% and 1.01% in the earlier gentle preset. Metal render checks covered setup, the chase camera and an oblique terrain view. Terrain-control limit cases and percentage conversion can be run separately with `godot --headless --path . --script scripts/smoke.gd -- --terrain-controls`.
