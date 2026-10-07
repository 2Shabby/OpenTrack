class_name RallyGenerator
extends RefCounted

const Stage := preload("res://scripts/rally_stage.gd")
const Terrain := preload("res://scripts/terrain_builder.gd")
const MIN_LENGTH := 500
const MAX_LENGTH := 5000
# Grade is peak curvature: 1 tight, 6 fast. Length is independent of grade.
const RADII := [14.0, 22.0, 35.0, 55.0, 90.0, 140.0]
const MIN_ANGLES := [80.0, 60.0, 40.0, 25.0, 18.0, 12.0]
const MAX_ANGLES := [130.0, 110.0, 90.0, 70.0, 50.0, 35.0]
const SAMPLE_SPACING := 2.0
const CLEARANCE := Stage.ROAD_WIDTH + 2.0
const CELL_SIZE := CLEARANCE
const END_STRAIGHT := 40.0

var error := ""
var _rng := RandomNumberGenerator.new()
var _occupied: Dictionary = {}

func generate(seed_value: int, requested_length: int, terrain_settings: Resource) -> Resource:
	error = ""
	if requested_length < MIN_LENGTH or requested_length > MAX_LENGTH:
		error = "Stage length must be between %d and %d metres." % [MIN_LENGTH, MAX_LENGTH]
		return null
	_rng.seed = seed_value
	for _attempt in 12:
		var stage := Stage.new()
		stage.seed_value = seed_value
		stage.length_m = requested_length
		stage.requested_length_m = requested_length
		stage.centers.append(Vector3.ZERO)
		stage.headings.append(0.0)
		stage.distances.append(0.0)
		_occupied.clear()
		_index_station(Vector3.ZERO, 0.0)
		_append(stage, _sample(stage, "straight", 80.0, 0.0), "straight", 80.0, 0, 0.0, 1)
		var linked_left := _rng.randi_range(1, 3)
		var surface := 1
		var surface_end := _rng.randf_range(250.0, 450.0)
		var last_long_straight := 0.0
		var failed := false
		while stage.distances[-1] < requested_length - 0.01:
			var progress: float = stage.distances[-1]
			var remaining := requested_length - progress
			if remaining < 135.0:
				var ending := _sample(stage, "straight", remaining, 0.0)
				if not _clear(ending, progress):
					failed = true
					break
				_append(stage, ending, "straight", remaining, 0, 0.0, surface)
				break
			if linked_left == 0:
				var length := _rng.randf_range(150.0, 220.0) if progress - last_long_straight > 350.0 else _rng.randf_range(50.0, 130.0)
				length = minf(snappedf(length, 10.0), remaining - END_STRAIGHT)
				var samples := _sample(stage, "straight", length, 0.0)
				if not _clear(samples, progress):
					failed = true
					break
				# Surface changes happen between phrases, rather than per mesh slice.
				if progress >= surface_end:
					surface = 1 - surface
					surface_end = progress + (_rng.randf_range(250.0, 450.0) if surface == 1 else _rng.randf_range(90.0, 180.0))
				_append(stage, samples, "straight", length, 0, 0.0, surface)
				if length >= 150.0:
					last_long_straight = progress
				linked_left = _rng.randi_range(1, 3)
				continue
			var accepted := false
			for _candidate in 32:
				var grade := _rng.randi_range(1, 6)
				var angle := deg_to_rad(_rng.randf_range(MIN_ANGLES[grade - 1], MAX_ANGLES[grade - 1]))
				var length := snappedf(2.0 * RADII[grade - 1] * angle, 5.0)
				# Integrating the eased heading gives a peak radius of RADII[grade-1].
				angle = length / (2.0 * RADII[grade - 1])
				var direction := -1.0 if _rng.randf() < 0.5 else 1.0
				var yaw: float = stage.headings[-1]
				if absf(yaw) > 0.6:
					direction = -signf(yaw)
				angle *= direction
				if absf(yaw + angle) > 1.45 or length > remaining - END_STRAIGHT:
					continue
				var samples := _sample(stage, "corner", length, angle)
				if not _clear(samples, progress):
					continue
				_append(stage, samples, "corner", length, grade, angle, surface)
				linked_left -= 1
				accepted = true
				break
			if not accepted:
				failed = true
				break
		if not failed:
			var terrain_builder := Terrain.new()
			if not terrain_builder.apply(stage, terrain_settings):
				error = terrain_builder.error
				return null
			return stage
	error = "Could not generate a stage with sufficient road clearance. Try another seed."
	return null

func _sample(stage: Resource, kind: String, length: float, angle: float) -> Dictionary:
	var centers := PackedVector3Array()
	var headings := PackedFloat64Array()
	var steps := maxi(1, ceili(length / SAMPLE_SPACING))
	var step := length / steps
	var point: Vector3 = stage.centers[-1]
	var yaw: float = stage.headings[-1]
	for i in range(1, steps + 1):
		var t := (float(i) - 0.5) / steps
		var mid_yaw := yaw + angle * (t - sin(TAU * t) / TAU) if kind == "corner" else yaw
		point += Vector3(sin(mid_yaw), 0, cos(mid_yaw)) * step
		t = float(i) / steps
		centers.append(point)
		headings.append(yaw + angle * (t - sin(TAU * t) / TAU) if kind == "corner" else yaw)
	return {"centers": centers, "headings": headings, "step": step}

func _clear(samples: Dictionary, start_distance: float) -> bool:
	var points: PackedVector3Array = samples["centers"]
	for i in points.size():
		var point := points[i]
		var distance: float = start_distance + (i + 1) * samples["step"]
		var cell := _cell(point)
		for x in range(cell.x - 1, cell.x + 2):
			for y in range(cell.y - 1, cell.y + 2):
				for previous: Vector4 in _occupied.get(Vector2i(x, y), []):
					if distance - previous.w < 24.0:
						continue
					if point.distance_squared_to(Vector3(previous.x, 0, previous.z)) < CLEARANCE * CLEARANCE:
						return false
	return true

func _append(stage: Resource, samples: Dictionary, kind: String, length: float, grade: int, angle: float, surface: int) -> void:
	var start: float = stage.distances[-1]
	var first: int = stage.centers.size() - 1
	var points: PackedVector3Array = samples["centers"]
	stage.centers.append_array(points)
	stage.headings.append_array(samples["headings"])
	for i in points.size():
		var distance: float = start + (i + 1) * samples["step"]
		stage.distances.append(distance)
		_index_station(points[i], distance)
	if kind == "straight" and not stage.features.is_empty() and stage.features[-1]["kind"] == "straight" and stage.features[-1]["surface"] == surface:
		var previous: Dictionary = stage.features[-1]
		previous["length_m"] += length
		previous["end_m"] = start + length
		previous["last_station"] = stage.centers.size() - 1
	else:
		stage.features.append({"kind": kind, "length_m": length, "start_m": start, "end_m": start + length, "grade": grade, "direction": "Left" if angle > 0 else "Right", "angle": angle, "surface": surface, "first_station": first, "last_station": stage.centers.size() - 1})

func _cell(point: Vector3) -> Vector2i:
	return Vector2i(floori(point.x / CELL_SIZE), floori(point.z / CELL_SIZE))

func _index_station(point: Vector3, distance: float) -> void:
	var cell := _cell(point)
	if not _occupied.has(cell):
		_occupied[cell] = []
	_occupied[cell].append(Vector4(point.x, 0, point.z, distance))
