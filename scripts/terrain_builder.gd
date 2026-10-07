extends RefCounted

const Field := preload("res://scripts/terrain_field.gd")
const CELL_SIZE := 32.0
const MAX_GRADE_CHANGE := 0.006
const MAX_SHOULDER_GRADIENT := 0.20
const MAX_TERRAIN_GRADIENT := 0.35
const EPSILON := 0.00001

var error := ""
var _noise := FastNoiseLite.new()
var _segments: Dictionary = {}
var _neighbors := PackedVector3Array()
var _config: Resource

func apply(stage: Resource, config: Resource) -> bool:
	if not config.valid():
		error = "Terrain controls are outside their supported ranges."
		return false
	_config = config
	stage.terrain_settings = config.duplicate()
	_noise.seed = int(stage.seed_value & 0x7fffffff)
	_noise.noise_type = FastNoiseLite.TYPE_SIMPLEX_SMOOTH
	_noise.fractal_type = FastNoiseLite.FRACTAL_FBM
	_noise.fractal_octaves = 3
	_noise.fractal_gain = 0.35
	_noise.frequency = 1.0 / config.wavelength
	_noise.offset = Vector3(331, 0, 719)
	var heights := PackedFloat64Array()
	for center: Vector3 in stage.centers:
		heights.append(_raw(Vector2(center.x, center.z)))
	heights = _smooth(heights, stage.distances)
	_find_neighbors(stage)
	_constrain(stage, heights)
	for i in stage.centers.size():
		stage.centers[i].y = heights[i]
	stage.rebuild_road()
	_index_segments(stage)
	stage.terrain = _heightfield(stage)
	# The same heightfield is also checked after conservative cut-and-fill.
	if not _terrain_is_gentle(stage.terrain):
		error = "Terrain is too steep around this stage. Reduce relief or increase wavelength/shoulder blending."
		return false
	return true

func _raw(point: Vector2) -> float:
	return _noise.get_noise_2d(point.x, point.y) * _config.amplitude

func _smooth(heights: PackedFloat64Array, distances: PackedFloat64Array) -> PackedFloat64Array:
	var result := heights.duplicate()
	for i in heights.size():
		var total := 0.0
		var weight := 0.0
		for j in range(maxi(0, i - 16), mini(heights.size(), i + 17)):
			var d := distances[i] - distances[j]
			var w := exp(-d * d / 288.0)
			total += heights[j] * w
			weight += w
		result[i] = total / weight
	return result

func _find_neighbors(stage: Resource) -> void:
	_neighbors.clear()
	var buckets := {}
	var radius: float = stage.ROAD_WIDTH + 2 * _config.blend_distance
	var reach := ceili(radius / CELL_SIZE)
	for i in stage.centers.size():
		var point: Vector3 = stage.centers[i]
		var cell := _cell(Vector2(point.x, point.z))
		for x in range(cell.x - reach, cell.x + reach + 1):
			for z in range(cell.y - reach, cell.y + reach + 1):
				for j: int in buckets.get(Vector2i(x, z), []):
					var path_distance: float = stage.distances[i] - stage.distances[j]
					if path_distance < 24:
						continue
					var distance := point.distance_to(stage.centers[j])
					if distance > radius:
						continue
					var limit: float = maxf(0, distance - stage.ROAD_WIDTH) * MAX_SHOULDER_GRADIENT
					if limit < minf(2 * _config.amplitude, path_distance * _config.max_gradient):
						_neighbors.append(Vector3(j, i, limit))
		if not buckets.has(cell):
			buckets[cell] = []
		buckets[cell].append(i)

func _constrain(stage: Resource, heights: PackedFloat64Array) -> void:
	var lengths := PackedFloat64Array()
	var limits := PackedFloat64Array()
	for i in range(1, stage.centers.size()):
		var left: Vector3 = stage.centers[i] - _right(stage.headings[i]) * stage.ROAD_WIDTH * 0.5
		var previous_left: Vector3 = stage.centers[i - 1] - _right(stage.headings[i - 1]) * stage.ROAD_WIDTH * 0.5
		var right: Vector3 = stage.centers[i] + _right(stage.headings[i]) * stage.ROAD_WIDTH * 0.5
		var previous_right: Vector3 = stage.centers[i - 1] + _right(stage.headings[i - 1]) * stage.ROAD_WIDTH * 0.5
		lengths.append(stage.distances[i] - stage.distances[i - 1])
		limits.append(minf(lengths[-1], minf(left.distance_to(previous_left), right.distance_to(previous_right))) * _config.max_gradient)
	for _pass in 128:
		var violation := 0.0
		for i in lengths.size():
			var delta := heights[i + 1] - heights[i]
			var excess := delta - clampf(delta, -limits[i], limits[i])
			heights[i] += excess * 0.5
			heights[i + 1] -= excess * 0.5
			violation = maxf(violation, absf(excess))
		for i in range(1, heights.size() - 1):
			var a := 1.0 / lengths[i - 1]
			var c := 1.0 / lengths[i]
			var b := -a - c
			var change := a * heights[i - 1] + b * heights[i] + c * heights[i + 1]
			var limit := MAX_GRADE_CHANGE * (lengths[i - 1] + lengths[i]) * 0.5
			var excess := change - clampf(change, -limit, limit)
			var correction := excess / (a * a + b * b + c * c)
			heights[i - 1] -= correction * a
			heights[i] -= correction * b
			heights[i + 1] -= correction * c
			violation = maxf(violation, absf(excess))
		for pair in _neighbors:
			var first := int(pair.x)
			var last := int(pair.y)
			var delta := heights[last] - heights[first]
			var excess := delta - clampf(delta, -pair.z, pair.z)
			heights[first] += excess * 0.5
			heights[last] -= excess * 0.5
			violation = maxf(violation, absf(excess))
		if violation < EPSILON:
			break
	# All constraints are homogeneous differences. Reduce relief uniformly if
	# local projection did not fully converge; never relax a safety constraint.
	var scale := 1.0
	for i in lengths.size():
		scale = minf(scale, limits[i] / maxf(absf(heights[i + 1] - heights[i]), EPSILON))
	for i in range(1, heights.size() - 1):
		var change := absf((heights[i + 1] - heights[i]) / lengths[i] - (heights[i] - heights[i - 1]) / lengths[i - 1])
		scale = minf(scale, MAX_GRADE_CHANGE * (lengths[i - 1] + lengths[i]) * 0.5 / maxf(change, EPSILON))
	for pair in _neighbors:
		var difference := absf(heights[int(pair.y)] - heights[int(pair.x)])
		if difference > pair.z:
			scale = minf(scale, pair.z / difference)
	var mean := 0.0
	for height in heights:
		mean += height / heights.size()
	for i in heights.size():
		heights[i] = mean + (heights[i] - mean) * scale

func _index_segments(stage: Resource) -> void:
	_segments.clear()
	var radius: float = stage.ROAD_WIDTH * 0.5 + _config.blend_distance + Field.SPACING * sqrt(2.0)
	for i in range(stage.centers.size() - 1):
		var a: Vector3 = stage.centers[i]
		var b: Vector3 = stage.centers[i + 1]
		var first := _cell(Vector2(minf(a.x, b.x) - radius, minf(a.z, b.z) - radius))
		var last := _cell(Vector2(maxf(a.x, b.x) + radius, maxf(a.z, b.z) + radius))
		for x in range(first.x, last.x + 1):
			for z in range(first.y, last.y + 1):
				var cell := Vector2i(x, z)
				if not _segments.has(cell):
					_segments[cell] = []
				_segments[cell].append(i)

func _heightfield(stage: Resource) -> Resource:
	var field := Field.new()
	var first := Vector2(INF, INF)
	var last := Vector2(-INF, -INF)
	for point: Vector3 in stage.centers:
		first = first.min(Vector2(point.x, point.z))
		last = last.max(Vector2(point.x, point.z))
	var chunk_size := Field.SPACING * Field.CHUNK_CELLS
	field.origin = Vector2(floor((first.x - Field.MARGIN) / chunk_size), floor((first.y - Field.MARGIN) / chunk_size)) * chunk_size
	last = Vector2(ceil((last.x + Field.MARGIN) / chunk_size), ceil((last.y + Field.MARGIN) / chunk_size)) * chunk_size
	field.size = Vector2i((last - field.origin) / Field.SPACING) + Vector2i.ONE
	field.heights.resize(field.size.x * field.size.y)
	for z in field.size.y:
		for x in field.size.x:
			var point: Vector2 = field.origin + Vector2(x, z) * Field.SPACING
			var base := _raw(point)
			var nearest := INF
			var target := 0.0
			for i: int in _segments.get(_cell(point), []):
				var a: Vector3 = stage.centers[i]
				var b: Vector3 = stage.centers[i + 1]
				var edge := Vector2(b.x - a.x, b.z - a.z)
				var t := clampf((point - Vector2(a.x, a.z)).dot(edge) / edge.length_squared(), 0, 1)
				var distance := point.distance_squared_to(Vector2(a.x, a.z) + edge * t)
				if distance < nearest:
					nearest = distance
					target = lerpf(a.y, b.y, t)
			var distance := sqrt(nearest)
			var shoulder: float = maxf(0, distance - stage.ROAD_WIDTH * 0.5 - Field.SPACING * sqrt(2.0))
			var t := clampf(shoulder / _config.blend_distance, 0, 1)
			var weight := 1.0 - t * t * t * (t * (6 * t - 15) + 10)
			field.heights[z * field.size.x + x] = lerpf(base, target - Field.ROAD_CLEARANCE, weight)
	_cap_road_cells(stage, field)
	_limit_terrain_slopes(field)
	return field

func _limit_terrain_slopes(field: Resource) -> void:
	# A lowering-only distance transform preserves the road clearance caps.
	# Four raster directions cover all quadrants. Bounding both axis slopes
	# by gradient/sqrt(2) bounds either triangle's full gradient as well.
	var step := MAX_TERRAIN_GRADIENT * Field.SPACING / sqrt(2.0)
	for direction: Vector2i in [Vector2i(1, 1), Vector2i(-1, -1), Vector2i(-1, 1), Vector2i(1, -1)]:
		var z_start: int = 0 if direction.y > 0 else field.size.y - 1
		var z_end: int = field.size.y if direction.y > 0 else -1
		var x_start: int = 0 if direction.x > 0 else field.size.x - 1
		var x_end: int = field.size.x if direction.x > 0 else -1
		for z in range(z_start, z_end, direction.y):
			for x in range(x_start, x_end, direction.x):
				var i: int = z * field.size.x + x
				if x != x_start:
					field.heights[i] = minf(field.heights[i], field.heights[i - direction.x] + step)
				if z != z_start:
					field.heights[i] = minf(field.heights[i], field.heights[i - direction.y * field.size.x] + step)

func _cap_road_cells(stage: Resource, field: Resource) -> void:
	# Each cell intersecting a road triangle is kept below that triangle's
	# extended plane. This also protects edges from coarse-grid interpolation.
	for i in range(stage.centers.size() - 1):
		_cap_triangle(field, stage.left_edges[i], stage.right_edges[i], stage.left_edges[i + 1])
		_cap_triangle(field, stage.right_edges[i], stage.right_edges[i + 1], stage.left_edges[i + 1])

func _cap_triangle(field: Resource, a: Vector3, b: Vector3, c: Vector3) -> void:
	var up := (c - a).cross(b - a).normalized()
	var first := Vector2(minf(a.x, minf(b.x, c.x)), minf(a.z, minf(b.z, c.z)))
	var last := Vector2(maxf(a.x, maxf(b.x, c.x)), maxf(a.z, maxf(b.z, c.z)))
	var begin := Vector2i((first - field.origin) / Field.SPACING)
	var end := Vector2i((last - field.origin) / Field.SPACING) + Vector2i.ONE
	for z in range(maxi(0, begin.y), mini(field.size.y - 1, end.y) + 1):
		for x in range(maxi(0, begin.x), mini(field.size.x - 1, end.x) + 1):
			var point: Vector2 = field.origin + Vector2(x, z) * Field.SPACING
			var height := a.y - (up.x * (point.x - a.x) + up.z * (point.y - a.z)) / up.y - Field.ROAD_CLEARANCE
			var index: int = z * field.size.x + x
			field.heights[index] = minf(field.heights[index], height)

func _terrain_is_gentle(field: Resource) -> bool:
	for z in range(field.size.y - 1):
		for x in range(field.size.x - 1):
			var a: float = field.heights[z * field.size.x + x]
			var b: float = field.heights[z * field.size.x + x + 1]
			var c: float = field.heights[(z + 1) * field.size.x + x]
			var d: float = field.heights[(z + 1) * field.size.x + x + 1]
			if maxf(Vector2(b - a, c - a).length(), Vector2(d - c, d - b).length()) / Field.SPACING > MAX_TERRAIN_GRADIENT + EPSILON:
				return false
	return true

static func _right(yaw: float) -> Vector3:
	return Vector3(cos(yaw), 0, -sin(yaw))

static func _cell(point: Vector2) -> Vector2i:
	return Vector2i(floori(point.x / CELL_SIZE), floori(point.y / CELL_SIZE))
