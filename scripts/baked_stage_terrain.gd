extends Resource

# Saved worlds use their native colliders for support queries. No source noise,
# chunk sampling, terrain triangulation or collision construction runs on load.
const SUPPORT_MASK := 3
var error := ""
var stats: Dictionary = {}
var _bounds := AABB()
var _space: PhysicsDirectSpaceState3D
var _query := PhysicsRayQueryParameters3D.new()

func initialize(stage: Resource, space: PhysicsDirectSpaceState3D) -> void:
	_bounds = stage.baked_bounds
	_space = space
	stats = stage.baked_stats.duplicate()
	_query.collision_mask = SUPPORT_MASK
	_query.hit_back_faces = true

func height_at(point: Vector2) -> float:
	if _space == null or point.x < _bounds.position.x or point.x > _bounds.end.x or point.y < _bounds.position.z or point.y > _bounds.end.z:
		return NAN
	_query.from = Vector3(point.x, _bounds.end.y + 1.0, point.y)
	_query.to = Vector3(point.x, _bounds.position.y - 1.0, point.y)
	return ray_support(_space, _query)

# Independently compressed native shapes can leave a float-sized seam at a
# shared edge. Retry within one world-coordinate float step only after a miss;
# ordinary holes remain unsupported. This also stabilizes exact station probes.
static func ray_support(space: PhysicsDirectSpaceState3D, query: PhysicsRayQueryParameters3D) -> float:
	var hit := space.intersect_ray(query)
	if not hit.is_empty():
		return hit.position.y
	var start := query.from
	var end := query.to
	var tolerance := maxf(0.001, maxf(absf(start.x), absf(start.z)) * 1.2e-7)
	for offset: Vector3 in [Vector3(tolerance, 0, 0), Vector3(-tolerance, 0, 0), Vector3(0, 0, tolerance), Vector3(0, 0, -tolerance)]:
		query.from = start + offset
		query.to = end + offset
		hit = space.intersect_ray(query)
		if not hit.is_empty():
			break
	query.from = start
	query.to = end
	return hit.position.y if not hit.is_empty() else NAN
