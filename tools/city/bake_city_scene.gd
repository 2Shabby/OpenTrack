extends SceneTree

const ASPHALT := preload("res://resources/surfaces/asphalt.tres")

func _initialize() -> void:
	_bake.call_deferred()

func _bake() -> void:
	var input := ""
	var output := ""
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--input="): input = arg.trim_prefix("--input=")
		if arg.begins_with("--output="): output = arg.trim_prefix("--output=")
	var parsed: Variant = JSON.parse_string(FileAccess.get_file_as_string(input + "/track.json"))
	if not parsed is Dictionary or parsed.get("format") != "city-track-v1" or output.is_empty():
		_fail("Invalid city bake manifest or output")
		return
	var data: Dictionary = parsed
	var binary := FileAccess.open(input + "/track.bin", FileAccess.READ)
	if binary == null:
		_fail("Missing city mesh data")
		return
	var city := Node3D.new()
	city.name = "Bengaluru"
	city.set_meta("city_track", data.stats)
	city.set_meta("source_sha256", data.source_sha256)
	city.set_meta("lane_policy", data.policy)
	root.add_child(city)
	var materials := {"asphalt": ASPHALT.material}
	for kind in ["median", "paint", "bridge", "structure"]:
		var material := StandardMaterial3D.new()
		material.albedo_color = Color("c5b581") if kind == "median" else (Color("dedbd1") if kind == "paint" else Color("55585b"))
		material.roughness = 0.94
		materials[kind] = material
	var expected_vertices := 0
	for item: Dictionary in data.meshes:
		binary.seek(int(item.offset))
		var values := binary.get_buffer(int(item.bytes)).to_float32_array()
		if values.size() != int(item.triangles) * 9:
			_fail("Incomplete city mesh")
			return
		var vertices := PackedVector3Array()
		var normals := PackedVector3Array()
		var uvs := PackedVector2Array()
		for i in range(0, values.size(), 9):
			var a := Vector3(values[i], values[i + 1], values[i + 2])
			var b := Vector3(values[i + 3], values[i + 4], values[i + 5])
			var c := Vector3(values[i + 6], values[i + 7], values[i + 8])
			var normal := (b - a).cross(c - a).normalized()
			if normal.length_squared() < .9:
				_fail("Degenerate city triangle")
				return
			# Geometry arrives counterclockwise; Godot front faces are clockwise.
			for point in [a, c, b]:
				vertices.append(point)
				normals.append(normal)
				uvs.append(Vector2(.25, 0))
		var arrays: Array = []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		arrays[Mesh.ARRAY_NORMAL] = normals
		arrays[Mesh.ARRAY_TEX_UV] = uvs
		var mesh := ArrayMesh.new()
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		mesh.surface_set_material(0, materials[item.kind])
		var node: Node3D
		if item.collision:
			var body := StaticBody3D.new()
			body.collision_layer = TrackGeometry.ROAD_LAYER
			body.collision_mask = 0
			body.add_to_group("Road", true)
			body.set_meta("surface", ASPHALT)
			var collider := CollisionShape3D.new()
			var shape := ConcavePolygonShape3D.new()
			shape.backface_collision = true
			shape.set_faces(vertices)
			collider.shape = shape
			body.add_child(collider)
			node = body
			expected_vertices += vertices.size()
		else:
			node = Node3D.new()
		node.name = str(item.kind) + "_" + str(city.get_child_count())
		node.position = Vector3(item.origin[0], item.origin[1], item.origin[2])
		node.add_to_group("City_" + str(item.kind), true)
		var visual := MeshInstance3D.new()
		visual.mesh = mesh
		node.add_child(visual)
		city.add_child(node)
	var spawn := Marker3D.new()
	spawn.name = "Spawn"
	spawn.position = Vector3(data.spawn.position[0], data.spawn.position[1], data.spawn.position[2])
	var forward := Vector3(data.spawn.forward[0], data.spawn.forward[1], data.spawn.forward[2])
	spawn.basis = Basis(Vector3.UP.cross(forward), Vector3.UP, forward)
	city.add_child(spawn)
	if data.get("bridge_spawn") is Dictionary:
		var bridge_spawn := Marker3D.new()
		bridge_spawn.name = "BridgeSpawn"
		var spec: Dictionary = data.bridge_spawn
		bridge_spawn.position = Vector3(spec.position[0], spec.position[1], spec.position[2])
		var direction := Vector3(spec.forward[0], spec.forward[1], spec.forward[2])
		bridge_spawn.basis = Basis(Vector3.UP.cross(direction), Vector3.UP, direction)
		city.add_child(bridge_spawn)
	await physics_frame
	await physics_frame
	var space := city.get_world_3d().direct_space_state
	var seam_probes := 0
	for p: Array in data.probes:
		var point := Vector3(p[0], p[1], p[2])
		var hit := _support(space, point)
		# A graph vertex can lie exactly on a quantized triangle edge. Verify
		# the adjacent road within 2 mm instead of requiring an edge hit.
		if hit.is_empty():
			for offset in [Vector3(.002, 0, 0), Vector3(-.002, 0, 0), Vector3(0, 0, .002), Vector3(0, 0, -.002), Vector3(.002, 0, .002), Vector3(-.002, 0, -.002)]:
				hit = _support(space, point + offset)
				if not hit.is_empty():
					seam_probes += 1
					break
		if hit.is_empty() or hit.position.distance_to(point) > .08:
			_fail("Missing native road support at " + str(point) + "; hit " + str(hit))
			return
	_own(city, city)
	city.set_meta("native_collision_vertices", expected_vertices)
	city.set_meta("support_probes", data.probes.size())
	city.set_meta("triangle_edge_probes", seam_probes)
	var packed := PackedScene.new()
	if packed.pack(city) != OK:
		_fail("Cannot pack city scene")
		return
	var temporary := output.trim_suffix(".scn") + ".pending.scn"
	if ResourceSaver.save(packed, temporary, ResourceSaver.FLAG_COMPRESS) != OK or DirAccess.rename_absolute(temporary, output) != OK:
		_fail("Cannot save city scene")
		return
	print("CITY_BAKED triangles ", data.stats.triangles, " · chunks ", data.stats.chunks, " · support probes ", data.probes.size(), " · triangle-edge probes ", seam_probes)
	city.queue_free()
	await process_frame
	quit()

func _own(node: Node, owner_root: Node) -> void:
	for child in node.get_children():
		child.owner = owner_root
		_own(child, owner_root)

func _support(space: PhysicsDirectSpaceState3D, point: Vector3) -> Dictionary:
	var query := PhysicsRayQueryParameters3D.create(point + Vector3.UP * .04, point - Vector3.UP * .08, TrackGeometry.ROAD_LAYER)
	return space.intersect_ray(query)

func _fail(message: String) -> void:
	push_error(message)
	quit(1)
