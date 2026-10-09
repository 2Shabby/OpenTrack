extends SceneTree

const FIELD := preload("res://scripts/terrain_field.gd")
const BAKE_VERSION := "saved-world-v1"
var _catalog_locked := false
const CATALOG_LOCK := "res://.stage_authoring/catalog.lock"

func _initialize() -> void:
	_bake.call_deferred()

func _bake() -> void:
	var id := ""
	var force := false
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--stage="):
			id = argument.trim_prefix("--stage=")
		force = force or argument == "--force"
	var catalog := StageCatalog.new()
	var entry := catalog.entry_for(id)
	if entry.is_empty():
		_fail("Specify a stage ID with -- --stage=<id>.")
		return
	var stage := ResourceLoader.load(entry.path, "", ResourceLoader.CACHE_MODE_IGNORE) as RallyStage
	if stage == null:
		_fail("Could not load " + id)
		return
	var fingerprint := _fingerprint(stage)
	if not force and stage.baked_fingerprint == fingerprint and ResourceLoader.exists(stage.baked_scene_path):
		print("WORLD_ALREADY_BAKED ", id)
		quit()
		return
	var started := Time.get_ticks_msec()
	stage.terrain = FIELD.new()
	stage.terrain.initialize(stage, stage.terrain_settings)
	stage.terrain.authoring_only = true
	var extension_path := "res://.stage_authoring/native/accelerator.gdextension"
	if FileAccess.file_exists(extension_path):
		var result := GDExtensionManager.load_extension(extension_path)
		if result != GDExtensionManager.LOAD_STATUS_OK and result != GDExtensionManager.LOAD_STATUS_ALREADY_LOADED:
			_fail("Could not load native offline terrain accelerator.")
			return
		stage.terrain.native_authoring = ClassDB.instantiate("TerrainBakeAccelerator")
		stage.terrain.native_authoring.configure(stage.centers, stage.terrain._segments, stage.terrain_settings.amplitude, stage.terrain_settings.wavelength, stage.terrain._seed, stage.road_width * 0.5, stage.terrain_settings.blend_distance)
	print("WORLD_AUTHORING ", id, " · ", stage.terrain.stats.chunk_count, " chunks")
	var geometry := TrackGeometry.new()
	geometry.name = "SavedStageWorld"
	root.add_child(geometry)
	if not await geometry.author(stage):
		_fail(id + ": " + stage.terrain.error)
		return
	var ground := geometry.get_node_or_null("Ground")
	var shoulders := geometry.get_node_or_null("Shoulders")
	if ground == null or ground.get_child_count() != stage.terrain.stats.chunk_count or shoulders == null or shoulders.get_child_count() != 4:
		_fail("Incomplete world geometry: " + id)
		return
	var road_count := 0
	for body in geometry.find_children("*", "StaticBody3D", true, false):
		if body.collision_layer == TrackGeometry.ROAD_LAYER:
			road_count += 1
		var shapes := body.find_children("*", "CollisionShape3D", true, false)
		var visuals := body.find_children("*", "MeshInstance3D", true, false)
		if shapes.is_empty() or visuals.size() != 1 or visuals[0].mesh == null or visuals[0].mesh.get_surface_count() != 1:
			_fail("Incomplete native surface: " + id)
			return
		var collision_vertices := 0
		for collider in shapes:
			if not collider.shape is ConcavePolygonShape3D or collider.shape.get_faces().is_empty():
				_fail("Incomplete native collision: " + id)
				return
			collision_vertices += collider.shape.get_faces().size()
		if collision_vertices != visuals[0].mesh.get_faces().size():
			_fail("Collision does not cover the visual surface: " + id)
			return
	if road_count != stage.features.size():
		_fail("Incomplete saved road: " + id)
		return
	var bounds := _mesh_bounds(geometry)
	if bounds.size == Vector3.ZERO:
		_fail("World contains no geometry: " + id)
		return
	await physics_frame
	var space := geometry.get_world_3d().direct_space_state
	for i in range(0, stage.centers.size(), 25):
		var center := stage.centers[i]
		var probes := [center]
		for edge: Vector3 in [stage.left_edges[i], stage.right_edges[i]]:
			probes.append(edge + (edge - center).normalized() * 0.1)
		for point: Vector3 in probes:
			var query := PhysicsRayQueryParameters3D.create(Vector3(point.x, bounds.end.y + 10, point.z), Vector3(point.x, bounds.position.y - 10, point.z), TrackGeometry.SUPPORT_MASK)
			if not is_finite(preload("res://scripts/baked_stage_terrain.gd").ray_support(space, query)):
				_fail("Missing native road/verge support: " + id + " station " + str(i))
				return
	# All native bodies, surface groups, material references and ConcavePolygon
	# collision shapes are owned by the saved scene, including ground/shoulders.
	_own_children(geometry, geometry)
	# The baked root is a plain Node3D; it has no generation script at runtime.
	geometry.set_script(null)
	var packed := PackedScene.new()
	if packed.pack(geometry) != OK:
		_fail("Could not pack " + id)
		return
	DirAccess.make_dir_recursive_absolute("res://resources/stages/baked")
	var scene_path := "res://resources/stages/baked/" + id + ".scn"
	var temporary_path := "res://resources/stages/baked/" + id + ".pending.scn"
	if ResourceSaver.save(packed, temporary_path, ResourceSaver.FLAG_COMPRESS) != OK:
		_fail("Could not save world " + id)
		return
	if DirAccess.rename_absolute(temporary_path, scene_path) != OK:
		_fail("Could not publish world " + id)
		return
	stage.baked_scene_path = scene_path
	stage.baked_bounds = bounds
	stage.baked_stats = stage.terrain.stats.duplicate()
	stage.baked_stats["authoring_ms"] = Time.get_ticks_msec() - started
	stage.baked_stats["bake_version"] = BAKE_VERSION
	stage.baked_fingerprint = fingerprint
	stage.terrain = null
	var stage_temporary_path: String = entry.path.trim_suffix(".res") + ".pending.res"
	if ResourceSaver.save(stage, stage_temporary_path, ResourceSaver.FLAG_COMPRESS) != OK or DirAccess.rename_absolute(stage_temporary_path, entry.path) != OK:
		_fail("Could not save stage metadata " + id)
		return
	if not await _lock_catalog():
		_fail("Timed out waiting to publish catalog")
		return
	catalog.refresh()
	entry = catalog.entry_for(id)
	if entry.is_empty():
		_fail("Stage disappeared from catalog: " + id)
		return
	entry.baked_scene_path = scene_path
	entry.baked_fingerprint = fingerprint
	entry.baked_stats = stage.baked_stats
	var index := Resource.new()
	index.set_meta("version", 1)
	index.set_meta("stages", catalog.entries)
	var catalog_temporary_path := "res://resources/stages/catalog.pending.tres"
	if ResourceSaver.save(index, catalog_temporary_path) != OK or DirAccess.rename_absolute(catalog_temporary_path, StageCatalog.INDEX_PATH) != OK:
		_fail("Could not update catalog")
		return
	var file := FileAccess.open("res://resources/stages/catalog.pending.json", FileAccess.WRITE)
	file.store_string(JSON.stringify({"version": 1, "stages": catalog.entries}, "\t") + "\n")
	file.close()
	if DirAccess.rename_absolute("res://resources/stages/catalog.pending.json", "res://resources/stages/catalog.json") != OK:
		_fail("Could not publish JSON catalog")
		return
	_unlock_catalog()
	print("WORLD_BAKED ", id, " · ", stage.baked_stats.authoring_ms, " ms · ", scene_path)
	geometry.queue_free()
	await process_frame
	quit()

func _own_children(node: Node, scene_root: Node) -> void:
	for child in node.get_children():
		child.owner = scene_root
		_own_children(child, scene_root)

func _mesh_bounds(node: Node) -> AABB:
	var result := AABB()
	var found := false
	for child in node.find_children("*", "MeshInstance3D", true, false):
		var bounds: AABB = child.global_transform * child.get_aabb()
		result = result.merge(bounds) if found else bounds
		found = true
	return result

func _fingerprint(stage: RallyStage) -> String:
	return preload("res://tools/stage_bake_fingerprint.gd").compute(stage)

func _lock_catalog() -> bool:
	var started := Time.get_ticks_msec()
	while DirAccess.make_dir_absolute(CATALOG_LOCK) != OK:
		if Time.get_ticks_msec() - started > 60000:
			return false
		await process_frame
	_catalog_locked = true
	var owner_file := FileAccess.open(CATALOG_LOCK + "/pid", FileAccess.WRITE)
	owner_file.store_string(str(OS.get_process_id()))
	owner_file.close()
	return true

func _unlock_catalog() -> void:
	if _catalog_locked:
		DirAccess.remove_absolute(CATALOG_LOCK + "/pid")
		DirAccess.remove_absolute(CATALOG_LOCK)
		_catalog_locked = false

func _fail(message: String) -> void:
	_unlock_catalog()
	push_error(message)
	quit(1)
