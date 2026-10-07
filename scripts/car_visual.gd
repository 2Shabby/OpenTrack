extends Node3D

func bind_wheels() -> Dictionary:
	_fix_materials($Model)
	transform = Transform3D(Basis(Vector3.UP, PI), Vector3.ZERO)
	var front_left := _bake($Model.get_node("SportsCar_FrontLeftWheel"), 0)
	var front_right := _bake($Model.get_node("SportsCar_FrontRightWheel"), 0)
	if front_left["center"].x > front_right["center"].x:
		var swap := front_left
		front_left = front_right
		front_right = swap
	var rear_source: MeshInstance3D = $Model.get_node("SportsCar_BackWheels")
	var rear_left := _bake(rear_source, -1)
	var rear_right := _bake(rear_source, 1)
	rear_source.visible = false
	rear_source.queue_free()
	return {
		"front_left": front_left,
		"front_right": front_right,
		"rear_left": rear_left,
		"rear_right": rear_right,
	}

# Local transforms also work before scene-tree entry, when the spawn is prepared.
func to_car_space(node: Node3D) -> Transform3D:
	var result := node.transform
	var ancestor := node.get_parent() as Node3D
	while ancestor != get_parent():
		result = ancestor.transform * result
		ancestor = ancestor.get_parent() as Node3D
	return result

func _bake(source: MeshInstance3D, side: int) -> Dictionary:
	var to_body := to_car_space(source)
	var normal_basis := to_body.basis.inverse().transposed()
	var mesh := source.mesh as ArrayMesh
	var kept: Array[Dictionary] = []
	var minimum := Vector3(INF, INF, INF)
	var maximum := Vector3(-INF, -INF, -INF)
	for surface in mesh.get_surface_count():
		var arrays := mesh.surface_get_arrays(surface)
		var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
		var normals: PackedVector3Array = arrays[Mesh.ARRAY_NORMAL] if arrays[Mesh.ARRAY_NORMAL] != null else PackedVector3Array()
		var uvs: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV] if arrays[Mesh.ARRAY_TEX_UV] != null else PackedVector2Array()
		var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX] if arrays[Mesh.ARRAY_INDEX] != null else PackedInt32Array()
		if indices.is_empty():
			for index in vertices.size():
				indices.append(index)
		for index in range(0, indices.size(), 3):
			var points: Array[Vector3] = []
			var centroid := Vector3.ZERO
			for corner in 3:
				var point: Vector3 = to_body * vertices[indices[index + corner]]
				points.append(point)
				centroid += point
			centroid /= 3.0
			if side != 0 and signf(centroid.x) != float(side):
				continue
			var record := {"points": points, "surface": surface}
			if not normals.is_empty():
				var face_normals: Array[Vector3] = []
				for corner in 3:
					face_normals.append((normal_basis * normals[indices[index + corner]]).normalized())
				record["normals"] = face_normals
			if not uvs.is_empty():
				var face_uvs: Array[Vector2] = []
				for corner in 3:
					face_uvs.append(uvs[indices[index + corner]])
				record["uvs"] = face_uvs
			kept.append(record)
			for point in points:
				minimum = minimum.min(point)
				maximum = maximum.max(point)
	var center := (minimum + maximum) * 0.5
	var size := maximum - minimum
	var baked := ArrayMesh.new()
	var by_surface: Dictionary = {}
	for record in kept:
		var surface: int = record["surface"]
		if not by_surface.has(surface):
			by_surface[surface] = {"vertices": PackedVector3Array(), "normals": PackedVector3Array(), "uvs": PackedVector2Array(), "indices": PackedInt32Array()}
		var bucket: Dictionary = by_surface[surface]
		var vertices: PackedVector3Array = bucket["vertices"]
		var base := vertices.size()
		var points: Array = record["points"]
		for corner in 3:
			vertices.append(points[corner] - center)
			bucket["indices"].append(base + corner)
		bucket["vertices"] = vertices
		if record.has("normals"):
			var normals: PackedVector3Array = bucket["normals"]
			for normal in record["normals"]:
				normals.append(normal)
			bucket["normals"] = normals
		if record.has("uvs"):
			var uvs: PackedVector2Array = bucket["uvs"]
			for uv in record["uvs"]:
				uvs.append(uv)
			bucket["uvs"] = uvs
	for surface in by_surface:
		var bucket: Dictionary = by_surface[surface]
		var arrays: Array = []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = bucket["vertices"]
		if not (bucket["normals"] as PackedVector3Array).is_empty():
			arrays[Mesh.ARRAY_NORMAL] = bucket["normals"]
		if not (bucket["uvs"] as PackedVector2Array).is_empty():
			arrays[Mesh.ARRAY_TEX_UV] = bucket["uvs"]
		arrays[Mesh.ARRAY_INDEX] = bucket["indices"]
		baked.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		baked.surface_set_material(baked.get_surface_count() - 1, source.get_active_material(surface))
	if side == 0:
		source.visible = false
		source.queue_free()
	return {"mesh": baked, "center": center, "radius": maxf(size.y, size.z) * 0.5, "width": size.x}

func _fix_materials(node: Node) -> void:
	if node is MeshInstance3D:
		var mesh_instance := node as MeshInstance3D
		var mesh := mesh_instance.mesh
		if mesh:
			for surface in mesh.get_surface_count():
				var material := mesh_instance.get_active_material(surface)
				if material is StandardMaterial3D:
					var copy := material.duplicate() as StandardMaterial3D
					copy.transparency = BaseMaterial3D.TRANSPARENCY_DISABLED
					copy.albedo_color.a = 1.0
					mesh_instance.set_surface_override_material(surface, copy)
	for child in node.get_children():
		_fix_materials(child)
