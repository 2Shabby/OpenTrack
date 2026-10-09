class_name TrackGeometry
extends Node3D

const ROAD_LAYER := 1
const TERRAIN_LAYER := 2
const CAR_LAYER := 4
const SUPPORT_MASK := ROAD_LAYER | TERRAIN_LAYER
const SURFACES: Array[SurfaceProfile] = [
	preload("res://resources/surfaces/asphalt.tres"),
	preload("res://resources/surfaces/dirt.tres"),
	preload("res://resources/surfaces/grass.tres"),
]
const GRASS: SurfaceProfile = SURFACES[2]
const FINISH_MATERIAL := preload("res://materials/finish.tres")
const Field := preload("res://scripts/terrain_field.gd")
const SavedTerrain := preload("res://scripts/baked_stage_terrain.gd")
const Shoulders := preload("res://scripts/road_shoulders.gd")

func build(stage: Resource) -> bool:
	if not stage.stage_id.is_empty():
		return await _load_saved(stage)
	return await author(stage)

# Offline authoring and procedural tests share the same geometry pipeline.
func author(stage: Resource) -> bool:
	for piece: Dictionary in stage.pieces():
		var profile := SURFACES[int(piece["surface"])]
		var arrays: Array = []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = piece["vertices"]
		arrays[Mesh.ARRAY_NORMAL] = piece["normals"]
		arrays[Mesh.ARRAY_TEX_UV] = piece["uvs"]
		arrays[Mesh.ARRAY_INDEX] = piece["indices"]
		var road_mesh := ArrayMesh.new()
		road_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		road_mesh.surface_set_material(0, profile.material)
		var shape := road_mesh.create_trimesh_shape()
		shape.backface_collision = true
		_add_surface(road_mesh, shape, profile, ROAD_LAYER)
		stage.terrain.register_surface(road_mesh)
		if piece.has("finish"):
			_add_finish_marker(piece["finish"], stage.road_width)
	_add_shoulders(stage)
	if not await stage.terrain.build(get_tree()):
		return false
	_add_ground(stage.terrain)
	return true

func _add_shoulders(stage: Resource) -> void:
	var shoulders := Node3D.new()
	shoulders.name = "Shoulders"
	add_child(shoulders)
	var meshes: Array[ArrayMesh] = Shoulders.new().build(stage)
	for mesh in meshes:
		mesh.surface_set_material(0, GRASS.material)
		var body := _add_surface(mesh, null, GRASS, TERRAIN_LAYER)
		_add_shoulder_collision(body, mesh)
		body.reparent(shoulders)
		stage.terrain.register_surface(mesh)

# Keep the continuous visual strip, but bound each native collision mesh to
# 32 m. A whole-stage collider quantizes its vertices over several kilometres,
# which can collapse distinct grid crossings into degenerate triangles.
func _add_shoulder_collision(body: StaticBody3D, mesh: Mesh) -> void:
	var cells := {}
	var faces := mesh.get_faces()
	for i in range(0, faces.size(), 3):
		var midpoint := (faces[i] + faces[i + 1] + faces[i + 2]) / 3.0
		var key := Vector2i(floori(midpoint.x / Field.CHUNK_METERS), floori(midpoint.z / Field.CHUNK_METERS))
		if not cells.has(key):
			cells[key] = []
		var origin := Vector3(key.x * Field.CHUNK_METERS, 0, key.y * Field.CHUNK_METERS)
		for j in 3:
			cells[key].append(faces[i + j] - origin)
	for key: Vector2i in cells:
		var shape := ConcavePolygonShape3D.new()
		shape.set_faces(PackedVector3Array(cells[key]))
		_add_collision(body, shape, Vector3(key.x * Field.CHUNK_METERS, 0, key.y * Field.CHUNK_METERS))

func _add_ground(field: Resource) -> void:
	var ground := Node3D.new()
	ground.name = "Ground"
	add_child(ground)
	for mesh: ArrayMesh in field.meshes:
		mesh.surface_set_material(0, GRASS.material)
		var shape := mesh.create_trimesh_shape()
		shape.backface_collision = true
		var body := _add_surface(mesh, shape, GRASS, TERRAIN_LAYER)
		body.reparent(ground)
		field.register_surface(mesh)

func _add_surface(mesh: Mesh, shape: Shape3D, profile: SurfaceProfile, layer: int) -> StaticBody3D:
	var body := StaticBody3D.new()
	body.collision_layer = layer
	body.collision_mask = 0
	body.add_to_group(profile.group_name, true)
	body.set_meta("surface", profile)
	var visual := MeshInstance3D.new()
	visual.mesh = mesh
	body.add_child(visual)
	if shape != null:
		_add_collision(body, shape)
	add_child(body)
	return body

func _add_collision(body: StaticBody3D, shape: Shape3D, origin := Vector3.ZERO) -> void:
	var collider := CollisionShape3D.new()
	collider.shape = shape
	collider.position = origin
	body.add_child(collider)

func _add_finish_marker(marker: Transform3D, width: float) -> void:
	var mesh := BoxMesh.new()
	mesh.size = Vector3(width, 0.08, 0.9)
	mesh.material = FINISH_MATERIAL
	var visual := MeshInstance3D.new()
	visual.mesh = mesh
	visual.transform = marker
	add_child(visual)

func _load_saved(stage: Resource) -> bool:
	stage.terrain = SavedTerrain.new()
	var path: String = stage.baked_scene_path
	if path.is_empty() or not ResourceLoader.exists(path):
		stage.terrain = SavedTerrain.new()
		stage.terrain.error = "This stage's saved world is unavailable."
		return false
	var result := ResourceLoader.load_threaded_request(path, "PackedScene", false, ResourceLoader.CACHE_MODE_IGNORE)
	if result != OK:
		stage.terrain = SavedTerrain.new()
		stage.terrain.error = "Could not load the saved stage world."
		return false
	var status := ResourceLoader.load_threaded_get_status(path)
	while status == ResourceLoader.THREAD_LOAD_IN_PROGRESS:
		await get_tree().process_frame
		status = ResourceLoader.load_threaded_get_status(path)
	if status != ResourceLoader.THREAD_LOAD_LOADED:
		stage.terrain = SavedTerrain.new()
		stage.terrain.error = "The saved stage world is invalid."
		return false
	var scene := ResourceLoader.load_threaded_get(path) as PackedScene
	if scene == null:
		stage.terrain.error = "The saved stage world is invalid."
		return false
	add_child(scene.instantiate())
	stage.terrain = SavedTerrain.new()
	stage.terrain.initialize(stage, get_world_3d().direct_space_state)
	# Make native collision available before the car is placed on the road.
	await get_tree().physics_frame
	return true
