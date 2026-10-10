extends Resource

# Full-resolution authoring. Every selected chunk is built and remains resident.
const SPACING := 0.1
const CHUNK_METERS := 32.0
const CHUNK_CELLS := 320
const QUERY_METERS := 4.0
const BORDER_METERS := 0.5
const MAX_GRADIENT := 0.35
const Sampler := preload("res://scripts/terrain_sampler.gd")
const Mesher := preload("res://scripts/terrain_mesher.gd")

var chunks: Dictionary = {}
var meshes: Array[ArrayMesh] = []
var stats: Dictionary = {}
var error := ""
var _chunk_data: Array[Dictionary] = []
var _centers := PackedVector3Array()
var _config: Resource
var _half_width := 6.0
var _seed := 0
var _sampler: RefCounted
var _segments: Dictionary = {}
var _road_polygons: Dictionary = {}
var _border_segments: Dictionary = {}
var _surfaces: Array[Dictionary] = []
var _support: Dictionary = {}
# Optional offline acceleration; saved worlds never depend on the extension.
var native_authoring: RefCounted
var authoring_only := false
var _road_bounds: Dictionary = {}

func initialize(stage: Resource, config: Resource) -> void:
	_centers = stage.centers.duplicate()
	_config = config.duplicate()
	_half_width = stage.road_width * 0.5
	_seed = int(stage.seed_value & 0x7fffffff)
	_sampler = new_sampler()
	var padding := maxf(96.0, _half_width + config.blend_distance + 2.0 * config.amplitude / MAX_GRADIENT)
	padding = ceil(padding / CHUNK_METERS) * CHUNK_METERS
	for i in range(_centers.size() - 1):
		var a := Vector2(_centers[i].x, _centers[i].z)
		var b := Vector2(_centers[i + 1].x, _centers[i + 1].z)
		for key in _keys_for_bounds(a.min(b) - Vector2.ONE * padding, a.max(b) + Vector2.ONE * padding):
			var midpoint := (Vector2(key) + Vector2.ONE * 0.5) * CHUNK_METERS
			if midpoint.distance_to(Geometry2D.get_closest_point_to_segment(midpoint, a, b)) <= padding + CHUNK_METERS * sqrt(0.5):
				chunks[key] = {"coord": key}
		# The index also covers every corner of the selected boundary chunks.
		var reach := padding + CHUNK_METERS * 2.0
		for key in _keys_for_bounds(a.min(b) - Vector2.ONE * reach, a.max(b) + Vector2.ONE * reach):
			if not _segments.has(key):
				_segments[key] = PackedInt32Array()
			_segments[key].append(i)
	_fill_footprint_holes()
	for key: Vector2i in chunks:
		_chunk_data.append(chunks[key])
	stats = {"voxel_m": SPACING, "chunk_count": chunks.size(), "area_m2": chunks.size() * CHUNK_METERS * CHUNK_METERS, "sample_count": chunks.size() * (CHUNK_CELLS + 1) * (CHUNK_CELLS + 1)}

func _fill_footprint_holes() -> void:
	var first := Vector2i(2147483647, 2147483647)
	var last := -first
	for key: Vector2i in chunks:
		first = first.min(key)
		last = last.max(key)
	first -= Vector2i.ONE
	last += Vector2i.ONE
	var exterior := {first: true}
	var pending: Array[Vector2i] = [first]
	var cursor := 0
	while cursor < pending.size():
		var point := pending[cursor]
		cursor += 1
		for step: Vector2i in [Vector2i.LEFT, Vector2i.RIGHT, Vector2i.UP, Vector2i.DOWN]:
			var next := point + step
			if next.x < first.x or next.y < first.y or next.x > last.x or next.y > last.y or exterior.has(next) or chunks.has(next):
				continue
			exterior[next] = true
			pending.append(next)
	for z in range(first.y + 1, last.y):
		for x in range(first.x + 1, last.x):
			var key := Vector2i(x, z)
			if not exterior.has(key) and not chunks.has(key):
				chunks[key] = {"coord": key}

# x = source elevation; y = centreline distance. These are direct samples.
func new_sampler() -> RefCounted:
	return Sampler.new(_centers, _config, _seed, _half_width)

func road_candidates(point: Vector2) -> PackedInt32Array:
	return _segments.get(chunk_key(point), PackedInt32Array())

func sample(point: Vector2) -> Vector2:
	if native_authoring != null:
		return native_authoring.sample_point(point)
	return _sampler.sample(point, road_candidates(point))

func sample_height(point: Vector2) -> float:
	return sample(point).x

func sample_normal(point: Vector2) -> Vector3:
	var dx := sample_height(point + Vector2(SPACING, 0)) - sample_height(point - Vector2(SPACING, 0))
	var dz := sample_height(point + Vector2(0, SPACING)) - sample_height(point - Vector2(0, SPACING))
	return Vector3(-dx, 2.0 * SPACING, -dz).normalized()

func smooth_distance() -> float:
	return _half_width + 2.0 + _config.blend_distance

func set_road_border(paths: Array[PackedVector3Array], borders: Array[PackedVector3Array]) -> void:
	for i in range(paths[0].size() - 1):
		var polygon := PackedVector2Array([Vector2(paths[0][i].x, paths[0][i].z), Vector2(paths[1][i].x, paths[1][i].z), Vector2(paths[1][i + 1].x, paths[1][i + 1].z), Vector2(paths[0][i + 1].x, paths[0][i + 1].z)])
		var bounds := Rect2(polygon[0], Vector2.ZERO)
		for p in polygon:
			bounds = bounds.expand(p)
		for key in _keys_for_bounds(bounds.position, bounds.end):
			if not _road_polygons.has(key):
				_road_polygons[key] = []
				_road_bounds[key] = []
			_road_polygons[key].append(polygon)
			_road_bounds[key].append(bounds)
	for path in borders:
		for i in range(path.size() - 1):
			_index_border(path[i], path[i + 1])

func _index_border(a: Vector3, b: Vector3) -> void:
	var first := Vector2(a.x, a.z)
	var last := Vector2(b.x, b.z)
	for key in _border_keys(first.min(last) - Vector2.ONE * 0.001, first.max(last) + Vector2.ONE * 0.001):
		if not _border_segments.has(key):
			_border_segments[key] = []
		_border_segments[key].append([a, b])

func road_polygons(point: Vector2) -> Array:
	return _road_polygons.get(chunk_key(point), [])

func road_polygons_for_bounds(first: Vector2, last: Vector2) -> Array:
	var key := chunk_key((first + last) * 0.5)
	var query := Rect2(first, last - first).grow(0.001)
	var polygons: Array = _road_polygons.get(key, [])
	var bounds: Array = _road_bounds.get(key, [])
	var result: Array = []
	for i in polygons.size():
		if query.intersects(bounds[i], true):
			result.append(polygons[i])
	return result

func border_segments(first: Vector2, last: Vector2) -> Array:
	var result: Array = []
	for key in _border_keys(first.min(last) - Vector2.ONE * 0.001, first.max(last) + Vector2.ONE * 0.001):
		result.append_array(_border_segments.get(key, []))
	return result

static func _border_keys(first: Vector2, last: Vector2) -> Array[Vector2i]:
	var result: Array[Vector2i] = []
	var begin := Vector2i(floori(first.x / BORDER_METERS), floori(first.y / BORDER_METERS))
	var end := Vector2i(floori(last.x / BORDER_METERS), floori(last.y / BORDER_METERS))
	for z in range(begin.y, end.y + 1):
		for x in range(begin.x, end.x + 1):
			result.append(Vector2i(x, z))
	return result

func border_height(point: Vector2) -> float:
	for segment: Array in border_segments(point, point):
		var a := Vector2(segment[0].x, segment[0].z)
		var b := Vector2(segment[1].x, segment[1].z)
		var nearest := Geometry2D.get_closest_point_to_segment(point, a, b)
		if point.distance_squared_to(nearest) < 0.00000025:
			var t := (nearest - a).length() / maxf((b - a).length(), 0.000001)
			return lerpf(segment[0].y, segment[1].y, t)
	return NAN

func in_road(point: Vector2) -> bool:
	for polygon: PackedVector2Array in road_polygons(point):
		if Geometry2D.is_point_in_polygon(point, polygon):
			return true
	return false

func build(tree: SceneTree) -> bool:
	var started := Time.get_ticks_msec()
	var group := WorkerThreadPool.add_group_task(_sample_chunk, _chunk_data.size(), -1, false, "10 cm terrain sampling")
	await _wait_group(tree, group)
	for chunk in _chunk_data:
		if chunk.has("error"):
			error = chunk.error
			return false
	group = WorkerThreadPool.add_group_task(_mesh_chunk, _chunk_data.size(), -1, false, "Resident terrain meshing")
	await _wait_group(tree, group)
	for _pass in 12:
		var requested := {}
		for chunk in _chunk_data:
			for index: int in chunk.refine:
				var leaf: Dictionary = chunk.leaves[index]
				requested[leaf.rect] = true
				# A small patch can inherit error from a coarse neighbour's edge.
				# Refine the neighbouring rectangles too, rather than opening a seam.
				var rect: Rect2i = leaf.rect
				for point: Vector2i in [rect.position, Vector2i(rect.end.x, rect.position.y), Vector2i(rect.position.x, rect.end.y), rect.end]:
					for offset: Vector2i in [Vector2i.ZERO, Vector2i.LEFT, Vector2i.UP, -Vector2i.ONE]:
						var neighbor := leaf_at(point + offset)
						if not neighbor.is_empty() and neighbor.rect.size != Vector2i.ONE:
							requested[neighbor.rect] = true
		if requested.is_empty():
			break
		print("Terrain surface refinement ", _pass + 1, ": ", requested.size(), " patches")
		for chunk in _chunk_data:
			chunk["needs_refine"] = PackedInt32Array()
			for i in chunk.leaves.size():
				if requested.has(chunk.leaves[i].rect):
					chunk.needs_refine.append(i)
			chunk["mesh_dirty"] = not chunk.needs_refine.is_empty()
			if not chunk.mesh_dirty:
				for rect: Rect2i in chunk.mesh_dependencies:
					if requested.has(rect):
						chunk.mesh_dirty = true
						break
		group = WorkerThreadPool.add_group_task(_refine_chunk, _chunk_data.size(), -1, false, "Conforming terrain refinement")
		await _wait_group(tree, group)
		group = WorkerThreadPool.add_group_task(_mesh_chunk, _chunk_data.size(), -1, false, "Conforming terrain meshing")
		await _wait_group(tree, group)
	var triangles := 0
	var maximum_error := 0.0
	var max_gradient := 0.0
	for chunk in _chunk_data:
		if not chunk.mesh_error.is_empty():
			error = chunk.mesh_error
			return false
		if not chunk.refine.is_empty():
			for suspect in _chunk_data:
				if not suspect.get("refine", []).is_empty():
					print("DIAGNOSTIC_CHUNK ", suspect.coord, " max_error=", suspect.max_error)
					for index in suspect.refine:
						var patch: Dictionary = suspect.leaves[index]
						var result: Dictionary = native_authoring.validate_top(patch.rect, suspect.arrays[Mesh.ARRAY_VERTEX], 0, suspect.samples, suspect.distances, suspect.coord, smooth_distance())
						print("DIAGNOSTIC_PATCH ", patch.rect, " ", result)
			error = "Terrain could not meet its surface error bound."
			return false
		var mesh := ArrayMesh.new()
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, chunk.arrays)
		meshes.append(mesh)
		triangles += chunk.arrays[Mesh.ARRAY_VERTEX].size() / 3
		maximum_error = maxf(maximum_error, chunk.max_error)
		max_gradient = maxf(max_gradient, chunk.source_max_gradient)
		chunk.erase("arrays")
		chunk.erase("owners")
		chunk.erase("leaves")
		chunk.erase("samples")
		chunk.erase("distances")
		chunk.erase("refine")
		chunk.erase("needs_refine")
		chunk.erase("mesh_dependencies")
		chunk.erase("mesh_dirty")
	stats.merge({"generation_ms": Time.get_ticks_msec() - started, "terrain_triangles": triangles, "max_surface_error_m": maximum_error, "source_max_gradient": max_gradient})
	print("Resident terrain: ", stats)
	return true

func _wait_group(tree: SceneTree, group: int) -> void:
	while not WorkerThreadPool.is_group_task_completed(group):
		await tree.process_frame
	WorkerThreadPool.wait_for_group_task_completion(group)

func _sample_chunk(index: int) -> void:
	var chunk := _chunk_data[index]
	chunk.merge(native_authoring.sample_chunk(chunk.coord) if native_authoring != null else Mesher.new().sample_chunk(self, chunk.coord))
	if index % 50 == 0:
		print("Terrain sampled chunk ", index + 1, "/", _chunk_data.size())

func _mesh_chunk(index: int) -> void:
	var chunk := _chunk_data[index]
	if not get_meta("full_remesh", false) and not chunk.get("mesh_dirty", true):
		return
	chunk.merge(Mesher.new().mesh_chunk(self, chunk), true)

func _refine_chunk(index: int) -> void:
	var chunk := _chunk_data[index]
	if not chunk.needs_refine.is_empty():
		Mesher.new().refine_chunk(self, chunk)

func source_sample(cell: Vector2i) -> Vector2:
	for offset: Vector2i in [Vector2i.ZERO, Vector2i.LEFT, Vector2i.UP, -Vector2i.ONE]:
		var key := Vector2i(floori(float(cell.x + offset.x) / CHUNK_CELLS), floori(float(cell.y + offset.y) / CHUNK_CELLS))
		if chunks.has(key):
			var chunk: Dictionary = chunks[key]
			var local := cell - key * CHUNK_CELLS
			var index := local.y * (CHUNK_CELLS + 1) + local.x
			return Vector2(chunk.samples[index], chunk.distances[index])
	return sample(Vector2(cell) * SPACING)

func leaf_at(cell: Vector2i) -> Dictionary:
	var key := Vector2i(floori(float(cell.x) / CHUNK_CELLS), floori(float(cell.y) / CHUNK_CELLS))
	if not chunks.has(key):
		return {}
	var chunk: Dictionary = chunks[key]
	var local := cell - key * CHUNK_CELLS
	return chunk.leaves[chunk.owners[local.y * CHUNK_CELLS + local.x]]

# Register the actual finished surfaces, including road and shoulders. Queries
# therefore use the same planes as the native colliders, never the noise recipe.
func register_surface(mesh: ArrayMesh) -> void:
	if authoring_only:
		return
	var arrays := mesh.surface_get_arrays(0)
	var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX] if arrays[Mesh.ARRAY_INDEX] != null else PackedInt32Array()
	if indices.is_empty():
		indices.resize(vertices.size())
		for i in indices.size():
			indices[i] = i
	var surface := _surfaces.size()
	_surfaces.append({"vertices": vertices, "indices": indices})
	for i in range(0, indices.size(), 3):
		var a := vertices[indices[i]]
		var b := vertices[indices[i + 1]]
		var c := vertices[indices[i + 2]]
		if absf((b - a).cross(c - a).y) < 0.0000001:
			continue
		var first := Vector2i(floori(minf(a.x, minf(b.x, c.x)) / QUERY_METERS), floori(minf(a.z, minf(b.z, c.z)) / QUERY_METERS))
		var last := Vector2i(floori(maxf(a.x, maxf(b.x, c.x)) / QUERY_METERS), floori(maxf(a.z, maxf(b.z, c.z)) / QUERY_METERS))
		for z in range(first.y, last.y + 1):
			for x in range(first.x, last.x + 1):
				var key := Vector2i(x, z)
				if not _support.has(key):
					_support[key] = PackedInt64Array()
				_support[key].append((surface << 32) | i)

func height_at(point: Vector2) -> float:
	if not chunks.has(chunk_key(point)):
		return NAN
	var result := -INF
	var key := Vector2i(floori(point.x / QUERY_METERS), floori(point.y / QUERY_METERS))
	for entry: int in _support.get(key, PackedInt64Array()):
		var surface := _surfaces[entry >> 32]
		var i := entry & 0xffffffff
		var a: Vector3 = surface.vertices[surface.indices[i]]
		var b: Vector3 = surface.vertices[surface.indices[i + 1]]
		var c: Vector3 = surface.vertices[surface.indices[i + 2]]
		var height := triangle_height(point, a, b, c)
		if is_finite(height):
			result = maxf(result, height)
	return result if is_finite(result) else NAN

static func triangle_height(point: Vector2, a: Vector3, b: Vector3, c: Vector3) -> float:
	var ab := Vector2(b.x - a.x, b.z - a.z)
	var ac := Vector2(c.x - a.x, c.z - a.z)
	var ap := point - Vector2(a.x, a.z)
	var determinant := ab.cross(ac)
	if absf(determinant) < 0.0000001:
		return NAN
	var u := ap.cross(ac) / determinant
	var v := ab.cross(ap) / determinant
	if u < -0.0001 or v < -0.0001 or u + v > 1.0001:
		return NAN
	return a.y + u * (b.y - a.y) + v * (c.y - a.y)

static func chunk_key(point: Vector2) -> Vector2i:
	return Vector2i(floori(point.x / CHUNK_METERS), floori(point.y / CHUNK_METERS))

static func _keys_for_bounds(first: Vector2, last: Vector2) -> Array[Vector2i]:
	var result: Array[Vector2i] = []
	var begin := chunk_key(first)
	var end := chunk_key(last)
	for z in range(begin.y, end.y + 1):
		for x in range(begin.x, end.x + 1):
			result.append(Vector2i(x, z))
	return result
