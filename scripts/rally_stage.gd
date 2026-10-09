class_name RallyStage
extends Resource

# The recipe, rendered road, collision road and pacenotes share these stations.
@export var stage_id := ""
@export var display_name := "Procedural test stage"
@export var region := ""
@export var source_url := ""
@export var source_gpx := ""
@export var source_sha256 := ""
@export var source_length_m := 0.0
@export var road_width := 12.0
@export var baked_scene_path := ""
@export var baked_bounds := AABB()
@export var baked_stats: Dictionary = {}
@export var baked_fingerprint := ""
@export var generator_version := "rally-voxel-v4"
@export var engine_version: String = Engine.get_version_info()["string"]
@export var seed_value := 0
@export var length_m := 0.0
@export var requested_length_m := 0
@export var features: Array[Dictionary] = []
@export var centers := PackedVector3Array()
@export var headings := PackedFloat64Array()
@export var distances := PackedFloat64Array()
@export var left_edges := PackedVector3Array()
@export var right_edges := PackedVector3Array()
@export var road_normals := PackedVector3Array()
@export var terrain_settings: Resource
@export var terrain: Resource

const ROAD_WIDTH := 12.0

func info() -> Dictionary:
	return {"seed": seed_value, "generator": generator_version, "engine": engine_version, "length_m": length_m, "feature_count": features.size(), "track_width": road_width, "id": stage_id, "name": display_name, "region": region, "source_url": source_url}

func spawn_pose() -> Transform3D:
	return road_pose(5.0)

func rebuild_road() -> void:
	left_edges.resize(centers.size())
	right_edges.resize(centers.size())
	road_normals.resize(centers.size() * 2)
	road_normals.fill(Vector3.ZERO)
	distances.resize(centers.size())
	distances[0] = 0.0
	for i in centers.size():
		var right := Vector3(cos(headings[i]), 0, -sin(headings[i])) * road_width * 0.5
		left_edges[i] = centers[i] - right
		right_edges[i] = centers[i] + right
		if i > 0:
			distances[i] = distances[i - 1] + centers[i].distance_to(centers[i - 1])
	for i in range(centers.size() - 1):
		var a := i * 2
		_accumulate_normal(a, a + 1, a + 2, left_edges[i], right_edges[i], left_edges[i + 1])
		_accumulate_normal(a + 1, a + 3, a + 2, right_edges[i], right_edges[i + 1], left_edges[i + 1])
	for i in road_normals.size():
		road_normals[i] = road_normals[i].normalized()
	length_m = distances[-1]
	for feature in features:
		feature["start_m"] = distances[feature["first_station"]]
		feature["end_m"] = distances[feature["last_station"]]
		feature["length_m"] = feature["end_m"] - feature["start_m"]

func _accumulate_normal(i: int, j: int, k: int, a: Vector3, b: Vector3, c: Vector3) -> void:
	var up := (c - a).cross(b - a)
	road_normals[i] += up
	road_normals[j] += up
	road_normals[k] += up

func road_pose(distance: float) -> Transform3D:
	var i := clampi(distances.bsearch(clampf(distance, 0, length_m)) - 1, 0, centers.size() - 2)
	var t := clampf((distance - distances[i]) / (distances[i + 1] - distances[i]), 0, 1)
	var yaw := lerp_angle(headings[i], headings[i + 1], t)
	var right := Vector3(cos(yaw), 0, -sin(yaw))
	var forward := (centers[i + 1] - centers[i]).slide(right).normalized()
	var up := forward.cross(right).normalized()
	return Transform3D(Basis(right, up, forward), centers[i].lerp(centers[i + 1], t))

func crossed_finish(previous: Vector3, current: Vector3) -> bool:
	var inverse := road_pose(length_m).affine_inverse()
	var start := inverse * previous
	var finish := inverse * current
	if start.z >= 0.0 or finish.z < 0.0:
		return false
	var t := start.z / (start.z - finish.z)
	var crossing := start.lerp(finish, t)
	return absf(crossing.x) <= road_width * 0.5 and crossing.y >= -1.5 and crossing.y <= 20.0

func pieces() -> Array[Dictionary]:
	var result: Array[Dictionary] = []
	for feature in features:
		var vertices := PackedVector3Array()
		var normals := PackedVector3Array()
		var uvs := PackedVector2Array()
		var indices := PackedInt32Array()
		for station in range(feature["first_station"], feature["last_station"] + 1):
			vertices.append_array(PackedVector3Array([left_edges[station], right_edges[station]]))
			normals.append_array(PackedVector3Array([road_normals[station * 2], road_normals[station * 2 + 1]]))
			uvs.append_array(PackedVector2Array([Vector2(0, distances[station]), Vector2(1, distances[station])]))
			if station > feature["first_station"]:
				var a := vertices.size() - 4
				# Godot uses clockwise front faces; normals still point up.
				indices.append_array(PackedInt32Array([a, a + 1, a + 2, a + 1, a + 3, a + 2]))
		var piece := {"vertices": vertices, "normals": normals, "uvs": uvs, "indices": indices, "surface": feature["surface"]}
		if feature == features.back():
			var pose := road_pose(length_m)
			pose.origin += pose.basis.y * 0.04
			piece["finish"] = pose
		result.append(piece)
	return result

func project_distance(position: Vector3, hint: int) -> Dictionary:
	var result := _project(position, maxi(0, hint - 30), mini(centers.size() - 1, hint + 90))
	if result["offset_squared"] > 400.0:
		result = _project(position, 0, centers.size() - 1)
	return result

func _project(position: Vector3, first: int, last: int) -> Dictionary:
	var best := {"distance": 0.0, "station": 0, "offset_squared": INF}
	for i in range(first, last):
		var edge := centers[i + 1] - centers[i]
		var t := clampf((position - centers[i]).dot(edge) / edge.length_squared(), 0.0, 1.0)
		var offset := position.distance_squared_to(centers[i] + edge * t)
		if offset < best["offset_squared"]:
			best = {"distance": lerpf(distances[i], distances[i + 1], t), "station": i, "offset_squared": offset}
	return best

func pacenote(progress: float) -> String:
	for i in features.size():
		var feature := features[i]
		if feature["kind"] != "corner" or feature["end_m"] <= progress:
			continue
		var note := "%s %d · %d m" % [feature["direction"], feature["grade"], roundi(feature["length_m"])]
		if progress < feature["start_m"]:
			note = "%d m → %s" % [roundi(feature["start_m"] - progress), note]
		else:
			note += " · %d m remaining" % roundi(feature["end_m"] - progress)
		if i + 1 < features.size():
			var next := features[i + 1]
			if next["kind"] == "corner":
				note += "  into %s %d · %d m" % [next["direction"], next["grade"], roundi(next["length_m"])]
			else:
				note += "  then %d m straight" % roundi(next["length_m"])
		return note
	return "Finish · %d m" % maxi(0, roundi(length_m - progress))
