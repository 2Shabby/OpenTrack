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
const Shoulders := preload("res://scripts/road_shoulders.gd")

func build(stage: Resource) -> void:
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
		if piece.has("finish"):
			_add_finish_marker(piece["finish"], stage.ROAD_WIDTH)
	_add_shoulders(stage)
	_add_ground(stage.terrain)

func _add_shoulders(stage: Resource) -> void:
	var shoulders := Node3D.new()
	shoulders.name = "Shoulders"
	add_child(shoulders)
	var meshes: Array[ArrayMesh] = Shoulders.new().build(stage)
	for mesh in meshes:
		mesh.surface_set_material(0, GRASS.material)
		var body := _add_surface(mesh, mesh.create_trimesh_shape(), GRASS, TERRAIN_LAYER)
		body.reparent(shoulders)

func _add_ground(field: Resource) -> void:
	var ground := Node3D.new()
	ground.name = "Ground"
	add_child(ground)
	for z in range(0, field.size.y - 1, Field.CHUNK_CELLS):
		for x in range(0, field.size.x - 1, Field.CHUNK_CELLS):
			var cells := Vector2i(mini(Field.CHUNK_CELLS, field.size.x - 1 - x), mini(Field.CHUNK_CELLS, field.size.y - 1 - z))
			var mesh: ArrayMesh = field.chunk_mesh(Vector2i(x, z), cells)
			mesh.surface_set_material(0, GRASS.material)
			# Collision uses these exact triangles, including the cell diagonal.
			var body := _add_surface(mesh, mesh.create_trimesh_shape(), GRASS, TERRAIN_LAYER)
			body.name = "Terrain_%d_%d" % [x, z]
			body.reparent(ground)

func _add_surface(mesh: Mesh, shape: Shape3D, profile: SurfaceProfile, layer: int) -> StaticBody3D:
	var body := StaticBody3D.new()
	body.collision_layer = layer
	body.collision_mask = 0
	body.add_to_group(profile.group_name)
	body.set_meta("surface", profile)
	var visual := MeshInstance3D.new()
	visual.mesh = mesh
	body.add_child(visual)
	var collider := CollisionShape3D.new()
	collider.shape = shape
	body.add_child(collider)
	add_child(body)
	return body

func _add_finish_marker(marker: Transform3D, width: float) -> void:
	var mesh := BoxMesh.new()
	mesh.size = Vector3(width, 0.08, 0.9)
	mesh.material = FINISH_MATERIAL
	var visual := MeshInstance3D.new()
	visual.mesh = mesh
	visual.transform = marker
	add_child(visual)
