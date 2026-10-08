# Road and terrain shoulder audit — 2026-10-08

The audited default stage at commit `51c1f44` had a real geometric and collision gap between the road and terrain. The largest sampled road-edge gap was **4.45 m near 98 m**, in the first tight corner. A side view confirmed the floating road. The findings below describe that baseline; the repair is recorded at the end.

## Reproduction and measurement

Commit: `51c1f44`. Godot 4.7.2 with built-in Jolt. Default seed `1592598566`, requested length 1200 m, relief 16 m, wavelength 220 m, maximum grade 10%, shoulder blend 16 m. Generated road length: 1200.757 m.

Sample both road edges at every section boundary except the terminal endpoint, and at every segment midpoint. Compare each road vertex/interpolated edge height to the terrain's exact piecewise-triangular `height_at` at the same X/Z. These are vertical separations, not shortest surface distances.

| Heightfield phase | Mean edge gap | 95th percentile | Maximum |
| --- | ---: | ---: | ---: |
| Initial distance-based stamping | 0.080 m | 0.087 m | 0.101 m |
| After conservative triangle capping | 0.110 m | 0.201 m | 0.305 m |
| After terrain slope limiting / shipped geometry | 0.224 m | 0.855 m | **4.455 m** |

The diagnostic reconstruction exactly matched the production heightfield. At station 49, 98.042 m along the stage, the sampled edge is `(2.160403, 0.060448, 100.755)` and terrain beneath it is approximately -4.394 m. Independent native raycasts 2 cm inside that edge hit the dirt road at 0.060401 m and grass terrain at -4.388449 m: **4.448850 m separation**. Both bodies have identity local transforms.

The same defaults at 1200 m also reproduced substantial maximum edge gaps with seeds 0 (4.884 m), 1 (1.715 m), and 1492 (4.552 m). This is not confined to the default seed.

## Findings

1. **High: the slope limiter destroys road-height anchoring.** `terrain_builder.gd:195` uses a lowering-only distance transform. Every update takes the minimum of a cell's height and a neighboring height plus a slope allowance. Lower surrounding terrain therefore propagates into the stamped road corridor. The pass protects the invariant that terrain never protrudes through the road, but has no opposing constraint to keep terrain close to the road. This is the primary cause of the metre-scale gap. Narrow blending with the sharper defaults exposes the conflict between the road plateau and surrounding relief.

2. **Medium: conservative capping depresses shoulders unnecessarily.** `terrain_builder.gd:220` applies each road triangle's extended plane across its entire grid-aligned bounding rectangle. It does not test triangle/cell intersection. With 4 m terrain cells and curved/pitched road triangles, this affects vertices outside the actual footprint and extrapolates road planes into the shoulder. In the measured stage, this increases the maximum gap from 0.101 m to 0.305 m before slope limiting.

3. **Medium: the intended clearance is exposed at the road boundary.** `terrain_field.gd:8` defines an 8 cm clearance. `terrain_builder.gd:190` subtracts it throughout the stamped corridor, including the shoulder apron. Blending starts roughly 5.66 m outside the nominal road edge because the apron is `SPACING * sqrt(2)`. `track_geometry.gd` builds an independent thin road surface and terrain chunks; it builds no geometry joining their edges. Even perfectly stamped flat ground retains an exposed 8 cm step.

4. **High validation gap: all existing clearance assertions are one-sided.** `smoke.gd:93` requires terrain to remain at least 8 cm below sampled road triangles, with no maximum separation. A road floating metres above a slope-limited terrain still passes. The current terrain slope test also passes because it checks the terrain alone. Flat off-road fixtures and conservative centreline traversals do not exercise the faulty default shoulder.

The materials contain no displacement, and the geometry builder adds no road-height translation. This is a heightfield/shoulder construction issue. Rendering and collision use the same affected triangles; driving over the worst edge can encounter a substantial drop instead of a coherent verge.

## Recommended repair

- Protect road-border height constraints during slope limiting. Permit local fill as well as cut, or broaden the affected terrain region when necessary, rather than lowering the road corridor to satisfy distant terrain. Preserve the existing road grade and nearby-road constraints.
- Build connected grass shoulders from the exact road cross-section edge vertices to the surrounding terrain. Use identical triangles for rendering and collision and the existing Grass profile. An edge strip must connect to a properly constrained terrain; bridging the current multi-metre depression with a narrow strip would create an excessively steep verge.
- Keep any buried clearance beneath the road interior. Changing `ROAD_CLEARANCE` to zero alone does not repair the limiter and can introduce overlapping surfaces.
- Refine conservative capping to operate on actual intersecting terrain cells instead of every cell in a triangle's bounding rectangle.
- Add two-sided clearance checks, exact road/shoulder joint checks, cross-verge gradient limits, and native wheel traversal across both edges near the default first corner. Cover road/terrain chunk seams, tight bends and terrain-control extremes.

## Implemented repair

Generation now uses `rally-terrain-v3`, with no compatibility branch. Actual triangle/cell intersection limits conservative capping. Road caps establish a slope-limited corridor and paired cut/fill envelopes, preventing distant valleys from lowering that corridor. Road layout, elevation, grade limits and handling are unchanged.

Closed 2 m Grass shoulder strips connect the exact road vertices to terrain triangles, including the start and finish. Their outer edges split at terrain grid edges and diagonals; rendering and native collision use the same strip meshes. The 8 cm clearance remains underneath the road and shoulders rather than becoming an exposed step.

Using the same default sampling as the audit, the buried heightfield separation is now 0.100 m mean, 0.155 m at the 95th percentile and **0.215 m maximum**, down from 4.455 m. The limiter no longer increases that maximum beyond the local triangle caps. This buried separation is covered by the connected shoulders; it is not a road/grass surface discontinuity. The default shoulder mesh's maximum gradient is 23.45%.

Native raycasts at the former worst corner confirm support across both road/grass boundaries and agreement with terrain at the outer shoulder joints. The actual car crosses both edges in both directions with **zero airborne ticks**. New regression checks bound buried clearance from both sides, verify exact road/shoulder and end-strip vertices, constrain verge gradients, and compare outer seams with the terrain's actual triangular interpolation. Metal side and overhead renders confirm the floating belt is gone.

Verification completed with zero smoke-test failures: 42 generation/replay cases across 500–5000 m, terrain-control extremes, native resource round trips, shoulder crossings, vehicle/surface/airborne checks, menus and hotseat lifecycle, and complete native car traversals of 500 m, 1200 m and 5000 m stages. The additional end-strip joint checks also passed across the four shoulder regression seeds. Godot editor import completed successfully.

Baseline renders: `/tmp/opentrack-shoulder-before-worst-edge.png` and `/tmp/opentrack-shoulder-before-corner-overview.png`. Repaired renders: `/tmp/opentrack-shoulder-worst-edge.png` and `/tmp/opentrack-shoulder-corner-overview.png`. Repaired heightfield measurements: `/tmp/opentrack-shoulder-after-audit.log`.
