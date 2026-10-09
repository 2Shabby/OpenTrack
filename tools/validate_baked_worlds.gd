extends SceneTree

var failures := 0

func _initialize() -> void:
	_run.call_deferred()

func check(condition: bool, message: String) -> void:
	if not condition:
		failures += 1
		push_error(message)

func _run() -> void:
	var catalog := StageCatalog.new()
	check(catalog.entries.size() == 50, "Expected 50 worlds")
	var selected_id := ""
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--stage="):
			selected_id = argument.trim_prefix("--stage=")
	var report: Array[Dictionary] = []
	for entry in catalog.entries:
		if not selected_id.is_empty() and entry.id != selected_id:
			continue
		var before := failures
		var stage := catalog.load_stage(entry.id)
		check(stage != null, "Cannot load " + entry.id + ": " + catalog.error)
		if stage == null:
			continue
		check(stage.baked_fingerprint == preload("res://tools/stage_bake_fingerprint.gd").compute(stage), "Bake inputs changed: " + entry.id)
		check(entry.get("baked_fingerprint", "") == stage.baked_fingerprint and entry.get("baked_scene_path", "") == stage.baked_scene_path, "Catalog bake metadata differs: " + entry.id)
		var geometry := TrackGeometry.new()
		root.add_child(geometry)
		var started := Time.get_ticks_msec()
		check(await geometry.build(stage), "Cannot instantiate saved world: " + entry.id)
		check(stage.terrain.get_script() == preload("res://scripts/baked_stage_terrain.gd"), "Runtime regenerated terrain: " + entry.id)
		var bodies := geometry.find_children("*", "StaticBody3D", true, false)
		check(not bodies.is_empty(), "Missing collision: " + entry.id)
		var terrain_triangles := 0
		var chunks := 0
		for body: StaticBody3D in bodies:
			check(body.get_groups().size() == 1 and body.has_meta("surface"), "Missing tire profile: " + entry.id)
			var profile: Resource = body.get_meta("surface")
			check(body.is_in_group(profile.group_name), "Wrong tire group: " + entry.id)
			check(body.collision_layer == 1 or body.collision_layer == 2, "Wrong collision layer: " + entry.id)
			var visuals := body.find_children("*", "MeshInstance3D", true, false)
			var shapes := body.find_children("*", "CollisionShape3D", true, false)
			check(visuals.size() == 1 and not shapes.is_empty(), "Missing native mesh or collision: " + entry.id)
			if visuals.size() != 1 or shapes.is_empty():
				continue
			var mesh: Mesh = visuals[0].mesh
			check(mesh.surface_get_material(0) == profile.material, "Wrong saved material: " + entry.id)
			var collision_vertices := 0
			for collider in shapes:
				check(collider.shape is ConcavePolygonShape3D, "Wrong saved collision shape: " + entry.id)
				if collider.shape is ConcavePolygonShape3D:
					collision_vertices += collider.shape.get_faces().size()
			check(collision_vertices == mesh.get_faces().size(), "Collision coverage differs: " + entry.id)
			if body.get_parent().name == "Ground":
				chunks += 1
				var arrays := mesh.surface_get_arrays(0)
				var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
				terrain_triangles += vertices.size() / 3
				var midpoint: Vector3 = visuals[0].get_aabb().get_center()
				check(is_finite(stage.terrain.height_at(Vector2(midpoint.x, midpoint.z))), "Ground chunk has no support: " + entry.id + " at " + str(midpoint))
		check(chunks == stage.baked_stats.chunk_count, "Ground chunk count differs: " + entry.id)
		check(terrain_triangles == stage.baked_stats.terrain_triangles, "Terrain triangle count differs: " + entry.id)
		check(stage.baked_stats.voxel_m == 0.1 and stage.baked_stats.max_surface_error_m <= 0.10011, "Terrain quality differs: " + entry.id)
		for i in range(0, stage.centers.size(), 25):
			var point := stage.centers[i]
			var height: float = stage.terrain.height_at(Vector2(point.x, point.z))
			check(is_finite(height) and absf(height - point.y) < 0.05, "Road support differs: " + entry.id + " station " + str(i))
			for edge: Vector3 in [stage.left_edges[i], stage.right_edges[i]]:
				var verge := edge + (edge - point).normalized() * 0.1
				check(is_finite(stage.terrain.height_at(Vector2(verge.x, verge.z))), "Missing verge support: " + entry.id + " station " + str(i))
		var scene_bytes := FileAccess.get_file_as_bytes(stage.baked_scene_path).size()
		report.append({"id": entry.id, "scene_path": stage.baked_scene_path, "scene_bytes": scene_bytes, "scene_sha256": FileAccess.get_sha256(stage.baked_scene_path), "fingerprint": stage.baked_fingerprint, "chunk_count": chunks, "terrain_triangles": terrain_triangles, "failures": failures - before})
		print("BAKED_WORLD_VERIFIED ", entry.id, " · ", Time.get_ticks_msec() - started, " ms · ", chunks, " chunks · ", failures - before, " failures")
		geometry.queue_free()
		stage = null
		await process_frame
		await physics_frame
	var report_path := "res://.stage_authoring/world_validation.json"
	if not selected_id.is_empty():
		report_path = "res://.stage_authoring/verified-" + selected_id + ".json"
	var output := FileAccess.open(report_path, FileAccess.WRITE)
	output.store_string(JSON.stringify({"engine": Engine.get_version_info()["string"], "failures": failures, "worlds": report}, "\t") + "\n")
	print("ALL_BAKED_WORLDS_VERIFIED ", report.size(), " worlds · ", failures, " failures")
	quit(1 if failures else 0)
