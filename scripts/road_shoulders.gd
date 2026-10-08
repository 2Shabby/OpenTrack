extends RefCounted

const WIDTH := 2.0
const Field := preload("res://scripts/terrain_field.gd")

# Four strips form a closed border. Their inner vertices lie on the road;
# their outer edges follow the exact terrain triangles, including diagonals.
func build(stage: Resource) -> Array[ArrayMesh]:
	var meshes: Array[ArrayMesh] = []
	var outer_paths: Array[PackedVector3Array] = []
	for side in 2:
		var inner: PackedVector3Array = stage.left_edges if side == 0 else stage.right_edges
		var outer := PackedVector3Array()
		var normals := PackedVector3Array()
		for i in inner.size():
			var point: Vector3 = inner[i] + (inner[i] - stage.centers[i]).normalized() * WIDTH
			if i == 0:
				point -= _forward(stage, 0) * WIDTH
			elif i == inner.size() - 1:
				point += _forward(stage, i - 1) * WIDTH
			outer.append(point)
			normals.append(stage.road_normals[i * 2 + side])
		outer_paths.append(outer)
		meshes.append(_strip(stage.terrain, inner, outer, normals))
	for i: int in [0, stage.centers.size() - 1]:
		var inner := PackedVector3Array([stage.left_edges[i], stage.right_edges[i]])
		var outer := PackedVector3Array([outer_paths[0][i], outer_paths[1][i]])
		var normals := PackedVector3Array([stage.road_normals[i * 2], stage.road_normals[i * 2 + 1]])
		meshes.append(_strip(stage.terrain, inner, outer, normals))
	return meshes

func _forward(stage: Resource, i: int) -> Vector3:
	return (stage.centers[i + 1] - stage.centers[i]).slide(Vector3.UP).normalized()

func _strip(field: Resource, inner: PackedVector3Array, outer: PackedVector3Array, inner_normals: PackedVector3Array) -> ArrayMesh:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var indices := PackedInt32Array()
	for i in range(inner.size() - 1):
		var breaks := _terrain_breaks(field, outer[i], outer[i + 1])
		for j in breaks.size():
			if i > 0 and j == 0:
				continue
			var t := breaks[j]
			var point := outer[i] if t == 0.0 else outer[i + 1] if t == 1.0 else outer[i].lerp(outer[i + 1], t)
			point.y = field.height_at(Vector2(point.x, point.z))
			# Preserve shared cross-sections exactly: lerp at 1 can round differently.
			vertices.append(inner[i] if t == 0.0 else inner[i + 1] if t == 1.0 else inner[i].lerp(inner[i + 1], t))
			vertices.append(point)
			normals.append(inner_normals[i].lerp(inner_normals[i + 1], t).normalized())
			normals.append(field.normal_at(Vector2(point.x, point.z)))
			if vertices.size() > 2:
				var a := vertices.size() - 4
				for triangle: Vector3i in [Vector3i(a, a + 1, a + 2), Vector3i(a + 1, a + 3, a + 2)]:
					var cross := (vertices[triangle.y] - vertices[triangle.x]).cross(vertices[triangle.z] - vertices[triangle.x])
					# A grid crossing can round onto a station on one strip edge.
					if cross.y == 0:
						continue
					if cross.y > 0:
						indices.append_array(PackedInt32Array([triangle.x, triangle.z, triangle.y]))
					else:
						indices.append_array(PackedInt32Array([triangle.x, triangle.y, triangle.z]))
	var arrays: Array = []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_NORMAL] = normals
	arrays[Mesh.ARRAY_INDEX] = indices
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	return mesh

func _terrain_breaks(field: Resource, a: Vector3, b: Vector3) -> PackedFloat64Array:
	var first: Vector2 = (Vector2(a.x, a.z) - field.origin) / Field.SPACING
	var last: Vector2 = (Vector2(b.x, b.z) - field.origin) / Field.SPACING
	var values: Array[float] = [0.0, 1.0]
	# Terrain cell edges and its x+z diagonal partition this line into planes.
	for axis: Vector2 in [Vector2.RIGHT, Vector2.DOWN, Vector2.ONE]:
		var start := first.dot(axis)
		var finish := last.dot(axis)
		if is_equal_approx(start, finish):
			continue
		for boundary in range(floori(minf(start, finish)) + 1, ceili(maxf(start, finish))):
			values.append((boundary - start) / (finish - start))
	values.sort()
	var result := PackedFloat64Array()
	for value in values:
		if result.is_empty() or value - result[-1] > 0.000001:
			result.append(value)
	result[-1] = 1.0
	return result
