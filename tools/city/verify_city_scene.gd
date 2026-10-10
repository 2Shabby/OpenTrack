extends SceneTree

func _initialize() -> void:
	_run.call_deferred()

func _frames(count: int) -> void:
	for _i in count:
		await physics_frame

func _capture(label: String) -> void:
	if DisplayServer.get_name() == "headless": return
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("res://.stage_authoring/bengaluru/review/godot_" + label + ".png")

func _run() -> void:
	var preview = load("res://scenes/city_preview.tscn").instantiate()
	root.add_child(preview)
	var started := Time.get_ticks_msec()
	while not preview.ready_to_drive:
		if Time.get_ticks_msec() - started > 15000:
			_fail("City preview did not load: " + preview.label.text)
			return
		await process_frame
	var city: Node3D = preview.city
	var vertices := 0
	var bodies := city.find_children("*", "StaticBody3D", true, false)
	for body in bodies:
		if not body.is_in_group("Road") or body.collision_layer != TrackGeometry.ROAD_LAYER:
			_fail("Baked city body has no road surface binding")
			return
		var visual: MeshInstance3D = body.get_child(body.get_child_count() - 1)
		var collider: CollisionShape3D = body.get_child(0)
		var faces: PackedVector3Array = collider.shape.get_faces()
		if faces.size() != visual.mesh.get_faces().size():
			_fail("City rendering and collision differ")
			return
		var arrays: Array = visual.mesh.surface_get_arrays(0)
		var points: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
		var normals: PackedVector3Array = arrays[Mesh.ARRAY_NORMAL]
		if (points[1] - points[0]).cross(points[2] - points[0]).dot(normals[0]) >= 0:
			_fail("City mesh front face points away from its authored normal")
			return
		vertices += faces.size()
		var aabb := visual.get_aabb()
		if aabb.size.x > 32.01 or aabb.size.z > 32.01:
			_fail("City collision chunk exceeds 32 m")
			return
	if vertices != int(city.get_meta("native_collision_vertices")):
		_fail("Saved city collision is incomplete")
		return
	preview.overview = true
	preview.map_camera.current = true
	await _frames(5)
	await _capture("overview")
	preview.overview = false
	preview.camera.current = true
	for marker_name in ["Spawn", "BridgeSpawn"]:
		var marker := city.get_node_or_null(marker_name) as Marker3D
		if marker == null:
			_fail("Missing " + marker_name)
			return
		preview.spawn_pose = marker.global_transform
		preview._spawn()
		await _frames(240)
		var car: RallyCar = preview.car
		if car.contact_count() != 3 or car.global_basis.y.dot(Vector3.UP) < .9:
			_fail("Rickshaw lacks stable support at " + marker_name + ": " + str(car.telemetry()))
			return
		await _capture("road" if marker_name == "Spawn" else "bridge")
		var position := car.global_position
		Input.action_press("throttle_positive")
		await _frames(240)
		Input.action_release("throttle_positive")
		if car.global_position.distance_to(position) < 1 or car.contact_count() < 2:
			_fail("Rickshaw could not drive on " + marker_name + ": distance " + str(car.global_position.distance_to(position)) + "; " + str(car.telemetry()))
			return
		print("CITY_DRIVE_VERIFIED ", marker_name, " · distance ", car.global_position.distance_to(position), " · ", car.telemetry())
	print("CITY_SCENE_VERIFIED native bodies ", bodies.size(), " · collision vertices ", vertices, " · source crossing checks ", city.get_meta("city_track").structures.crossings_checked)
	preview.queue_free()
	await process_frame
	quit()

func _fail(message: String) -> void:
	push_error(message)
	quit(1)
