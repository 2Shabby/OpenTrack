# Bengaluru road layout

The city tools read the BBBike OpenStreetMap road extract without changing it.
The selected Bengaluru rectangle uses motorway, trunk, primary, secondary and
their links, compressed 1:20. Lane tags are optional. Buildings are reserved for
later work. The locked source has 8,470 road parts and covers approximately
2.70 × 2.13 km in game coordinates.

The native scene uses two 3 m lanes each way on ground roads (13 m overall),
three each way on motorways (19 m), and one each way on bridges (7 m). Each
cross section includes a 1 m median. Bridge approaches taper into the adjoining
ground width. Source tags and OSM IDs remain provenance; the game cross section
defines traffic. Roads use X east and Z south, projected around a local UTM
origin. The normal game uses its procedural track; the city has a separate
driving preview.

## Directory layout

Generated assets are ignored by Git and Godot under `.stage_authoring/`:

```text
.stage_authoring/
  .gdignore
  bengaluru/
    source/   locked reference, source audit and cached layout seed
    current/  segmented layout, review surfaces and native mesh bake inputs
    review/   road, junction, branch and Godot previews
```

`source/roads.json` and `report.json` contain the locked source selection;
`overlaps.csv` is its width audit. `field_inventory.json` describes available
source road fields, and `discarded_groups.json` records excluded small groups.
`layout_seed.json` caches the aligned and overlap-repaired layout before
intersection splitting. Its original OSM provenance is preserved.
`source/pruning.json` stores approved cuts by structure profile on the original
polylines. Both cached and source rebuilds apply those cuts; the locked road
selection and unpruned alignment seed remain available as source inputs.

`current/` contains `roads.json`, `report.json`, `surfaces.json`,
`ground_road_surface.obj` and `ground_medians.obj`. Load both OBJ files for complete
ground-profile coverage; median tiles occupy their own space. These review
surfaces retain the approved 13 m layout policy. The native baker derives the
13/19/7 m game widths from the same centerlines without modifying this input.
`track.json` records the native mesh manifest, lane policy, spawn points, support
probes, crossing clearance and steep-ramp locations. `track.bin` contains its
local float32 triangles. The playable scene is
`assets/city/bengaluru/city.scn`, also ignored by Git. Generated assets are local
files, with no hard links to the source extract or saved maps.

`review/overview.png` displays the current road surface and medians;
`junctions.png` and `junctions.json` show explicit junction endpoints.
`dangling_overview.png`, `dangling_details.png` and `dangling.json` contain the
current branch review. Branch IDs correspond between the previews and JSON.
The rendered verifier writes `godot_overview.png`, `godot_road.png` and
`godot_bridge.png` here.

City tools and tests live in `tools/city/`. The Python environment is
`.venv-city/`. Saved vehicle/terrain diagnostics live in
`tools/authoring_diagnostics/`, outside the generated city assets. Source extracts
remain outside this repository, and neither generated assets nor the environment
require Git LFS.

## Build and verify

Create the local environment if needed:

```sh
uv venv --python python3 .venv-city
uv pip install --python .venv-city/bin/python 'shapely>=2.1' pyproj matplotlib scipy
```

Build the current layout and reviews from the cached layout seed:

```sh
.venv-city/bin/python tools/city/build_city_layout.py
.venv-city/bin/python -m unittest discover -s tools/city -p 'test_city_*.py'
```

To recompute alignment and overlap repairs from the locked source selection:

```sh
.venv-city/bin/python tools/city/build_city_layout.py --from-source
```

To apply every initial red arm and follow-up candidate in the current pruning
review, then rebuild all current geometry and reviews:

```sh
.venv-city/bin/python tools/city/build_city_layout.py --remove-candidates
```

The command checks that the approved marks match the current roads and surface.
It resolves ground paths against the retained physical network, splits original
polylines at the cuts, and removes explicitly marked grade segments. Through
roads and other structure profiles are preserved at crossings. It subsequently
peels source-graph dead-end chains and retains the largest connected group.
Surfaces, median openings, both OBJ meshes and all reviews are regenerated from
the remaining roads. The plan and current assets are replaced only after the
build succeeds. Normal rebuilds retain the approved pruning.
Actual source cuts can expose further physical arms that the sampled scenario
did not predict. Check the regenerated review and repeat the command when new
candidates are marked. With no candidates, the command preserves the current
layout and exits successfully.

The builder validates the source checksum in
`tools/city/bengaluru_backbone.json`. It uses temporary work directories, replaces
`current/` and `review/` after successful checks, and removes intermediate
exports. The GeoPackage is not reread by either command. Changing source
selection requires an explicit recipe update and a fresh source extraction with
`audit_city_roads.py` and `review_city_backbone.py`.

## Native Godot roads and bridges

Bake the current layout with Godot available on `PATH`:

```sh
.venv-city/bin/python tools/city/author_city_track.py
godot --headless --path . --script tools/city/verify_city_scene.gd \
  -- --city=res://assets/city/bengaluru/city.scn
godot --path . res://scenes/city_preview.tscn \
  -- --city=res://assets/city/bengaluru/city.scn
```

Use `--roads-only` on the Python baker for a ground-only build. `--output`
selects another scene destination. Rebuild the native scene after changing the
layout or recipe: loading a scene does not regenerate geometry. The baker uses
a temporary directory and only replaces the saved scene and mesh inputs after
native support checks succeed.

The scene contains ArrayMeshes and matching ConcavePolygonShape3D collision in
32 m horizontal chunks. Ground corridors are welded, median openings follow
junctions, and lane dividers are decorative paint. Structure footprints are
welded by authored elevation level, with continuous triangulated heights,
0.3 m deck thickness and side faces. Existing asphalt surface bindings support
the vehicle's wheel contact model. Geometry generation remains offline.

Bridge heights are game geometry rather than surveyed elevations. Positive
source layers become levels 1 or 2; tunnels and negative layers become -1 or -2.
Shared source transitions meet ground roads at zero height. Crossings without
shared source connections retain separation, with a 4.5 m clearance requirement
and a 6 m nominal level spacing. Small structures without road crossings can
use lower heights. Horizontal centerlines remain unchanged.

The fixed 1:20 layout includes extremely short approaches. Keeping that layout
can produce effectively vertical ramps, which are not established as drivable.
`maximum_ramp_grade` in the recipe is a review threshold, not a constraint that
relocates roads. The manifest lists affected segments under
`stats.structures.steep_ramps`. Crossing checks measure centerline clearance;
they do not certify clearance across every lane or vehicle traversal of every
ramp. Native support probes check source ground segments and structure
start/middle/end positions; wheel driving checks exercise the ground and flat
bridge test spawns.

The preview loads its scene only from the explicit `--city` argument. It starts
with the rickshaw; `V` switches vehicle, `B` selects the bridge test spawn, `G`
returns to the ground spawn, `R` resets and `M` toggles the map camera. Run the
verifier without `--headless` to save the three rendered review images.

## Intersections and branch review

Roads split at every same-profile centerline crossing, endpoint-on-road contact
and collinear overlap endpoint. Existing shared junction vertices and bridge/grade
attachments also become explicit segment endpoints. Duplicate collinear
intervals are stored once. Different structure profiles stay separate at geometric
crossings; existing shared source transition vertices preserve their connections.

Noding checks ensure that no same-profile intersection lies inside a segment,
unique surviving centerline coverage is preserved, and the full graph stays connected.
Unmarked bridge geometry is preserved. The repaired road surface remains the union of the
approved 13 m corridor footprints. Its disjoint road bodies, shared junctions and
median tiles avoid stacked surfaces, and medians open at shared junctions.
OBJ export preserves holes, upward normals and checked surface area at Godot's
float32 precision.

`dangling.json` contains two complementary geometric checks:

- `graph_branches`: branches of the exact segmented centerline graph. This
  includes degree-one tips, entire singly attached groups of collapsed return
  loops, and distinct single-entry loops. Nested return loops are reviewed as a
  whole arm rather than only the last loop.
- `surface_branches`: complete arms of the welded ground road footprint, sampled
  at 1 m and reduced to a diagnostic skeleton. Overlapping source tracks occupy
  one physical arm. This check can expose tails hidden by the source graph.

Small side fragments below 13 m reach and 26 m centerline length are retained in
`short_surface_details`; coalescing these local fragments lets long arms trace
through corner and sampling details. Reach is straight-line distance from a
branch attachment; centerline length is recorded separately. Bridge/level
continuations at a tip are flagged in purple. A bridge attachment along the
arm does not hide an exposed tip. Exposed tails are red; small details and
single-entry loops are gold. Reviews describe the current layout after any
approved pruning; they do not silently apply further removals.

The sampled skeleton is a review aid with metre-scale positioning uncertainty;
it does not define gameplay topology or certify lane connectivity. Very short
branches may be corner artefacts. Source tags supply structure distinctions,
not bridge heights. Both checks operate on the current derived layout and keep
its geometry unchanged.

## Checks after simulated removal

The builder also generates `review/pruning.json` and `pruning_overview.png`.
Run that check independently with:

```sh
.venv-city/bin/python tools/city/review_city_pruning.py \
  .stage_authoring/bengaluru/current --review .stage_authoring/bengaluru/review
```

The scenario removes the red exposed arms from the sampled network. Explicit
shared source vertices attach bridges, tunnels and other layers to ground roads;
crossings without those connections stay separate. Ground details are coalesced
while retaining grade attachments. Analysis connectors remain valid only while
both the ground arm and its grade road remain present.

Each pass marks additional dangling arms and newly disconnected groups. It
excludes those groups and exposed ground arms in the simulation, then repeats
until no further removals are found. Structure-bearing dangling branches and
new single-entry loops remain marked for review. Short newly exposed ground
details are marked before coalescing them in the analysis. Existing separate
baseline groups are distinguished from groups split by the proposed cuts.

The preview uses faded red for the initial cuts, orange for further dangling
arms, blue for disconnected groups and purple for structure-bearing branches.
Labels use `round.mark`; JSON records each pass and its resulting marks. Source
grade segment IDs and sampled paths support inspection. The current roads,
surfaces and meshes remain unchanged by this conditional review.

Map data attribution: OpenStreetMap contributors; extract created by BBBike.
Retain attribution with derived maps.
