# Authored rally library

50 complete routes from nine regions, about 806 km in total. Lengths range from Slate Mountain (about 1.6 km) to Chuchilla Nevada – Characato (about 33.8 km). `selection.json` records each source map, source stage ID, original stage name, surface assignment, road width and GPX path. Each saved RallyStage carries the source file's SHA-256 hash.

The route data is the Rally-Maps data used by [rally2gpx](https://github.com/joagonca/rally2gpx). The imported maps' spline handles were expanded into their actual paths before generating the source GPX files. The upstream interactive scraper did not complete against the current page; acquisition read the same published stage objects. No rally2gpx source code is vendored into the game. Source map URLs are retained in both manifests and Resources; the upstream tool's GPL license is not a license grant for the source maps.

Only complete special-stage routes are included. Service parks, point markers, disconnected paths and unsafe overlapping/folded routes were excluded. GPX supplies horizontal route coordinates without surveyed elevations. Terrain elevations and pacenotes are game authoring choices. No route is cropped, scaled, reversed or assembled from repeated laps.

## Authoring

The large baked `.scn` worlds are local artifacts ignored by Git. Source GPX, the catalog, native road Resources and authoring tools use ordinary Git storage; Git LFS is not required. Existing local worlds remain available to offline previews. On a fresh clone, bake a desired world before previewing it.

With Godot 4.7.2 available as `godot`:

```sh
python3 tools/author_stages.py
godot --headless --path . --script tools/bake_stages.gd
python3 -u tools/bake_stage_worlds.py
```

The first two commands rebuild road Resources and reset world references. Run them when changing source routes or road authoring. The final command runs the expensive one-off terrain pass, shortest routes first. It isolates each stage in its own Godot process to release memory between worlds, saves compressed meshes/collision to `resources/stages/baked/`, and publishes ready catalog entries atomically. Setup exposes only published worlds. Returning to setup refreshes the available library while a batch is still running.

Resume an interrupted pass by running the last command again. Completed worlds are skipped when their geometry/source/configuration/engine fingerprint matches. Use `--force` to rebuild all worlds, or `--stage wales-slate-mountain-17119` to bake one. Material resources remain external references, so visual material edits do not require terrain resampling. Keep the computer awake for the batch. The optional offline accelerator uses the same 10 cm source grid and error bounds, with native sampling, partitioning and final triangle checks. On this Mac, the checked sampling chunks match the GDScript reference within a micrometre. The extension is never loaded by the game or required to play saved worlds.

With local Godot 4.7 `godot-cpp` bindings in `extensions/godot-cpp` and CMake, build and check the optional accelerator before authoring:

```sh
python3 tools/build_native_bake.py
godot --headless --path . --script tools/validate_native_bake.gd
godot --headless --path . --script tools/validate_terrain_refinement.gd
python3 -u tools/bake_stage_worlds.py --jobs 2
```

The build helper currently targets macOS arm64. Without the accelerator descriptor in `.stage_authoring/native/`, the same baker uses the GDScript reference. `--jobs` defaults to one; simultaneous workers consume more memory. The completed library pass uses `--jobs 3 --large-stage-km 18`; stages longer than that threshold use at most two workers. Terrain triangulates in local coordinates to retain small patches kilometres from the stage origin. Clipped regions split at repeated vertices and remove zero-area backtracks while retaining ordinary road border vertices. Failed nondegenerate triangulation prevents publication. Refinement records dependencies on neighbouring leaves and reuses unchanged chunk meshes; the comparison tool verifies identical final vertices and normals against full remeshing. Continuous shoulder visuals use collision sections bounded to 32 m, preventing whole-stage collision quantization from collapsing small triangles. A publication mutex refreshes the catalog before each update, preserving other workers' completed entries. The runner requires an explicit successful publication message, and rejects script/engine errors even when Godot returns zero. The baker checks complete bodies, collision triangle coverage and native road/verge ray support before publishing.

Progress is in `.stage_authoring/progress.json`, with a separate log for each stage. These operational files are ignored by Git. The progress file includes the runner PID and current Godot child PID. Sending SIGTERM to the runner stops its active children and leaves completed assets intact.

Refinement continues until every patch meets its error bound; it reports failure if no remaining patch can be subdivided below the 10 cm source resolution. Thin clipped regions that fail normal triangulation get a second near-duplicate cleanup within two coordinate float steps. These repairs preserve successfully triangulated geometry and the same surface error limits.

The 50 saved worlds occupy about 10.2 GB and contain 225,399 ground chunks. The project's Jolt body limit is 32,768 so the largest full stage can register all ground and road colliders, with room for the car and world transitions.

## Validation

```sh
godot --headless --path . --script tools/validate_stages.gd
godot --headless --path . --script tools/validate_stages.gd -- --world
godot --headless --path . --script tools/validate_stages.gd -- --require-baked
python3 tools/validate_stage_worlds.py --jobs 2
```

The first command verifies all 50 native road assets, source hashes, feature coverage, unfolded road triangles, finish gates, setup validation and shuffle cycles. `--world` loads the shortest published baked scene and checks native collision/support, persistent tire groups, driving, timing, pause, retry and hotseat. `--require-baked` requires all 50 worlds to have been saved. `validate_stage_worlds.py` runs `validate_baked_worlds.gd` separately for each stage to release resident geometry between checks; a single-process sweep aborted in this Godot build after repeated large-world loads. Each scene loads through the runtime path, verifies saved meshes/materials/colliders/tire groups, checks ground chunk and triangle counts against bake metadata, and raycasts the road, verges and each ground chunk. Native support queries retry within one world-coordinate float step (at least 1 mm) after an exact ray miss at a compressed shape seam; larger gaps remain unsupported. The runner aggregates scene hashes and results to `.stage_authoring/world_validation.json`.

For a rendered preview of a baked stage:

```sh
godot --path . --script tools/terrain_preview.gd -- --stage=wales-slate-mountain-17119
```

The same preview defaults to procedural test mode when no saved-stage ID is supplied. Export the project with all resources so the native catalog, stage `.res` files and baked `.scn` scenes are packaged. The JSON authoring manifests and source GPX files are not needed to drive a baked world.
