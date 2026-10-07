class_name TerrainField
extends Resource

# One lattice supplies terrain rendering, native collision and height sampling.
const SPACING := 4.0
const CHUNK_CELLS := 32
const MARGIN := 256.0
const ROAD_CLEARANCE := 0.08

@export var origin := Vector2.ZERO
@export var size := Vector2i.ZERO
@export var heights := PackedFloat32Array()

func vertex(x: int, z: int) -> Vector3:
	return Vector3(origin.x + x * SPACING, heights[z * size.x + x], origin.y + z * SPACING)

func normal(x: int, z: int) -> Vector3:
	var left := maxi(0, x - 1)
	var right := mini(size.x - 1, x + 1)
	var back := maxi(0, z - 1)
	var front := mini(size.y - 1, z + 1)
	var dx := (heights[z * size.x + right] - heights[z * size.x + left]) / ((right - left) * SPACING)
	var dz := (heights[front * size.x + x] - heights[back * size.x + x]) / ((front - back) * SPACING)
	return Vector3(-dx, 1, -dz).normalized()

func height_at(position: Vector2) -> float:
	var grid := (position - origin) / SPACING
	if grid.x < 0 or grid.y < 0 or grid.x > size.x - 1 or grid.y > size.y - 1:
		return NAN
	var x := mini(floori(grid.x), size.x - 2)
	var z := mini(floori(grid.y), size.y - 2)
	var t := grid - Vector2(x, z)
	var a := heights[z * size.x + x]
	var b := heights[z * size.x + x + 1]
	var c := heights[(z + 1) * size.x + x]
	var d := heights[(z + 1) * size.x + x + 1]
	# Match the rendered/collision diagonal, rather than bilinear interpolation.
	return a + t.x * (b - a) + t.y * (c - a) if t.x + t.y <= 1 else d + (1 - t.x) * (c - d) + (1 - t.y) * (b - d)

func chunk_mesh(first: Vector2i, cells: Vector2i) -> ArrayMesh:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var indices := PackedInt32Array()
	var width := cells.x + 1
	for z in range(first.y, first.y + cells.y + 1):
		for x in range(first.x, first.x + cells.x + 1):
			var up := normal(x, z)
			vertices.append(vertex(x, z))
			normals.append(up)
	for z in cells.y:
		for x in cells.x:
			var a := z * width + x
			indices.append_array(PackedInt32Array([a, a + 1, a + width, a + 1, a + width + 1, a + width]))
	var arrays: Array = []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_NORMAL] = normals
	arrays[Mesh.ARRAY_INDEX] = indices
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	return mesh
