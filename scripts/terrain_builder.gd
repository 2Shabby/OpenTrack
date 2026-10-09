extends RefCounted

const Field := preload("res://scripts/terrain_field.gd")
const CELL_SIZE := 32.0
const MAX_GRADE_CHANGE := 0.006
const MAX_SHOULDER_GRADIENT := 0.20
const MAX_TERRAIN_GRADIENT := 0.35
const EPSILON := 0.00001

var error := ""
var _noise := FastNoiseLite.new()
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
	stage.terrain = Field.new()
	stage.terrain.initialize(stage, config)
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
	var radius: float = stage.road_width + 2 * _config.blend_distance
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
					var limit: float = maxf(0, distance - stage.road_width) * MAX_SHOULDER_GRADIENT
					if limit < minf(2 * _config.amplitude, path_distance * _config.max_gradient):
						_neighbors.append(Vector3(j, i, limit))
		if not buckets.has(cell):
			buckets[cell] = []
		buckets[cell].append(i)

func _constrain(stage: Resource, heights: PackedFloat64Array) -> void:
	var lengths := PackedFloat64Array()
	var limits := PackedFloat64Array()
	for i in range(1, stage.centers.size()):
		var left: Vector3 = stage.centers[i] - _right(stage.headings[i]) * stage.road_width * 0.5
		var previous_left: Vector3 = stage.centers[i - 1] - _right(stage.headings[i - 1]) * stage.road_width * 0.5
		var right: Vector3 = stage.centers[i] + _right(stage.headings[i]) * stage.road_width * 0.5
		var previous_right: Vector3 = stage.centers[i - 1] + _right(stage.headings[i - 1]) * stage.road_width * 0.5
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

static func _right(yaw: float) -> Vector3:
	return Vector3(cos(yaw), 0, -sin(yaw))

static func _cell(point: Vector2) -> Vector2i:
	return Vector2i(floori(point.x / CELL_SIZE), floori(point.y / CELL_SIZE))
