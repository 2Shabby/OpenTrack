# Saved rally stages and procedural testing

The normal game mode uses the 50 complete routes listed in `resources/stages/catalog.tres`. Source GPX files and authoring choices live in `assets/tracks/`. The length chooser filters complete stages (under 10 km, 10–20 km, or 20 km and longer); it never scales or crops them. Region filters, named selection and a shuffle bag select the next stage. Retries and hotseat handoffs reuse the current world and preserve its session best times.

`tools/author_stages.py` converts GPX into road stations and pacenote features. `tools/bake_stages.gd` saves native road Resources, including fixed draped elevations and terrain settings. `tools/bake_stage_worlds.py` then authors each complete road, shoulder and terrain world once, preserving the existing 10 cm terrain grid and full footprint. Compressed `.scn` assets contain meshes, materials, native collision shapes and persistent tire surface groups. Each world is published atomically, and its catalog entry becomes available in setup only after publication. Stages without a baked world never silently fall back to procedural generation.

Runtime uses background resource loading to instantiate the baked scene. Ground/recovery height queries raycast its actual native colliders, so they agree with the saved geometry. No road triangulation, terrain sampling or collision-shape construction runs when a saved stage starts. Procedural test mode continues to use the original generation/build pipeline and its editable terrain controls.

The 50 route shapes are from the Rally-Maps data used by [rally2gpx](https://github.com/joagonca/rally2gpx). Spline tangents are expanded before GPX export; they are not treated as route anchors. These GPX files contain horizontal coordinates without elevation data. Road elevations, landscape and pacenotes are authored game data, not surveyed real-world terrain or official co-driver notes. Full source endpoints remain intact; resampling and small kink smoothing alter path length by less than 1%. Saved roads are 6 m wide, with locally narrowed grass verges at tight bends to prevent folded shoulders.

See `assets/tracks/README.md` for provenance, regeneration, progress monitoring and validation. The sections below describe the procedural test mode and the shared terrain authoring pipeline.


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

`rally-voxel-v4` uses a private Godot RandomNumberGenerator seeded independently of global randomness and native FastNoiseLite terrain. Seed, requested length and terrain settings repeat a complete stage within the same generator/Godot version. The generator requires terrain settings, and the geometry builder requires a complete stage Resource. There are no old-format readers, flat-world branches, seed compatibility guarantees or custom C++ dependencies.

## Terrain draping and stamping

The route remains segment-based, sampled every 2 m, with no splines. FastNoiseLite generates low-frequency smooth Simplex fBm with three octaves. Elevations sampled at cross-section centers are smoothed over neighboring stations, then constrained by maximum road gradient and gradient changes. The grade bound also applies to the inside and outside edges of bends. Nearby nonadjacent stations constrain their height differences according to the available space between roads, preventing steep steps between hairpin legs. The existing layout generator still does not produce arbitrary looping routes or full hairpins.

Left and right edges share each station's elevation, so the road gains climbs, crests and descents without banking. Consecutive stations rebuild the triangles. Adjacent pieces reuse identical boundary vertices and area-weighted normals. Spawn and finish use pitched road frames; the finish gate checks the local road plane instead of a horizontal strip.

Terrain uses a complete 10 cm source grid within a footprint derived after track generation. Buffered route chunks are selected and enclosed footprint holes are filled; the old axis-aligned rectangle and 256 m margin are removed. Padding is `max(96 m, half road width + blend distance + 2 × amplitude / 0.35)`, rounded outward to 32 m.

Every source grid point is sampled directly from low-frequency Simplex fBm with road cut/fill. Terrain outside the smooth driving corridor uses vertices on 10 cm elevation levels, connected by flat or sloped planar facets. Rectangular patches merge when their slope planes fit every contained source sample within one voxel; smooth patches use a 5 mm tolerance. This simplification changes geometry, never source resolution. All 32 m chunks and their native colliders are built before driving, with no streaming or coarse distant substitutes.

The 12 m road and 2 m grass shoulders retain continuous elevations. Terrain is clipped out of their footprint; shared border vertices use the actual shoulder profiles. Conforming edges split at neighbouring patch junctions and share the coarser edge profile, including across chunk boundaries. Finished triangles are rechecked against the source grid and patches refine until their error bound is met. The road, shoulders and terrain register their finished triangles into one support-query index, so recovery samples real collider planes rather than the source noise.

Editable defaults live in `resources/terrain_settings.tres`, and setup exposes these controls:

| Control | Default | Range |
| --- | --- | --- |
| Terrain relief (noise multiplier) | 16 m | 0–20 m |
| Terrain wavelength | 220 m | 150–1000 m |
| Maximum road gradient | 10% | 1–10% |
| Shoulder blend distance | 16 m | 16–64 m |

The last control sets the nominal blend width; necessary cut-and-fill can extend beyond the nominal blend. Road gradient changes are bounded internally at 0.006 per metre for sharper crests and dips. Difficult nearby-road constraints may reduce road relief to preserve the grade/crest limits. Terrain amplitude zero is a setting in the same pipeline, not a separate legacy generator.

## Driving and hotseat

The stage follows the green terrain with blended grass shoulders. Terrain is removed beneath the road/shoulder footprint to keep support surfaces unambiguous. Guardrail geometry, materials and texture generation are removed. Four wheel queries select asphalt, dirt or grass independently; straddling the edge uses mixed grip and resistance. Going onto grass keeps the car supported and preserves the current run's timer.

The car uses GEVP suspension and tire forces on a native Jolt rigid body, with automatic transmission and 50/50 AWD. Road/grass wheel contacts are independent. Gravity and angular momentum continue through jumps and rollover; there is no height snapping or artificial airborne upright correction. Grass uses the library's rolling resistance rather than a speed cap. See [VehicleHandling.md](VehicleHandling.md) for current tuning and measurements.

Timing starts with the driver's first accepted throttle/brake input. A swept forward crossing of the pitched finish gate within road width finishes the run, including airborne crossings up to 20 m above the road. Reverse/off-width crossings are rejected. R retries; N selects the next driver on the same Resource. Both recreate the car and suppress held controls until release. Best times survive retries/handoffs in the current world, but are not persisted after leaving that stage session. Scene-tree pause stops native physics and time. Recovery is limited to leaving the finite world, falling below it or nonfinite body state; wheel contact loss alone never resets.

## Manual previews and performance

Run `godot --path . --script tools/terrain_preview.gd` for setup, chase and overview captures. Run the same preview headlessly for full-world generation statistics. Arguments select seed, stage length and terrain controls (see `-- --length=500 --seed=1492`). Resident terrain reports sample count, footprint area, generation time, triangles and maximum source error.

Inspect minimum/default/maximum stages, terrain-setting extremes, adjacent corners, hairpins, road/shoulder joins, chunk seams and world edges. Drive across both verges and verify grass deceleration and road re-entry. Preview statistics include build time, memory and driving frame times. The saved-library checks are in `tools/validate_stages.gd`; visual and driving passes remain necessary for stage quality.
