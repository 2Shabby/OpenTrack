extends RefCounted

# Each worker owns its sampler. Read-only precomputed segments and native noise
# avoid contention on one shared Resource during full-resolution authoring.
var _noise := FastNoiseLite.new()
var _starts := PackedVector2Array()
var _edges := PackedVector2Array()
var _ends := PackedVector2Array()
var _lengths := PackedFloat32Array()
var _heights := PackedVector2Array()
var _amplitude := 0.0
var _blend := 16.0
var _half_width := 6.0

func _init(centers: PackedVector3Array, config: Resource, seed_value: int, half_width: float) -> void:
	_amplitude = config.amplitude
	_blend = config.blend_distance
	_half_width = half_width
	_noise.seed = seed_value
	_noise.noise_type = FastNoiseLite.TYPE_SIMPLEX_SMOOTH
	_noise.fractal_type = FastNoiseLite.FRACTAL_FBM
	_noise.fractal_octaves = 3
	_noise.fractal_gain = 0.35
	_noise.frequency = 1.0 / config.wavelength
	_noise.offset = Vector3(331, 0, 719)
	for i in range(centers.size() - 1):
		var a := Vector2(centers[i].x, centers[i].z)
		var b := Vector2(centers[i + 1].x, centers[i + 1].z)
		_starts.append(a)
		_ends.append(b)
		_edges.append(b - a)
		_lengths.append((b - a).length_squared())
		_heights.append(Vector2(centers[i].y, centers[i + 1].y - centers[i].y))

func candidates(center: Vector2, radius: float, indices: PackedInt32Array) -> PackedInt32Array:
	var distances := PackedFloat32Array()
	var nearest := INF
	for i in indices:
		var distance := center.distance_to(Geometry2D.get_closest_point_to_segment(center, _starts[i], _ends[i]))
		distances.append(distance)
		nearest = minf(nearest, distance)
	var reach := sqrt((nearest + radius) * (nearest + radius) + 64.0) + radius
	var result := PackedInt32Array()
	for i in indices.size():
		if distances[i] <= reach:
			result.append(indices[i])
	return result

func sample(point: Vector2, indices: PackedInt32Array) -> Vector2:
	var base := _noise.get_noise_2d(point.x, point.y) * _amplitude
	var nearest := INF
	for i in indices:
		if (point - point.clamp(_starts[i].min(_ends[i]), _starts[i].max(_ends[i]))).length_squared() > nearest:
			continue
		var t := clampf((point - _starts[i]).dot(_edges[i]) / _lengths[i], 0, 1)
		nearest = minf(nearest, point.distance_squared_to(_starts[i] + _edges[i] * t))
	var total := 0.0
	var weighted := 0.0
	var reach := nearest + 64.0
	for i in indices:
		if (point - point.clamp(_starts[i].min(_ends[i]), _starts[i].max(_ends[i]))).length_squared() > reach:
			continue
		var t := clampf((point - _starts[i]).dot(_edges[i]) / _lengths[i], 0, 1)
		var squared := point.distance_squared_to(_starts[i] + _edges[i] * t)
		var compact := maxf(0.0, 1.0 - (squared - nearest) / 64.0)
		var weight := compact * compact / (squared + 4.0)
		total += weight
		weighted += weight * (_heights[i].x + _heights[i].y * t)
	var target := weighted / total if total > 0 else base
	var distance := sqrt(nearest)
	var shoulder_distance := maxf(0.0, distance - _half_width - 2.0)
	var t := clampf(shoulder_distance / _blend, 0, 1)
	var weight := t * t * t * (t * (6.0 * t - 15.0) + 10.0)
	var allowance := shoulder_distance * 0.10
	base = clampf(base, target - allowance, target + allowance)
	return Vector2(lerpf(target, base, weight), distance)
