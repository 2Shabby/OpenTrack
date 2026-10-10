extends RefCounted

# Sampling stays at 10 cm. Geometry coalesces only when every source sample
# fits the error bound; this is surface simplification, not a terrain LOD.
const SMOOTH_ERROR := 0.005
const SURFACE_ERROR := 0.1
const BLOCK_CELLS := 5

var _field: Resource
var _coord := Vector2i.ZERO
var _samples := PackedFloat32Array()
var _distances := PackedFloat32Array()
var _pyramid: Array[Dictionary] = []
var _leaves: Array[Dictionary] = []
var _owners := PackedInt32Array()
var _max_error := 0.0
var _smooth_distance := 0.0
var _vertices := PackedVector3Array()
var _normals := PackedVector3Array()
var _error := ""
var _edge_cache: Dictionary = {}
var _mesh_dependencies: Dictionary = {}

func sample_chunk(field: Resource, coord: Vector2i) -> Dictionary:
	_field = field
	_smooth_distance = field.smooth_distance()
	_coord = coord
	var n: int = field.CHUNK_CELLS
	var spacing: float = field.SPACING
	_samples.resize((n + 1) * (n + 1))
	_distances.resize(_samples.size())
	var sampler: RefCounted = field.new_sampler()
	var queries: Array[PackedInt32Array] = []
	for z in 32:
		for x in 32:
			var center = Vector2(coord) * field.CHUNK_METERS + Vector2(x + 0.5, z + 0.5)
			queries.append(sampler.candidates(center, sqrt(0.5), field.road_candidates(center)))
	var max_gradient = 0.0
	for z in range(n + 1):
		for x in range(n + 1):
			var point = Vector2(coord * n + Vector2i(x, z)) * spacing
			var value: Vector2 = sampler.sample(point, queries[mini(z / 10, 31) * 32 + mini(x / 10, 31)])
			var index = z * (n + 1) + x
			_samples[index] = value.x
			_distances[index] = value.y
			if not is_finite(value.x):
				return {"error": "Non-finite terrain height."}
			if x > 0 and z > 0:
				max_gradient = maxf(max_gradient, Vector2(value.x - _samples[index - 1], value.x - _samples[index - n - 1]).length() / spacing)
	_owners.resize(n * n)
	_build_pyramid()
	_partition(Vector2i.ZERO, 64, _pyramid.size() - 1)
	if not _error.is_empty():
		return {"error": _error}
	return {"leaves": _leaves, "owners": _owners, "max_error": _max_error, "source_max_gradient": max_gradient, "samples": _samples, "distances": _distances}

func _build_pyramid() -> void:
	var size = 64
	var ranges = PackedVector3Array()
	ranges.resize(size * size)
	for z in size:
		for x in size:
			ranges[z * size + x] = _range(Rect2i(x * BLOCK_CELLS, z * BLOCK_CELLS, BLOCK_CELLS, BLOCK_CELLS))
	_pyramid.append({"size": size, "ranges": ranges})
	while size > 1:
		var next_size = size / 2
		var next = PackedVector3Array()
		next.resize(next_size * next_size)
		for z in next_size:
			for x in next_size:
				var value = Vector3(INF, -INF, 0)
				for offset: Vector2i in [Vector2i.ZERO, Vector2i.RIGHT, Vector2i.DOWN, Vector2i.ONE]:
					var child = ranges[(z * 2 + offset.y) * size + x * 2 + offset.x]
					value.x = minf(value.x, child.x)
					value.y = maxf(value.y, child.y)
					value.z = maxf(value.z, child.z)
				next[z * next_size + x] = value
		size = next_size
		ranges = next
		_pyramid.append({"size": size, "ranges": ranges})

func _range(rect: Rect2i) -> Vector3:
	var result = Vector3(INF, -INF, 0)
	var stride: int = _field.CHUNK_CELLS + 1
	for z in range(rect.position.y, rect.end.y + 1):
		for x in range(rect.position.x, rect.end.x + 1):
			var index = z * stride + x
			result.x = minf(result.x, _samples[index])
			result.y = maxf(result.y, _samples[index])
			if _distances[index] <= _smooth_distance:
				result.z = 1
	return result

func _partition(block: Vector2i, width: int, level: int) -> void:
	var data = _pyramid[level]
	var range_value: Vector3 = data.ranges[(block.y / width) * data.size + block.x / width]
	var rect = Rect2i(block * BLOCK_CELLS, Vector2i.ONE * width * BLOCK_CELLS)
	if _try_leaf(rect, range_value):
		return
	if level == 0:
		_partition_small(rect)
		return
	var half = width / 2
	for offset: Vector2i in [Vector2i.ZERO, Vector2i.RIGHT, Vector2i.DOWN, Vector2i.ONE]:
		_partition(block + offset * half, half, level - 1)

func _partition_small(rect: Rect2i) -> void:
	if _try_leaf(rect, _range(rect)):
		return
	if rect.size == Vector2i.ONE:
		_error = "Terrain discontinuity exceeds one voxel at %s: %s" % [_coord * _field.CHUNK_CELLS + rect.position, _corners(rect)]
		return
	var left = maxi(1, rect.size.x / 2)
	var top = maxi(1, rect.size.y / 2)
	var widths = [left, rect.size.x - left] if rect.size.x > 1 else [1]
	var heights = [top, rect.size.y - top] if rect.size.y > 1 else [1]
	var z = rect.position.y
	for h: int in heights:
		var x = rect.position.x
		for w: int in widths:
			_partition_small(Rect2i(x, z, w, h))
			x += w
		z += h

func _try_leaf(rect: Rect2i, range_value: Vector3) -> bool:
	var heights = _corners(rect)
	var error = 0.0
	if range_value.z != 0 and maxi(rect.size.x, rect.size.y) > 20:
		return false
	# A fixed pair of planar slopes replaces terraces. Only authored elevations
	# outside the driving corridor snap to the shared 10 cm lattice.
	var points = [rect.position, Vector2i(rect.end.x, rect.position.y), Vector2i(rect.position.x, rect.end.y), rect.end]
	for i in 4:
		var point: Vector2i = points[i]
		if _distances[point.y * (_field.CHUNK_CELLS + 1) + point.x] > _smooth_distance:
			heights[i] = snappedf(heights[i], _field.SPACING)
	error = _plane_error(rect, heights)
	if error > SURFACE_ERROR and rect.size != Vector2i.ONE:
		return false
	if range_value.z != 0 and error > SMOOTH_ERROR and rect.size != Vector2i.ONE:
		return false
	var global_rect = Rect2i(_coord * _field.CHUNK_CELLS + rect.position, rect.size)
	var leaf = {"rect": global_rect, "heights": heights}
	var index = _leaves.size()
	_leaves.append(leaf)
	var n: int = _field.CHUNK_CELLS
	for z in range(rect.position.y, rect.end.y):
		for x in range(rect.position.x, rect.end.x):
			_owners[z * n + x] = index
	_max_error = maxf(_max_error, error)
	return true

func _corners(rect: Rect2i) -> Vector4:
	var n: int = _field.CHUNK_CELLS + 1
	return Vector4(_samples[rect.position.y * n + rect.position.x], _samples[rect.position.y * n + rect.end.x], _samples[rect.end.y * n + rect.position.x], _samples[rect.end.y * n + rect.end.x])

func _plane_error(rect: Rect2i, heights: Vector4) -> float:
	var error = 0.0
	var stride: int = _field.CHUNK_CELLS + 1
	for z in range(rect.position.y, rect.end.y + 1):
		for x in range(rect.position.x, rect.end.x + 1):
			var t = Vector2(x - rect.position.x, z - rect.position.y) / Vector2(rect.size)
			var value = heights.x + t.x * (heights.y - heights.x) + t.y * (heights.z - heights.x) if t.x + t.y <= 1 else heights.w + (1 - t.x) * (heights.z - heights.w) + (1 - t.y) * (heights.y - heights.w)
			error = maxf(error, absf(value - _samples[z * stride + x]))
	return error

func mesh_chunk(field: Resource, chunk: Dictionary) -> Dictionary:
	_field = field
	_smooth_distance = field.smooth_distance()
	_coord = chunk.coord
	_samples = chunk.samples
	_distances = chunk.distances
	var bad = PackedInt32Array()
	for i in chunk.leaves.size():
		var leaf: Dictionary = chunk.leaves[i]
		var start = _vertices.size()
		_emit_top(leaf)
		if not _validate_top(leaf, start):
			bad.append(i)
	var arrays: Array = []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = _vertices
	arrays[Mesh.ARRAY_NORMAL] = _normals
	return {"arrays": arrays, "refine": bad, "max_error": _max_error, "mesh_error": _error, "mesh_dependencies": _mesh_dependencies}

func refine_chunk(field: Resource, chunk: Dictionary) -> void:
	if field.native_authoring != null:
		chunk.merge(field.native_authoring.refine_chunk(chunk.coord, chunk.samples, chunk.distances, chunk.leaves, chunk.needs_refine), true)
		return
	_field = field
	_smooth_distance = field.smooth_distance()
	_coord = chunk.coord
	_samples = chunk.samples
	_distances = chunk.distances
	_owners.resize(field.CHUNK_CELLS * field.CHUNK_CELLS)
	for i in chunk.leaves.size():
		var leaf: Dictionary = chunk.leaves[i]
		var rect: Rect2i = leaf.rect
		var local = Rect2i(rect.position - chunk.coord * field.CHUNK_CELLS, rect.size)
		if chunk.needs_refine.has(i) and rect.size != Vector2i.ONE:
			var widths = [maxi(1, rect.size.x / 2), rect.size.x - maxi(1, rect.size.x / 2)] if rect.size.x > 1 else [1]
			var heights = [maxi(1, rect.size.y / 2), rect.size.y - maxi(1, rect.size.y / 2)] if rect.size.y > 1 else [1]
			var z: int = local.position.y
			for h: int in heights:
				var x: int = local.position.x
				for w: int in widths:
					_partition_small(Rect2i(x, z, w, h))
					x += w
				z += h
		else:
			_try_leaf(local, _range(local))
	chunk["leaves"] = _leaves
	chunk["owners"] = _owners

func _emit_top(leaf: Dictionary) -> void:
	var rect: Rect2i = leaf.rect
	var ring = _boundary_points(rect)
	var points = PackedVector3Array()
	for cell: Vector2i in ring:
		var p = Vector2(cell) * _field.SPACING
		points.append(Vector3(p.x, _edge_height(cell), p.y))
	var triangles: Array[PackedVector3Array] = []
	if points.size() == 4:
		triangles = [PackedVector3Array([points[0], points[1], points[3]]), PackedVector3Array([points[1], points[2], points[3]])]
	else:
		var center = rect.position + rect.size / 2
		var p = Vector2(center) * _field.SPACING
		var middle = Vector3(p.x, _source_height(center), p.y)
		for i in points.size():
			triangles.append(PackedVector3Array([middle, points[i], points[(i + 1) % points.size()]]))
	var roads: Array = _field.road_polygons_for_bounds(Vector2(rect.position) * _field.SPACING, Vector2(rect.end) * _field.SPACING)
	for triangle in triangles:
		if absf((triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]).y) < 0.00000001:
			continue
		var polygons: Array[PackedVector2Array] = [PackedVector2Array([Vector2(triangle[0].x, triangle[0].z), Vector2(triangle[1].x, triangle[1].z), Vector2(triangle[2].x, triangle[2].z)])]
		for road: PackedVector2Array in roads:
			var next: Array[PackedVector2Array] = []
			for polygon in polygons:
				next.append_array(Geometry2D.clip_polygons(polygon, road))
			polygons = next
			if polygons.is_empty():
				break
		for polygon in polygons:
			if not roads.is_empty():
				polygon = _refine_border(polygon)
			var parts: Array[PackedVector2Array] = []
			if roads.is_empty():
				parts.append(polygon)
			else:
				parts = _polygon_parts(polygon)
			for part in parts:
				polygon = part
				polygon = _clean_polygon(polygon)
				if polygon.size() < 3:
					continue
				# Triangulate near zero: at kilometre-scale world coordinates, the
				# native float-area calculation can discard whole 10 cm patches.
				var local_polygon := PackedVector2Array()
				for vertex in polygon:
					local_polygon.append(vertex - polygon[0])
				var indices := Geometry2D.triangulate_polygon(local_polygon)
				if indices.is_empty():
					# The ear-clipping epsilon is fixed in native float coordinates.
					# Magnify thin clipped polygons without changing saved positions.
					var scaled_polygon := PackedVector2Array()
					for vertex in local_polygon:
						scaled_polygon.append(vertex * 1000.0)
					indices = Geometry2D.triangulate_polygon(scaled_polygon)
				if indices.is_empty() and absf(_polygon_area(local_polygon)) > 0.00001:
					_error = "Could not triangulate terrain polygon at %s: %s (area %s)." % [_coord, local_polygon, _polygon_area(local_polygon)]
					return
				for i in range(0, indices.size(), 3):
					var vertices = PackedVector3Array()
					for j in 3:
						var p = polygon[indices[i + j]]
						var y: float = _field.border_height(p) if not roads.is_empty() else NAN
						if not is_finite(y):
							y = _plane_height(p, triangle[0], triangle[1], triangle[2])
						vertices.append(Vector3(p.x, y, p.y))
					_triangle(vertices[0], vertices[1], vertices[2], Vector3.UP)
# Track every external leaf read, including recursive edge-height reads. A
# refinement only remeshes chunks which actually depend on a changed leaf.
func _leaf_at(cell: Vector2i) -> Dictionary:
	var leaf: Dictionary = _field.leaf_at(cell)
	var origin: Vector2i = _coord * _field.CHUNK_CELLS
	if not leaf.is_empty() and (cell.x < origin.x or cell.y < origin.y or cell.x >= origin.x + _field.CHUNK_CELLS or cell.y >= origin.y + _field.CHUNK_CELLS):
		_mesh_dependencies[leaf.rect] = true
	return leaf

# Clipper can return regions touching at a repeated vertex. Split their
# closed loops before ear clipping; zero-area backtracks disappear naturally.
static func _polygon_parts(polygon: PackedVector2Array) -> Array[PackedVector2Array]:
	var parts: Array[PackedVector2Array] = []
	if polygon.size() < 3:
		return parts
	var path := PackedVector2Array()
	var positions := {}
	var closed := polygon.duplicate()
	closed.append(polygon[0])
	for point in closed:
		if positions.has(point):
			var first: int = positions[point]
			var loop := _clean_polygon(path.slice(first))
			if loop.size() >= 3:
				parts.append(loop)
			for i in range(first + 1, path.size()):
				positions.erase(path[i])
			path.resize(first + 1)
		else:
			positions[point] = path.size()
			path.append(point)
	return parts

# Clipping adjacent road quads can leave A-B-A spikes and a duplicated
# closing point. Merge points within the 0.5 mm border tolerance (or one
# float step at large coordinates). They have no meaningful area and make a valid region weakly simple.
# Keep ordinary collinear border vertices: their authored elevations matter.
static func _clean_polygon(polygon: PackedVector2Array) -> PackedVector2Array:
	var result := polygon.duplicate()
	var tolerance := 0.0005
	for point in polygon:
		tolerance = maxf(tolerance, maxf(absf(point.x), absf(point.y)) * 0.00000012)
	var tolerance_squared := tolerance * tolerance
	var changed := true
	while changed and result.size() >= 3:
		changed = false
		for i in result.size():
			var previous := (i + result.size() - 1) % result.size()
			var next := (i + 1) % result.size()
			if result[i].distance_squared_to(result[next]) < tolerance_squared or result[previous].distance_squared_to(result[next]) < tolerance_squared:
				result.remove_at(i)
				changed = true
				break
	return result

static func _polygon_area(polygon: PackedVector2Array) -> float:
	var area := 0.0
	for i in polygon.size():
		var a := polygon[i]
		var b := polygon[(i + 1) % polygon.size()]
		area += a.x * b.y - b.x * a.y
	return area * 0.5

func _boundary_points(rect: Rect2i) -> Array[Vector2i]:
	var result: Array[Vector2i] = []
	for side in 4:
		var horizontal = side % 2 == 0
		var start = rect.position.x if horizontal else rect.position.y
		var end = rect.end.x if horizontal else rect.end.y
		var plane = rect.position.y if side == 0 else rect.end.x if side == 1 else rect.end.y if side == 2 else rect.position.x
		var values: Array[int] = [start, end]
		var cursor = start
		while cursor < end:
			var cell = Vector2i(cursor, plane - 1 if side == 0 else plane) if horizontal else Vector2i(plane - 1 if side == 3 else plane, cursor)
			var neighbor: Dictionary = _leaf_at(cell)
			if neighbor.is_empty():
				break
			var next = mini(end, neighbor.rect.end.x if horizontal else neighbor.rect.end.y)
			if next <= cursor:
				break
			values.append(next)
			cursor = next
		values.sort()
		if side > 1:
			values.reverse()
		var previous = -2147483648
		for value in values:
			if value == previous or value == values[-1]:
				continue
			result.append(Vector2i(value, plane) if horizontal else Vector2i(plane, value))
			previous = value
	return result

# At T junctions use the coarser edge's actual linear profile. Both neighbours
# resolve the same recursive endpoints, so there are no skirts or hidden steps.
func _edge_height(cell: Vector2i) -> float:
	if not _edge_cache.has(cell):
		_edge_cache[cell] = _edge_height_uncached(cell)
	return _edge_cache[cell]

func _edge_height_uncached(cell: Vector2i) -> float:
	var best: Dictionary = {}
	var area = 0
	for offset: Vector2i in [Vector2i.ZERO, Vector2i.LEFT, Vector2i.UP, -Vector2i.ONE]:
		var leaf: Dictionary = _leaf_at(cell + offset)
		if leaf.is_empty():
			continue
		var rect: Rect2i = leaf.rect
		if rect.size.x * rect.size.y > area:
			area = rect.size.x * rect.size.y
			best = leaf
	if best.is_empty():
		return _source_height(cell)
	var rect: Rect2i = best.rect
	if cell.x > rect.position.x and cell.x < rect.end.x and (cell.y == rect.position.y or cell.y == rect.end.y):
		var a = Vector2i(rect.position.x, cell.y)
		var b = Vector2i(rect.end.x, cell.y)
		return lerpf(_edge_height(a), _edge_height(b), float(cell.x - a.x) / (b.x - a.x))
	if cell.y > rect.position.y and cell.y < rect.end.y and (cell.x == rect.position.x or cell.x == rect.end.x):
		var a = Vector2i(cell.x, rect.position.y)
		var b = Vector2i(cell.x, rect.end.y)
		return lerpf(_edge_height(a), _edge_height(b), float(cell.y - a.y) / (b.y - a.y))
	return _source_height(cell)

func _source_height(cell: Vector2i) -> float:
	var value: Vector2 = _field.source_sample(cell)
	return value.x if value.y <= _smooth_distance else snappedf(value.x, _field.SPACING)

func _validate_top(leaf: Dictionary, start: int) -> bool:
	if _field.native_authoring != null:
		var result: Dictionary = _field.native_authoring.validate_top(leaf.rect, _vertices, start, _samples, _distances, _coord, _smooth_distance)
		_max_error = maxf(_max_error, result.max_error)
		return result.good
	var good = true
	var stride: int = _field.CHUNK_CELLS + 1
	var spacing: float = _field.SPACING
	var origin: Vector2i = _coord * _field.CHUNK_CELLS
	for i in range(start, _vertices.size(), 3):
		var a = _vertices[i]
		var b = _vertices[i + 1]
		var c = _vertices[i + 2]
		var ab = Vector2(b.x - a.x, b.z - a.z)
		var ac = Vector2(c.x - a.x, c.z - a.z)
		var inverse = 1.0 / ab.cross(ac)
		var first = Vector2i(ceili(minf(a.x, minf(b.x, c.x)) / spacing - 0.001), ceili(minf(a.z, minf(b.z, c.z)) / spacing - 0.001)).max(leaf.rect.position)
		var last = Vector2i(floori(maxf(a.x, maxf(b.x, c.x)) / spacing + 0.001), floori(maxf(a.z, maxf(b.z, c.z)) / spacing + 0.001)).min(leaf.rect.end)
		for z in range(first.y, last.y + 1):
			for x in range(first.x, last.x + 1):
				var ap = Vector2(x * spacing - a.x, z * spacing - a.z)
				var u = ap.cross(ac) * inverse
				var v = ab.cross(ap) * inverse
				if u < -0.0001 or v < -0.0001 or u + v > 1.0001:
					continue
				var y = a.y + u * (b.y - a.y) + v * (c.y - a.y)
				var index = (z - origin.y) * stride + x - origin.x
				var error = absf(y - _samples[index])
				_max_error = maxf(_max_error, error)
				var bound: float = SMOOTH_ERROR if _distances[index] <= _smooth_distance else SURFACE_ERROR
				if error > bound + 0.0001:
					good = false
	return good

func _refine_border(polygon: PackedVector2Array) -> PackedVector2Array:
	var result = PackedVector2Array()
	for i in polygon.size():
		var a = polygon[i]
		var b = polygon[(i + 1) % polygon.size()]
		var edge = b - a
		var length_squared = edge.length_squared()
		if length_squared < 0.00000001:
			continue
		var values: Array[float] = [0.0]
		for segment: Array in _field.border_segments(a, b):
			for vertex: Vector3 in segment:
				var p = Vector2(vertex.x, vertex.z)
				var t = (p - a).dot(edge) / length_squared
				if t > 0.00001 and t < 0.99999 and (a + edge * t).distance_squared_to(p) < 0.00000025:
					values.append(t)
		values.sort()
		var previous = -1.0
		for t in values:
			if t - previous > 0.00001:
				result.append(a + edge * t)
				previous = t
	return result

func _triangle(a: Vector3, b: Vector3, c: Vector3, expected: Vector3) -> void:
	if not a.is_finite() or not b.is_finite() or not c.is_finite():
		_error = "Non-finite terrain triangle at %s." % _coord
		return
	var cross = (b - a).cross(c - a)
	if cross.length_squared() < 0.0000000001:
		return
	if cross.dot(expected) > 0:
		var swap = b
		b = c
		c = swap
	var normal = -(b - a).cross(c - a).normalized()
	_vertices.append_array(PackedVector3Array([a, b, c]))
	_normals.append_array(PackedVector3Array([normal, normal, normal]))

static func _plane_height(point: Vector2, a: Vector3, b: Vector3, c: Vector3) -> float:
	var ab = Vector2(b.x - a.x, b.z - a.z)
	var ac = Vector2(c.x - a.x, c.z - a.z)
	var ap = point - Vector2(a.x, a.z)
	var determinant = ab.cross(ac)
	return a.y + ap.cross(ac) / determinant * (b.y - a.y) + ab.cross(ap) / determinant * (c.y - a.y)

static func leaf_height(leaf: Dictionary, cell: Vector2) -> float:
	var rect: Rect2i = leaf.rect
	var h: Vector4 = leaf.heights
	var t = (cell - Vector2(rect.position)) / Vector2(rect.size)
	return h.x + t.x * (h.y - h.x) + t.y * (h.z - h.x) if t.x + t.y <= 1 else h.w + (1 - t.x) * (h.z - h.w) + (1 - t.y) * (h.y - h.w)
